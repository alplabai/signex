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

    pub(crate) fn active_pcb_snapshot(&self) -> Option<&PcbBoard> {
        self.active_pcb()
    }

    /// Move the active schematic to `to` once a Save As wrote it there.
    /// `finish` settles the engine where it is and returns whether the
    /// document is still dirty; only then is the engine moved, with no other
    /// code between its removal and its reinsertion, so a panic in `finish`
    /// leaves it at its old path. `false`, with nothing changed, when there
    /// is no active schematic tab.
    pub(crate) fn rekey_active_schematic(
        &mut self,
        to: PathBuf,
        finish: impl FnOnce(&mut signex_engine::Engine) -> bool,
    ) -> bool {
        let Some(old_path) = self.document_state.active_path.clone() else {
            return false;
        };
        if self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .is_none()
        {
            return false;
        }
        let was_dirty = self.document_state.is_document_dirty(&old_path);
        let Some(engine) = self.document_state.engines.get_mut(&old_path) else {
            return false;
        };
        let dirty = finish(engine);
        let Some(mut engine) = self.document_state.engines.remove(&old_path) else {
            return false;
        };
        engine.set_path(Some(to.clone()));
        self.document_state.insert_engine(to.clone(), engine);

        if let Some(tab) = self
            .document_state
            .tabs
            .get_mut(self.document_state.active_tab)
        {
            tab.title = to
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_else(|| "Schematic".to_string());
            tab.path = to.clone();
        }
        if to != old_path {
            // The old key no longer has an engine, so a lingering dirty
            // marker would make every later Save All fail on it.
            self.document_state.clear_document_dirty(&old_path);
            self.rekey_page_setup(&old_path, &to);
            // The new file is not what any sheet symbol places.
            self.document_state.tab_placements.forget(&old_path);
            self.rekey_undocked_windows(&old_path, &to);
        }
        // Write back only a change `finish` made: a dirty mark re-written
        // here would count as one from outside the history and keep undo
        // from ever clearing it.
        if to != old_path || dirty != was_dirty {
            if dirty {
                self.document_state.mark_document_dirty(&to);
            } else {
                self.document_state.mark_document_clean(&to);
            }
        }
        self.document_state.active_path = Some(to);
        true
    }

    pub(crate) fn park_active_schematic_session(&mut self) {
        // HashMap storage means engines never move — every schematic
        // tab's engine lives keyed by its on-disk path regardless of
        // which tab is active. Kept as an API seam so callers about to
        // swap `active_tab` don't need to know the storage changed.
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
    /// Returns whether the new engine was installed.
    #[cfg(test)]
    pub(crate) fn install_schematic_at(
        &mut self,
        path: &std::path::Path,
        sheet: SchematicSheet,
    ) -> bool {
        // The non-active arm of `install_reloaded_schematic`, without its
        // repair reports and settle steps, as on trunk.
        if self.text_edit_targets_sheet_at(path) {
            self.interaction_state.editing_text = None;
        }
        let result = signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf()));
        let install = self.document_state.replace_engine_at(path, result);
        self.project_replaced_engine(path, install.installed, &install.notices);
        install.installed
    }

    /// Make `result` the engine of the active tab at `p`; a failure reaches
    /// the Messages panel. The core half with its notices logged, as
    /// `apply_loaded_schematic` runs it. Returns whether it was installed.
    #[cfg(test)]
    fn install_active_engine(
        &mut self,
        p: PathBuf,
        result: Result<signex_engine::Engine, signex_engine::EngineError>,
    ) -> bool {
        let install = self.document_state.install_active(p, result);
        super::projector::log_notices(&install.notices);
        install.installed
    }

    /// The engine at `path` holds what was just read from disk: its history
    /// position is the clean point. Only when `installed`: a failed install
    /// leaves the older engine at `path`, which is not known to equal the file.
    pub(crate) fn mark_installed_engine_saved(&mut self, path: &std::path::Path, installed: bool) {
        self.document_state
            .mark_installed_engine_saved(path, installed);
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
        canvas.set_theme_colors(main.canvas_colors);
        canvas.set_grid(main.grid_visible, main.visible_grid_mm);
        canvas.set_board_grid(Some(signex_engine::pcb::placement::grid_step_mm(board)));
        canvas.set_board_marks(crate::pcb_canvas::BoardMarks::of(board));
        let view = self.pcb_layer_view(path);
        canvas.set_origin_marker_visible(view.origin_marker_visible());
        canvas.set_view_flipped(view.flipped());
        let snapshot = self.undocked_pcb_snapshot(path, board);
        canvas.set_renderer_snapshot(Some(snapshot));
        canvas.fit_to_board();
        Some(canvas)
    }

    pub(crate) fn open_schematic_tab(
        &mut self,
        path: PathBuf,
        title: String,
        mut sheet: SchematicSheet,
    ) {
        warn_if_read_only(&path, sheet.unknown.newer_minor_source.as_deref());
        report_unrecognized_sheet_size(&path, &sheet.paper_size);
        // Pins touching a wire with no dot are reported, never fixed on open.
        super::pin_junction_repair::report_bare_pin_taps(&path, &sheet);
        // A read-only sheet cannot be saved, so a repair to it stays a report.
        let repaired = super::sheet_repairs::log_sheet_repairs(&path, &sheet.take_load_repairs())
            && !sheet.is_read_only();
        self.park_active_schematic_session();
        let scan_path = path.clone();
        self.document_state.register_schematic_tab(path, title);

        let installed = self.apply_loaded_schematic(Some(sheet), true, true, true);
        // Read from disk: the opened document is the saved state, so undoing
        // every edit made since returns to a clean sheet. (A recovery copy
        // is installed by a different path and stays dirty-safe.) Every
        // schematic open funnels through here: File > Open, the project
        // tree, a child sheet and ERC navigation (#1208), so here autosave
        // is held for an earlier run's recovery copy and the per-placement
        // designator note is queued.
        self.document_state
            .settle_opened_sheet(&scan_path, installed, repaired);
        self.note_sheet_focused();
        crate::oversize_text::rearm_warning();

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
        // A copied file that repeats an id is told and repaired as it opens,
        // not when an agent first asks (#2172).
        self.reconcile_document_ids();
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
        self.document_state.reattach_parked_sheet(path, title);
        // `apply_loaded_schematic` with `schematic = None` skips the
        // `engines.insert` overwrite path and just refreshes the canvas /
        // panel against the existing engine.
        self.apply_loaded_schematic(None, true, true, true);
        self.note_sheet_focused();
    }

    pub(crate) fn open_pcb_tab(&mut self, path: PathBuf, title: String, mut board: PcbBoard) {
        report_unusable_zone_holes(&path, &board);
        let repaired = super::sheet_repairs::take_and_log_board(&path, &mut board);
        warn_if_read_only(&path, board.unknown.newer_minor_source.as_deref());
        self.park_active_schematic_session();
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        let engine = self.document_state.new_board_engine(&path, board);
        // An earlier run's recovery copy of this board (#1185): hold
        // autosave back and let `update` start the check, as for a sheet.
        self.document_state.autosave.begin_check(&path);
        self.document_state.recovery_check_due.push(path.clone());
        let dirty_path = path.clone();
        self.document_state.tabs.push(TabInfo {
            title,
            path,
            cached_document: Some(TabDocument::Pcb(engine)),
            project_id,
            kind: super::TabKind::Pcb,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.apply_loaded_pcb_document(true, true);
        if repaired {
            // The file on disk still holds what was repaired (#1886).
            self.document_state.mark_document_dirty(&dirty_path);
        }
        self.reconcile_document_ids();
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
        if !is_pcb {
            // The status bar's board angle field goes with the board (#2316).
            self.close_view_angle_entry_in(None);
        }

        if is_schematic {
            // Selection UUIDs belong to the document they were picked in;
            // carrying them to another schematic tab leaves ghost
            // selections that delete/move would target (#102).
            let previous_path = self.document_state.active_path.clone();
            if self.document_state.activate_tab_engine() {
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
        // The engine stays resident; only closing its tab removes it
        // (`close_tab_at_index` in `handlers/document_tabs.rs`, #51).
        self.document_state.detach_active_schematic();
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
            // A board opened into the tab starts upright (#2316).
            if self.interaction_state.pcb_canvas.set_view_rotation(0.0) {
                self.close_view_angle_entry_in(None);
            }
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
        self.note_pcb_board_changed();
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
    /// first. `fit_sheet` queues a fit of the paper at the next canvas event
    /// (the sheet's first show).
    ///
    /// There is no "commit to the active tab" step: since the parked-session
    /// refactor, schematic tabs keep their document in
    /// `document_state.engines` (keyed by path) and carry
    /// `cached_document: None`. `TabDocument` has no `Schematic` variant to
    /// write back to, and the mutation gateway derives the dirty mark from
    /// the engine history (`mark_active_document_dirty`).
    /// Returns whether `schematic` was installed as a new active engine.
    pub(crate) fn apply_loaded_schematic(
        &mut self,
        schematic: Option<SchematicSheet>,
        clear_bg_cache: bool,
        fit_sheet: bool,
        refresh_panel_ctx: bool,
    ) -> bool {
        let installed = match schematic {
            Some(schematic) => {
                let install = self
                    .document_state
                    .sync_engine_from_schematic(Some(schematic));
                super::projector::log_notices(&install.notices);
                install.installed
            }
            None => false,
        };
        if !self.document_state.has_active_engine() {
            return installed;
        }
        self.sync_canvas_from_visible_schematic(crate::schematic_runtime::RenderInvalidation::FULL);
        if let Some(snapshot) = self.active_render_snapshot() {
            for lib_id in crate::schematic_runtime::symbols_without_geometry(snapshot) {
                crate::diagnostics::log_warning(format!(
                    "Library entry for symbol '{lib_id}' is missing on this sheet; it is drawn as a crossed-out placeholder box"
                ));
            }
        }

        // The first time a sheet is shown it opens fitted and centred, whatever
        // it holds; the fit is taken at the next canvas event (#1941).
        if fit_sheet {
            self.interaction_state.active_canvas().fit_to_sheet();
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
        installed
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
#[path = "load_seam_tests.rs"]
mod load_seam_tests;

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

    /// #1854 — a restore whose install failed keeps a read-only older
    /// engine dirty instead of reading it as matching the disk.
    #[test]
    fn a_failed_restore_install_keeps_the_old_read_only_sheet_dirty() {
        let path = PathBuf::from("readonly-1854.snxsch");
        let (mut app, _boot) = Signex::new();
        let mut sheet = SchematicSheet::default();
        sheet.unknown.newer_minor_source = Some("1.99".to_string());
        let engine = signex_engine::Engine::new_with_path(sheet, Some(path.clone())).unwrap();
        assert!(engine.document().is_read_only());
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.mark_document_dirty(&path);

        app.settle_restored_schematic(&path, false, false, None);
        assert!(app.document_state.is_document_dirty(&path));
        assert!(!app.document_state.engines[&path].is_at_save_point());

        app.settle_restored_schematic(&path, true, false, None);
        assert!(!app.document_state.is_document_dirty(&path));
    }

    /// #1854 — a failed install must not mark the older, edited engine saved.
    #[test]
    fn a_failed_install_does_not_mark_the_old_engine_saved() {
        let path = PathBuf::from("stale-1854.snxsch");
        let (mut app, _boot) = Signex::new();
        let engine =
            signex_engine::Engine::new_with_path(SchematicSheet::default(), Some(path.clone()))
                .unwrap();
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.active_path = Some(path.clone());

        let installed =
            app.install_active_engine(path.clone(), Err(signex_engine::EngineError::MissingPath));
        assert!(!installed);
        app.mark_installed_engine_saved(&path, installed);
        assert!(!app.document_state.engines[&path].is_at_save_point());

        let fresh =
            signex_engine::Engine::new_with_path(SchematicSheet::default(), Some(path.clone()));
        let installed = app.install_active_engine(path.clone(), fresh);
        assert!(installed);
        app.mark_installed_engine_saved(&path, installed);
        assert!(app.document_state.engines[&path].is_at_save_point());
    }

    /// #1916 — a disk-conflict Reload whose install failed keeps the older
    /// edited sheet dirty, the old baseline and the save failure; an installed
    /// one is clean and adopts the disk hash. Covers only the settle rule:
    /// `Engine::new_with_path` cannot fail, so no seam makes the real
    /// `apply_reloaded_reporting` install fail.
    #[test]
    fn a_failed_disk_reload_install_keeps_the_conflict_state() {
        use crate::persist::ContentHash;
        let path = PathBuf::from("reload-1916.snxsch");
        let (mut app, _boot) = Signex::new();
        let engine =
            signex_engine::Engine::new_with_path(SchematicSheet::default(), Some(path.clone()))
                .unwrap();
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.active_path = Some(path.clone());
        app.document_state.mark_document_dirty(&path);
        let old = ContentHash::of(b"old");
        let disk = ContentHash::of(b"external");
        app.record_disk_baseline(&path, old);
        app.document_state
            .document_saves
            .record_failure(path.clone(), "disk changed".into());

        assert!(!app.settle_disk_reload(&path, disk, false, false));
        assert!(app.document_state.is_document_dirty(&path));
        assert_eq!(app.document_state.document_saves.baseline(&path), Some(old));
        assert!(app.document_state.document_saves.failure(&path).is_some());

        assert!(app.settle_disk_reload(&path, disk, true, false));
        assert!(!app.document_state.is_document_dirty(&path));
        assert!(app.document_state.engines[&path].is_at_save_point());
        assert_eq!(
            app.document_state.document_saves.baseline(&path),
            Some(disk)
        );
        assert!(app.document_state.document_saves.failure(&path).is_none());
    }
}
