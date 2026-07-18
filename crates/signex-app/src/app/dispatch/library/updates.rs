//! Library Updates Available — scanning the open schematic for placed
//! Symbols whose `library_version` drifted from the source row, and
//! applying the updates.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

// ─────────────────────────────────────────────────────────────────────
// Stage 16 — Library Updates Available scan + apply (§3.5)
// ─────────────────────────────────────────────────────────────────────

impl Signex {
    /// Toggle one row's checkbox in the Library Updates modal.
    pub(super) fn handle_library_updates_toggle_selection(
        &mut self,
        symbol_uuid: uuid::Uuid,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_updates.as_mut() {
            state.toggle(symbol_uuid);
        }
        Task::none()
    }

    /// User clicked Skip All — close the modal and record the path on
    /// `skipped_updates_for` so the status bar can flag it persistently.
    pub(super) fn handle_library_updates_skip_all(&mut self) -> Task<Message> {
        if let Some(state) = self.library.library_updates.take() {
            self.library
                .skipped_updates_for
                .insert(state.schematic_path);
        }
        Task::none()
    }

    /// Scan the schematic at `path` for placed Symbols whose
    /// `library_version` drifted from the source row's current
    /// version. Splits into two control paths:
    ///
    /// * **Personal** workflow mode — auto-applies every drift to the
    ///   schematic engine silently and dirty-marks the path.
    /// * **Team** workflow mode — populates
    ///   `LibraryState::library_updates` with the entries so
    ///   `view_main_for` opens the modal on the next tick.
    ///
    /// Adapter / library-mount failures degrade to a single
    /// `tracing::warn` line per affected library and skip those
    /// entries — a missing library doesn't abort the schematic open.
    /// Symbols without a `library_id` (Standard-imported, hand-built)
    /// are skipped silently.
    pub(crate) fn scan_library_updates_for_open_schematic(
        &mut self,
        schematic_path: std::path::PathBuf,
    ) {
        use signex_library::WorkflowMode;
        // Snapshot the placed-Symbol identity tuples first so we can
        // mutate the engine in the apply loop below without holding
        // the engine borrow across the library-set lookups.
        let symbol_refs: Vec<(uuid::Uuid, String, uuid::Uuid, RowId, String)> =
            match self.document_state.engines.get(&schematic_path) {
                Some(engine) => engine
                    .document()
                    .symbols
                    .iter()
                    .filter_map(|s| {
                        let library_id = s.library_id?;
                        let row_uuid = s.row_id?;
                        Some((
                            s.uuid,
                            s.reference.clone(),
                            library_id,
                            RowId::from_uuid(row_uuid),
                            s.library_version.clone(),
                        ))
                    })
                    .collect(),
                None => return,
            };

        if symbol_refs.is_empty() {
            return;
        }

        // Group entries by Team-mode source library — Personal-mode
        // libraries auto-apply silently and short-circuit the modal.
        let mut team_entries: Vec<crate::library::updates_dialog::LibraryUpdateEntry> = Vec::new();
        let mut personal_apply: Vec<(uuid::Uuid, String)> = Vec::new();

        // Adapter access lives on the LibrarySet; keep the borrow
        // alive only inside this closure-scope so the apply mutation
        // below can take a different `&mut self.document_state`.
        for (symbol_uuid, ref_des, library_id, row_id, current_version) in &symbol_refs {
            // Resolve the library entry (display_name + path) +
            // adapter handle.
            let Some(open_lib) = self
                .library
                .open_libraries
                .iter()
                .find(|lib| lib.library_id == *library_id)
            else {
                tracing::warn!(
                    target: "signex::library",
                    library_id = %library_id,
                    symbol = %symbol_uuid,
                    "library_updates_scan: library not mounted; skipping drift check"
                );
                continue;
            };
            let library_path = open_lib.root.clone();
            let library_name = open_lib.display_name.clone();

            let Some(adapter) = self.library.set.get(*library_id) else {
                tracing::warn!(
                    target: "signex::library",
                    library_id = %library_id,
                    "library_updates_scan: adapter missing on LibrarySet; skipping"
                );
                continue;
            };

            let mode = adapter.manifest().workflow.mode;

            // Locate the row inside its table — we don't know the
            // table name on the schematic side, so iterate the
            // adapter's tables and look for `row_id`.
            let Ok(table_names) = adapter.list_tables() else {
                continue;
            };
            let mut found: Option<(String, signex_library::ComponentRow)> = None;
            for name in &table_names {
                if let Ok(row) = adapter.read_row(name, *row_id) {
                    found = Some((name.clone(), row));
                    break;
                }
            }
            let Some((_table, row)) = found else {
                tracing::warn!(
                    target: "signex::library",
                    library_id = %library_id,
                    row_id = %row_id,
                    "library_updates_scan: row not found in any table; skipping"
                );
                continue;
            };

            if row.version == *current_version {
                continue; // No drift.
            }

            match mode {
                WorkflowMode::Personal => {
                    personal_apply.push((*symbol_uuid, row.version.clone()));
                }
                WorkflowMode::Team | WorkflowMode::Enterprise => {
                    let bump_kind = crate::library::updates_dialog::classify_bump(
                        current_version,
                        &row.version,
                    );
                    team_entries.push(crate::library::updates_dialog::LibraryUpdateEntry {
                        symbol_uuid: *symbol_uuid,
                        ref_des: ref_des.clone(),
                        library_id: *library_id,
                        library_name: library_name.clone(),
                        row_id: *row_id,
                        library_path: library_path.clone(),
                        current_version: current_version.clone(),
                        latest_version: row.version.clone(),
                        bump_kind,
                        selected: false,
                    });
                }
            }
        }

        // Apply the Personal-mode auto-updates first (silent path).
        if !personal_apply.is_empty()
            && let Some(engine) = self.document_state.engines.get_mut(&schematic_path)
        {
            let mut document = engine.document().clone();
            let mut applied = 0usize;
            for symbol in &mut document.symbols {
                if let Some((_, latest)) =
                    personal_apply.iter().find(|(uuid, _)| uuid == &symbol.uuid)
                {
                    symbol.library_version = latest.clone();
                    applied += 1;
                }
            }
            engine.set_document(document);
            self.document_state
                .dirty_paths
                .insert(schematic_path.clone());
            tracing::info!(
                target: "signex::library",
                schematic = %schematic_path.display(),
                count = applied,
                "library_updates_scan: auto-applied {} update(s) under Personal mode",
                applied
            );
        }

        // Surface the Team-mode entries via the modal state.
        if !team_entries.is_empty() {
            let state = crate::library::updates_dialog::LibraryUpdatesState::new(
                schematic_path.clone(),
                team_entries,
            );
            self.library.library_updates = Some(state);
        } else {
            // Re-scan with no drift — clear any persistent indicator
            // tied to this schematic.
            self.library.skipped_updates_for.remove(&schematic_path);
        }
    }

    /// Apply the user's selected updates from the Library Updates
    /// modal to the schematic engine. Drops the modal state on
    /// success; on apply, dirty-marks the schematic and clears its
    /// "skipped" indicator (the user committed an update, the path is
    /// no longer ambiguously skipped).
    pub(crate) fn handle_library_updates_apply(&mut self) {
        let Some(state) = self.library.library_updates.take() else {
            return;
        };
        let schematic_path = state.schematic_path.clone();
        let updates: Vec<(uuid::Uuid, String)> = state
            .entries
            .iter()
            .filter(|e| e.selected)
            .map(|e| (e.symbol_uuid, e.latest_version.clone()))
            .collect();
        if updates.is_empty() {
            // User clicked Apply with nothing selected — record as
            // skipped so the indicator stays visible.
            self.library.skipped_updates_for.insert(schematic_path);
            return;
        }
        if let Some(engine) = self.document_state.engines.get_mut(&schematic_path) {
            let mut document = engine.document().clone();
            for symbol in &mut document.symbols {
                if let Some((_, latest)) = updates.iter().find(|(uuid, _)| uuid == &symbol.uuid) {
                    symbol.library_version = latest.clone();
                }
            }
            engine.set_document(document);
            self.document_state
                .dirty_paths
                .insert(schematic_path.clone());
        }
        // If some entries were left unchecked, treat the schematic as
        // "still has skipped drift" so the status bar keeps its
        // indicator. Equal counts (everyone selected) clear the flag.
        let any_unchecked = state.entries.iter().any(|e| !e.selected);
        if any_unchecked {
            self.library.skipped_updates_for.insert(schematic_path);
        } else {
            self.library.skipped_updates_for.remove(&schematic_path);
        }
    }
}
