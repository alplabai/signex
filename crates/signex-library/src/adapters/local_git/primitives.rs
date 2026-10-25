//! Primitive read/write + git-commit inherent methods on `LocalGitAdapter`.

use super::helpers::*;
use super::*;
use crate::git_identity::identity_for_repo;

impl LocalGitAdapter {
    /// Refuse before a write when the commit that must follow cannot be made.
    /// A library without `.git/` never commits, so it never needs an identity.
    /// Called first in every write-then-commit path so a refusal changes
    /// nothing on disk or in memory.
    fn ensure_commit_identity(&self) -> Result<(), LibraryError> {
        if !self.root_dir.join(".git").exists() {
            return Ok(());
        }
        let repo = git2::Repository::open(&self.root_dir)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        identity_for_repo(&repo).map(|_| ())
    }

    fn primitive_dir(&self, kind: PrimitiveKind) -> PathBuf {
        self.root_dir.join(primitive_subdir(kind))
    }

    fn primitive_path(&self, kind: PrimitiveKind, uuid: Uuid) -> PathBuf {
        self.primitive_dir(kind)
            .join(format!("{uuid}.{}", primitive_ext(kind)))
    }

    /// Bytes of the primitive file at `<root>/<subdir>/<uuid>.<ext>`.
    fn read_primitive_bytes(
        &self,
        kind: PrimitiveKind,
        uuid: Uuid,
    ) -> Result<Vec<u8>, LibraryError> {
        let path = self.primitive_path(kind, uuid);
        if !path.exists() {
            return Err(LibraryError::NotFound(format!(
                "{} {uuid}",
                primitive_kind_str(kind)
            )));
        }
        Ok(fs::read(&path)?)
    }

    /// Read the first footprint of the `.snxfpt` TOML+TSV envelope (v0.18.4).
    ///
    /// Typed per kind rather than generic over `T: DeserializeOwned`: the
    /// generic form had to serialise the footprint to JSON and parse it back
    /// just to hand the caller the value it already had (#534).
    pub(super) fn read_footprint(&self, uuid: Uuid) -> Result<Footprint, LibraryError> {
        let bytes = self.read_primitive_bytes(PrimitiveKind::Footprint, uuid)?;
        crate::primitive::FootprintFile::from_bytes(&bytes)
            .map_err(|e| LibraryError::Backend(format!("read .snxfpt: {e}")))?
            .footprints
            .into_iter()
            .next()
            .ok_or_else(|| LibraryError::Backend("empty FootprintFile".into()))
    }

    /// Read the first model of the `.snxsim` TOML envelope (v0.18.5).
    pub(super) fn read_sim(&self, uuid: Uuid) -> Result<SimModel, LibraryError> {
        let bytes = self.read_primitive_bytes(PrimitiveKind::Sim, uuid)?;
        SimFile::from_bytes(&bytes)
            .map_err(|e| LibraryError::Backend(format!("read .snxsim: {e}")))?
            .models
            .into_iter()
            .next()
            .ok_or_else(|| LibraryError::Backend("empty SimFile".into()))
    }

    /// Persist `fp` into `<root>/footprints/<uuid>.snxfpt`, merging into the
    /// existing envelope (HI-7) so sibling footprints survive, then commit.
    pub(super) fn write_footprint(
        &self,
        fp: &Footprint,
        message: &str,
    ) -> Result<(), LibraryError> {
        let (rel_path, abs_path) =
            self.prepare_primitive_write(PrimitiveKind::Footprint, fp.uuid)?;
        // The container is read, merged and rewritten: another Signex
        // process doing the same would drop this edit (#893, see
        // `write_lock`). The cascade that follows takes the lock itself.
        let _write_lock = self.lock_for_write()?;
        let mut file = match fs::read(&abs_path) {
            Ok(existing) => crate::primitive::FootprintFile::from_bytes(&existing)
                .map_err(|e| LibraryError::Backend(format!("re-read .snxfpt: {e}")))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                crate::primitive::FootprintFile::from_footprint(fp.clone())
            }
            Err(e) => return Err(LibraryError::from(e)),
        };
        // Replace by uuid; preserve siblings.
        if let Some(slot) = file.footprints.iter_mut().find(|f| f.uuid == fp.uuid) {
            *slot = fp.clone();
        } else {
            file.footprints.push(fp.clone());
        }
        file.updated = chrono::Utc::now();
        let bytes = file
            .to_toml_string()
            .map_err(|e| LibraryError::Backend(format!("emit .snxfpt: {e}")))?
            .into_bytes();
        self.persist_primitive(
            PrimitiveKind::Footprint,
            fp.uuid,
            &rel_path,
            &abs_path,
            &bytes,
            message,
        )
    }

    /// Persist `model` into `<root>/sims/<uuid>.snxsim`, merging into the
    /// existing envelope (HI-7) so a multi-model file is not clobbered.
    pub(super) fn write_sim(&self, model: &SimModel, message: &str) -> Result<(), LibraryError> {
        let (rel_path, abs_path) = self.prepare_primitive_write(PrimitiveKind::Sim, model.uuid)?;
        // The container is read, merged and rewritten: another Signex
        // process doing the same would drop this edit (#893, see
        // `write_lock`). The cascade that follows takes the lock itself.
        let _write_lock = self.lock_for_write()?;
        let mut file = match fs::read(&abs_path) {
            Ok(existing) => SimFile::from_bytes(&existing)
                .map_err(|e| LibraryError::Backend(format!("re-read .snxsim: {e}")))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                SimFile::from_model(model.clone())
            }
            Err(e) => return Err(LibraryError::from(e)),
        };
        if let Some(slot) = file.models.iter_mut().find(|m| m.uuid == model.uuid) {
            *slot = model.clone();
        } else {
            file.models.push(model.clone());
        }
        file.updated = chrono::Utc::now();
        let bytes = file
            .to_toml_string()
            .map_err(|e| LibraryError::Backend(format!("emit .snxsim: {e}")))?
            .into_bytes();
        self.persist_primitive(
            PrimitiveKind::Sim,
            model.uuid,
            &rel_path,
            &abs_path,
            &bytes,
            message,
        )
    }

    /// Refuse before touching disk if the commit cannot be made, create the
    /// primitive directory, and return the (repo-relative, absolute) path.
    /// Symbols go through `save_symbol_in_container` instead.
    fn prepare_primitive_write(
        &self,
        kind: PrimitiveKind,
        uuid: Uuid,
    ) -> Result<(String, PathBuf), LibraryError> {
        self.ensure_commit_identity()?;
        fs::create_dir_all(self.primitive_dir(kind))?;
        let rel_path = format!("{}/{uuid}.{}", primitive_subdir(kind), primitive_ext(kind));
        let abs_path = self.root_dir.join(&rel_path);
        Ok((rel_path, abs_path))
    }

    fn persist_primitive(
        &self,
        kind: PrimitiveKind,
        uuid: Uuid,
        rel_path: &str,
        abs_path: &Path,
        bytes: &[u8],
        message: &str,
    ) -> Result<(), LibraryError> {
        // HI-6: atomic write — primitive containers (`.snxsim`,
        // `.snxfpt`, `.snxsym`) are TOML+TSV envelopes; a half-written
        // file destroys all primitives in the container, so we cannot
        // afford the in-place truncate that `fs::write` does on
        // existing destinations.
        signex_types::atomic_io::atomic_write(abs_path, bytes)?;
        let fallback = format!("save {} {uuid}", primitive_kind_str(kind));
        self.commit_path(rel_path, message, &fallback)
    }

    /// Stage `rel_path` and create a new commit, unless the staged tree
    /// matches HEAD's (an unchanged save). Used by primitive saves
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
        self.commit_change(None, rel_path, message, fallback_message)
    }

    /// Like [`commit_path`](Self::commit_path), but also drops
    /// `removed_rel_path` from the index, so a file moved to `rel_path` is
    /// recorded as a rename rather than left tracked at its old name.
    pub(super) fn commit_change(
        &self,
        removed_rel_path: Option<&str>,
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
        let (sig_name, sig_email) = identity_for_repo(&repo)?;
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

        let commit_message = if message.is_empty() {
            fallback_message.to_string()
        } else {
            message.to_string()
        };
        retry_on_git_lock(repo.path(), || {
            stage_and_commit(
                &repo,
                removed_rel_path.map(Path::new),
                Path::new(rel_path),
                &sig,
                &commit_message,
            )
        })
        .map_err(|e| LibraryError::Backend(format!("git commit {rel_path}: {e}")))?;
        Ok(())
    }

    // ── Table helpers ──────────────────────────────────────────────────────

    pub(super) fn snxlib_rel_path(&self) -> Result<String, LibraryError> {
        file_name_str(&self.file_path)
    }

    /// Persist the in-memory `library_file` to disk. Caller already holds
    /// the appropriate read/write lock.
    fn persist_library_file(&self, lf: &LibraryFile) -> Result<(), LibraryError> {
        let text = lf.write()?;
        // HI-6: atomic write — never half-write the manifest.
        signex_types::atomic_io::atomic_write(&self.file_path, text.as_bytes())?;
        self.note_own_write(&text)
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
        self.mutate_library_file_if_changed(
            |lf| {
                f(lf)?;
                Ok(true)
            },
            message,
            fallback,
        )
    }

    /// Like [`Self::mutate_library_file`], but the closure reports whether it
    /// changed anything. `Ok(false)` skips both the write and the commit so a
    /// no-op never lands an empty commit in the library's history.
    pub(super) fn mutate_library_file_if_changed<F>(
        &self,
        f: F,
        message: &str,
        fallback: &str,
    ) -> Result<(), LibraryError>
    where
        F: FnOnce(&mut LibraryFile) -> Result<bool, LibraryError>,
    {
        self.ensure_commit_identity()?;
        // Another Signex process could otherwise reload and write between
        // our reload and our write (see `write_lock`). Taken before the
        // in-process guard, and held until the write is done.
        let _write_lock = self.lock_for_write()?;
        let mut guard = self
            .library_file
            .write()
            .map_err(|_| LibraryError::Backend("library_file write lock poisoned".into()))?;
        // An external edit must be merged in, not overwritten by a stale
        // copy; an unreadable file refuses the edit (see `disk_sync`).
        self.reload_if_stale(&mut guard, super::disk_sync::Verify::Content)?;
        if !f(&mut guard)? {
            return Ok(());
        }
        self.persist_library_file(&guard)?;
        let rel = self.snxlib_rel_path()?;
        self.commit_path(&rel, message, fallback)
    }

    /// Read the table named `table` as a snapshot of [`ComponentRow`]s,
    /// scoped to whatever the legacy [`TABLE_HEADER`] columns are.
    /// Returns an empty vec for unknown tables (matches the old
    /// `tables/<name>.tsv` "missing file = empty" semantics).
    pub(super) fn snapshot_table(&self, name: &str) -> Result<Vec<ComponentRow>, LibraryError> {
        self.refresh_for_read();
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
            self.record_symbol_problems(&[]);
            return Ok(Vec::new());
        }
        let suffix = format!(".{SYMBOL_EXT}");
        let mut out: Vec<(PathBuf, SymbolFile)> = Vec::new();
        let mut problems: Vec<SymbolFileProblem> = Vec::new();
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
            // #690 — a container that cannot be read or parsed affects only
            // itself: skip it, never rewrite it, and record it so it is
            // reported (see `symbol_file_problems`).
            let parsed = fs::read(path)
                .map_err(|e| e.to_string())
                // TOML-only (alpha policy, no legacy JSON support —
                // pre-v0.18.4 libraries must be regenerated). `from_bytes`
                // decodes UTF-8 and hands off to `from_toml_str`; the
                // format-token check inside surfaces `UnsupportedFormat`
                // for any drift.
                .and_then(|bytes| SymbolFile::from_bytes(&bytes).map_err(|e| e.to_string()));
            match parsed {
                Ok(file) => out.push((path.to_path_buf(), file)),
                Err(error) => problems.push(SymbolFileProblem::new(path.to_path_buf(), &error)),
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        self.record_symbol_problems(&problems);
        Ok(out)
    }

    /// Containers the most recent symbol scan skipped because they could not
    /// be read or parsed. Empty when every `.snxsym` is healthy.
    pub fn symbol_file_problems(&self) -> Vec<SymbolFileProblem> {
        self.symbol_problems
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current()
    }

    /// Update the ledger and log each newly seen problem once. The app's
    /// logger routes `tracing::warn!` into the Messages panel.
    fn record_symbol_problems(&self, found: &[SymbolFileProblem]) {
        let fresh = self
            .symbol_problems
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .observe(found);
        for p in fresh {
            tracing::warn!(
                "Skipped unreadable symbol file {} (left untouched): {}",
                p.path.display(),
                p.error
            );
        }
    }

    fn refuse_if_skipped_file_mentions(&self, uuid: Uuid) -> Result<(), LibraryError> {
        let hyphenated = uuid.hyphenated().to_string();
        let simple = uuid.simple().to_string();
        for p in self.symbol_file_problems() {
            // A file we cannot read may hold this symbol, so treat it as a match.
            let mentions = fs::read(&p.path)
                .map(|bytes| {
                    let text = String::from_utf8_lossy(&bytes).to_lowercase();
                    text.contains(&hyphenated) || text.contains(&simple)
                })
                .unwrap_or(true);
            if mentions {
                return Err(LibraryError::Backend(format!(
                    "symbol {uuid} appears in unreadable symbol file {} ({}); \
                     not saving a second copy",
                    p.path.display(),
                    p.error
                )));
            }
        }
        Ok(())
    }

    /// Error for a symbol lookup that found nothing while some containers
    /// were skipped: the symbol may live in one of them, so "not found"
    /// would be a guess.
    pub(super) fn symbol_lookup_undetermined(&self, uuid: Uuid) -> Option<LibraryError> {
        let problems = self.symbol_file_problems();
        if problems.is_empty() {
            return None;
        }
        let names: Vec<String> = problems
            .iter()
            .map(|p| p.path.display().to_string())
            .collect();
        Some(LibraryError::Backend(format!(
            "symbol {uuid} not found; {} symbol file(s) could not be read: {}",
            problems.len(),
            names.join(", ")
        )))
    }

    pub(super) fn save_symbol_in_container(
        &self,
        sym: Symbol,
        message: &str,
    ) -> Result<(), LibraryError> {
        self.ensure_commit_identity()?;
        // The container is read, merged and rewritten: another Signex
        // process doing the same would drop this edit (#893, see
        // `write_lock`). The cascade that follows takes the lock itself.
        let _write_lock = self.lock_for_write()?;
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
                // A skipped container whose text mentions this uuid may
                // already hold the symbol; writing a second copy elsewhere
                // would fork it. Refuse that one case, name the file.
                self.refuse_if_skipped_file_mentions(sym.uuid)?;
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

    /// One [`PrimitiveSummary`] per `<uuid>.<ext>` file directly under
    /// `<root>/<subdir>`, sorted by name.
    ///
    /// Only the primitive's *name* survives, so the envelope is parsed
    /// into its concrete type and `.name` lifted straight off it. This
    /// used to be generic over `T: DeserializeOwned` with a `name_of`
    /// extractor, which forced a full `serde_json` serialise +
    /// deserialise **per file** purely to teach the compiler that
    /// `T == Footprint` / `T == SimModel` — every byte of which was
    /// discarded on the next line (#99).
    pub(super) fn list_primitive_summaries(
        &self,
        kind: PrimitiveKind,
    ) -> Result<Vec<PrimitiveSummary>, LibraryError> {
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
            let name = match primitive_name_in_envelope(kind, &bytes, name)? {
                Ok(n) => n,
                Err(e) => {
                    self.warn_footprint_skipped_once(path, &e);
                    continue;
                }
            };
            out.push(PrimitiveSummary {
                uuid,
                name,
                kind,
                used_by_count: 0,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Warn about a skipped `.snxfpt` once per (path, mtime): listings run
    /// on every library refresh and must not repeat the same line.
    /// Returns whether this call was the first for that file version.
    pub(super) fn warn_footprint_skipped_once(&self, path: &std::path::Path, err: &str) -> bool {
        let mtime = fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        let first = self
            .skipped_footprints
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert((path.to_path_buf(), mtime));
        if first {
            tracing::warn!(
                "Skipped unreadable footprint {} (left untouched): {err}",
                path.display()
            );
        }
        first
    }
}

/// Name of the first primitive inside a `.snxfpt` / `.snxsim` envelope.
///
/// #924 — like #690 for symbols, a `.snxfpt` that fails to parse (e.g. a
/// `nan`/`inf` value) affects only itself: the inner `Err` carries the
/// parse error so the caller can skip and warn. `.snxsim` stays strict —
/// a parse failure is an outer `Err`, as before.
///
/// `file_name` is diagnostic only — it names the offending file in the
/// error text, which is what the user sees when a container is corrupt.
/// A well-formed envelope carrying *zero* primitives is corrupt, not
/// empty, and must fail loudly rather than vanish from the listing.
///
/// HI-8: [`PrimitiveKind::Symbol`] cannot arrive here. Symbol
/// containers are `<slug>.snxsym`, so the uuid-stem parse in
/// `list_primitive_summaries` would skip them anyway, and
/// `list_symbols` walks them through `scan_symbol_files` instead. The
/// match is spelled out per variant with no `_` arm, so a kind added
/// to [`PrimitiveKind`] later is a compile error here rather than a
/// listing that silently comes back empty.
fn primitive_name_in_envelope(
    kind: PrimitiveKind,
    bytes: &[u8],
    file_name: &str,
) -> Result<Result<String, String>, LibraryError> {
    match kind {
        PrimitiveKind::Footprint => {
            let file = match crate::primitive::FootprintFile::from_bytes(bytes) {
                Ok(f) => f,
                Err(e) => return Ok(Err(e.to_string())),
            };
            let fp = file
                .footprints
                .into_iter()
                .next()
                .ok_or_else(|| LibraryError::Backend(format!("empty .snxfpt {file_name}")))?;
            Ok(Ok(fp.name))
        }
        PrimitiveKind::Sim => {
            let file = SimFile::from_bytes(bytes)
                .map_err(|e| LibraryError::Backend(format!("list primitive {file_name}: {e}")))?;
            let model = file
                .models
                .into_iter()
                .next()
                .ok_or_else(|| LibraryError::Backend(format!("empty .snxsim {file_name}")))?;
            Ok(Ok(model.name))
        }
        PrimitiveKind::Symbol => {
            debug_assert!(
                false,
                "list_primitive_summaries(Symbol) must route through scan_symbol_files; \
                 a .snxsym is a multi-symbol container, not one uuid-named primitive"
            );
            Err(LibraryError::Backend(format!(
                "list primitive {file_name}: symbols are listed via scan_symbol_files"
            )))
        }
    }
}
