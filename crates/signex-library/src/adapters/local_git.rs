//! Local + git storage adapter — `*.snxlib/` directory backed by libgit2.
//!
//! Layout per `LIBRARY_PLAN.md` §6:
//!
//! ```text
//! MyComponents.snxlib/
//! ├── manifest.toml
//! ├── parts/
//! │   ├── <uuid>-<version>.snxpart       (one file per immutable revision)
//! │   ├── <uuid>.<field_set>.lock        (advisory locks)
//! │   └── .draft/<uuid>.snxpart          (mutable drafts)
//! └── .git/
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::adapter::{
    ComponentSummary, FieldSet, LibraryAdapter, LibraryError, LibraryQuery, PrimitiveSummary,
};
use crate::component::{Component, Revision};
use crate::identity::{ComponentClass, ComponentId, InternalPn, Version};
use crate::manifest::Manifest;
use crate::primitive::{Footprint, PrimitiveKind, SimModel, Symbol};
use crate::snxpart::{SCHEMA_VERSION, SnxPartFile, read_snxpart, snxpart_filename, write_snxpart};

const PARTS_DIR: &str = "parts";
const SYMBOLS_DIR: &str = "symbols";
const FOOTPRINTS_DIR: &str = "footprints";
const SIMS_DIR: &str = "sims";
const SYMBOL_EXT: &str = "snxsym";
const FOOTPRINT_EXT: &str = "snxfpt";
const SIM_EXT: &str = "snxsim";
const MANIFEST_FILE: &str = "manifest.toml";
const REVIEW_BRANCH_PREFIX: &str = "review/";

/// Adapter over a `*.snxlib/` directory + git repo.
#[derive(Debug)]
pub struct LocalGitAdapter {
    root: PathBuf,
    manifest: Manifest,
}

impl LocalGitAdapter {
    /// Initialise a fresh `.snxlib/` at `root` and run `git init` on it.
    ///
    /// Creates the directory layout, writes `manifest.toml`, and stages the
    /// initial commit. Fails with `Conflict` if `root` already contains a
    /// manifest.
    pub fn init(root: impl AsRef<Path>, manifest: Manifest) -> Result<Self, LibraryError> {
        let root = root.as_ref().to_path_buf();
        let manifest_path = root.join(MANIFEST_FILE);
        if manifest_path.exists() {
            return Err(LibraryError::Conflict(format!(
                "library already exists at {}",
                root.display()
            )));
        }

        fs::create_dir_all(root.join(PARTS_DIR))?;
        let manifest_text = manifest
            .write()
            .map_err(|e| LibraryError::Backend(format!("manifest serialise: {e}")))?;
        fs::write(&manifest_path, &manifest_text)?;

        // Run `git init` and seed the manifest so the working tree has a HEAD.
        let repo = git2::Repository::init(&root)
            .map_err(|e| LibraryError::Backend(format!("git init: {e}")))?;
        let (sig_name, sig_email) = identity_for_repo(&repo);
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

        let mut index = repo
            .index()
            .map_err(|e| LibraryError::Backend(format!("git index: {e}")))?;
        index
            .add_path(Path::new(MANIFEST_FILE))
            .map_err(|e| LibraryError::Backend(format!("git add manifest: {e}")))?;
        index
            .write()
            .map_err(|e| LibraryError::Backend(format!("git index write: {e}")))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| LibraryError::Backend(format!("git write tree: {e}")))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| LibraryError::Backend(format!("git find tree: {e}")))?;
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "chore: initialise library",
            &tree,
            &[],
        )
        .map_err(|e| LibraryError::Backend(format!("git initial commit: {e}")))?;

        Ok(Self { root, manifest })
    }

    /// Open an existing `.snxlib/` directory.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, LibraryError> {
        let root = root.as_ref().to_path_buf();
        let manifest_path = root.join(MANIFEST_FILE);
        if !manifest_path.exists() {
            return Err(LibraryError::NotFound(format!(
                "no manifest at {}",
                manifest_path.display()
            )));
        }
        let text = fs::read_to_string(&manifest_path)?;
        let manifest = Manifest::parse(&text)
            .map_err(|e| LibraryError::Backend(format!("manifest parse: {e}")))?;
        // Validate the repo opens; surfaces dirty installs early.
        git2::Repository::open(&root)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        Ok(Self { root, manifest })
    }

    fn parts_dir(&self) -> PathBuf {
        self.root.join(PARTS_DIR)
    }

    fn primitive_dir(&self, kind: PrimitiveKind) -> PathBuf {
        self.root.join(primitive_subdir(kind))
    }

    fn primitive_path(&self, kind: PrimitiveKind, uuid: Uuid) -> PathBuf {
        self.primitive_dir(kind)
            .join(format!("{uuid}.{}", primitive_ext(kind)))
    }

    fn lock_path(&self, id: ComponentId, field_set: FieldSet) -> PathBuf {
        self.parts_dir()
            .join(format!("{id}.{}.lock", field_set_slug(field_set)))
    }

    /// Path to the on-disk `<uuid>.snxprt` file for `id`. WS-C will use this
    /// from the `library_id` + primitive CRUD wiring; the search path uses
    /// `load_components` rather than reading by id.
    #[allow(dead_code)]
    fn component_file_path(&self, id: ComponentId) -> PathBuf {
        self.parts_dir().join(snxpart_filename(id))
    }

    /// Compute the auto-bump heuristic per `v0.9-library-refactor-plan.md`
    /// §7 step B5.
    ///
    /// Returns the new version that `revision` should take, given the
    /// previous head revision on disk (if any). Any change in the bound
    /// primitive refs (symbol/footprint/sim) or pin-map overrides counts as a
    /// major bump; pure metadata swaps (MPN, parameters, supply, datasheet)
    /// stay minor. First revision starts at `1.0`.
    pub fn auto_bump(prev: Option<&Revision>, next: &Revision) -> Version {
        let Some(prev) = prev else {
            return Version::new(1, 0);
        };
        let major = prev.symbol_ref != next.symbol_ref
            || prev.footprint_ref != next.footprint_ref
            || prev.sim_ref != next.sim_ref
            || prev.pin_map_overrides != next.pin_map_overrides;
        if major {
            prev.version.bump_major()
        } else {
            prev.version.bump_minor()
        }
    }

    /// Read a primitive JSON file at `<root>/<subdir>/<uuid>.<ext>`.
    fn read_primitive<T: DeserializeOwned>(
        &self,
        kind: PrimitiveKind,
        uuid: Uuid,
    ) -> Result<T, LibraryError> {
        let path = self.primitive_path(kind, uuid);
        if !path.exists() {
            return Err(LibraryError::NotFound(format!(
                "{} {uuid}",
                primitive_kind_str(kind)
            )));
        }
        let bytes = fs::read(&path)?;
        let value: T = serde_json::from_slice(&bytes)
            .map_err(|e| LibraryError::Backend(format!("read primitive: {e}")))?;
        Ok(value)
    }

    /// Persist a primitive JSON file under `<root>/<subdir>/<uuid>.<ext>`,
    /// stage + commit it via libgit2 with the supplied message. Mirrors the
    /// per-kind save path that drives the
    /// `save_symbol/save_footprint/save_sim` arms of the trait.
    fn write_primitive<T: Serialize>(
        &self,
        kind: PrimitiveKind,
        uuid: Uuid,
        value: &T,
        message: &str,
    ) -> Result<(), LibraryError> {
        let dir = self.primitive_dir(kind);
        fs::create_dir_all(&dir)?;
        let rel_path = format!("{}/{uuid}.{}", primitive_subdir(kind), primitive_ext(kind));
        let abs_path = self.root.join(&rel_path);
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|e| LibraryError::Backend(format!("write primitive: {e}")))?;
        fs::write(&abs_path, bytes)?;

        // Stage + commit. We don't switch branches here — primitives live on
        // trunk; the review-required workflow only redirects component saves
        // (per plan §6 step C2).
        let repo = git2::Repository::open(&self.root)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        let (sig_name, sig_email) = identity_for_repo(&repo);
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

        let mut index = repo
            .index()
            .map_err(|e| LibraryError::Backend(format!("git index: {e}")))?;
        index
            .add_path(Path::new(&rel_path))
            .map_err(|e| LibraryError::Backend(format!("git add: {e}")))?;
        index
            .write()
            .map_err(|e| LibraryError::Backend(format!("git index write: {e}")))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| LibraryError::Backend(format!("git write tree: {e}")))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| LibraryError::Backend(format!("git find tree: {e}")))?;

        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.as_ref().map(|c| vec![c]).unwrap_or_default();
        let commit_message = if message.is_empty() {
            format!("save {} {uuid}", primitive_kind_str(kind))
        } else {
            message.to_string()
        };
        repo.commit(Some("HEAD"), &sig, &sig, &commit_message, &tree, &parents)
            .map_err(|e| LibraryError::Backend(format!("git commit: {e}")))?;
        Ok(())
    }

    /// Walk a primitive directory and produce one [`PrimitiveSummary`] per
    /// `<uuid>.<ext>` file. `name` is read from the file's payload (a
    /// trait-bound is added at the call site so we can reach the field).
    fn list_primitive_summaries<T>(
        &self,
        kind: PrimitiveKind,
        name_of: impl Fn(&T) -> &str,
    ) -> Result<Vec<PrimitiveSummary>, LibraryError>
    where
        T: DeserializeOwned,
    {
        let dir = self.primitive_dir(kind);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let suffix = format!(".{}", primitive_ext(kind));
        let mut out: Vec<PrimitiveSummary> = Vec::new();
        for entry in walkdir::WalkDir::new(&dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !name.ends_with(&suffix) {
                continue;
            }
            let stem = &name[..name.len() - suffix.len()];
            let Ok(uuid) = stem.parse::<Uuid>() else {
                continue;
            };
            let bytes = fs::read(path)?;
            let value: T = serde_json::from_slice(&bytes)
                .map_err(|e| LibraryError::Backend(format!("list primitive {name}: {e}")))?;
            out.push(PrimitiveSummary {
                uuid,
                name: name_of(&value).to_string(),
                kind,
                used_by_count: 0,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Walk `parts/` and load every `.snxprt` file. One file per component
    /// since the v0.9 refactor — revisions live inside the file.
    fn load_components(&self) -> Result<BTreeMap<ComponentId, Component>, LibraryError> {
        let mut by_uuid: BTreeMap<ComponentId, Component> = BTreeMap::new();
        let parts = self.parts_dir();
        if !parts.exists() {
            return Ok(by_uuid);
        }
        for entry in walkdir::WalkDir::new(&parts)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = match path.file_name().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };
            if !name.ends_with(".snxprt") || name.contains(".lock") {
                continue;
            }
            let file = read_snxpart(path)
                .map_err(|e| LibraryError::Backend(format!("read {name}: {e}")))?;
            by_uuid.insert(file.component.uuid, file.component);
        }
        for comp in by_uuid.values_mut() {
            comp.revisions.sort_by_key(|r| r.version);
        }
        Ok(by_uuid)
    }
}

impl LibraryAdapter for LocalGitAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn search(&self, query: &LibraryQuery) -> Result<Vec<ComponentSummary>, LibraryError> {
        let comps = self.load_components()?;
        let mut hits: Vec<ComponentSummary> = comps
            .values()
            .filter_map(|c| {
                let head = c.head_revision()?;
                Some(ComponentSummary {
                    uuid: c.uuid,
                    internal_pn: c.internal_pn.clone(),
                    mpn: head.primary_mpn.mpn.clone(),
                    head: c.head,
                    state: head.state,
                    description: head.primary_mpn.manufacturer.clone(),
                })
            })
            .collect();
        if let Some(text) = &query.text {
            let needle = text.to_ascii_lowercase();
            hits.retain(|h| {
                h.internal_pn
                    .as_str()
                    .to_ascii_lowercase()
                    .contains(&needle)
                    || h.mpn.to_ascii_lowercase().contains(&needle)
                    || h.description.to_ascii_lowercase().contains(&needle)
            });
        }
        hits.sort_by(|a, b| a.internal_pn.cmp(&b.internal_pn));
        Ok(hits)
    }

    fn get_component(&self, id: ComponentId) -> Result<Component, LibraryError> {
        self.load_components()?
            .remove(&id)
            .ok_or_else(|| LibraryError::NotFound(format!("component {id}")))
    }

    fn get_revision(&self, id: ComponentId, version: Version) -> Result<Revision, LibraryError> {
        let comp = self.get_component(id)?;
        comp.revisions
            .into_iter()
            .find(|r| r.version == version)
            .ok_or_else(|| LibraryError::NotFound(format!("{id}-{version}")))
    }

    fn save_revision(
        &self,
        id: ComponentId,
        mut revision: Revision,
        message: &str,
    ) -> Result<(), LibraryError> {
        // Pick the parent revision (if any) so we can compute the next version.
        let comps = self.load_components()?;
        let prev_component = comps.get(&id);
        let prev_head = prev_component.and_then(|c| c.head_revision());

        let new_version = Self::auto_bump(prev_head, &revision);
        revision.version = new_version;
        revision.refresh_content_hash();

        // Build the up-to-date Component (append revision, advance head).
        let mut component = match prev_component.cloned() {
            Some(mut c) => {
                c.revisions.push(revision);
                c.head = new_version;
                c
            }
            None => Component {
                uuid: id,
                internal_pn: InternalPn::new(format!("PART_{id}")),
                class: ComponentClass::generic(),
                category: PathBuf::new(),
                family: None,
                head: new_version,
                revisions: vec![revision],
            },
        };
        component.revisions.sort_by_key(|r| r.version);

        let file = SnxPartFile {
            schema_version: SCHEMA_VERSION,
            component,
        };
        let parts_dir = self.parts_dir();
        fs::create_dir_all(&parts_dir)?;
        let rel_path = format!("{}/{}", PARTS_DIR, snxpart_filename(id));
        let abs_path = self.root.join(&rel_path);

        // Open repo once so we can switch branches before writing the file.
        let repo = git2::Repository::open(&self.root)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        let (sig_name, sig_email) = identity_for_repo(&repo);
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

        let trunk_branch = head_branch_name(&repo)?;
        let target_branch = if self.manifest.workflow.review_required {
            Some(format!("{REVIEW_BRANCH_PREFIX}{id}"))
        } else {
            None
        };

        if let Some(branch) = &target_branch {
            checkout_or_create_branch(&repo, branch, &trunk_branch)?;
        }

        write_snxpart(&abs_path, &file)
            .map_err(|e| LibraryError::Backend(format!("write snxpart: {e}")))?;

        let mut index = repo
            .index()
            .map_err(|e| LibraryError::Backend(format!("git index: {e}")))?;
        index
            .add_path(Path::new(&rel_path))
            .map_err(|e| LibraryError::Backend(format!("git add: {e}")))?;
        index
            .write()
            .map_err(|e| LibraryError::Backend(format!("git index write: {e}")))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| LibraryError::Backend(format!("git write tree: {e}")))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| LibraryError::Backend(format!("git find tree: {e}")))?;

        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.as_ref().map(|c| vec![c]).unwrap_or_default();
        let commit_message = if message.is_empty() {
            format!("save {id} v{new_version}")
        } else {
            message.to_string()
        };
        repo.commit(Some("HEAD"), &sig, &sig, &commit_message, &tree, &parents)
            .map_err(|e| LibraryError::Backend(format!("git commit: {e}")))?;

        // Hop back to trunk so subsequent reads see the manifest's main branch.
        if target_branch.is_some() {
            checkout_branch(&repo, &trunk_branch)?;
        }

        Ok(())
    }

    fn try_lock(&self, id: ComponentId, field_set: FieldSet) -> Result<(), LibraryError> {
        let path = self.lock_path(id, field_set);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // O_CREAT | O_EXCL — fails if another holder already wrote the file.
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                use std::io::Write;
                let holder = lock_holder();
                writeln!(f, "{holder}")?;
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let holder = fs::read_to_string(&path).unwrap_or_default();
                let holder = holder.trim().to_string();
                Err(LibraryError::Locked {
                    holder: if holder.is_empty() {
                        "unknown".into()
                    } else {
                        holder
                    },
                    field_set: field_set_slug(field_set).into(),
                })
            }
            Err(e) => Err(LibraryError::Io(e)),
        }
    }

    fn release_lock(&self, id: ComponentId, field_set: FieldSet) -> Result<(), LibraryError> {
        // H5: TOCTOU — the previous `path.exists() && remove_file(&path)` race
        // could delete a *fresh* lock written by another process in the
        // microsecond between the two syscalls. `remove_file` already returns
        // `NotFound` atomically when the file is gone, so we collapse the
        // check into a single syscall and translate the error directly.
        let path = self.lock_path(id, field_set);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(LibraryError::NotFound(
                format!("no lock for {id}.{}", field_set_slug(field_set)),
            )),
            Err(e) => Err(LibraryError::Io(e)),
        }
    }

    fn get_symbol(&self, uuid: Uuid) -> Result<Symbol, LibraryError> {
        self.read_primitive::<Symbol>(PrimitiveKind::Symbol, uuid)
    }

    fn get_footprint(&self, uuid: Uuid) -> Result<Footprint, LibraryError> {
        self.read_primitive::<Footprint>(PrimitiveKind::Footprint, uuid)
    }

    fn get_sim(&self, uuid: Uuid) -> Result<SimModel, LibraryError> {
        self.read_primitive::<SimModel>(PrimitiveKind::Sim, uuid)
    }

    fn save_symbol(&self, sym: Symbol, message: &str) -> Result<(), LibraryError> {
        self.write_primitive(PrimitiveKind::Symbol, sym.uuid, &sym, message)
    }

    fn save_footprint(&self, fp: Footprint, message: &str) -> Result<(), LibraryError> {
        self.write_primitive(PrimitiveKind::Footprint, fp.uuid, &fp, message)
    }

    fn save_sim(&self, sm: SimModel, message: &str) -> Result<(), LibraryError> {
        self.write_primitive(PrimitiveKind::Sim, sm.uuid, &sm, message)
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitive_summaries::<Symbol>(PrimitiveKind::Symbol, |s| &s.name)
    }

    fn list_footprints(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitive_summaries::<Footprint>(PrimitiveKind::Footprint, |f| &f.name)
    }

    fn list_sims(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitive_summaries::<SimModel>(PrimitiveKind::Sim, |s| &s.name)
    }

    fn root_path(&self) -> Option<PathBuf> {
        Some(self.root.clone())
    }
}

fn primitive_subdir(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => SYMBOLS_DIR,
        PrimitiveKind::Footprint => FOOTPRINTS_DIR,
        PrimitiveKind::Sim => SIMS_DIR,
    }
}

fn primitive_ext(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => SYMBOL_EXT,
        PrimitiveKind::Footprint => FOOTPRINT_EXT,
        PrimitiveKind::Sim => SIM_EXT,
    }
}

fn primitive_kind_str(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => "symbol",
        PrimitiveKind::Footprint => "footprint",
        PrimitiveKind::Sim => "sim",
    }
}

fn field_set_slug(fs: FieldSet) -> &'static str {
    match fs {
        FieldSet::Symbol => "symbol",
        FieldSet::Footprint => "footprint",
        FieldSet::Model3d => "model_3d",
        FieldSet::SharedParams => "shared_params",
        FieldSet::SharedSupplyChain => "shared_supply_chain",
        FieldSet::SharedSimulation => "shared_simulation",
        FieldSet::Lifecycle => "lifecycle",
    }
}

fn lock_holder() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn identity_for_repo(repo: &git2::Repository) -> (String, String) {
    let cfg = repo.config().ok();
    let name = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.name").ok())
        .unwrap_or_else(|| "Signex Library".to_string());
    let email = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.email").ok())
        .unwrap_or_else(|| "library@signex.local".to_string());
    (name, email)
}

fn head_branch_name(repo: &git2::Repository) -> Result<String, LibraryError> {
    match repo.head() {
        Ok(head) => head
            .shorthand()
            .map(|s| s.to_string())
            .ok_or_else(|| LibraryError::Backend("HEAD has no shorthand".into())),
        Err(_) => Ok("master".to_string()),
    }
}

fn checkout_or_create_branch(
    repo: &git2::Repository,
    branch: &str,
    base: &str,
) -> Result<(), LibraryError> {
    if repo.find_branch(branch, git2::BranchType::Local).is_err() {
        let base_commit = repo
            .find_branch(base, git2::BranchType::Local)
            .map_err(|e| LibraryError::Backend(format!("base branch {base}: {e}")))?
            .get()
            .peel_to_commit()
            .map_err(|e| LibraryError::Backend(format!("base commit: {e}")))?;
        repo.branch(branch, &base_commit, false)
            .map_err(|e| LibraryError::Backend(format!("create branch {branch}: {e}")))?;
    }
    checkout_branch(repo, branch)
}

fn checkout_branch(repo: &git2::Repository, branch: &str) -> Result<(), LibraryError> {
    let refname = format!("refs/heads/{branch}");
    let obj = repo
        .revparse_single(&refname)
        .map_err(|e| LibraryError::Backend(format!("revparse {branch}: {e}")))?;
    repo.checkout_tree(&obj, None)
        .map_err(|e| LibraryError::Backend(format!("checkout tree {branch}: {e}")))?;
    repo.set_head(&refname)
        .map_err(|e| LibraryError::Backend(format!("set head {branch}: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_set_slugs_are_stable() {
        assert_eq!(field_set_slug(FieldSet::Symbol), "symbol");
        assert_eq!(field_set_slug(FieldSet::SharedParams), "shared_params");
    }
}
