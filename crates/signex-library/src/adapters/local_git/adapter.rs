//! `LibraryAdapter` trait implementation for `LocalGitAdapter`.

use super::helpers::*;
use super::*;
use crate::adapter::UncommittedChanges;

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
            // The file is already renamed on disk, so a failed commit must
            // not leave the adapter on the old path: the rename stays
            // pending and the next commit records it (#858).
            self.commit_change(
                Some(&old_rel),
                &new_rel,
                "",
                &format!("rename {old_rel} to {new_rel}"),
            );
        }
        let moved_root = new_root != self.root_dir;
        self.root_dir = new_root;
        self.file_path = new_file.to_path_buf();
        if moved_root {
            self.reset_uncommitted_for_new_root();
        }
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
        check_table_name(&new_trimmed)?;
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
                // Through the file, so the table keeps its unknown keys (#1435).
                lf.rename_table(&old_owned, &new_owned);
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
                // Through the file, so the class keeps its unknown keys (#1435).
                lf.rename_class(&owned_old, new_entry.clone());
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
        check_table_name(trimmed)?;
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
        self.read_component_row(table, row_id)
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
        let new_row_id = row_id.to_string();
        let new_pn = row.internal_pn.to_string();
        let fallback = format!("insert row {row_id} into {table_owned}");
        self.save_row_edit(
            table,
            |lf| {
                if !lf.tables.contains_key(&table_owned) {
                    check_table_name(&table_owned)?;
                }
                check_insert_unique(lf, &new_row_id, &new_pn)?;
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
            &[],
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
        self.update_row_with_assets(table, row, msg, &[])
    }

    fn update_row_with_assets(
        &self,
        table: &str,
        row: ComponentRow,
        msg: &str,
        asset_rel_paths: &[String],
    ) -> Result<Option<u64>, LibraryError> {
        self.check_datasheet_assets(asset_rel_paths)?;
        let table_owned = table.to_string();
        let row_id = row.row_id;
        let base = row.server_revision;
        let lib_row = component_to_library_row(&row)?;
        let new_base = row_base(&lib_row);
        let new_pn = row.internal_pn.to_string();
        let fallback = format!("update row {row_id} in {table_owned}");
        self.save_row_edit(
            table,
            |lf| {
                check_pn_change(lf, &table_owned, &row_id.to_string(), &new_pn)?;
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
            asset_rel_paths,
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
            &[],
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
        cancel: &std::sync::atomic::AtomicBool,
        msg: &str,
    ) -> Result<crate::pricing::StoreOutcome, LibraryError> {
        let mut outcome = crate::pricing::StoreOutcome::default();
        let fallback = format!("refresh pricing for {} rows", updates.len());
        // Every update is checked against the row as it is under the write
        // lock, after the reload, so a fetch that raced an edit cannot
        // attach a price to a SKU the row no longer has.
        self.mutate_library_file_if_changed(
            |lf| {
                // Last chance to stop: the lock is held, the write is next.
                if cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    return Ok(false);
                }
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
        if let Some((_, file)) = self.locate_symbol_file(uuid)?
            && let Some(sym) = file.get_symbol(uuid)
        {
            return Ok(sym.clone());
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
            &[],
            message,
            &fallback,
        )?;
        Ok(changed)
    }

    /// The whole batch in one `.snxlib` write and one commit. A target
    /// that is missing or unreadable is that target's `Err` and the rest
    /// still land; a failed write or commit refuses the whole batch. The
    /// commit message is built from the rows the edit changed.
    fn update_rows(
        &self,
        targets: &[(String, RowId)],
        edit: &mut crate::adapter::RowEdit<'_>,
        message: &crate::adapter::RowsMessage<'_>,
    ) -> Result<Vec<crate::adapter::RowEditOutcome>, LibraryError> {
        let outcomes = std::cell::RefCell::new(Vec::with_capacity(targets.len()));
        let fallback = format!("update {} rows", targets.len());
        self.mutate_library_file_deferred(
            |lf| {
                // Rows are found and edited under the write lock, after the
                // reload, so a field another writer changed since the
                // caller's scan is kept.
                let mut any = false;
                let mut done = outcomes.borrow_mut();
                for (table, row_id) in targets {
                    let outcome = edit_one_row(lf, table, *row_id, edit);
                    any |= matches!(outcome, Ok(true));
                    done.push((*row_id, outcome));
                }
                Ok(any)
            },
            &[],
            &|| {
                let changed: Vec<RowId> = outcomes
                    .borrow()
                    .iter()
                    .filter(|(_, o)| matches!(o, Ok(true)))
                    .map(|(id, _)| *id)
                    .collect();
                message(&changed)
            },
            &fallback,
        )?;
        Ok(outcomes.into_inner())
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
        // The file is written: its commit, failed or not, is reported by
        // the adapter (see `uncommitted`) and never undoes the write.
        if written {
            self.commit_external_change(abs_path, commit_message)?;
        }
        Ok(written)
    }

    fn uncommitted_changes(&self) -> Option<UncommittedChanges> {
        self.pending_changes()
    }

    fn commit_pending(&self, message: &str) -> Result<CommitOutcome, LibraryError> {
        Ok(self.commit_pending_changes(message))
    }

    fn commit_external_change(
        &self,
        abs_path: &Path,
        message: &str,
    ) -> Result<CommitOutcome, LibraryError> {
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
        Ok(self.commit_path(&rel_path, message, &fallback))
    }

    fn history(&self, primitive_path: &Path) -> Result<Vec<HistoryEntry>, LibraryError> {
        // Stage 17 scaffold: walk the repo's commit graph newest-first
        // and keep commits whose tree differs from at least one parent
        // at `primitive_path`. Mirrors `git log --max-count 50
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
        crate::git_history::path_history(&repo, &rel_str, MAX_ENTRIES)
    }
}

/// Refuse a table name that is empty or holds a character a file browser
/// or the `.snxlib` table header cannot carry; shared by every table
/// write so a bad name fails the same way everywhere.
fn check_table_name(name: &str) -> Result<(), LibraryError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(LibraryError::Backend("table name cannot be empty".into()));
    }
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
    Ok(())
}

/// Edit one row of an open library file for `update_rows`: `Ok(true)` when
/// the edit changed it and the row was replaced, `Ok(false)` when it left
/// the row alone.
fn edit_one_row(
    lf: &mut LibraryFile,
    table: &str,
    row_id: RowId,
    edit: &mut crate::adapter::RowEdit<'_>,
) -> Result<bool, LibraryError> {
    let target = row_id.as_uuid().to_string();
    let entry = lf
        .tables
        .get_mut(table)
        .ok_or_else(|| LibraryError::NotFound(format!("table {table} not in library")))?;
    validate_legacy_header(table, &entry.columns)?;
    let index = entry
        .rows
        .iter()
        .position(|r| r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(target.as_str()))
        .ok_or_else(|| LibraryError::NotFound(format!("row {target} in table {table}")))?;
    let mut row = library_row_to_component(&entry.rows[index])?;
    if !edit(&mut row) {
        return Ok(false);
    }
    entry.rows[index] = Arc::new(component_to_library_row(&row)?);
    Ok(true)
}
