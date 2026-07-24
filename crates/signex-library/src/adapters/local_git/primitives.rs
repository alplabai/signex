//! Primitive read/write + git-commit inherent methods on `LocalGitAdapter`.

use super::helpers::*;
use super::*;

impl LocalGitAdapter {
    fn primitive_dir(&self, kind: PrimitiveKind) -> PathBuf {
        self.root_dir.join(primitive_subdir(kind))
    }

    fn primitive_path(&self, kind: PrimitiveKind, uuid: Uuid) -> PathBuf {
        self.primitive_dir(kind)
            .join(format!("{uuid}.{}", primitive_ext(kind)))
    }

    /// Read a primitive JSON file at `<root>/<subdir>/<uuid>.<ext>`.
    pub(super) fn read_primitive<T: DeserializeOwned>(
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
        // v0.18.4 — `.snxfpt` and `.snxsym` ship as TOML+TSV
        // envelopes. v0.18.5 — `.snxsim` ships as TOML envelope.
        // For each primitive kind that's been migrated, build the
        // file envelope, pull the first contained primitive, then
        // serde-round-trip through JSON to recover the generic T.
        if matches!(kind, PrimitiveKind::Footprint) {
            let file = crate::primitive::FootprintFile::from_bytes(&bytes)
                .map_err(|e| LibraryError::Backend(format!("read .snxfpt: {e}")))?;
            let fp = file
                .footprints
                .into_iter()
                .next()
                .ok_or_else(|| LibraryError::Backend("empty FootprintFile".into()))?;
            let buf = serde_json::to_vec(&fp)
                .map_err(|e| LibraryError::Backend(format!("re-serialise footprint: {e}")))?;
            let value: T = serde_json::from_slice(&buf)
                .map_err(|e| LibraryError::Backend(format!("read primitive: {e}")))?;
            return Ok(value);
        }
        if matches!(kind, PrimitiveKind::Sim) {
            let file = SimFile::from_bytes(&bytes)
                .map_err(|e| LibraryError::Backend(format!("read .snxsim: {e}")))?;
            let model = file
                .models
                .into_iter()
                .next()
                .ok_or_else(|| LibraryError::Backend("empty SimFile".into()))?;
            let buf = serde_json::to_vec(&model)
                .map_err(|e| LibraryError::Backend(format!("re-serialise sim: {e}")))?;
            let value: T = serde_json::from_slice(&buf)
                .map_err(|e| LibraryError::Backend(format!("read primitive: {e}")))?;
            return Ok(value);
        }
        // HI-8: Symbol files are TOML+TSV envelopes since v0.18.4 and
        // are read via `get_symbol`/`scan_symbol_files` which dispatch
        // through `SymbolFile::from_bytes`. The generic JSON fallback
        // below is unreachable on supported kinds; guard it so a
        // future refactor that adds e.g. `Model3d` doesn't silently
        // try to parse a TOML file as JSON.
        debug_assert!(
            !matches!(kind, PrimitiveKind::Symbol),
            "read_primitive::<Symbol> must route through get_symbol; this generic path \
             would attempt serde_json on a TOML envelope and fail at runtime"
        );
        let value: T = serde_json::from_slice(&bytes)
            .map_err(|e| LibraryError::Backend(format!("read primitive: {e}")))?;
        Ok(value)
    }

    /// Persist a primitive file under `<root>/<subdir>/<uuid>.<ext>`,
    /// stage + commit it via libgit2 with the supplied message.
    ///
    /// `.snxfpt` files emit as TOML+TSV envelope (v0.18.4); `.snxsim`
    /// files emit as TOML envelope (v0.18.5). `.snxsym` is handled
    /// outside this generic path via `save_symbol_in_container` so
    /// multi-symbol containers are preserved.
    pub(super) fn write_primitive<T: Serialize>(
        &self,
        kind: PrimitiveKind,
        uuid: Uuid,
        value: &T,
        message: &str,
    ) -> Result<(), LibraryError> {
        let dir = self.primitive_dir(kind);
        fs::create_dir_all(&dir)?;
        let rel_path = format!("{}/{uuid}.{}", primitive_subdir(kind), primitive_ext(kind));
        let abs_path = self.root_dir.join(&rel_path);
        let bytes = if matches!(kind, PrimitiveKind::Footprint) {
            // T is Footprint here — round-trip through JSON to obtain
            // the typed value, then merge into the existing envelope
            // on disk if present (HI-7) so a multi-footprint file
            // doesn't get clobbered by a single-element rewrite.
            let buf = serde_json::to_vec(value)
                .map_err(|e| LibraryError::Backend(format!("re-serialise footprint: {e}")))?;
            let fp: crate::primitive::Footprint = serde_json::from_slice(&buf)
                .map_err(|e| LibraryError::Backend(format!("write primitive: {e}")))?;
            let mut file = match fs::read(&abs_path) {
                Ok(existing) => crate::primitive::FootprintFile::from_bytes(&existing)
                    .map_err(|e| LibraryError::Backend(format!("re-read .snxfpt: {e}")))?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    crate::primitive::FootprintFile::from_footprint(fp.clone())
                }
                Err(e) => return Err(LibraryError::from(e)),
            };
            // Replace by uuid; preserve siblings.
            let target_uuid = fp.uuid;
            if let Some(slot) = file.footprints.iter_mut().find(|f| f.uuid == target_uuid) {
                *slot = fp;
            } else {
                file.footprints.push(fp);
            }
            file.updated = chrono::Utc::now();
            file.to_toml_string()
                .map_err(|e| LibraryError::Backend(format!("emit .snxfpt: {e}")))?
                .into_bytes()
        } else if matches!(kind, PrimitiveKind::Sim) {
            // T is SimModel here — same JSON round-trip recovery
            // pattern, then merge into the existing envelope (HI-7)
            // so the SPICE / Verilog-A multi-element file doesn't
            // get clobbered.
            let buf = serde_json::to_vec(value)
                .map_err(|e| LibraryError::Backend(format!("re-serialise sim: {e}")))?;
            let model: SimModel = serde_json::from_slice(&buf)
                .map_err(|e| LibraryError::Backend(format!("write primitive: {e}")))?;
            let mut file = match fs::read(&abs_path) {
                Ok(existing) => SimFile::from_bytes(&existing)
                    .map_err(|e| LibraryError::Backend(format!("re-read .snxsim: {e}")))?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    SimFile::from_model(model.clone())
                }
                Err(e) => return Err(LibraryError::from(e)),
            };
            let target_uuid = model.uuid;
            if let Some(slot) = file.models.iter_mut().find(|m| m.uuid == target_uuid) {
                *slot = model;
            } else {
                file.models.push(model);
            }
            file.updated = chrono::Utc::now();
            file.to_toml_string()
                .map_err(|e| LibraryError::Backend(format!("emit .snxsim: {e}")))?
                .into_bytes()
        } else {
            // HI-8: Symbols are written via `save_symbol_in_container`,
            // not this generic path. The pretty-JSON branch is left
            // for forward-compat with future kinds whose on-disk
            // format hasn't been migrated.
            debug_assert!(
                !matches!(kind, PrimitiveKind::Symbol),
                "write_primitive::<Symbol> must route through save_symbol_in_container; \
                 the JSON fallback would write a non-canonical format"
            );
            serde_json::to_vec_pretty(value)
                .map_err(|e| LibraryError::Backend(format!("write primitive: {e}")))?
        };
        // HI-6: atomic write — primitive containers (`.snxsim`,
        // `.snxfpt`, `.snxsym`) are TOML+TSV envelopes; a half-written
        // file destroys all primitives in the container, so we cannot
        // afford the in-place truncate that `fs::write` does on
        // existing destinations.
        signex_types::atomic_io::atomic_write(&abs_path, &bytes)?;

        let fallback = format!("save {} {uuid}", primitive_kind_str(kind));
        self.commit_path(&rel_path, message, &fallback)
    }

    /// Stage `rel_path` and create a new commit. Used by primitive saves
    /// (`*.snx*` files) and table writes (the `.snxlib` itself). When
    /// the parent directory has no `.git/`, this is a no-op — the file
    /// has already been written to disk by the caller, and the user
    /// opted out of version control at create time. They can opt in
    /// later via the (forthcoming) Enable Version Control flow.
    pub(super) fn commit_path(
        &self,
        rel_path: &str,
        message: &str,
        fallback_message: &str,
    ) -> Result<(), LibraryError> {
        if !self.root_dir.join(".git").exists() {
            return Ok(());
        }
        // HI-11: serialise concurrent commits in this process. Two
        // threads each opening their own `git2::Repository` and
        // calling `index().add_path()` race on `.git/index.lock`; the
        // loser would surface as a `git add: error` failure to the
        // user despite no real conflict.
        let _git_guard = self.git_lock.lock().unwrap_or_else(|e| e.into_inner());
        let repo = git2::Repository::open(&self.root_dir)
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
        // (corrupt ref, locked ref) propagates so we don't silently produce
        // an orphan commit on a broken repo.
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

    fn snxlib_rel_path(&self) -> Result<String, LibraryError> {
        file_name_str(&self.file_path)
    }

    /// Persist the in-memory `library_file` to disk. Caller already holds
    /// the appropriate read/write lock.
    fn persist_library_file(&self, lf: &LibraryFile) -> Result<(), LibraryError> {
        let text = lf.write()?;
        // HI-6: atomic write — never half-write the manifest.
        signex_types::atomic_io::atomic_write(&self.file_path, text.as_bytes())?;
        Ok(())
    }

    /// Mutate the in-memory `library_file`, persist it, and commit the
    /// `.snxlib` with the supplied message.
    pub(super) fn mutate_library_file<F>(
        &self,
        f: F,
        message: &str,
        fallback: &str,
    ) -> Result<(), LibraryError>
    where
        F: FnOnce(&mut LibraryFile) -> Result<(), LibraryError>,
    {
        let mut guard = self
            .library_file
            .write()
            .map_err(|_| LibraryError::Backend("library_file write lock poisoned".into()))?;
        f(&mut guard)?;
        self.persist_library_file(&guard)?;
        let rel = self.snxlib_rel_path()?;
        self.commit_path(&rel, message, fallback)
    }

    /// Read the table named `table` as a snapshot of [`ComponentRow`]s,
    /// scoped to whatever the legacy [`TABLE_HEADER`] columns are.
    /// Returns an empty vec for unknown tables (matches the old
    /// `tables/<name>.tsv` "missing file = empty" semantics).
    pub(super) fn snapshot_table(&self, name: &str) -> Result<Vec<ComponentRow>, LibraryError> {
        let guard = self
            .library_file
            .read()
            .map_err(|_| LibraryError::Backend("library_file read lock poisoned".into()))?;
        let Some(table) = guard.tables.get(name) else {
            return Ok(Vec::new());
        };
        // Verify the on-disk header matches the legacy schema. Once
        // Stage 12 lifts the fixed-column constraint this guard goes
        // away, but for v0.9 we want a loud error if a hand-edited
        // .snxlib drifts the schema.
        validate_legacy_header(name, &table.columns)?;
        let mut out = Vec::with_capacity(table.rows.len());
        for row in &table.rows {
            out.push(library_row_to_component(row)?);
        }
        Ok(out)
    }

    // ── Symbol container helpers (v0.9 phase 2 multi-symbol files) ────────

    pub(super) fn scan_symbol_files(&self) -> Result<Vec<(PathBuf, SymbolFile)>, LibraryError> {
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
            // TOML-only (alpha policy, no legacy JSON support — pre-v0.18.4
            // libraries must be regenerated). `from_bytes` decodes UTF-8 and
            // hands off to `from_toml_str`; the format-token check inside
            // surfaces `UnsupportedFormat` for any drift.
            let file = SymbolFile::from_bytes(&bytes)
                .map_err(|e| LibraryError::Backend(format!("read symbol file {name}: {e}")))?;
            out.push((path.to_path_buf(), file));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }

    pub(super) fn save_symbol_in_container(
        &self,
        sym: Symbol,
        message: &str,
    ) -> Result<(), LibraryError> {
        let dir = self.primitive_dir(PrimitiveKind::Symbol);
        fs::create_dir_all(&dir)?;

        let target_path = match self.locate_symbol_file(sym.uuid)? {
            Some((path, mut file)) => {
                if !file.upsert(sym.clone()) {
                    file.symbols.push(sym.clone());
                    file.updated = chrono::Utc::now();
                }
                // v0.18.4 — emit TOML envelope.
                let text = file
                    .to_toml_string()
                    .map_err(|e| LibraryError::Backend(format!("write symbol container: {e}")))?;
                // HI-6: atomic write — the symbol container is a TOML+TSV
                // envelope holding every symbol; an in-place truncate by
                // `fs::write` would destroy them all on a crash mid-save.
                signex_types::atomic_io::atomic_write(&path, text.as_bytes())?;
                path
            }
            None => {
                let file = SymbolFile::from_symbol(sym.clone());
                let path = self.fresh_symbol_file_path(&dir, &file)?;
                // v0.18.4 — emit TOML envelope.
                let text = file
                    .to_toml_string()
                    .map_err(|e| LibraryError::Backend(format!("write symbol container: {e}")))?;
                // HI-6: atomic write — the symbol container is a TOML+TSV
                // envelope holding every symbol; an in-place truncate by
                // `fs::write` would destroy them all on a crash mid-save.
                signex_types::atomic_io::atomic_write(&path, text.as_bytes())?;
                path
            }
        };

        let rel_path = target_path
            .strip_prefix(&self.root_dir)
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

    pub(super) fn list_primitive_summaries<T>(
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
            // v0.18.4/v0.18.5 — `.snxfpt` and `.snxsim` migrated to
            // TOML envelopes. Read the envelope, pull the first
            // contained primitive, then JSON-round-trip into the
            // generic T (= Footprint or = SimModel).
            let value: T = if matches!(kind, PrimitiveKind::Footprint) {
                let file = crate::primitive::FootprintFile::from_bytes(&bytes)
                    .map_err(|e| LibraryError::Backend(format!("list primitive {name}: {e}")))?;
                let fp = file
                    .footprints
                    .into_iter()
                    .next()
                    .ok_or_else(|| LibraryError::Backend(format!("empty .snxfpt {name}")))?;
                let buf = serde_json::to_vec(&fp).map_err(|e| {
                    LibraryError::Backend(format!("re-serialise .snxfpt {name}: {e}"))
                })?;
                serde_json::from_slice(&buf)
                    .map_err(|e| LibraryError::Backend(format!("list primitive {name}: {e}")))?
            } else if matches!(kind, PrimitiveKind::Sim) {
                let file = SimFile::from_bytes(&bytes)
                    .map_err(|e| LibraryError::Backend(format!("list primitive {name}: {e}")))?;
                let model = file
                    .models
                    .into_iter()
                    .next()
                    .ok_or_else(|| LibraryError::Backend(format!("empty .snxsim {name}")))?;
                let buf = serde_json::to_vec(&model).map_err(|e| {
                    LibraryError::Backend(format!("re-serialise .snxsim {name}: {e}"))
                })?;
                serde_json::from_slice(&buf)
                    .map_err(|e| LibraryError::Backend(format!("list primitive {name}: {e}")))?
            } else {
                serde_json::from_slice(&bytes)
                    .map_err(|e| LibraryError::Backend(format!("list primitive {name}: {e}")))?
            };
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
