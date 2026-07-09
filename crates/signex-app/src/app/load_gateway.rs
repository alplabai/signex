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

        let Some((title, _tab_path, dirty)) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| (tab.title.clone(), tab.path.clone(), tab.dirty))
        else {
            self.document_state.engines.insert(old_path, engine);
            return None;
        };

        // Panic-safe update: if the closure panics, the engine would
        // otherwise be dropped along with `session`. Catch + resume
        // ensures the engine returns to the HashMap at its original
        // path before the panic propagates.
        let session = SchematicTabSession::new(engine, title, old_path.clone(), dirty);
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
                    tab.dirty = dirty;
                }

                // Mirror tab dirty state into the project-scoped
                // `dirty_paths` set so the Projects-panel red dot
                // survives tab close and is single-source-of-truth
                // for "the file has unsaved edits".
                if dirty {
                    self.document_state.dirty_paths.insert(new_path.clone());
                } else {
                    self.document_state.dirty_paths.remove(&new_path);
                }

                self.document_state.engines.insert(new_path.clone(), engine);
                self.document_state.active_path = Some(new_path);
                Some(result)
            }
            Err(payload) => {
                // Engine was consumed by the panicking session; the
                // HashMap is now missing its entry. We can't restore
                // the engine (it unwound), but we can at least clear
                // `active_path` so callers don't see a dangling key.
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

    fn sync_engine_from_schematic(&mut self, schematic: Option<SchematicSheet>) {
        let path = self.active_tab_path();
        match (path, schematic) {
            (Some(p), Some(sheet)) => {
                match signex_engine::Engine::new_with_path(sheet, Some(p.clone())) {
                    Ok(engine) => {
                        self.document_state.engines.insert(p.clone(), engine);
                        self.document_state.active_path = Some(p);
                    }
                    Err(_) => self.document_state.clear_active_engine(),
                }
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
        if let Some(engine) = self.document_state.active_engine() {
            if let Some(cache) = self
                .interaction_state
                .active_canvas_mut()
                .render_cache
                .as_mut()
            {
                cache.update_from_sheet(engine.document(), invalidation);
            } else {
                self.interaction_state
                    .active_canvas_mut()
                    .set_render_cache(Some(
                        crate::schematic_runtime::SchematicRenderCache::from_sheet(
                            engine.document(),
                        ),
                    ));
            }
            return;
        }

        self.interaction_state
            .active_canvas_mut()
            .set_render_cache(None);
    }

    pub(crate) fn sync_pcb_canvas_from_visible_board(&mut self) {
        let renderer_snapshot = self
            .active_pcb()
            .map(signex_renderer::pcb::PcbSnapshot::from_board);

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
        self.park_active_schematic_session();
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        let scan_path = path.clone();
        self.document_state.tabs.push(TabInfo {
            title,
            path,
            cached_document: None,
            dirty: false,
            project_id,
            kind: super::TabKind::Schematic,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;

        self.apply_loaded_schematic(Some(sheet), true, true, true, true);

        // Stage 16 §3.5 — Library Updates Available scan.
        // Walks placed Symbols whose `library_id` is set and
        // surfaces drift against the source library row's current
        // version. Personal-mode libraries auto-apply silently; Team
        // mode opens the "Library Updates Available" modal on the
        // next view tick.
        self.scan_library_updates_for_open_schematic(scan_path);
    }

    /// Reattach a tab to a schematic engine that's already parked in
    /// `document_state.engines`. Used when the user reopens a file
    /// that was closed while dirty — we kept the engine alive in
    /// `close_tab_now` precisely so the in-memory edits survive the
    /// reopen. Re-parsing from disk would discard those edits.
    ///
    /// Pre-condition: `document_state.engines.contains_key(&path)`.
    /// Post-condition: a new tab exists pointing at `path` with
    /// `dirty: true`, the active engine is the parked entry, and the
    /// canvas reflects the parked sheet's current state.
    pub(crate) fn attach_parked_schematic_tab(&mut self, path: PathBuf, title: String) {
        self.park_active_schematic_session();
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        // The parked engine is, by definition, dirty — `close_tab_now`
        // only keeps engines for paths in `dirty_paths`. Mirror that
        // into the new tab so the chrome (red dot, etc.) stays
        // consistent the moment the tab opens.
        let dirty = self.document_state.dirty_paths.contains(&path);
        self.document_state.tabs.push(TabInfo {
            title,
            path: path.clone(),
            cached_document: None,
            dirty,
            project_id,
            kind: super::TabKind::Schematic,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        // Point active_path at the parked entry. `apply_loaded_schematic`
        // with `schematic = None` skips the `engines.insert` overwrite
        // path and just refreshes the canvas / panel against the
        // existing engine.
        self.document_state.active_path = Some(path);
        self.apply_loaded_schematic(None, true, true, true, true);
    }

    pub(crate) fn open_pcb_tab(&mut self, path: PathBuf, title: String, board: PcbBoard) {
        self.park_active_schematic_session();
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        self.document_state.tabs.push(TabInfo {
            title,
            path,
            cached_document: Some(TabDocument::Pcb(board)),
            dirty: false,
            project_id,
            kind: super::TabKind::Pcb,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.apply_loaded_pcb_document(true, true);
    }

    pub(crate) fn load_schematic_into_active_tab(&mut self, sheet: SchematicSheet) {
        self.apply_loaded_schematic(Some(sheet), false, false, true, false);
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
            if self.activate_active_schematic_session() {
                self.apply_loaded_schematic(None, true, false, false, false);
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

    fn apply_loaded_empty_document(&mut self, refresh_panel_ctx: bool) {
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

    pub(crate) fn apply_loaded_schematic(
        &mut self,
        schematic: Option<SchematicSheet>,
        clear_bg_cache: bool,
        fit_to_paper: bool,
        commit_to_active_tab: bool,
        refresh_panel_ctx: bool,
    ) {
        if let Some(schematic) = schematic {
            self.sync_engine_from_schematic(Some(schematic));
        }
        if !self.document_state.has_active_engine() {
            return;
        }
        self.sync_canvas_from_visible_schematic(crate::schematic_runtime::RenderInvalidation::FULL);

        if fit_to_paper {
            self.interaction_state.active_canvas_mut().fit_to_paper();
        }
        if clear_bg_cache {
            self.interaction_state.active_canvas_mut().clear_bg_cache();
        }
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();

        let _ = commit_to_active_tab;

        if refresh_panel_ctx {
            self.refresh_panel_ctx();
        } else {
            self.sync_panel_ctx_from_visible_document();
        }
    }

    fn sync_panel_ctx_from_visible_document(&mut self) {
        self.document_state.panel_ctx.has_schematic = self.has_active_schematic();
        self.document_state.panel_ctx.has_pcb = self.has_active_pcb();

        let document_summary = if let Some(snapshot) = self.active_render_snapshot() {
            (
                snapshot.symbols.len(),
                snapshot.wires.len(),
                snapshot.labels.len(),
                snapshot.junctions.len(),
                snapshot.lib_symbols.len(),
                snapshot.lib_symbols.keys().cloned().collect(),
                snapshot
                    .symbols
                    .iter()
                    .map(|symbol| {
                        (
                            symbol.reference.clone(),
                            symbol.value.clone(),
                            symbol.footprint.clone(),
                            symbol.lib_id.clone(),
                        )
                    })
                    .collect(),
                // Older documents may carry an empty paper_size (serde
                // default); normalize so the Page Options selector and the
                // drawn sheet agree on the effective A4 fallback.
                if snapshot.paper_size.is_empty() {
                    "A4".to_string()
                } else {
                    snapshot.paper_size.clone()
                },
            )
        } else if let Some(snapshot) = self.active_pcb_snapshot() {
            (
                snapshot.footprints.len(),
                snapshot.segments.len(),
                snapshot.texts.len(),
                snapshot.vias.len(),
                0,
                Vec::new(),
                snapshot
                    .footprints
                    .iter()
                    .map(|footprint| {
                        (
                            footprint.reference.clone(),
                            footprint.value.clone(),
                            footprint.footprint_id.clone(),
                            footprint.layer.clone(),
                        )
                    })
                    .collect(),
                format!("PCB • {} layers", snapshot.layers.len()),
            )
        } else {
            (0, 0, 0, 0, 0, Vec::new(), Vec::new(), "A4".to_string())
        };

        self.document_state.panel_ctx.sym_count = document_summary.0;
        self.document_state.panel_ctx.wire_count = document_summary.1;
        self.document_state.panel_ctx.label_count = document_summary.2;
        self.document_state.panel_ctx.junction_count = document_summary.3;
        self.document_state.panel_ctx.lib_symbol_count = document_summary.4;
        self.document_state.panel_ctx.lib_symbol_names = document_summary.5;
        self.document_state.panel_ctx.placed_symbols = document_summary.6;
        self.document_state.panel_ctx.paper_size = document_summary.7;

        // Drive the drawn sheet from the (re)loaded document's paper size —
        // without this the canvas keeps the previous tab's dimensions and the
        // sheet rectangle stops matching the stored A-series format.
        if self.has_active_schematic() {
            self.apply_page_dimensions_to_canvas();
        }
    }
}
