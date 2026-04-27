//! Local + git storage adapter — `*.snxlib/` directory backed by libgit2.
//!
//! Speaks the DBLib row model: a "component" is a row inside
//! `tables/<category>.tsv`, not a file holding a revision chain.
//! Row CRUD — table read/write, insert / update / delete by row id,
//! lookup by `internal_pn` — sits alongside the primitive
//! (`Symbol` / `Footprint` / `SimModel`) flows.
//!
//! Layout:
//!
//! ```text
//! MyComponents.snxlib/
//! ├── library.toml
//! ├── tables/<category>.tsv          (one row per component, TSV)
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
use crate::component::ComponentRow;
use crate::identity::{InternalPn, RowId};
use crate::manifest::Manifest;
use crate::primitive::{Footprint, PrimitiveKind, SimModel, Symbol, SymbolFile};
use crate::tables;

const SYMBOLS_DIR: &str = "symbols";
const FOOTPRINTS_DIR: &str = "footprints";
const SIMS_DIR: &str = "sims";
const TABLES_DIR: &str = "tables";
const SYMBOL_EXT: &str = "snxsym";
const FOOTPRINT_EXT: &str = "snxfpt";
const SIM_EXT: &str = "snxsim";
const TABLE_EXT: &str = "tsv";
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

        let fallback = format!("save {} {uuid}", primitive_kind_str(kind));
        self.commit_path(&rel_path, message, &fallback)
    }

    /// Stage `rel_path` and create a new commit. Used by both primitive
    /// saves (`*.snx*` files) and table writes (`tables/*.tsv`). The two
    /// share a single signature/index/tree/commit dance; only the relative
    /// path and commit message vary.
    fn commit_path(
        &self,
        rel_path: &str,
        message: &str,
        fallback_message: &str,
    ) -> Result<(), LibraryError> {
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

    /// Walk `<root>/symbols/*.snxsym` and parse each as a
    /// [`SymbolFile`]. Returns `(file_path, parsed_file)` pairs in
    /// filename order. Legacy single-Symbol files are wrapped into
    /// one-element containers via [`SymbolFile::from_json`] so the
    /// rest of the adapter can treat every file uniformly.
    fn scan_symbol_files(&self) -> Result<Vec<(PathBuf, SymbolFile)>, LibraryError> {
        let dir = self.primitive_dir(PrimitiveKind::Symbol);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let suffix = format!(".{SYMBOL_EXT}");
        let mut out: Vec<(PathBuf, SymbolFile)> = Vec::new();
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
            let bytes = fs::read(path)?;
            let file = SymbolFile::from_json(&bytes)
                .map_err(|e| LibraryError::Backend(format!("read symbol file {name}: {e}")))?;
            out.push((path.to_path_buf(), file));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }

    /// Persist `sym` into a `.snxsym` container. Locates the existing
    /// file holding the symbol's uuid (upsert path); if the symbol is
    /// new, creates a fresh container at `<symbols>/<slug>.snxsym`
    /// (slug derived from `sym.name`, fallback to the symbol's uuid
    /// if the slug collides). Commits via libgit2 so the on-disk
    /// history matches every other adapter write.
    fn save_symbol_in_container(&self, sym: Symbol, message: &str) -> Result<(), LibraryError> {
        let dir = self.primitive_dir(PrimitiveKind::Symbol);
        fs::create_dir_all(&dir)?;

        let target_path = match self.locate_symbol_file(sym.uuid)? {
            Some((path, mut file)) => {
                if !file.upsert(sym.clone()) {
                    file.symbols.push(sym.clone());
                    file.updated = chrono::Utc::now();
                }
                let bytes = serde_json::to_vec_pretty(&file).map_err(|e| {
                    LibraryError::Backend(format!("write symbol container: {e}"))
                })?;
                fs::write(&path, bytes)?;
                path
            }
            None => {
                let file = SymbolFile::from_symbol(sym.clone());
                let path = self.fresh_symbol_file_path(&dir, &file)?;
                let bytes = serde_json::to_vec_pretty(&file).map_err(|e| {
                    LibraryError::Backend(format!("write symbol container: {e}"))
                })?;
                fs::write(&path, bytes)?;
                path
            }
        };

        let rel_path = target_path
            .strip_prefix(&self.root)
            .map_err(|_| {
                LibraryError::Backend(format!(
                    "could not relativise {} against root",
                    target_path.display()
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let fallback = format!("save symbol {} into {rel_path}", sym.uuid);
        self.commit_path(&rel_path, message, &fallback)
    }

    /// Find the existing `.snxsym` file containing the given symbol
    /// uuid. Returns `None` when the symbol is new (no container holds
    /// it yet). Reads but does not mutate.
    fn locate_symbol_file(
        &self,
        uuid: Uuid,
    ) -> Result<Option<(PathBuf, SymbolFile)>, LibraryError> {
        for (path, file) in self.scan_symbol_files()? {
            if file.symbols.iter().any(|s| s.uuid == uuid) {
                return Ok(Some((path, file)));
            }
        }
        Ok(None)
    }

    /// Pick a unique filename for a freshly-created `SymbolFile`.
    /// Slug derives from the file's `display_name` (or first symbol's
    /// name); collisions fall back to the file's UUID.
    fn fresh_symbol_file_path(
        &self,
        dir: &Path,
        file: &SymbolFile,
    ) -> Result<PathBuf, LibraryError> {
        let raw = if !file.display_name.is_empty() {
            file.display_name.as_str()
        } else if let Some(first) = file.symbols.first() {
            first.name.as_str()
        } else {
            "Untitled"
        };
        let slug = slugify(raw);
        let candidate = dir.join(format!("{slug}.{SYMBOL_EXT}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
        // Collision — fall back to the file uuid which is guaranteed unique.
        Ok(dir.join(format!("{}.{SYMBOL_EXT}", file.file_uuid)))
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

    // ── Tables ─────────────────────────────────────────────────────────────

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        let dir = self.tables_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out: Vec<String> = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            // Only `.tsv` files count — sibling `.toml` lock files or stray
            // editor backups (`.tsv~`) are ignored.
            if path.extension().and_then(|s| s.to_str()) != Some(TABLE_EXT) {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                out.push(stem.to_string());
            }
        }
        out.sort();
        Ok(out)
    }

    fn read_table(&self, name: &str) -> Result<Vec<ComponentRow>, LibraryError> {
        // Delegate to the file-format module — `tables::read_table` returns an
        // empty `Vec` for missing files, so the caller doesn't have to
        // existence-check up front.
        tables::read_table(&self.table_path(name))
    }

    fn iter_rows(&self) -> Result<Vec<(String, ComponentRow)>, LibraryError> {
        let mut out: Vec<(String, ComponentRow)> = Vec::new();
        for name in self.list_tables()? {
            let rows = self.read_table(&name)?;
            for row in rows {
                out.push((name.clone(), row));
            }
        }
        Ok(out)
    }

    fn read_row(&self, table: &str, row_id: RowId) -> Result<ComponentRow, LibraryError> {
        let target = row_id.as_uuid();
        let rows = self.read_table(table)?;
        rows.into_iter()
            .find(|r| r.row_id == target)
            .ok_or_else(|| LibraryError::NotFound(format!("row {row_id} in table {table}")))
    }

    /// Linear scan across every table — O(total rows). Acceptable at v0.9
    /// scale (libraries are O(thousands)). When the search index lands the
    /// call should redirect through it; until then, avoid hot-loop usage.
    fn read_row_by_pn(&self, pn: &InternalPn) -> Result<(String, ComponentRow), LibraryError> {
        for (table, row) in self.iter_rows()? {
            if &row.internal_pn == pn {
                return Ok((table, row));
            }
        }
        Err(LibraryError::NotFound(format!("internal_pn {pn}")))
    }

    fn insert_row(&self, table: &str, row: ComponentRow, msg: &str) -> Result<(), LibraryError> {
        let path = self.table_path(table);
        // Ensure `<root>/tables/` exists; the file is created (header-only)
        // by `append_row` if missing.
        fs::create_dir_all(self.tables_dir())?;
        tables::append_row(&path, &row)?;

        let rel = Self::table_rel_path(table);
        let fallback = format!("insert row {} into {}", row.row_id, table);
        self.commit_path(&rel, msg, &fallback)
    }

    fn update_row(&self, table: &str, row: ComponentRow, msg: &str) -> Result<(), LibraryError> {
        let path = self.table_path(table);
        let row_id = row.row_id;
        tables::update_row(&path, &row)?;

        let rel = Self::table_rel_path(table);
        let fallback = format!("update row {row_id} in {table}");
        self.commit_path(&rel, msg, &fallback)
    }

    fn delete_row(&self, table: &str, row_id: RowId, msg: &str) -> Result<(), LibraryError> {
        let path = self.table_path(table);
        tables::delete_row(&path, row_id)?;

        let rel = Self::table_rel_path(table);
        let fallback = format!("delete row {row_id} from {table}");
        self.commit_path(&rel, msg, &fallback)
    }

    fn get_symbol(&self, uuid: Uuid) -> Result<Symbol, LibraryError> {
        // Multi-symbol containers — scan each .snxsym file for the
        // requested uuid. SymbolFile::from_json handles legacy
        // single-Symbol files transparently (one-element container).
        for (_, file) in self.scan_symbol_files()? {
            if let Some(sym) = file.get_symbol(uuid) {
                return Ok(sym.clone());
            }
        }
        Err(LibraryError::NotFound(format!("symbol {uuid}")))
    }

    fn get_footprint(&self, uuid: Uuid) -> Result<Footprint, LibraryError> {
        self.read_primitive::<Footprint>(PrimitiveKind::Footprint, uuid)
    }

    fn get_sim(&self, uuid: Uuid) -> Result<SimModel, LibraryError> {
        self.read_primitive::<SimModel>(PrimitiveKind::Sim, uuid)
    }

    fn save_symbol(&self, sym: Symbol, message: &str) -> Result<(), LibraryError> {
        self.save_symbol_in_container(sym, message)
    }

    fn save_footprint(&self, fp: Footprint, message: &str) -> Result<(), LibraryError> {
        self.write_primitive(PrimitiveKind::Footprint, fp.uuid, &fp, message)
    }

    fn save_sim(&self, sm: SimModel, message: &str) -> Result<(), LibraryError> {
        self.write_primitive(PrimitiveKind::Sim, sm.uuid, &sm, message)
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        // Flatten every SymbolFile container into per-symbol summaries.
        let mut out: Vec<PrimitiveSummary> = Vec::new();
        for (_, file) in self.scan_symbol_files()? {
            for sym in &file.symbols {
                out.push(PrimitiveSummary {
                    uuid: sym.uuid,
                    name: sym.name.clone(),
                    kind: PrimitiveKind::Symbol,
                    used_by_count: 0,
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
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

/// Slugify a human-facing name into a safe filename component.
/// Lowercased, ASCII-only, runs of non-alphanumeric chars collapsed to
/// `-`. Empty result falls back to `"untitled"`.
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
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
            pins: vec![SymbolPin::new("1", "1")],
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
