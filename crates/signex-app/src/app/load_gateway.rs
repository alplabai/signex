use std::path::PathBuf;

use signex_types::pcb::PcbBoard;
use signex_types::schematic::SchematicSheet;

use super::*;

impl Signex {
    fn active_tab_cached_document(&self) -> Option<&TabDocument> {
        self.document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|tab| tab.cached_document.as_ref())
    }

    pub(crate) fn active_schematic(&self) -> Option<&SchematicSheet> {
        self.document_state
            .active_engine()
            .map(|engine| engine.document())
    }

    pub(crate) fn active_pcb(&self) -> Option<&PcbBoard> {
        self.active_tab_cached_document()
            .and_then(TabDocument::as_pcb)
    }

    pub(crate) fn has_active_schematic(&self) -> bool {
        self.active_schematic().is_some()
    }

    pub(crate) fn has_active_pcb(&self) -> bool {
        self.active_pcb().is_some()
    }

    pub(crate) fn active_render_snapshot(
        &self,
    ) -> Option<&crate::schematic_runtime::SchematicRenderSnapshot> {
        self.interaction_state.active_canvas().active_snapshot()
    }

    pub(crate) fn active_pcb_snapshot(&self) -> Option<&PcbBoard> {
        self.active_pcb()
    }

    pub(crate) fn with_active_schematic_session_mut<R>(
        &mut self,
        update: impl FnOnce(&mut SchematicTabSession) -> R,
    ) -> Option<R> {
        use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

        // Temporarily remove the active engine from the HashMap so we
        // can hand the caller a `SchematicTabSession` (legacy API that
        // owns its engine). save/save_as mutate `session.path` — when
        // that happens, we reinsert at the new key so the HashMap
        // follows the tab's on-disk location.
        let old_path = self.document_state.active_path.clone()?;
        let engine = self.document_state.engines.remove(&old_path)?;

        let Some(title) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.title.clone())
        else {
            self.document_state.engines.insert(old_path, engine);
            return None;
        };

        let was_dirty = self.document_state.is_document_dirty(&old_path);
        // Panic guard: the engine moves into `session`, so a panicking
        // closure drops it. We cannot get it back; catch the unwind only
        // to clear `active_path` (no dangling key) before re-raising.
        let session = SchematicTabSession::new(engine, title, old_path.clone(), was_dirty);
        let update_result = catch_unwind(AssertUnwindSafe(move || {
            let mut session = session;
            let r = update(&mut session);
            (r, session.into_parts())
        }));

        match update_result {
            Ok((result, (engine, title, new_path, dirty))) => {
                if let Some(tab) = self
                    .document_state
                    .tabs
                    .get_mut(self.document_state.active_tab)
                {
                    tab.title = title;
                    tab.path = new_path.clone();
                }

                // The session carried the dirty flag in and out; write it
                // back so the mark survives tab close.
                if new_path != old_path {
                    // Save As re-keyed the document; the old key no longer
                    // has an engine, so a lingering dirty marker would make
                    // every later Save All fail on it.
                    self.document_state.mark_document_clean(&old_path);
                    self.rekey_page_setup(&old_path, &new_path);
                    // The new file is not what any sheet symbol places.
                    self.document_state.tab_placements.forget(&old_path);
                    self.rekey_undocked_windows(&old_path, &new_path);
                }
                self.document_state.insert_engine(new_path.clone(), engine);

                // Write back only a change the closure made: a dirty mark
                // re-written here would count as one from outside the
                // history and keep undo from ever clearing it.
                if new_path != old_path || dirty != was_dirty {
                    if dirty {
                        self.document_state.mark_document_dirty(&new_path);
                    } else {
                        self.document_state.mark_document_clean(&new_path);
                    }
                }
                self.document_state.active_path = Some(new_path);
                Some(result)
            }
            Err(payload) => {
                // Engine was consumed by the panicking session; the
                // HashMap is now missing its entry and cannot be restored.
                tracing::error!(
                    path = %old_path.display(),
                    "schematic session panicked; its engine was dropped"
                );
                self.document_state.active_path = None;
                resume_unwind(payload);
            }
        }
    }

    pub(crate) fn park_active_schematic_session(&mut self) {
        // HashMap storage means engines never move — every schematic
        // tab's engine lives keyed by its on-disk path regardless of
        // which tab is active. Kept as an API seam so callers about to
        // swap `active_tab` don't need to know the storage changed.
    }

    fn activate_active_schematic_session(&mut self) -> bool {
        // Point `active_path` at the active tab's schematic engine if
        // one exists. Returns true iff the active engine is now live
        // and ready for `active_engine()` lookups.
        let path = self.active_tab_path();
        if let Some(p) = path.as_ref()
            && self.document_state.engines.contains_key(p)
        {
            self.document_state.active_path = path;
            true
        } else {
            self.document_state.active_path = None;
            false
        }
    }

    pub(crate) fn active_tab_path(&self) -> Option<PathBuf> {
        self.document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.clone())
    }

    /// Install a reloaded `sheet` as the engine of the (non-active) schematic
    /// at `path`, leaving the canvas and the user's tool alone. A sheet the
    /// engine rejects drops the stale engine, so a save cannot undo it.
    pub(crate) fn install_schematic_at(&mut self, path: &std::path::Path, sheet: SchematicSheet) {
        // Inline text typed into a window on this sheet targets an object of
        // the engine being replaced.
        if self.text_edit_targets_sheet_at(path) {
            self.interaction_state.editing_text = None;
        }
        match signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf())) {
            Ok(engine) => {
                self.forget_net_colors_of(path);
                self.document_state
                    .engines
                    .insert(path.to_path_buf(), engine);
                self.adopt_net_colors_of(path);
                self.clear_net_color_steps(path);
                // An undocked window on this sheet must not keep drawing the
                // replaced engine, nor select its objects.
                self.refresh_slots_from_engine(
                    path,
                    false,
                    crate::schematic_runtime::RenderInvalidation::FULL,
                    true,
                );
            }
            Err(error) => {
                self.document_state.engines.remove(path);
                crate::diagnostics::log_warning(format!(
                    "[git] restore: {} was restored but its engine could not start: {error}",
                    path.display()
                ));
            }
        }
    }

    /// Make `result` the engine of the active tab at `p`. A failed engine is
    /// reported to the Messages panel and the engine already open there is
    /// kept: it may hold unsaved edits that dropping it would lose silently.
    fn install_active_engine(
        &mut self,
        p: PathBuf,
        result: Result<signex_engine::Engine, signex_engine::EngineError>,
    ) {
        match result {
            Ok(engine) => {
                // The incoming sheet's saved colours replace those
                // of the engine it stands in for.
                self.forget_net_colors_of(&p);
                self.document_state.insert_engine(p.clone(), engine);
                self.adopt_net_colors_of(&p);
                self.clear_net_color_steps(&p);
                self.document_state.active_path = Some(p);
            }
            Err(error) => crate::diagnostics::log_warning(format!(
                "{} could not be loaded into the editor ({error}); the sheet already open was kept",
                p.display()
            )),
        }
    }

    fn sync_engine_from_schematic(&mut self, schematic: Option<SchematicSheet>) {
        let path = self.active_tab_path();
        match (path, schematic) {
            (Some(p), Some(sheet)) => {
                let result = signex_engine::Engine::new_with_path(sheet, Some(p.clone()));
                self.install_active_engine(p, result);
            }
            _ => self.document_state.clear_active_engine(),
        }
    }

    pub(crate) fn sync_canvas_from_visible_schematic(
        &mut self,
        invalidation: crate::schematic_runtime::RenderInvalidation,
    ) {
        // Active engine drives the render cache — if it's gone the
        // cache is cleared. There is no parked-schematic fallback with
        // HashMap storage: an open schematic tab always has its engine
        // resident in `document_state.engines`.
        if let Some(path) = self
            .document_state
            .active_path
            .clone()
            .filter(|p| self.document_state.engines.contains_key(p))
        {
            self.refresh_slots_from_engine(&path, true, invalidation, false);
            return;
        }

        self.interaction_state
            .active_canvas_mut()
            .set_render_cache(None);
    }

    /// Redraw every canvas slot showing the engine at `path` from it: the
    /// undocked windows on that sheet (#171) and, with `include_main`, the
    /// main window's slot. `clear_selection` also drops each slot's
    /// selection, for an engine that was replaced wholesale: its UUIDs may
    /// not exist in the new document (#102).
    pub(crate) fn refresh_slots_from_engine(
        &mut self,
        path: &std::path::Path,
        include_main: bool,
        invalidation: crate::schematic_runtime::RenderInvalidation,
        clear_selection: bool,
    ) {
        let Some(engine) = self.document_state.engines.get(path) else {
            return;
        };
        let sheet = engine.document();
        let (base, revision) = (engine.base_revision(), engine.revision());
        let windows = &self.ui_state.windows;
        let main = include_main.then_some(&mut self.interaction_state.canvas);
        let slots = self
            .interaction_state
            .canvases
            .iter_mut()
            .filter(|(id, _)| {
                matches!(
                    windows.get(id),
                    Some(super::state::WindowKind::UndockedTab { path: shown, .. })
                        if shown == path
                )
            })
            .map(|(_, slot)| slot)
            .chain(main);
        for slot in slots {
            if let Some(cache) = slot.render_cache.as_mut() {
                cache.update_from_sheet(sheet, invalidation, base, revision);
            } else {
                slot.set_render_cache(Some(
                    crate::schematic_runtime::SchematicRenderCache::from_sheet_at(sheet, revision),
                ));
            }
            if clear_selection {
                slot.selected.clear();
            }
            // The drawn geometry rebuilds only on a clear or a camera move.
            slot.clear_content_cache();
        }
    }

    /// Whether the open inline text edit targets an object of the engine at
    /// `path`.
    fn text_edit_targets_sheet_at(&self, path: &std::path::Path) -> bool {
        use signex_types::schematic::SelectedKind;
        let (Some(edit), Some(engine)) = (
            self.interaction_state.editing_text.as_ref(),
            self.document_state.engines.get(path),
        ) else {
            return false;
        };
        let sheet = engine.document();
        match edit.kind {
            SelectedKind::Label => sheet.labels.iter().any(|l| l.uuid == edit.uuid),
            SelectedKind::TextNote => sheet.text_notes.iter().any(|t| t.uuid == edit.uuid),
            SelectedKind::SymbolRefField | SelectedKind::SymbolValField | SelectedKind::Symbol => {
                sheet.symbols.iter().any(|s| s.uuid == edit.uuid)
            }
            _ => false,
        }
    }

    /// A restore is about to replace (or drop) the engine behind the visible
    /// schematic. Inline text, a pending move and the selection point into
    /// the old document, so they must not outlive it.
    pub(crate) fn discard_state_of_replaced_engine(&mut self) {
        self.interaction_state.editing_text = None;
        self.cancel_move_pick();
        self.interaction_state.active_canvas_mut().selected.clear();
        self.update_selection_info();
    }

    /// Refresh only the main window's canvas from the engine at `path` after
    /// an edit made through an undocked window's swapped-in slot. The other
    /// undocked slots were already refreshed by that edit's own sync.
    pub(crate) fn refresh_main_canvas_from_engine(&mut self, path: &std::path::Path) {
        let Some(engine) = self.document_state.engines.get(path) else {
            return;
        };
        let slot = &mut self.interaction_state.canvas;
        let sheet = engine.document();
        let revision = engine.revision();
        match slot.render_cache.as_mut() {
            Some(cache) => cache.update_from_sheet(
                sheet,
                crate::schematic_runtime::RenderInvalidation::FULL,
                engine.base_revision(),
                revision,
            ),
            None => slot.set_render_cache(Some(
                crate::schematic_runtime::SchematicRenderCache::from_sheet_at(sheet, revision),
            )),
        }
        slot.clear_content_cache();
    }

    /// The board shown in `window_id`: the active tab's in the main window,
    /// the undocked tab's in an undocked one.
    pub(crate) fn pcb_for_window(&self, window_id: iced::window::Id) -> Option<&PcbBoard> {
        if self.ui_state.main_window_id == Some(window_id) {
            return self.active_pcb();
        }
        let Some(super::state::WindowKind::UndockedTab { path, .. }) =
            self.ui_state.windows.get(&window_id)
        else {
            return None;
        };
        self.document_state
            .tabs
            .iter()
            .find(|tab| &tab.path == path)
            .and_then(|tab| tab.cached_document.as_ref())
            .and_then(TabDocument::as_pcb)
    }

    /// A PCB canvas for an undocked window showing the PCB tab at `path`,
    /// or `None` when that tab is not a PCB. It starts with the app-wide
    /// settings the main canvas carries and its own camera, fitted to the
    /// board.
    pub(crate) fn new_pcb_canvas_for(
        &self,
        path: &std::path::Path,
    ) -> Option<crate::pcb_canvas::PcbCanvas> {
        let board = self
            .document_state
            .tabs
            .iter()
            .find(|tab| tab.path == path)
            .and_then(|tab| tab.cached_document.as_ref())
            .and_then(TabDocument::as_pcb)?;
        let main = &self.interaction_state.pcb_canvas;
        let mut canvas = crate::pcb_canvas::PcbCanvas::new();
        canvas.gpu_render = main.gpu_render;
        canvas.set_theme_colors(main.theme_bg, main.theme_grid);
        canvas.canvas_colors = main.canvas_colors;
        canvas.set_grid(main.grid_visible, main.visible_grid_mm);
        canvas.set_renderer_snapshot(Some(signex_renderer::pcb::PcbSnapshot::from_board(board)));
        canvas.fit_to_board();
        Some(canvas)
    }

    pub(crate) fn sync_pcb_canvas_from_visible_board(&mut self) {
        let renderer_snapshot = self
            .active_pcb()
            .map(signex_renderer::pcb::PcbSnapshot::from_board);

        // An undocked window showing the same tab draws the same board
        // (#494). Its camera stays its own.
        let active_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.as_path());
        for (id, canvas) in &mut self.interaction_state.pcb_canvases {
            let shows_active = matches!(
                self.ui_state.windows.get(id),
                Some(super::state::WindowKind::UndockedTab { path, .. })
                    if Some(path.as_path()) == active_path
            );
            if shows_active {
                canvas.set_renderer_snapshot(renderer_snapshot.clone());
                canvas.clear_content_cache();
            }
        }
        self.interaction_state
            .pcb_canvas
            .set_renderer_snapshot(renderer_snapshot);
    }

    pub(crate) fn open_schematic_tab(
        &mut self,
        path: PathBuf,
        title: String,
        sheet: SchematicSheet,
    ) {
        warn_if_read_only(&path, sheet.unknown.newer_minor_source.as_deref());
        report_unrecognized_sheet_size(&path, &sheet.paper_size);
        self.park_active_schematic_session();
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        let scan_path = path.clone();
        self.document_state.tabs.push(TabInfo {
            title,
            path,
            cached_document: None,
            project_id,
            kind: super::TabKind::Schematic,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;

        self.apply_loaded_schematic(Some(sheet), true, true, true);
        // Read from disk: the opened document is the saved state, so undoing
        // every edit made since returns to a clean sheet. (A recovery copy
        // is installed by a different path and stays dirty-safe.)
        if let Some(engine) = self.document_state.engines.get_mut(&scan_path) {
            engine.mark_saved();
        }
        self.note_sheet_focused();
        // Every schematic open funnels through here; `finish_update` starts
        // the (once per project) per-placement designator note off-thread.
        self.document_state.instance_note_due = true;

        // An earlier run's recovery copy of this file may exist. Hold
        // autosave back from now on, and let `update` start the check:
        // File > Open, the project tree, a child sheet and ERC navigation
        // all open through here (#1208).
        self.document_state.autosave.begin_check(&scan_path);
        self.document_state
            .recovery_check_due
            .push(scan_path.clone());

        // A document naming a font this machine lacks is told to the user
        // once per open (#510); until the scan lands this is a no-op and
        // the scan's completion reports instead.
        self.forget_missing_font_reports(&scan_path);
        self.report_missing_fonts(&scan_path);

        // Stage 16 §3.5 — Library Updates Available scan.
        // Walks placed Symbols whose `library_id` is set and
        // surfaces drift against the source library row's current
        // version. Personal-mode libraries auto-apply silently; Team
        // mode opens the "Library Updates Available" modal on the
        // next view tick.
        self.scan_library_updates_for_open_schematic(scan_path);
        self.library.drift.check_pending = true;
    }

    /// Reattach a tab to a schematic engine that's already parked in
    /// `document_state.engines`. Used when the user reopens a file
    /// that was closed while dirty — we kept the engine alive in
    /// `close_tab_now` precisely so the in-memory edits survive the
    /// reopen. Re-parsing from disk would discard those edits.
    ///
    /// Pre-condition: `document_state.engines.contains_key(&path)`.
    /// Post-condition: a new tab exists pointing at `path` with
    /// the parked entry's dirty mark, the active engine is the parked entry, and the
    /// canvas reflects the parked sheet's current state.
    pub(crate) fn attach_parked_schematic_tab(&mut self, path: PathBuf, title: String) {
        self.park_active_schematic_session();
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        self.document_state.tabs.push(TabInfo {
            title,
            path: path.clone(),
            cached_document: None,
            project_id,
            kind: super::TabKind::Schematic,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        // Point active_path at the parked entry. `apply_loaded_schematic`
        // with `schematic = None` skips the `engines.insert` overwrite
        // path and just refreshes the canvas / panel against the
        // existing engine.
        self.document_state.active_path = Some(path);
        self.apply_loaded_schematic(None, true, true, true);
        self.note_sheet_focused();
    }

    pub(crate) fn open_pcb_tab(&mut self, path: PathBuf, title: String, board: PcbBoard) {
        report_unusable_zone_holes(&path, &board);
        warn_if_read_only(&path, board.unknown.newer_minor_source.as_deref());
        self.park_active_schematic_session();
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        self.document_state.stamp_pcb_revision(&path);
        // An earlier run's recovery copy of this board (#1185): hold
        // autosave back and let `update` start the check, as for a sheet.
        self.document_state.autosave.begin_check(&path);
        self.document_state.recovery_check_due.push(path.clone());
        self.document_state.tabs.push(TabInfo {
            title,
            path,
            cached_document: Some(TabDocument::Pcb(board)),
            project_id,
            kind: super::TabKind::Pcb,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.apply_loaded_pcb_document(true, true);
    }

    pub(crate) fn sync_visible_document_from_active_tab(&mut self) {
        self.interaction_state.editing_text = None;

        // Schematic tabs store their engine in `document_state.engines`,
        // keyed by the tab's path. PCB tabs still hold their board in
        // `cached_document`. Dispatch on whichever backing storage owns
        // the active tab's document.
        let active_path = self.active_tab_path();
        let is_schematic = active_path
            .as_ref()
            .map(|p| self.document_state.engines.contains_key(p))
            .unwrap_or(false);
        let is_pcb = matches!(self.active_tab_cached_document(), Some(TabDocument::Pcb(_)));

        if is_schematic {
            // Selection UUIDs belong to the document they were picked in;
            // carrying them to another schematic tab leaves ghost
            // selections that delete/move would target (#102).
            let previous_path = self.document_state.active_path.clone();
            if self.activate_active_schematic_session() {
                if previous_path != self.document_state.active_path {
                    self.interaction_state.active_canvas_mut().selected.clear();
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    // In-flight drawing buffers belong to the document too.
                    self.clear_drawing_buffers();
                    // So does a part on the cursor: it was armed for the
                    // sheet that is no longer showing.
                    self.cancel_placement_session();
                    self.cancel_move_pick();
                    // Also resets the mirrored panel_ctx selection fields.
                    self.update_selection_info();
                }
                self.apply_loaded_schematic(None, true, false, false);
            }
        } else if is_pcb {
            self.apply_loaded_pcb_document(false, false);
        } else {
            self.apply_loaded_empty_document(false);
        }
    }

    fn clear_schematic_ui_state(&mut self) {
        // Detach from the currently-visible schematic engine, but
        // leave its entry in `document_state.engines` untouched — the
        // engine should outlive a tab switch so switching back to a
        // parked schematic (e.g. Schematic → PCB → Schematic from the
        // project tree) finds its engine still resident. The only
        // authoritative removal point is `close_tab_at_index` in
        // `handlers/document_tabs.rs`, which explicitly prunes the
        // closing tab's entry. GitHub issue #51.
        self.document_state.active_path = None;
        self.cancel_placement_session();
        self.interaction_state
            .active_canvas_mut()
            .set_render_cache(None);
        self.interaction_state.active_canvas_mut().selected.clear();
        self.interaction_state
            .active_canvas_mut()
            .wire_preview
            .clear();
        self.interaction_state.active_canvas_mut().drawing_mode = false;
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
        self.interaction_state.current_tool = Tool::Select;
    }

    pub(crate) fn apply_loaded_pcb_document(
        &mut self,
        fit_to_board: bool,
        refresh_panel_ctx: bool,
    ) {
        self.clear_schematic_ui_state();
        self.sync_pcb_canvas_from_visible_board();
        if fit_to_board {
            self.interaction_state.pcb_canvas.fit_to_board();
        }
        self.interaction_state.pcb_canvas.clear_bg_cache();
        self.interaction_state.pcb_canvas.clear_content_cache();

        if refresh_panel_ctx {
            self.refresh_panel_ctx();
        } else {
            self.sync_panel_ctx_from_visible_document();
        }
    }

    pub(crate) fn apply_loaded_empty_document(&mut self, refresh_panel_ctx: bool) {
        self.clear_schematic_ui_state();
        self.interaction_state
            .pcb_canvas
            .set_renderer_snapshot(None);
        self.interaction_state.pcb_canvas.clear_bg_cache();
        self.interaction_state.pcb_canvas.clear_content_cache();

        if refresh_panel_ctx {
            self.refresh_panel_ctx();
        } else {
            self.sync_panel_ctx_from_visible_document();
        }
    }

    /// Refresh the canvas and panel context against the active schematic
    /// engine, optionally installing `schematic` as that engine's document
    /// first.
    ///
    /// There is no "commit to the active tab" step: since the parked-session
    /// refactor, schematic tabs keep their document in
    /// `document_state.engines` (keyed by path) and carry
    /// `cached_document: None`. `TabDocument` has no `Schematic` variant to
    /// write back to, and dirty tracking runs through
    /// `with_active_schematic_session_mut` in the mutation gateway.
    pub(crate) fn apply_loaded_schematic(
        &mut self,
        schematic: Option<SchematicSheet>,
        clear_bg_cache: bool,
        fit_to_content: bool,
        refresh_panel_ctx: bool,
    ) {
        if let Some(schematic) = schematic {
            self.sync_engine_from_schematic(Some(schematic));
        }
        if !self.document_state.has_active_engine() {
            return;
        }
        self.sync_canvas_from_visible_schematic(crate::schematic_runtime::RenderInvalidation::FULL);
        if let Some(snapshot) = self.active_render_snapshot() {
            for lib_id in crate::schematic_runtime::symbols_without_geometry(snapshot) {
                crate::diagnostics::log_warning(format!(
                    "Library entry for symbol '{lib_id}' is missing on this sheet; it is drawn as a crossed-out placeholder box"
                ));
            }
        }

        if fit_to_content {
            self.interaction_state.active_canvas_mut().fit_to_content();
        }
        if clear_bg_cache {
            self.interaction_state.active_canvas_mut().clear_bg_cache();
        }
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();

        if refresh_panel_ctx {
            self.refresh_panel_ctx();
        } else {
            self.sync_panel_ctx_from_visible_document();
        }
    }

    fn sync_panel_ctx_from_visible_document(&mut self) {
        self.document_state.panel_ctx.has_schematic = self.has_active_schematic();
        self.document_state.panel_ctx.has_pcb = self.has_active_pcb();

        // Counts and paper label for whichever document is visible; the
        // per-row symbol lists come from the same builder `refresh_panel_ctx`
        // uses, so the two paths cannot disagree about what a row holds.
        let (counts, paper_size) = if let Some(snapshot) = self.active_render_snapshot() {
            (
                (
                    snapshot.symbols.len(),
                    snapshot.wires.len(),
                    snapshot.labels.len(),
                    snapshot.junctions.len(),
                    snapshot.lib_symbols.len(),
                ),
                // Older documents may carry an empty paper_size (serde
                // default); normalize so the Page Options selector and the
                // drawn sheet agree on the effective A4 fallback.
                effective_paper_size(&snapshot.paper_size),
            )
        } else if let Some(snapshot) = self.active_pcb_snapshot() {
            (
                (
                    snapshot.footprints.len(),
                    snapshot.segments.len(),
                    snapshot.texts.len(),
                    snapshot.vias.len(),
                    0,
                ),
                format!("PCB • {} layers", snapshot.layers.len()),
            )
        } else {
            ((0, 0, 0, 0, 0), "A4".to_string())
        };

        {
            let ctx = &mut self.document_state.panel_ctx;
            ctx.sym_count = counts.0;
            ctx.wire_count = counts.1;
            ctx.label_count = counts.2;
            ctx.junction_count = counts.3;
            ctx.lib_symbol_count = counts.4;
            ctx.paper_size = paper_size;
        }
        self.fill_symbol_lists(None);
        // Written outside `fill_symbol_lists`; the next refresh must rebuild.
        self.document_state.panel_ctx.symbol_lists_key = None;

        // Drive the drawn sheet from the (re)loaded document's paper size —
        // without this the canvas keeps the previous tab's dimensions and the
        // sheet rectangle stops matching the stored A-series format.
        if self.has_active_schematic() {
            self.apply_page_dimensions_to_canvas();
        }
    }
}

/// Tell the user, once per open or reload, about zone holes the renderer
/// will not cut out, naming each zone and why (#897). Both render paths skip
/// them, so without this the zone would just look solid there.
pub(crate) fn report_unusable_zone_holes(path: &std::path::Path, board: &PcbBoard) {
    let zones: Vec<String> = board
        .zones
        .iter()
        .filter_map(|zone| {
            let problems = zone.hole_problems();
            if problems.is_empty() {
                return None;
            }
            let holes: Vec<String> = problems
                .iter()
                .map(|(i, problem)| format!("hole {i} {}", problem.describe()))
                .collect();
            let net = if zone.net_name.is_empty() {
                format!("net {}", zone.net)
            } else {
                zone.net_name.clone()
            };
            Some(format!(
                "zone {net} on {}: {}",
                zone.layer,
                holes.join(", ")
            ))
        })
        .collect();
    if !zones.is_empty() {
        // Details first: the Messages panel truncates long entries.
        crate::diagnostics::log_warning(format!(
            "{}: {}; drawn as solid copper (a hole must be a simple ring strictly inside \
             its outline, apart from the other holes)",
            path.display(),
            zones.join("; ")
        ));
    }
}

/// Paper token for the panel context: older documents carry an empty
/// `paper_size` (serde default), which every reader treats as A4.
pub(crate) fn effective_paper_size(paper_size: &str) -> String {
    if paper_size.is_empty() {
        "A4".to_string()
    } else {
        paper_size.to_string()
    }
}

/// Tell the user, once per sheet load, that the sheet's `paper_size` names no
/// size this build knows. The token is kept as written, so a save does not
/// rewrite it, but the page is drawn and printed as A4 landscape meanwhile.
pub(crate) fn report_unrecognized_sheet_size(path: &std::path::Path, paper_size: &str) {
    let (size, _) = signex_types::schematic::SheetSize::parse(paper_size);
    if !size.is_recognized() {
        crate::diagnostics::log_warning(format!(
            "{} has the paper size {paper_size:?}, which this build does not recognise; \
             it is drawn and printed as A4 landscape and the stored value is left unchanged",
            path.display()
        ));
    }
}

/// A document written by a newer Signex loads for viewing only: saving it
/// would drop content this build cannot represent (the writer refuses).
fn warn_if_read_only(path: &std::path::Path, newer_minor_source: Option<&str>) {
    if let Some(token) = newer_minor_source {
        crate::diagnostics::log_warning(format!(
            "{} was written by a newer Signex ({token}); it is read-only in this build -- saving is refused",
            path.display()
        ));
    }
}

#[cfg(test)]
mod sheet_size_report_tests {
    use crate::app::handlers::menu::export::tests::app_workspace;

    fn mentions(marker: &str) -> usize {
        let _ = crate::diagnostics::init_logging();
        crate::diagnostics::recent_entries()
            .iter()
            .filter(|entry| entry.message.contains(marker))
            .count()
    }

    /// Opens a sheet named `<name>_<uuid>` and counts only the diagnostics
    /// that mention that unique name, so entries logged by tests running in
    /// parallel cannot change the count.
    fn open(paper_size: &str, name: &str) -> usize {
        // Install the logger before the sheet opens: the warning is emitted
        // during `open_schematic_tab`, and without a logger it is dropped,
        // which would make this test depend on which test ran first.
        let _ = crate::diagnostics::init_logging();
        let name = format!("{name}_{}", uuid::Uuid::now_v7().simple());
        let mut app = app_workspace("/w/proj", &[]);
        let path = std::path::PathBuf::from(format!("/w/proj/{name}.snxsch"));
        let sheet = signex_types::schematic::SchematicSheet {
            uuid: uuid::Uuid::now_v7(),
            paper_size: paper_size.to_string(),
            ..Default::default()
        };
        app.open_schematic_tab(path, name.clone(), sheet);
        mentions(&name)
    }

    /// #1082 — an unknown size is told to the Messages panel once per load
    /// instead of silently becoming A4.
    #[test]
    fn an_unrecognised_paper_size_is_reported_once_per_load() {
        assert_eq!(open("Foolscap", "unknown_size_sheet"), 1);
    }

    #[test]
    fn a_recognised_paper_size_is_not_reported() {
        for token in ["A4", "A3 portrait", "ANSI C", "Custom 300x200"] {
            let name = format!("known_size_{}", token.replace(' ', "_"));
            assert_eq!(open(token, &name), 0, "{token}");
        }
    }
}

#[cfg(test)]
mod effective_paper_size_tests {
    use super::effective_paper_size;

    #[test]
    fn an_empty_paper_size_reads_as_a4_everywhere() {
        assert_eq!(effective_paper_size(""), "A4");
        assert_eq!(effective_paper_size("A3"), "A3");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #1470 — an engine that fails to start must not displace the open one
    /// (and its unsaved edits); the failure reaches the Messages panel.
    #[test]
    fn a_failed_engine_keeps_the_open_one_and_is_logged() {
        let path = PathBuf::from("keep-1470.snxsch");
        let needle = "keep-1470.snxsch could not be loaded";
        let before = crate::diagnostics::logged_count(needle);
        let (mut app, _boot) = Signex::new();
        let engine =
            signex_engine::Engine::new_with_path(SchematicSheet::default(), Some(path.clone()))
                .unwrap();
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.active_path = Some(path.clone());

        let failure = signex_engine::EngineError::MissingPath;
        app.install_active_engine(path.clone(), Err(failure));

        assert!(app.document_state.engines.contains_key(&path));
        assert_eq!(app.document_state.active_path, Some(path));
        assert_eq!(crate::diagnostics::logged_count(needle), before + 1);
    }
}
