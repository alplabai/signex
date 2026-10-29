//! Library Updates Available — scanning the open schematic for placed
//! Symbols whose `library_version` drifted from the source row, and
//! applying the updates.
//!
//! Personal-mode libraries are updated silently at open, from `update`.
//! Team-mode Apply reads the library symbols off-thread
//! ([`LibraryMessage::LibraryUpdateSymbolsRead`]) and adopts them on
//! completion, one undo step per `lib_id`.

use super::*;
use crate::library::drift::{
    ApplyInput, SymbolRead, SymbolsRead, VERSION_NOT_STAMPED, apply_input, rebound_row_warning,
    row_binds_lib_id,
};
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
    /// `skipped_updates_for` (nothing renders that set yet).
    pub(super) fn handle_library_updates_skip_all(&mut self) -> Task<Message> {
        if let Some(state) = self.library.library_updates.take() {
            self.library
                .skipped_updates_for
                .insert(state.schematic_path);
        }
        self.show_next_queued_updates();
        Task::none()
    }

    /// Open the next sheet's queued Library Updates modal, if none is open.
    pub(super) fn show_next_queued_updates(&mut self) {
        if self.library.library_updates.is_none() && !self.library.queued_library_updates.is_empty()
        {
            self.library.library_updates = Some(self.library.queued_library_updates.remove(0));
        }
    }

    /// Take the pending modal state for `sheet`, wherever it waits (open
    /// modal or queue).
    fn take_pending_updates_for(
        &mut self,
        sheet: &std::path::Path,
    ) -> Option<crate::library::updates_dialog::LibraryUpdatesState> {
        if self
            .library
            .library_updates
            .as_ref()
            .is_some_and(|open| open.schematic_path == sheet)
        {
            return self.library.library_updates.take();
        }
        let at = self
            .library
            .queued_library_updates
            .iter()
            .position(|queued| queued.schematic_path == sheet)?;
        Some(self.library.queued_library_updates.remove(at))
    }

    /// Scan the schematic at `path` for placed Symbols whose
    /// `library_version` drifted from the source row's current
    /// version. Splits into two control paths:
    ///
    /// * **Personal** workflow mode - adopts the library's current symbol
    ///   and stamps the new version silently, without an undo step (the
    ///   user did not ask for it), and dirty-marks the path. After a late
    ///   mount (`only` set) it is one normal undo step, since the user may
    ///   have edited the sheet already.
    /// * **Team** workflow mode - populates
    ///   `LibraryState::library_updates` with the entries so
    ///   `view_main_for` opens the modal on the next tick.
    ///
    /// The embedded copy is shared by every instance of a `lib_id`, so the
    /// decision is per `lib_id`: it is auto-applied only when EVERY placed
    /// instance has a readable Personal-mode row that still binds it. When a
    /// stale instance is Team-mode, the stale instances go to the modal;
    /// when only an unreadable, re-bound or Team sibling blocks it, nothing
    /// is applied and the blocking reference is named in Messages.
    ///
    /// Adapter / library-mount / missing-row failures degrade to a single
    /// `tracing::warn` line per affected library and skip those
    /// entries - a missing library doesn't abort the schematic open.
    /// Symbols without a `library_id` (Standard-imported, hand-built)
    /// are skipped silently.
    ///
    /// `pub` (not `pub(crate)`) because the integration tests in
    /// `tests/regression` drive the open-time scan directly.
    pub fn scan_library_updates_for_open_schematic(&mut self, schematic_path: std::path::PathBuf) {
        self.scan_library_updates(schematic_path, None);
    }

    /// Re-run the version scan for every open sheet that has a symbol
    /// linked to `library_id`, now that the library has mounted (#1449).
    /// The open-time scan skipped those symbols while it was unmounted.
    /// Only that library's symbols are scanned, so other still-unmounted
    /// libraries are not warned about a second time, and a sheet with no
    /// matching link is left alone.
    pub(super) fn rescan_library_updates_after_mount(&mut self, library_id: uuid::Uuid) {
        let sheets: Vec<std::path::PathBuf> = self
            .document_state
            .engines
            .keys()
            .filter(|path| {
                self.placed_library_symbols(path)
                    .iter()
                    .any(|s| s.library_id == library_id)
            })
            .cloned()
            .collect();
        for sheet in sheets {
            self.scan_library_updates(sheet, Some(library_id));
        }
    }

    /// The scan behind both entry points; `only` restricts it to the
    /// `lib_id` groups with an instance linked to one library.
    fn scan_library_updates(
        &mut self,
        schematic_path: std::path::PathBuf,
        only: Option<uuid::Uuid>,
    ) {
        // A read-only sheet (written by a newer Signex) takes no update, not
        // even a silent Personal one: it would turn dirty on open (#1402).
        let read_only = self
            .document_state
            .engines
            .get(&schematic_path)
            .is_some_and(|engine| engine.document().is_read_only());
        if read_only {
            return;
        }
        let mut placed = self.placed_library_symbols(&schematic_path);
        if let Some(only) = only {
            // Every instance of a `lib_id` shares one embedded copy, so the
            // whole group is scanned (the Personal auto-apply checks each
            // instance), even when a sibling links to a still-unmounted
            // library.
            let wanted: std::collections::BTreeSet<String> = placed
                .iter()
                .filter(|s| s.library_id == only)
                .map(|s| s.lib_id.clone())
                .collect();
            placed.retain(|s| wanted.contains(&s.lib_id));
        }
        if placed.is_empty() {
            return;
        }
        let mut by_lib: std::collections::BTreeMap<String, Vec<Instance>> =
            std::collections::BTreeMap::new();
        // Libraries already reported unreadable: a sheet of 200 parts from
        // one unmounted library logs once, not 200 times.
        let mut warned = std::collections::BTreeSet::new();
        for symbol in placed {
            let row = self.instance_row(&symbol, &mut warned);
            by_lib
                .entry(symbol.lib_id.clone())
                .or_default()
                .push((symbol, row));
        }
        let mut any_failed = false;
        // One library read per `lib_id`, however many instances share it.
        let mut read_cache = std::collections::HashMap::new();
        let mut team_entries = Vec::new();
        let mut personal_apply: PersonalUpdates = std::collections::BTreeMap::new();
        for (lib_id, group) in by_lib {
            let stale = stale_bound(&group, &lib_id, &mut any_failed);
            if stale.is_empty() {
                continue;
            }
            let team = stale
                .iter()
                .any(|(_, r)| r.mode != signex_library::WorkflowMode::Personal);
            let blocked = group
                .iter()
                .find_map(|(s, r)| blocker(s, r.as_ref(), &lib_id));
            if team {
                team_entries.extend(stale.into_iter().map(|(s, r)| team_entry(s.clone(), r)));
            } else if let Some(why) = blocked {
                crate::diagnostics::log_warning(why);
                any_failed = true;
            } else {
                for (symbol, row) in stale {
                    any_failed |= !self.collect_personal_update(
                        symbol.clone(),
                        &row.row.version,
                        &mut read_cache,
                        &mut personal_apply,
                    );
                }
            }
        }

        // Personal-mode: the same replace-and-stamp path as "Update from
        // library", so content and version move together and a connectivity
        // refusal leaves both. No undo step: opening a sheet must not leave
        // something for the first Ctrl+Z to revert.
        for (lib_id, (fresh, stamps)) in personal_apply {
            // A late mount can land after the user's first edit, which the
            // unrecorded path refuses, so it becomes one normal undo step.
            any_failed |=
                !self.adopt_library_symbol(&schematic_path, &lib_id, fresh, stamps, only.is_some());
        }
        self.record_scan_outcome(schematic_path, team_entries, any_failed, only.is_some());
    }

    /// Surface the Team-mode entries via the modal state, or record whether
    /// an update is still pending for the sheet.
    fn record_scan_outcome(
        &mut self,
        schematic_path: std::path::PathBuf,
        team_entries: Vec<crate::library::updates_dialog::LibraryUpdateEntry>,
        any_failed: bool,
        partial: bool,
    ) {
        if !team_entries.is_empty() {
            // A partial (one-library) scan adds to the sheet's pending
            // modal rather than replacing the other libraries' entries;
            // entries already there keep their checkbox, only new ones get
            // the default.
            let pending = self.take_pending_updates_for(&schematic_path);
            let mut kept = std::collections::HashMap::new();
            let (team_entries, any_failed) = match pending {
                Some(old) if partial => {
                    let any_failed = any_failed || old.scan_failed;
                    let known: std::collections::HashSet<_> =
                        old.entries.iter().map(|e| e.symbol_uuid).collect();
                    kept.extend(old.entries.iter().map(|e| (e.symbol_uuid, e.selected)));
                    let fresh = team_entries
                        .into_iter()
                        .filter(|e| !known.contains(&e.symbol_uuid));
                    (old.entries.into_iter().chain(fresh).collect(), any_failed)
                }
                _ => (team_entries, any_failed),
            };
            let mut state = crate::library::updates_dialog::LibraryUpdatesState::new(
                schematic_path.clone(),
                team_entries,
            );
            for entry in &mut state.entries {
                if let Some(selected) = kept.get(&entry.symbol_uuid) {
                    entry.selected = *selected;
                }
            }
            state.scan_failed = any_failed;
            state.engine_instance = self
                .document_state
                .engines
                .get(&schematic_path)
                .map_or(0, signex_engine::Engine::instance_id);
            // One modal at a time: another sheet's waits its turn.
            if self.library.library_updates.is_none() {
                self.library.library_updates = Some(state);
            } else {
                self.library.queued_library_updates.push(state);
            }
        } else if any_failed {
            // An update that was refused or unreadable is still pending.
            self.library.skipped_updates_for.insert(schematic_path);
        } else if !partial {
            // Re-scan with no drift - clear the pending-update record
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

    /// The symbol's library row, flagged stale when its version moved on
    /// from the placed one; `None` when the library/row cannot be read
    /// (logged, and the schematic open carries on).
    fn instance_row(
        &self,
        symbol: &PlacedLibrarySymbol,
        warned: &mut std::collections::BTreeSet<uuid::Uuid>,
    ) -> Option<InstanceRow> {
        let library_id = symbol.library_id;
        let Some(open_lib) = self
            .library
            .open_libraries
            .iter()
            .find(|lib| lib.library_id == library_id)
        else {
            if warned.insert(library_id) {
                tracing::warn!(
                    target: "signex::library",
                    library_id = %library_id,
                    "library_updates_scan: library not mounted; skipping drift check"
                );
            }
            return None;
        };
        let Some(adapter) = self.library.set.get(library_id) else {
            if warned.insert(library_id) {
                tracing::warn!(
                    target: "signex::library",
                    library_id = %library_id,
                    "library_updates_scan: adapter missing on LibrarySet; skipping"
                );
            }
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
            if warned.insert(library_id) {
                tracing::warn!(
                    target: "signex::library",
                    library_id = %library_id,
                    row_id = %symbol.row_id,
                    "library_updates_scan: row not found in any table; skipping"
                );
            }
            return None;
        };
        Some(InstanceRow {
            stale: row.version != symbol.version,
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
        symbol: PlacedLibrarySymbol,
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
    /// The modal is taken now; the library symbols and the rows' versions
    /// are read off-thread and the schematic is updated by
    /// [`Self::handle_library_update_symbols_read`].
    ///
    /// The embedded copy is shared by every instance of a `lib_id`, so the
    /// modal selects per `lib_id` and every instance of a selected `lib_id`
    /// is read, each getting its own row's version; each `lib_id` is one
    /// undo step. The library's symbol and
    /// the row versions are read fresh rather than taken from the scan or the
    /// drift pass, which may be stale: stamping a version over old content
    /// would hide the update for good.
    pub(crate) fn handle_library_updates_apply(&mut self) -> Task<Message> {
        let Some(state) = self.library.library_updates.take() else {
            return Task::none();
        };
        self.show_next_queued_updates();
        let Some(doc) = self
            .document_state
            .engines
            .get(&state.schematic_path)
            .map(|e| e.document())
        else {
            crate::diagnostics::log_warning(format!(
                "Library update not applied: {} is no longer open.",
                state.schematic_path.display()
            ));
            self.library
                .skipped_updates_for
                .insert(state.schematic_path);
            return Task::none();
        };
        let Some(input) = apply_input(&state, doc) else {
            // User clicked Apply with nothing selected — record it as
            // skipped (Messages shows nothing; the set is not rendered yet).
            self.library
                .skipped_updates_for
                .insert(state.schematic_path);
            return Task::none();
        };
        self.spawn_apply_read(input)
    }

    /// Run the off-thread apply read for `input`; its completion is
    /// [`Self::handle_library_update_symbols_read`]. The one path both the
    /// modal's Apply and "Update from library" take.
    pub(crate) fn spawn_apply_read(&mut self, input: ApplyInput) -> Task<Message> {
        let libs = self.mounted_libraries();
        let fallback = input.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::library::drift::read_symbols(input, &libs)
                })
                .await
                .map_err(|e| format!("library update worker failed: {e}"))
            },
            move |result| {
                let read = result.unwrap_or_else(|problem| SymbolsRead::failed(&fallback, problem));
                Message::Library(LibraryMessage::LibraryUpdateSymbolsRead(Box::new(read)))
            },
        )
    }

    /// Completion of the Apply read: adopt each library symbol. A sheet
    /// closed (or closed and reopened) in the meantime is logged and
    /// skipped. The sheet's `skipped_updates_for` entry stays when entries
    /// were left unchecked, the open-time scan or the worker failed, or any
    /// `lib_id` could not be read or was refused by the engine; nothing
    /// renders that set yet, so the user sees only the Messages lines.
    pub(crate) fn handle_library_update_symbols_read(
        &mut self,
        read: SymbolsRead,
    ) -> Task<Message> {
        let SymbolsRead {
            sheet,
            engine_instance,
            any_unchecked,
            scan_failed,
            tracks_pending,
            problem,
            reads,
        } = read;
        let same_open = self
            .document_state
            .engines
            .get(&sheet)
            .is_some_and(|e| e.instance_id() == engine_instance);
        if let Some(problem) = &problem {
            crate::diagnostics::log_warning(problem);
        }
        if !same_open {
            if problem.is_none() {
                crate::diagnostics::log_warning(format!(
                    "Library update not applied: {} was closed, reloaded or renamed before its library symbols were read.",
                    sheet.display()
                ));
            }
            return Task::none();
        }
        let all_applied = self.apply_reads(&sheet, reads) && problem.is_none();
        // The drift rows adopted above no longer show on the selection.
        self.update_selection_info();
        if !tracks_pending {
            // "Update from library" speaks for one `lib_id` only.
        } else if any_unchecked || scan_failed || !all_applied {
            self.library.skipped_updates_for.insert(sheet);
        } else {
            self.library.skipped_updates_for.remove(&sheet);
        }
        Task::none()
    }

    /// Adopt each read's symbol on `sheet` and report what was left alone.
    /// `true` when every `lib_id` landed with every instance stamped.
    fn apply_reads(&mut self, sheet: &std::path::Path, reads: Vec<SymbolRead>) -> bool {
        let mut all_applied = true;
        for r in reads {
            for why in &r.skipped {
                crate::diagnostics::log_warning(why);
                all_applied = false;
            }
            if r.held_back {
                continue;
            }
            match r.symbol {
                Ok(fresh) => {
                    all_applied &=
                        self.adopt_library_symbol(sheet, &r.lib_id, fresh, r.stamps, true);
                }
                Err(why) => {
                    crate::diagnostics::log_warning(format!(
                        "Library update not applied: could not read {} ({why}).",
                        self.library.drift.display_name(&r.lib_id)
                    ));
                    all_applied = false;
                }
            }
        }
        all_applied
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
#[derive(Clone)]
struct PlacedLibrarySymbol {
    uuid: uuid::Uuid,
    reference: String,
    library_id: uuid::Uuid,
    row_id: RowId,
    version: String,
    lib_id: String,
}

/// A placed symbol's library row.
struct InstanceRow {
    /// Its version differs from the placed symbol's.
    stale: bool,
    mode: signex_library::WorkflowMode,
    library_name: String,
    library_path: std::path::PathBuf,
    row: signex_library::ComponentRow,
}

/// Per `lib_id`: the library's symbol and the `(placed symbol, version)`
/// stamps to apply with it.
type PersonalUpdates = std::collections::BTreeMap<String, (LibSymbol, Vec<(uuid::Uuid, String)>)>;

/// A placed instance with its row (`None`: unreadable).
type Instance = (PlacedLibrarySymbol, Option<InstanceRow>);

/// The stale instances of `lib_id` whose row still binds it. A stale row
/// re-bound to another symbol is logged and counted in `any_failed`.
fn stale_bound<'a>(
    group: &'a [Instance],
    lib_id: &str,
    any_failed: &mut bool,
) -> Vec<(&'a PlacedLibrarySymbol, &'a InstanceRow)> {
    let mut stale = Vec::new();
    for (symbol, row) in group {
        let Some(row) = row.as_ref().filter(|r| r.stale) else {
            continue;
        };
        if row_binds_lib_id(&row.row, lib_id) {
            stale.push((symbol, row));
        } else {
            crate::diagnostics::log_warning(rebound_row_warning(
                &symbol.reference,
                symbol.row_id,
                lib_id,
                VERSION_NOT_STAMPED,
            ));
            *any_failed = true;
        }
    }
    stale
}

/// Why `symbol` stops its `lib_id` from being auto-applied: the shared copy
/// moves only when every instance has a readable Personal-mode row that
/// still binds it. The Messages line names the instance.
fn blocker(
    symbol: &PlacedLibrarySymbol,
    row: Option<&InstanceRow>,
    lib_id: &str,
) -> Option<String> {
    let reason = match row {
        None => "its library row is unreadable",
        Some(r) if !row_binds_lib_id(&r.row, lib_id) => "its row binds a different symbol",
        Some(r) if r.mode != signex_library::WorkflowMode::Personal => "its library is Team-mode",
        Some(_) => return None,
    };
    Some(format!(
        "{}: {reason}, so the shared copy was not auto-updated ({lib_id}).",
        symbol.reference
    ))
}

/// The Team-mode modal row for a stale `symbol`.
fn team_entry(
    symbol: PlacedLibrarySymbol,
    stale: &InstanceRow,
) -> crate::library::updates_dialog::LibraryUpdateEntry {
    let bump_kind =
        crate::library::updates_dialog::classify_bump(&symbol.version, &stale.row.version);
    crate::library::updates_dialog::LibraryUpdateEntry {
        symbol_uuid: symbol.uuid,
        ref_des: symbol.reference,
        lib_id: symbol.lib_id,
        library_id: symbol.library_id,
        library_name: stale.library_name.clone(),
        row_id: symbol.row_id,
        library_path: stale.library_path.clone(),
        current_version: symbol.version,
        latest_version: stale.row.version.clone(),
        bump_kind,
        selected: false,
    }
}
