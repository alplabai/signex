//! `LibraryAdapter` trait implementation for `LocalGitAdapter`.

use super::helpers::*;
use super::*;

impl LibraryAdapter for LocalGitAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest_synth
    }

    fn library_file(&self) -> Option<&LibraryFile> {
        // Returning `&LibraryFile` from inside a `RwLock` would require a
        // self-referential guard, which the trait shape can't express.
        // Stage 5 will replace this with a closure-based accessor; for
        // now the trait method stays `None` for git-backed adapters and
        // callers wanting the parsed view go through other surfaces
        // (`list_tables` / `read_table`). The DB adapter returns `None`
        // because it has no `.snxlib` on disk.
        None
    }

    fn root_dir(&self) -> Option<&Path> {
        Some(&self.root_dir)
    }

    fn library_file_path(&self) -> Option<&Path> {
        Some(&self.file_path)
    }

    fn relocate_library_file(&mut self, new_file: &Path) -> Result<(), LibraryError> {
        Self::validate_file_path(new_file)?;
        let new_root = parent_dir(new_file)?;
        if new_root == self.root_dir {
            // Record the rename in the library's own history before the
            // adapter moves, so a failed commit leaves it on the old path.
            let old_rel = self.snxlib_rel_path()?;
            let new_rel = file_name_str(new_file)?;
            self.commit_change(
                Some(&old_rel),
                &new_rel,
                "",
                &format!("rename {old_rel} to {new_rel}"),
            )?;
        }
        self.root_dir = new_root;
        self.file_path = new_file.to_path_buf();
        Ok(())
    }

    // ── Tables ─────────────────────────────────────────────────────────────

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        self.refresh_for_read();
        let guard = self
            .library_file
            .read()
            .map_err(|_| LibraryError::Backend("library_file read lock poisoned".into()))?;
        let mut names: Vec<String> = guard.tables.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    fn rename_table(&self, old: &str, new: &str, msg: &str) -> Result<(), LibraryError> {
        let old_owned = old.to_string();
        let new_trimmed = new.trim().to_string();
        if new_trimmed.is_empty() {
            return Err(LibraryError::Backend("table name cannot be empty".into()));
        }
        if new_trimmed.chars().any(|c| {
            matches!(
                c,
                '/' | '\\' | '.' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        }) {
            return Err(LibraryError::Backend(format!(
                "table name {new_trimmed:?} contains illegal characters"
            )));
        }
        if old_owned == new_trimmed {
            return Ok(());
        }
        let new_owned = new_trimmed;
        let fallback = format!("rename table {old_owned} → {new_owned}");
        self.mutate_library_file(
            move |lf| {
                if !lf.tables.contains_key(&old_owned) {
                    return Err(LibraryError::NotFound(format!(
                        "table {old_owned:?} not found"
                    )));
                }
                if lf.tables.contains_key(&new_owned) {
                    return Err(LibraryError::Conflict(format!(
                        "table {new_owned:?} already exists"
                    )));
                }
                let entry = lf.tables.remove(&old_owned).expect("contains_key checked");
                lf.tables.insert(new_owned, entry);
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn delete_empty_table(&self, name: &str, msg: &str) -> Result<(), LibraryError> {
        let owned = name.to_string();
        let fallback = format!("delete table {owned}");
        self.mutate_library_file(
            move |lf| {
                // Single-pass check + remove via `entry()` — avoids
                // the read-then-write borrow pattern that NLL only
                // tolerates because the first borrow ends at the
                // `if`. Using `entry()` keeps the BTreeMap touch
                // atomic and is robust under future reorganisation.
                use std::collections::btree_map::Entry;
                match lf.tables.entry(owned.clone()) {
                    Entry::Vacant(_) => {
                        Err(LibraryError::NotFound(format!("table {owned:?} not found")))
                    }
                    Entry::Occupied(occ) if !occ.get().rows.is_empty() => {
                        Err(LibraryError::Conflict(format!(
                            "table {owned:?} is not empty ({} rows)",
                            occ.get().rows.len()
                        )))
                    }
                    Entry::Occupied(occ) => {
                        occ.remove();
                        Ok(())
                    }
                }
            },
            msg,
            &fallback,
        )
    }

    fn library_classes(&self) -> Vec<crate::library_file::ClassEntry> {
        self.refresh_for_read();
        classes_or_report(&self.library_file, &self.file_path)
    }

    fn update_library_classes(
        &self,
        classes: Vec<crate::library_file::ClassEntry>,
        msg: &str,
    ) -> Result<(), LibraryError> {
        self.mutate_library_file(
            move |lf| {
                lf.manifest.classes = classes;
                Ok(())
            },
            msg,
            "update class registry",
        )
    }

    fn add_library_class(
        &self,
        entry: crate::library_file::ClassEntry,
        msg: &str,
    ) -> Result<(), LibraryError> {
        // Atomic override of the trait default — read + check +
        // append all happen inside one `mutate_library_file` borrow
        // so concurrent callers can't interleave a duplicate add.
        let fallback = format!("add class {}", entry.key);
        self.mutate_library_file(
            move |lf| {
                if lf.manifest.classes.iter().any(|c| c.key == entry.key) {
                    return Err(LibraryError::Conflict(format!(
                        "class with key {:?} already exists",
                        entry.key
                    )));
                }
                lf.manifest.classes.push(entry);
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn remove_library_class(&self, key: &str, msg: &str) -> Result<(), LibraryError> {
        let owned_key = key.to_string();
        let fallback = format!("remove class {owned_key}");
        self.mutate_library_file_if_changed(
            move |lf| {
                let before = lf.manifest.classes.len();
                lf.manifest.classes.retain(|c| c.key != owned_key);
                // Absent key: no error (idempotent `×`), no write, no commit.
                Ok(lf.manifest.classes.len() != before)
            },
            msg,
            &fallback,
        )
    }

    fn rename_library_class(
        &self,
        old_key: &str,
        new_entry: crate::library_file::ClassEntry,
        msg: &str,
    ) -> Result<(), LibraryError> {
        let owned_old = old_key.to_string();
        let fallback = format!("rename class {owned_old} → {}", new_entry.key);
        self.mutate_library_file(
            move |lf| {
                if !lf.manifest.classes.iter().any(|c| c.key == owned_old) {
                    return Err(LibraryError::NotFound(format!(
                        "class {owned_old:?} not found"
                    )));
                }
                if new_entry.key != owned_old
                    && lf.manifest.classes.iter().any(|c| c.key == new_entry.key)
                {
                    return Err(LibraryError::Conflict(format!(
                        "class with key {:?} already exists",
                        new_entry.key
                    )));
                }
                for c in lf.manifest.classes.iter_mut() {
                    if c.key == owned_old {
                        *c = new_entry.clone();
                        break;
                    }
                }
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn create_empty_table(&self, name: &str, msg: &str) -> Result<(), LibraryError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(LibraryError::Backend("table name cannot be empty".into()));
        }
        // Reject the same characters we reject elsewhere for filenames /
        // identifiers — avoids surprising the user with a name they can't
        // see in their file browser.
        if trimmed.chars().any(|c| {
            matches!(
                c,
                '/' | '\\' | '.' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        }) {
            return Err(LibraryError::Backend(format!(
                "table name {trimmed:?} contains illegal characters"
            )));
        }
        let owned = trimmed.to_string();
        let fallback = format!("create empty table {owned}");
        self.mutate_library_file(
            move |lf| {
                if lf.tables.contains_key(&owned) {
                    return Err(LibraryError::Conflict(format!(
                        "table {owned:?} already exists"
                    )));
                }
                lf.tables.insert(
                    owned,
                    LibraryTable {
                        columns: legacy_columns(),
                        rows: Vec::new(),
                        column_types: std::collections::BTreeMap::new(),
                        pricing: Default::default(),
                    },
                );
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn read_table(&self, name: &str) -> Result<Vec<ComponentRow>, LibraryError> {
        self.snapshot_table(name)
    }

    fn iter_rows(&self) -> Result<Vec<(String, ComponentRow)>, LibraryError> {
        let mut out: Vec<(String, ComponentRow)> = Vec::new();
        for name in self.list_tables()? {
            for row in self.snapshot_table(&name)? {
                out.push((name.clone(), row));
            }
        }
        Ok(out)
    }

    fn read_row(&self, table: &str, row_id: RowId) -> Result<ComponentRow, LibraryError> {
        let target = row_id.as_uuid();
        self.snapshot_table(table)?
            .into_iter()
            .find(|r| r.row_id == target)
            .ok_or_else(|| LibraryError::NotFound(format!("row {row_id} in table {table}")))
    }

    /// Linear scan across every table — O(total rows). Acceptable at
    /// v0.9 scale (libraries are O(thousands)). When the search index
    /// lands the call should redirect through it.
    fn read_row_by_pn(&self, pn: &InternalPn) -> Result<(String, ComponentRow), LibraryError> {
        for (table, row) in self.iter_rows()? {
            if &row.internal_pn == pn {
                return Ok((table, row));
            }
        }
        Err(LibraryError::NotFound(format!("internal_pn {pn}")))
    }

    fn insert_row(&self, table: &str, row: ComponentRow, msg: &str) -> Result<(), LibraryError> {
        let table_owned = table.to_string();
        let row_id = row.row_id;
        let lib_row = component_to_library_row(&row)?;
        let fallback = format!("insert row {row_id} into {table_owned}");
        self.save_row_edit(
            table,
            |lf| {
                let entry = lf
                    .tables
                    .entry(table_owned.clone())
                    .or_insert_with(|| LibraryTable {
                        columns: legacy_columns(),
                        rows: Vec::new(),
                        column_types: std::collections::BTreeMap::new(),
                        pricing: Default::default(),
                    });
                validate_legacy_header(&table_owned, &entry.columns)?;
                declare_alternate_symbols_column(&mut entry.columns, &lib_row);
                entry.rows.push(Arc::new(lib_row));
                Ok(RowChange::Wrote(entry.rows.len() - 1))
            },
            msg,
            &fallback,
        )
    }

    fn update_row(
        &self,
        table: &str,
        row: ComponentRow,
        msg: &str,
    ) -> Result<Option<u64>, LibraryError> {
        let table_owned = table.to_string();
        let row_id = row.row_id;
        let base = row.server_revision;
        let lib_row = component_to_library_row(&row)?;
        let new_base = row_base(&lib_row);
        let fallback = format!("update row {row_id} in {table_owned}");
        self.save_row_edit(
            table,
            |lf| {
                let entry = lf.tables.get_mut(&table_owned).ok_or_else(|| {
                    LibraryError::NotFound(format!("table {table_owned} not in library"))
                })?;
                validate_legacy_header(&table_owned, &entry.columns)?;
                let row_id_s = row_id.to_string();
                let index = entry
                    .rows
                    .iter()
                    .position(|r| {
                        r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str)
                            == Some(row_id_s.as_str())
                    })
                    .ok_or_else(|| {
                        LibraryError::NotFound(format!("row {row_id} in table {table_owned}"))
                    })?;
                // Checked against the row as it is on disk now (the write
                // path just re-read it), not a cache.
                check_row_base(&entry.rows[index], base)?;
                declare_alternate_symbols_column(&mut entry.columns, &lib_row);
                entry.rows[index] = Arc::new(lib_row);
                Ok(RowChange::Wrote(index))
            },
            msg,
            &fallback,
        )?;
        Ok(Some(new_base))
    }

    fn delete_row(
        &self,
        table: &str,
        row_id: RowId,
        base_revision: Option<u64>,
        msg: &str,
    ) -> Result<(), LibraryError> {
        let table_owned = table.to_string();
        let fallback = format!("delete row {row_id} from {table_owned}");
        self.save_row_edit(
            table,
            |lf| {
                let entry = lf.tables.get_mut(&table_owned).ok_or_else(|| {
                    LibraryError::NotFound(format!("table {table_owned} not in library"))
                })?;
                let target = row_id.as_uuid().to_string();
                if let Some(stored) = entry.rows.iter().find(|r| {
                    r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(target.as_str())
                }) {
                    check_row_base(stored, base_revision)?;
                }
                let before = entry.rows.len();
                entry.rows.retain(|r| {
                    r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) != Some(target.as_str())
                });
                if entry.rows.len() == before {
                    return Err(LibraryError::NotFound(format!(
                        "row {row_id} in table {table_owned}"
                    )));
                }
                // A deleted row's cached prices go with it.
                entry.pricing.remove(&target);
                Ok(RowChange::Removed(target))
            },
            msg,
            &fallback,
        )
    }

    fn table_pricing(
        &self,
        table: &str,
    ) -> Result<std::collections::BTreeMap<Uuid, crate::pricing::RowPricing>, LibraryError> {
        self.refresh_for_read();
        let guard = self
            .library_file
            .read()
            .map_err(|_| LibraryError::Backend("library_file read lock poisoned".into()))?;
        let Some(entry) = guard.tables.get(table) else {
            return Ok(Default::default());
        };
        Ok(entry
            .pricing
            .iter()
            .filter_map(|(id, p)| Uuid::parse_str(id).ok().map(|id| (id, p.clone())))
            .collect())
    }

    fn store_pricing(
        &self,
        updates: Vec<crate::pricing::PricingUpdate>,
        msg: &str,
    ) -> Result<crate::pricing::StoreOutcome, LibraryError> {
        let mut outcome = crate::pricing::StoreOutcome::default();
        let fallback = format!("refresh pricing for {} rows", updates.len());
        // Every update is checked against the row as it is under the write
        // lock, after the reload, so a fetch that raced an edit cannot
        // attach a price to a SKU the row no longer has.
        self.mutate_library_file_if_changed(
            |lf| {
                for update in updates {
                    match apply_pricing_update(lf, &update) {
                        Ok(()) => outcome.stored += 1,
                        Err(reason) => outcome.dropped.push(crate::pricing::DroppedUpdate {
                            table: update.table,
                            row_id: update.row_id,
                            distributor: update.distributor,
                            reason,
                        }),
                    }
                }
                Ok(outcome.stored > 0)
            },
            msg,
            &fallback,
        )?;
        Ok(outcome)
    }

    fn get_symbol(&self, uuid: Uuid) -> Result<Symbol, LibraryError> {
        for (_, file) in self.scan_symbol_files()? {
            if let Some(sym) = file.get_symbol(uuid) {
                return Ok(sym.clone());
            }
        }
        if let Some(err) = self.symbol_lookup_undetermined(uuid) {
            return Err(err);
        }
        Err(LibraryError::NotFound(format!("symbol {uuid}")))
    }

    fn get_footprint(&self, uuid: Uuid) -> Result<Footprint, LibraryError> {
        self.read_footprint(uuid)
    }

    fn get_sim(&self, uuid: Uuid) -> Result<SimModel, LibraryError> {
        self.read_sim(uuid)
    }

    fn save_symbol(&self, sym: Symbol, message: &str) -> Result<(), LibraryError> {
        let uuid = sym.uuid;
        let new_version = sym.version.clone();
        self.save_symbol_in_container(sym, message)?;
        // Stage 15 cascade: propagate the new symbol version to bound
        // ComponentRows. Personal mode silently auto-bumps everything;
        // Team mode auto-bumps non-released rows + leaves released
        // rows flagged as stale (the Library Browser surface picks
        // them up via the existing stale-binding indicator).
        let mode = self.manifest_synth.workflow.mode;
        let _report = crate::cascade::cascade_after_symbol_save(self, uuid, &new_version, mode)?;
        Ok(())
    }

    fn save_footprint(&self, fp: Footprint, message: &str) -> Result<(), LibraryError> {
        let uuid = fp.uuid;
        let new_version = fp.version.clone();
        self.write_footprint(&fp, message)?;
        let mode = self.manifest_synth.workflow.mode;
        let _report = crate::cascade::cascade_after_footprint_save(self, uuid, &new_version, mode)?;
        Ok(())
    }

    fn save_sim(&self, sm: SimModel, message: &str) -> Result<(), LibraryError> {
        let uuid = sm.uuid;
        let new_version = sm.version.clone();
        self.write_sim(&sm, message)?;
        let mode = self.manifest_synth.workflow.mode;
        let _report = crate::cascade::cascade_after_sim_save(self, uuid, &new_version, mode)?;
        Ok(())
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
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

    fn symbol_listing_is_partial(&self) -> bool {
        !self.symbol_file_problems().is_empty()
    }

    fn list_footprints(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitive_summaries(PrimitiveKind::Footprint)
    }

    fn list_sims(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitive_summaries(PrimitiveKind::Sim)
    }

    fn bind_row_primitive_version(
        &self,
        table: &str,
        row_id: RowId,
        save: &crate::cascade::PrimitiveSave,
        message: &str,
    ) -> Result<bool, LibraryError> {
        let table_owned = table.to_string();
        let target = row_id.as_uuid().to_string();
        let fallback = format!("bind row {row_id} in {table} to v{}", save.version);
        let mut changed = false;
        // The row is found and rewritten under the write lock, after the
        // reload, so a field another writer changed since the caller read
        // the row is kept.
        self.save_row_edit(
            table,
            |lf| {
                let entry = lf.tables.get_mut(&table_owned).ok_or_else(|| {
                    LibraryError::NotFound(format!("table {table_owned} not in library"))
                })?;
                validate_legacy_header(&table_owned, &entry.columns)?;
                let index = entry
                    .rows
                    .iter()
                    .position(|r| {
                        r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(target.as_str())
                    })
                    .ok_or_else(|| {
                        LibraryError::NotFound(format!("row {target} in table {table_owned}"))
                    })?;
                let mut row = library_row_to_component(&entry.rows[index])?;
                if !crate::cascade::needs_bump(&row, save) {
                    return Ok(RowChange::None);
                }
                crate::cascade::apply_cascade_bump(&mut row, save);
                entry.rows[index] = Arc::new(component_to_library_row(&row)?);
                changed = true;
                Ok(RowChange::Wrote(index))
            },
            message,
            &fallback,
        )?;
        Ok(changed)
    }

    fn root_path(&self) -> Option<PathBuf> {
        Some(self.root_dir.clone())
    }

    fn rewrite_file_locked(
        &self,
        abs_path: &Path,
        commit_message: &str,
        rewrite: &mut crate::adapter::FileRewrite<'_>,
    ) -> Result<bool, LibraryError> {
        if !abs_path.starts_with(&self.root_dir) {
            return Err(LibraryError::Backend(format!(
                "rewrite_file_locked: {} is not under {}",
                abs_path.display(),
                self.root_dir.display(),
            )));
        }
        let _write_lock = self.lock_for_write()?;
        let written = crate::adapter::rewrite_file(abs_path, rewrite)?;
        if written && let Err(e) = self.commit_external_change(abs_path, commit_message) {
            tracing::warn!(
                target: "signex::library",
                path = %abs_path.display(),
                error = %e,
                "save primitive: file written, not committed",
            );
        }
        Ok(written)
    }

    fn commit_external_change(&self, abs_path: &Path, message: &str) -> Result<(), LibraryError> {
        let rel_path = abs_path
            .strip_prefix(&self.root_dir)
            .map_err(|_| {
                LibraryError::Backend(format!(
                    "commit_external_change: {} is not under {}",
                    abs_path.display(),
                    self.root_dir.display(),
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let fallback = format!("save {rel_path}");
        self.commit_path(&rel_path, message, &fallback)
    }

    fn history(&self, primitive_path: &Path) -> Result<Vec<HistoryEntry>, LibraryError> {
        // Stage 17 scaffold: walk the repo's commit graph newest-first
        // and keep commits whose tree differs from at least one parent
        // at `primitive_path`. Mirrors `git log --follow --max-count 50
        // -- <path>` semantics (per `v0.9-snxlib-as-file-plan.md` §3
        // "Performance"), minus the rename-follow heuristic — git2
        // doesn't expose `--follow` directly so a future stage layers
        // it on. For now plain pathspec match is enough; the file
        // names are uuid-keyed so renames are rare in practice.
        const MAX_ENTRIES: usize = 50;

        let rel_path = if primitive_path.is_absolute() {
            primitive_path
                .strip_prefix(&self.root_dir)
                .map_err(|_| {
                    LibraryError::NotFound(format!(
                        "history: {} is not under {}",
                        primitive_path.display(),
                        self.root_dir.display(),
                    ))
                })?
                .to_path_buf()
        } else {
            primitive_path.to_path_buf()
        };
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");

        // Libraries created with `enable_git = false` have no `.git/`
        // and therefore no history to walk. Surface that as an empty
        // log rather than a hard error so the History panel can show
        // an "(no version control)" placeholder.
        if !self.root_dir.join(".git").exists() {
            return Ok(Vec::new());
        }
        let repo = git2::Repository::open(&self.root_dir)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;

        let mut walk = repo
            .revwalk()
            .map_err(|e| LibraryError::Backend(format!("git revwalk: {e}")))?;
        // Topological + time so that on equal-second timestamps (the
        // common case in tests + back-to-back saves on Windows where
        // git2 stamps to one-second precision) child commits still
        // come before parents — matches `git log`'s default visual
        // order.
        walk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)
            .map_err(|e| LibraryError::Backend(format!("git revwalk sort: {e}")))?;

        // Unborn HEAD (fresh repo, no commits yet) is a legitimate
        // "no history" answer rather than an error — the editor
        // should render an empty list, not a red banner.
        match walk.push_head() {
            Ok(()) => {}
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(Vec::new()),
            Err(e) => return Err(LibraryError::Backend(format!("git push head: {e}"))),
        }

        let mut entries: Vec<HistoryEntry> = Vec::new();
        for oid_res in walk {
            if entries.len() >= MAX_ENTRIES {
                break;
            }
            let oid =
                oid_res.map_err(|e| LibraryError::Backend(format!("git revwalk oid: {e}")))?;
            let commit = repo
                .find_commit(oid)
                .map_err(|e| LibraryError::Backend(format!("git find commit: {e}")))?;

            if !commit_touches_path(&repo, &commit, &rel_str)? {
                continue;
            }

            entries.push(commit_to_history_entry(&commit));
        }
        Ok(entries)
    }
}
