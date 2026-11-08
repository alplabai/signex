//! Library Updates Available — scanning the open schematic for placed
//! Symbols whose `library_version` drifted from the source row, and
//! applying the updates.
//!
//! Personal-mode libraries are updated silently at open, from `update`.
//! Team-mode Apply reads the library symbols off-thread
//! ([`LibraryMessage::LibraryUpdateSymbolsRead`]) and adopts them on
//! completion, one undo step per `lib_id`.

use super::*;
use crate::library::drift::{ApplyInput, CapturedStamp, SymbolRead, SymbolsRead, apply_input};
use crate::library::part_sync::PartSync;
use signex_types::schematic::{LibSymbol, LinkStamp};

mod scan;
use scan::{
    Instance, InstanceRow, PersonalUpdates, PlacedLibrarySymbol, blocker, bound_instances,
    modal_entry, personal_link,
};

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

    /// The user answered one conflict of a part in the Library Updates modal.
    pub(super) fn handle_library_updates_conflict_answered(
        &mut self,
        symbol_uuid: uuid::Uuid,
        field: signex_types::schematic::StampField,
        choice: crate::library::part_sync::Choice,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_updates.as_mut() {
            state.answer_conflict(symbol_uuid, field, choice);
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
            let bound = bound_instances(&group, &lib_id, &mut any_failed);
            if bound.is_empty() {
                continue;
            }
            // Only the moved parts say whose mode rules; with none moved,
            // the parts that hold a conflict do.
            let any_stale = bound.iter().any(|(_, r)| r.stale);
            let team = bound.iter().any(|(_, r)| {
                (r.stale || !any_stale) && r.mode != signex_library::WorkflowMode::Personal
            });
            // The shared copy cannot move, and only a moved copy needs the
            // check below.
            let blocked = (!team && any_stale)
                .then(|| {
                    group
                        .iter()
                        .find_map(|(s, r)| blocker(s, r.as_ref(), &lib_id))
                })
                .flatten();
            if let Some(why) = blocked {
                crate::diagnostics::log_warning(why);
                any_failed = true;
                continue;
            }
            // Built once per `lib_id`: the stamps, the opened library and
            // each row's footprint name serve every instance of it.
            // ponytail: this opens libraries and reads footprints inline, like
            // `instance_row`; the whole scan moves off-thread under #893.
            let mut sync_check = self.part_sync_check(&schematic_path, &lib_id);
            for (symbol, row) in bound {
                let eval = sync_check
                    .as_mut()
                    .and_then(|check| check.eval(symbol.uuid, &row.row));
                let conflicts = eval.as_ref().map(|e| e.conflicts()).unwrap_or_default();
                if !row.stale && conflicts.is_empty() {
                    continue;
                }
                if team || !conflicts.is_empty() {
                    team_entries.push(modal_entry(symbol.clone(), row, conflicts));
                }
                if team || !row.stale {
                    continue;
                }
                let link =
                    personal_link(row.row.version.clone(), sync_check.is_some(), eval.as_ref());
                any_failed |= !self.collect_personal_update(
                    symbol.clone(),
                    link,
                    &mut read_cache,
                    &mut personal_apply,
                );
            }
        }

        // Personal-mode: the same replace-and-stamp path as "Update from
        // library", so content and version move together and a connectivity
        // refusal leaves both. No undo step: opening a sheet must not leave
        // something for the first Ctrl+Z to revert.
        for (lib_id, (fresh, stamps)) in personal_apply {
            // A late mount can land after the user's first edit, which the
            // unrecorded path refuses, so it becomes one normal undo step.
            any_failed |= !self.adopt_library_symbol(
                &schematic_path,
                &lib_id,
                fresh,
                stamps,
                Vec::new(),
                only.is_some(),
            );
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

    /// A check of the parts of `lib_id` on `sheet` against their rows: where
    /// they differ in footprint, Manufacturer or MPN, only "Update from
    /// library" may rewrite them. `None` when the sheet is not open.
    fn part_sync_check(
        &self,
        sheet: &std::path::Path,
        lib_id: &str,
    ) -> Option<crate::library::drift::PartSyncCheck> {
        let engine = self.document_state.engines.get(sheet)?;
        Some(crate::library::drift::PartSyncCheck::new(
            engine.document(),
            lib_id,
            self.mounted_libraries(),
        ))
    }

    /// Queue `symbol` for the silent Personal-mode update, reading the
    /// library's symbol once per `lib_id`. `false` when it was unreadable.
    fn collect_personal_update(
        &self,
        symbol: PlacedLibrarySymbol,
        link: Option<LinkStamp>,
        read_cache: &mut std::collections::HashMap<String, Result<LibSymbol, String>>,
        queue: &mut PersonalUpdates,
    ) -> bool {
        let read = read_cache
            .entry(symbol.lib_id.clone())
            .or_insert_with(|| self.read_library_symbol(&symbol.lib_id));
        match read {
            Ok(fresh) => {
                let entry = queue
                    .entry(symbol.lib_id.clone())
                    .or_insert_with(|| (fresh.clone(), Vec::new()));
                if let Some(link) = link {
                    entry.1.push((symbol.uuid, link));
                }
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
                    let (stamps, relinked) = self.stamps_still_linked(sheet, r.stamps);
                    let mut part_syncs = r.part_syncs;
                    part_syncs.retain(|sync| !relinked.contains(&sync.symbol));
                    let (mut stamps, part_syncs, held) =
                        match self.document_state.engines.get(sheet) {
                            Some(engine) => crate::library::part_sync::hold_back_locked(
                                engine, stamps, part_syncs,
                            ),
                            None => (stamps, part_syncs, Vec::new()),
                        };
                    for reference in &held {
                        crate::diagnostics::log_warning(format!(
                            "{reference} is locked; its footprint/MPN was not updated from the library. Unlock it to apply."
                        ));
                        all_applied = false;
                    }
                    let part_syncs = self.revalidated_part_syncs(sheet, part_syncs, &mut stamps);
                    let lines: Vec<String> = part_syncs.iter().flat_map(PartSync::lines).collect();
                    let landed = self
                        .adopt_library_symbol(sheet, &r.lib_id, fresh, stamps, part_syncs, true);
                    if landed {
                        lines.iter().for_each(crate::diagnostics::log_info);
                    }
                    all_applied &= landed;
                }
                Err(why) => {
                    crate::diagnostics::log_warning(format!(
                        "Library update not applied: could not read {} ({why}).",
                        self.library
                            .drift
                            .display_name(&r.lib_id, &self.library.open_libraries)
                    ));
                    all_applied = false;
                }
            }
        }
        all_applied
    }

    /// The worker decided each part against the values captured at Apply;
    /// check them against the parts as they are now, so an edit made while
    /// the worker ran is never overwritten (#2070). A part no longer on the
    /// sheet loses its sync.
    fn revalidated_part_syncs(
        &self,
        sheet: &std::path::Path,
        syncs: Vec<PartSync>,
        stamps: &mut [(uuid::Uuid, LinkStamp)],
    ) -> Vec<PartSync> {
        let Some(doc) = self.document_state.engines.get(sheet).map(|e| e.document()) else {
            return Vec::new();
        };
        crate::library::part_sync::revalidate_parts(syncs, stamps, doc)
    }

    /// The stamps for parts still on the row and version they were computed
    /// for (#2070), as `(placed symbol, stamp)`, and the parts that were
    /// re-linked while the worker ran: those get neither stamp nor sync, and
    /// a Messages line.
    fn stamps_still_linked(
        &self,
        sheet: &std::path::Path,
        stamps: Vec<CapturedStamp>,
    ) -> (Vec<(uuid::Uuid, LinkStamp)>, Vec<uuid::Uuid>) {
        let Some(doc) = self.document_state.engines.get(sheet).map(|e| e.document()) else {
            return (Vec::new(), Vec::new());
        };
        // One part is one link: a re-linked unit takes its siblings with it.
        let groups = signex_types::schematic::group_into_parts(
            doc.symbols
                .iter()
                .map(signex_types::schematic::Symbol::part_key),
        );
        let mut relinked = Vec::new();
        for c in &stamps {
            let Some(index) = doc.symbols.iter().position(|s| s.uuid == c.symbol) else {
                continue;
            };
            let live = &doc.symbols[index];
            if relinked.contains(&c.symbol)
                || (live.row_id == Some(c.row_id) && live.library_version == c.library_version)
            {
                continue;
            }
            crate::diagnostics::log_warning(format!(
                "{} was re-linked while the update ran; its update was skipped",
                live.reference
            ));
            let unit_ids = groups
                .iter()
                .find(|g| g.contains(&index))
                .into_iter()
                .flatten()
                .map(|&i| doc.symbols[i].uuid);
            for id in unit_ids {
                if !relinked.contains(&id) {
                    relinked.push(id);
                }
            }
        }
        let kept = stamps
            .into_iter()
            .filter(|c| !relinked.contains(&c.symbol))
            .map(|c| (c.symbol, c.link))
            .collect();
        (kept, relinked)
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
        stamps: Vec<(uuid::Uuid, LinkStamp)>,
        part_syncs: Vec<PartSync>,
        record_history: bool,
    ) -> bool {
        // The copy, the version stamps and the parts' footprint / MPN /
        // manufacturer edits are one undo step (#1867).
        let mut commands = vec![signex_engine::Command::ReplaceLibSymbol {
            lib_symbol: fresh,
            library_versions: stamps,
        }];
        if record_history {
            commands.extend(part_syncs.iter().flat_map(PartSync::commands));
        }
        let refused = if self.document_state.active_path.as_deref() == Some(sheet) {
            // Through the mutation gateway: canvas sync, dirty flag,
            // netlist and panel refresh. A refusal is already in Messages.
            let (outcome, _) = match commands.len() {
                1 => {
                    let command = commands.remove(0);
                    if record_history {
                        self.apply_engine_command_reporting(command, true, true)
                    } else {
                        self.apply_engine_command_unrecorded(command)
                    }
                }
                _ => self.apply_engine_commands_reporting(commands, true, true),
            };
            outcome == crate::app::mutation_gateway::CommandOutcome::Failed
        } else {
            self.replace_on_parked_sheet(sheet, commands, record_history)
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
        mut commands: Vec<signex_engine::Command>,
        record_history: bool,
    ) -> bool {
        let Some(engine) = self.document_state.engines.get_mut(sheet) else {
            return true;
        };
        let result = if !record_history {
            engine.execute_unrecorded(commands.remove(0))
        } else if commands.len() == 1 {
            engine.execute(commands.remove(0))
        } else {
            engine.execute_batch(commands)
        };
        match result {
            Ok(result) => {
                if result.changed {
                    if record_history {
                        self.document_state.mark_history_edit_dirty(sheet);
                    } else {
                        self.document_state.mark_document_dirty(sheet);
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
