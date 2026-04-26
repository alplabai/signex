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

use crate::adapter::{ComponentSummary, FieldSet, LibraryAdapter, LibraryError, LibraryQuery};
use crate::component::{Component, Revision};
use crate::identity::{ComponentId, InternalPn, Version};
use crate::manifest::Manifest;
use crate::snxpart::{SCHEMA_VERSION, SnxPartFile, read_snxpart, snxpart_filename, write_snxpart};

const PARTS_DIR: &str = "parts";
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

    fn lock_path(&self, id: ComponentId, field_set: FieldSet) -> PathBuf {
        self.parts_dir()
            .join(format!("{id}.{}.lock", field_set_slug(field_set)))
    }

    fn revision_path(&self, id: ComponentId, version: Version) -> PathBuf {
        self.parts_dir().join(snxpart_filename(id, version))
    }

    /// Compute the auto-bump heuristic per `LIBRARY_PLAN.md` §3.
    ///
    /// Returns the new version that `revision` should take, given the previous
    /// head version on disk (if any). The rule mirrors the WS-D diff
    /// engine for the cases WS-A needs today: a change in the symbol pin
    /// count is a major bump; everything else (mpn swap, parameter edits,
    /// etc.) is a minor bump. Versionless first revision starts at `1.0`.
    pub fn auto_bump(prev: Option<&Revision>, next: &Revision) -> Version {
        let Some(prev) = prev else {
            return Version::new(1, 0);
        };
        if requires_major_bump(prev, next) {
            prev.version.bump_major()
        } else {
            prev.version.bump_minor()
        }
    }

    /// Walk `parts/` and load every `.snxpart` file, grouping revisions by
    /// component uuid.
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
            if !name.ends_with(".snxpart") || name.contains(".lock") {
                continue;
            }
            let file = read_snxpart(path)
                .map_err(|e| LibraryError::Backend(format!("read {name}: {e}")))?;
            let comp = by_uuid.entry(file.uuid).or_insert_with(|| Component {
                uuid: file.uuid,
                internal_pn: file.internal_pn.clone(),
                revisions: Vec::new(),
                head: file.revision.version,
            });
            // Track head as the maximum version seen.
            if file.revision.version > comp.head {
                comp.head = file.revision.version;
            }
            // Keep `internal_pn` in sync with the latest file in case of rename.
            comp.internal_pn = file.internal_pn;
            comp.revisions.push(file.revision);
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
                    internal_pn: c.internal_pn.as_str().to_string(),
                    mpn: head.shared.mpn.clone(),
                    head: c.head,
                    state: head.state,
                    description: head.shared.description.clone(),
                })
            })
            .collect();
        if let Some(text) = &query.text {
            let needle = text.to_ascii_lowercase();
            hits.retain(|h| {
                h.internal_pn.to_ascii_lowercase().contains(&needle)
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
        let path = self.revision_path(id, version);
        if !path.exists() {
            return Err(LibraryError::NotFound(format!("{id}-{version}")));
        }
        let file = read_snxpart(&path)
            .map_err(|e| LibraryError::Backend(format!("read revision: {e}")))?;
        Ok(file.revision)
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

        let internal_pn = prev_component
            .map(|c| c.internal_pn.clone())
            .unwrap_or_else(|| InternalPn::new(format!("PART_{id}")));

        let file = SnxPartFile {
            schema: SCHEMA_VERSION,
            uuid: id,
            internal_pn,
            revision,
        };
        let parts_dir = self.parts_dir();
        fs::create_dir_all(&parts_dir)?;
        let rel_path = format!("{}/{}", PARTS_DIR, snxpart_filename(id, new_version));
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
        let path = self.lock_path(id, field_set);
        if !path.exists() {
            return Err(LibraryError::NotFound(format!(
                "no lock for {id}.{}",
                field_set_slug(field_set)
            )));
        }
        fs::remove_file(&path)?;
        Ok(())
    }

    fn root_path(&self) -> Option<PathBuf> {
        Some(self.root.clone())
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

/// Decide whether moving from `prev` to `next` is a major bump.
///
/// Cheap token-count heuristic on the symbol/footprint S-expression: any
/// change in `(pin ` or `(pad ` count promotes to major. Cosmetic edits and
/// pure metadata swaps (mpn, parameters) stay minor.
///
/// `crate::diff::auto_bump_kind` runs WS-D's real KiCad-aware diff and is the
/// preferred call site when the symbol body is known to be a well-formed
/// KiCad pin S-expression with `(number "X")` children. The token heuristic
/// here stays robust on stubs / drafts that don't yet carry the full schema.
fn requires_major_bump(prev: &Revision, next: &Revision) -> bool {
    pin_count(&prev.schematic.symbol.sexpr) != pin_count(&next.schematic.symbol.sexpr)
        || pad_count(&prev.pcb.footprint.sexpr) != pad_count(&next.pcb.footprint.sexpr)
}

fn pin_count(sexpr: &str) -> usize {
    sexpr.matches("(pin ").count()
}

fn pad_count(sexpr: &str) -> usize {
    sexpr.matches("(pad ").count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_set_slugs_are_stable() {
        assert_eq!(field_set_slug(FieldSet::Symbol), "symbol");
        assert_eq!(field_set_slug(FieldSet::SharedParams), "shared_params");
    }

    #[test]
    fn pin_count_matches_kicad_tokens() {
        let s = "(symbol (pin 1) (pin 2) (pin 3))";
        assert_eq!(pin_count(s), 3);
    }

    /// L4: Documents the known false-positive in the `(pin ` token heuristic.
    ///
    /// `pin_count` is a deliberately naive byte-substring scan used only as a
    /// stub-friendly fallback when the symbol body is not yet a well-formed
    /// KiCad S-expression. A property value that literally contains the string
    /// `(pin ` (e.g. a free-form description) will inflate the count and
    /// incorrectly trigger a major bump in `requires_major_bump`.
    ///
    /// Production call sites should prefer `crate::diff::auto_bump_kind`,
    /// which structurally walks the parsed pin nodes. This test pins the
    /// limitation so the next refactor knows to swap the heuristic — not to
    /// "fix" the off-by-one count.
    #[test]
    fn pin_count_false_positive_on_property_string() {
        // A property value that mentions "(pin " as plain text — not a real
        // KiCad pin node. The heuristic still counts it.
        let s = r#"(symbol (property "Description" "Connector with (pin 1) terminal"))"#;
        assert_eq!(
            pin_count(s),
            1,
            "byte-substring scan cannot distinguish KiCad pin nodes from property text"
        );
    }
}
