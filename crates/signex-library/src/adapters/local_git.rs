//! Local + git storage adapter — `*.snxlib/` directory backed by libgit2.
//!
//! Per `v0.9-refactor-2-plan.md` §7, this WS-1 step keeps the primitive
//! CRUD wiring intact (it remains correct under the row model) but the
//! component / revision pieces are *not yet* re-implemented for the row
//! model — that's WS-2's job. Trait methods that used to live on this
//! adapter (`get_component`, `get_revision`, `save_revision`, etc.) now
//! fall through to the trait's default `Backend("not impl")` errors so
//! callers see the gap explicitly.
//!
//! Layout per the refactor (§3):
//!
//! ```text
//! MyComponents.snxlib/
//! ├── library.toml
//! ├── tables/                       (WS-2 — `.tsv` files, one per category)
//! ├── symbols/<uuid>.snxsym
//! ├── footprints/<uuid>.snxfpt
//! ├── sims/<uuid>.snxsim
//! └── .git/
//! ```
//!
//! Every write — primitive or row — commits via libgit2 with the supplied
//! message. The TSV (de)serialisation is delegated to [`crate::tables`]; this
//! module is the on-disk + git glue.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::adapter::{LibraryAdapter, LibraryError, PrimitiveSummary};
use crate::manifest::Manifest;
use crate::primitive::{Footprint, PrimitiveKind, SimModel, Symbol};

const SYMBOLS_DIR: &str = "symbols";
const FOOTPRINTS_DIR: &str = "footprints";
const SIMS_DIR: &str = "sims";
const TABLES_DIR: &str = "tables";
const SYMBOL_EXT: &str = "snxsym";
const FOOTPRINT_EXT: &str = "snxfpt";
const SIM_EXT: &str = "snxsim";
const MANIFEST_FILE: &str = "library.toml";

/// Adapter over a `*.snxlib/` directory + git repo.
#[derive(Debug)]
pub struct LocalGitAdapter {
    root: PathBuf,
    manifest: Manifest,
}

impl LocalGitAdapter {
    /// Initialise a fresh `.snxlib/` at `root` and run `git init` on it.
    ///
    /// Creates the directory layout, writes `library.toml`, and stages the
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

        fs::create_dir_all(&root)?;
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

    /// Borrow the on-disk root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn primitive_dir(&self, kind: PrimitiveKind) -> PathBuf {
        self.root.join(primitive_subdir(kind))
    }

    fn primitive_path(&self, kind: PrimitiveKind, uuid: Uuid) -> PathBuf {
        self.primitive_dir(kind)
            .join(format!("{uuid}.{}", primitive_ext(kind)))
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
    /// stage + commit it via libgit2 with the supplied message.
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

        let repo = git2::Repository::open(&self.root)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        let (sig_name, sig_email) = identity_for_repo(&repo);
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

        let mut index = repo
            .index()
            .map_err(|e| LibraryError::Backend(format!("git index: {e}")))?;
        index
            .add_path(Path::new(rel_path))
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

        // Resolve the parent commit. An unborn HEAD (fresh repo, no commits
        // yet) is the only legitimate "no parent" case — every other error
        // (corrupt ref, locked ref, etc.) propagates so we don't silently
        // produce an orphan commit on a broken repo.
        let parent = match repo.head() {
            Ok(h) => h
                .peel_to_commit()
                .map_err(|e| LibraryError::Backend(format!("git peel to commit: {e}")))
                .map(Some)?,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(LibraryError::Backend(format!("git head: {e}"))),
        };
        let parents: Vec<&git2::Commit> = parent.as_ref().map(|c| vec![c]).unwrap_or_default();
        let commit_message = if message.is_empty() {
            fallback_message.to_string()
        } else {
            message.to_string()
        };
        repo.commit(Some("HEAD"), &sig, &sig, &commit_message, &tree, &parents)
            .map_err(|e| LibraryError::Backend(format!("git commit: {e}")))?;
        Ok(())
    }

    // ── Table helpers ──────────────────────────────────────────────────────

    /// Path to `<root>/tables/`.
    fn tables_dir(&self) -> PathBuf {
        self.root.join(TABLES_DIR)
    }

    /// Absolute path to a table file by name (no extension).
    fn table_path(&self, name: &str) -> PathBuf {
        self.tables_dir().join(format!("{name}.{TABLE_EXT}"))
    }

    /// Relative-to-root path for `git add` — always forward slashes regardless
    /// of platform so the index entries stay portable.
    fn table_rel_path(name: &str) -> String {
        format!("{TABLES_DIR}/{name}.{TABLE_EXT}")
    }

    /// Walk a primitive directory and produce one [`PrimitiveSummary`] per
    /// `<uuid>.<ext>` file.
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
}

impl LibraryAdapter for LocalGitAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    // Row CRUD lands in WS-2; until then the trait defaults
    // (`Backend("not impl")`) cover every row method so the adapter still
    // satisfies the trait shape.

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{LibraryMeta, LibraryMode, Manifest, UsersConfig, WorkflowConfig};
    use crate::primitive::{PinElectricalType, PinOrientation, Symbol, SymbolPin};
    use uuid::Uuid;

    fn fixture_manifest() -> Manifest {
        Manifest {
            library: LibraryMeta {
                name: "TestLib".into(),
                library_id: Uuid::now_v7(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            tables: Vec::new(),
        }
    }

    fn fixture_symbol(name: &str) -> Symbol {
        Symbol {
            uuid: Uuid::now_v7(),
            name: name.into(),
            anchor: [0.0, 0.0],
            pins: vec![SymbolPin {
                number: "1".into(),
                name: "1".into(),
                electrical: PinElectricalType::Unspecified,
                position: [0.0, 0.0],
                orientation: PinOrientation::Right,
                length: 2.54,
            }],
            graphics: Vec::new(),
            schematic_params: Default::default(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
        }
    }

    #[test]
    fn init_creates_manifest_and_repo() {
        let dir = tempfile::tempdir().unwrap();
        let _adapter =
            LocalGitAdapter::init(dir.path(), fixture_manifest()).expect("init succeeds");
        assert!(dir.path().join(MANIFEST_FILE).exists());
        assert!(dir.path().join(".git").exists());
    }

    #[test]
    fn save_then_load_symbol_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = LocalGitAdapter::init(dir.path(), fixture_manifest()).unwrap();
        let sym = fixture_symbol("R");
        let uuid = sym.uuid;
        adapter.save_symbol(sym.clone(), "save R").unwrap();
        let back = adapter.get_symbol(uuid).unwrap();
        assert_eq!(back.uuid, uuid);
        assert_eq!(back.name, "R");
    }

    #[test]
    fn list_symbols_returns_saved_entries() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = LocalGitAdapter::init(dir.path(), fixture_manifest()).unwrap();
        adapter
            .save_symbol(fixture_symbol("Aaa"), "init Aaa")
            .unwrap();
        adapter
            .save_symbol(fixture_symbol("Bbb"), "init Bbb")
            .unwrap();
        let summaries = adapter.list_symbols().unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].name, "Aaa");
        assert_eq!(summaries[1].name, "Bbb");
    }
}
