//! Library Updates Available — scanning the open schematic for placed
//! Symbols whose `library_version` drifted from the source row, and
//! applying the updates.
//!
//! Personal-mode libraries are updated silently at open, from `update`.
//! Team-mode Apply reads the library symbols off-thread
//! ([`LibraryMessage::LibraryUpdateSymbolsRead`]) and adopts them on
//! completion, one undo step per `lib_id`.

use super::*;
use crate::library::drift::{SymbolsRead, row_binds_lib_id};
use signex_types::schematic::LibSymbol;

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
    /// * **Personal** workflow mode — adopts the library's current symbol
    ///   and stamps the new version silently, without an undo step (the
    ///   user did not ask for it), and dirty-marks the path.
    /// * **Team** workflow mode — populates
    ///   `LibraryState::library_updates` with the entries so
    ///   `view_main_for` opens the modal on the next tick.
    ///
    /// Adapter / library-mount failures degrade to a single
    /// `tracing::warn` line per affected library and skip those
    /// entries — a missing library doesn't abort the schematic open.
    /// Symbols without a `library_id` (Standard-imported, hand-built)
    /// are skipped silently.
    ///
    /// `pub` (not `pub(crate)`) because the integration tests in
    /// `tests/regression` drive the open-time scan directly.
    pub fn scan_library_updates_for_open_schematic(&mut self, schematic_path: std::path::PathBuf) {
        let placed = self.placed_library_symbols(&schematic_path);
        if placed.is_empty() {
            return;
        }
        // One library read per `lib_id`, however many instances share it.
        let mut read_cache: std::collections::HashMap<String, Result<LibSymbol, String>> =
            std::collections::HashMap::new();
        let mut team_entries = Vec::new();
        let mut personal_apply: PersonalUpdates = std::collections::BTreeMap::new();
        let mut any_failed = false;
        for symbol in &placed {
            let Some(stale) = self.stale_row(symbol) else {
                continue;
            };
            if !row_binds_lib_id(&stale.row, &symbol.lib_id) {
                crate::diagnostics::log_warning(format!(
                    "{}: its library row now points at a different symbol than {}; update not applied.",
                    symbol.reference, symbol.lib_id
                ));
                any_failed = true;
                continue;
            }
            match stale.mode {
                signex_library::WorkflowMode::Personal => {
                    any_failed |= !self.collect_personal_update(
                        symbol,
                        &stale.row.version,
                        &mut read_cache,
                        &mut personal_apply,
                    );
                }
                _ => team_entries.push(team_entry(symbol, stale)),
            }
        }

        // Personal-mode: the same replace-and-stamp path as "Update from
        // library", so content and version move together and a connectivity
        // refusal leaves both. No undo step: opening a sheet must not leave
        // something for the first Ctrl+Z to revert.
        for (lib_id, (fresh, stamps)) in personal_apply {
            any_failed |=
                !self.adopt_library_symbol(&schematic_path, &lib_id, fresh, stamps, false);
        }

        // Surface the Team-mode entries via the modal state.
        if !team_entries.is_empty() {
            let state = crate::library::updates_dialog::LibraryUpdatesState::new(
                schematic_path.clone(),
                team_entries,
            );
            self.library.library_updates = Some(state);
        } else if any_failed {
            // An update that was refused or unreadable is still pending.
            self.library.skipped_updates_for.insert(schematic_path);
        } else {
            // Re-scan with no drift — clear any persistent indicator
            // tied to this schematic.
            self.library.skipped_updates_for.remove(&schematic_path);
        }
    }

    /// The library-backed symbols placed on `sheet`.
    fn placed_library_symbols(&self, sheet: &std::path::Path) -> Vec<PlacedLibrarySymbol> {
        let Some(engine) = self.document_state.engines.get(sheet) else {
            return Vec::new();
        };
        engine
            .document()
            .symbols
            .iter()
            .filter_map(|s| {
                Some(PlacedLibrarySymbol {
                    uuid: s.uuid,
                    reference: s.reference.clone(),
                    library_id: s.library_id?,
                    row_id: RowId::from_uuid(s.row_id?),
                    version: s.library_version.clone(),
                    lib_id: s.lib_id.clone(),
                })
            })
            .collect()
    }

    /// The symbol's library row when its version moved on from the placed
    /// one; `None` when current, or when the library/row cannot be read
    /// (logged, and the schematic open carries on).
    fn stale_row(&self, symbol: &PlacedLibrarySymbol) -> Option<StaleRow> {
        let library_id = symbol.library_id;
        let Some(open_lib) = self
            .library
            .open_libraries
            .iter()
            .find(|lib| lib.library_id == library_id)
        else {
            tracing::warn!(
                target: "signex::library",
                library_id = %library_id,
                symbol = %symbol.uuid,
                "library_updates_scan: library not mounted; skipping drift check"
            );
            return None;
        };
        let Some(adapter) = self.library.set.get(library_id) else {
            tracing::warn!(
                target: "signex::library",
                library_id = %library_id,
                "library_updates_scan: adapter missing on LibrarySet; skipping"
            );
            return None;
        };
        // The table name isn't stored on the schematic side, so look for the
        // row in each of the adapter's tables.
        // ponytail: sync `list_tables`/`read_row` (and the Personal-mode
        // symbol read below) on the open path; moving the whole open-time
        // scan off-thread is part of the #665 epic.
        let row = adapter
            .list_tables()
            .ok()?
            .iter()
            .find_map(|name| adapter.read_row(name, symbol.row_id).ok());
        let Some(row) = row else {
            tracing::warn!(
                target: "signex::library",
                library_id = %library_id,
                row_id = %symbol.row_id,
                "library_updates_scan: row not found in any table; skipping"
            );
            return None;
        };
        (row.version != symbol.version).then(|| StaleRow {
            mode: adapter.manifest().workflow.mode,
            library_name: open_lib.display_name.clone(),
            library_path: open_lib.root.clone(),
            row,
        })
    }

    /// Queue `symbol` for the silent Personal-mode update, reading the
    /// library's symbol once per `lib_id`. `false` when it was unreadable.
    fn collect_personal_update(
        &self,
        symbol: &PlacedLibrarySymbol,
        latest_version: &str,
        read_cache: &mut std::collections::HashMap<String, Result<LibSymbol, String>>,
        queue: &mut PersonalUpdates,
    ) -> bool {
        let read = read_cache
            .entry(symbol.lib_id.clone())
            .or_insert_with(|| self.read_library_symbol(&symbol.lib_id));
        match read {
            Ok(fresh) => {
                queue
                    .entry(symbol.lib_id.clone())
                    .or_insert_with(|| (fresh.clone(), Vec::new()))
                    .1
                    .push((symbol.uuid, latest_version.to_string()));
                true
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "{}: could not read the library symbol for its update ({why}); left unchanged.",
                    symbol.reference
                ));
                false
            }
        }
    }

    /// The library's current symbol for `lib_id` (`<library>:<symbol>`) as
    /// an embeddable copy — the same source the drift pass compares against.
    fn read_library_symbol(&self, lib_id: &str) -> Result<LibSymbol, String> {
        let (library, symbol) = crate::library::drift::split_lib_id(lib_id)
            .ok_or_else(|| "its id is not <library>:<symbol>".to_string())?;
        let adapter = self
            .library
            .set
            .get(library)
            .ok_or_else(|| "its library is not open".to_string())?;
        adapter
            .get_symbol(symbol)
            .map(|s| s.to_lib_symbol(lib_id.to_string()))
            .map_err(|e| e.to_string())
    }

    /// Apply the user's selected updates from the Library Updates modal.
    /// The modal is taken now; the library symbols are read off-thread and
    /// the schematic is updated by [`Self::handle_library_update_symbols_read`].
    ///
    /// The embedded copy is shared by every instance of a `lib_id`, so the
    /// modal selects per `lib_id`; each selected instance gets its own row's
    /// version, and each `lib_id` is one undo step. The library's symbol is
    /// read fresh rather than taken from the drift pass, which may not have
    /// landed yet (or be stale): stamping a version over old content would
    /// hide the update for good.
    pub(crate) fn handle_library_updates_apply(&mut self) -> Task<Message> {
        let Some(state) = self.library.library_updates.take() else {
            return Task::none();
        };
        let schematic_path = state.schematic_path.clone();
        let mut by_lib_id: std::collections::BTreeMap<String, Vec<(uuid::Uuid, String)>> =
            std::collections::BTreeMap::new();
        for e in state.entries.iter().filter(|e| e.selected) {
            by_lib_id
                .entry(e.lib_id.clone())
                .or_default()
                .push((e.symbol_uuid, e.latest_version.clone()));
        }
        if by_lib_id.is_empty() {
            // User clicked Apply with nothing selected — record as
            // skipped so the indicator stays visible.
            self.library.skipped_updates_for.insert(schematic_path);
            return Task::none();
        }
        // If some entries were left unchecked, the schematic still has
        // skipped drift and keeps its status bar indicator.
        let any_unchecked = state.entries.iter().any(|e| !e.selected);
        let libs = self.mounted_libraries();
        let wanted: Vec<_> = by_lib_id.into_iter().collect();
        let failed_sheet = schematic_path.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::library::drift::read_symbols(
                        schematic_path,
                        any_unchecked,
                        &libs,
                        wanted,
                    )
                })
                .await
                .map_err(|e| format!("library update worker failed: {e}"))
            },
            move |result| {
                let read = result.unwrap_or_else(|problem| {
                    crate::diagnostics::log_warning(&problem);
                    // Nothing was read: keep the indicator.
                    SymbolsRead {
                        sheet: failed_sheet,
                        any_unchecked: true,
                        reads: Vec::new(),
                    }
                });
                Message::Library(LibraryMessage::LibraryUpdateSymbolsRead(Box::new(read)))
            },
        )
    }

    /// Completion of the Apply read: adopt each library symbol. A sheet
    /// closed in the meantime is logged and skipped. The sheet keeps its
    /// "skipped" indicator when entries were left unchecked or any `lib_id`
    /// could not be read or was refused by the engine.
    pub(crate) fn handle_library_update_symbols_read(
        &mut self,
        read: SymbolsRead,
    ) -> Task<Message> {
        let SymbolsRead {
            sheet,
            any_unchecked,
            reads,
        } = read;
        if !self.document_state.engines.contains_key(&sheet) {
            crate::diagnostics::log_warning(format!(
                "Library update not applied: {} was closed before its library symbols were read.",
                sheet.display()
            ));
            return Task::none();
        }
        let mut all_applied = true;
        for r in reads {
            match r.symbol {
                Ok(fresh) => {
                    all_applied &=
                        self.adopt_library_symbol(&sheet, &r.lib_id, fresh, r.stamps, true);
                }
                Err(why) => {
                    crate::diagnostics::log_warning(format!(
                        "Library update not applied: could not read {} ({why}).",
                        r.lib_id
                    ));
                    all_applied = false;
                }
            }
        }
        if any_unchecked || !all_applied {
            self.library.skipped_updates_for.insert(sheet);
        } else {
            self.library.skipped_updates_for.remove(&sheet);
        }
        Task::none()
    }

    /// Adopt `fresh` as the embedded copy of `lib_id` on `sheet` and stamp
    /// `stamps` (`(placed symbol, row version)`) in one step. The engine
    /// refuses when a connected pin would vanish or move: then the copy and
    /// the versions are both left as they were and the reason goes to the
    /// Messages panel. `record_history = false` keeps the step off the undo
    /// stack (open-time maintenance). The content hash stays the truth; the
    /// version is informational (#950). Returns whether the update landed.
    fn adopt_library_symbol(
        &mut self,
        sheet: &std::path::Path,
        lib_id: &str,
        fresh: LibSymbol,
        stamps: Vec<(uuid::Uuid, String)>,
        record_history: bool,
    ) -> bool {
        let command = signex_engine::Command::ReplaceLibSymbol {
            lib_symbol: fresh,
            library_versions: stamps,
        };
        let refused = if self.document_state.active_path.as_deref() == Some(sheet) {
            // Through the mutation gateway: canvas sync, dirty flag,
            // netlist and panel refresh. A refusal is already in Messages.
            let (outcome, _) = if record_history {
                self.apply_engine_command_reporting(command, true, true)
            } else {
                self.apply_engine_command_unrecorded(command)
            };
            outcome == crate::app::mutation_gateway::CommandOutcome::Failed
        } else {
            self.replace_on_parked_sheet(sheet, command, record_history)
        };
        if refused {
            return false;
        }
        if let Some(d) = self.library.drift.drifted.get_mut(sheet) {
            d.remove(lib_id);
        }
        self.library.drift.check_pending = true;
        true
    }

    /// `adopt_library_symbol` for a sheet that is open but not the active
    /// tab: the engine has no canvas to sync, but its tab, dirty marker and
    /// the project netlist still have to follow. Returns `true` on refusal.
    fn replace_on_parked_sheet(
        &mut self,
        sheet: &std::path::Path,
        command: signex_engine::Command,
        record_history: bool,
    ) -> bool {
        let Some(engine) = self.document_state.engines.get_mut(sheet) else {
            return true;
        };
        let result = if record_history {
            engine.execute(command)
        } else {
            engine.execute_unrecorded(command)
        };
        match result {
            Ok(result) => {
                if result.changed {
                    if record_history {
                        // A recorded edit ends the net colour redo branch,
                        // as in the mutation gateway.
                        self.ui_state.net_color_redo.remove(sheet);
                    }
                    self.document_state.dirty_paths.insert(sheet.to_path_buf());
                    for tab in &mut self.document_state.tabs {
                        if tab.path == sheet {
                            tab.dirty = true;
                        }
                    }
                    self.ui_state.project_netlist = None;
                    self.refresh_project_netlist();
                    self.refresh_panel_ctx();
                }
                false
            }
            Err(error) => {
                crate::diagnostics::log_error(
                    "Library update not applied",
                    &anyhow::Error::new(error),
                );
                true
            }
        }
    }
}

/// A library-backed symbol placed on the sheet being scanned.
struct PlacedLibrarySymbol {
    uuid: uuid::Uuid,
    reference: String,
    library_id: uuid::Uuid,
    row_id: RowId,
    version: String,
    lib_id: String,
}

/// A library row whose version differs from the placed symbol's.
struct StaleRow {
    mode: signex_library::WorkflowMode,
    library_name: String,
    library_path: std::path::PathBuf,
    row: signex_library::ComponentRow,
}

/// Per `lib_id`: the library's symbol and the `(placed symbol, version)`
/// stamps to apply with it.
type PersonalUpdates = std::collections::BTreeMap<String, (LibSymbol, Vec<(uuid::Uuid, String)>)>;

/// The Team-mode modal row for a stale `symbol`.
fn team_entry(
    symbol: &PlacedLibrarySymbol,
    stale: StaleRow,
) -> crate::library::updates_dialog::LibraryUpdateEntry {
    let bump_kind =
        crate::library::updates_dialog::classify_bump(&symbol.version, &stale.row.version);
    crate::library::updates_dialog::LibraryUpdateEntry {
        symbol_uuid: symbol.uuid,
        ref_des: symbol.reference.clone(),
        lib_id: symbol.lib_id.clone(),
        library_id: symbol.library_id,
        library_name: stale.library_name,
        row_id: symbol.row_id,
        library_path: stale.library_path,
        current_version: symbol.version.clone(),
        latest_version: stale.row.version,
        bump_kind,
        selected: false,
    }
}
