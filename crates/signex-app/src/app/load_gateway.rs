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
        self.document_state.engine
            .as_ref()
            .map(|engine| engine.document())
            .or_else(|| self.active_tab_cached_schematic())
    }

    pub(crate) fn active_pcb(&self) -> Option<&PcbBoard> {
        self.active_tab_cached_document().and_then(TabDocument::as_pcb)
    }

    pub(crate) fn has_active_schematic(&self) -> bool {
        self.active_schematic().is_some()
    }

    pub(crate) fn has_active_pcb(&self) -> bool {
        self.active_pcb().is_some()
    }

    pub(crate) fn active_render_snapshot(
        &self,
    ) -> Option<&signex_render::schematic::SchematicRenderSnapshot> {
        self.interaction_state.canvas.active_snapshot()
    }

    pub(crate) fn active_pcb_snapshot(&self) -> Option<&signex_render::pcb::PcbRenderSnapshot> {
        self.interaction_state.pcb_canvas.active_snapshot()
    }

    fn active_tab_cached_schematic(&self) -> Option<&SchematicSheet> {
        self.active_tab_cached_document().and_then(TabDocument::as_schematic)
    }

    pub(crate) fn with_active_schematic_session_mut<R>(
        &mut self,
        update: impl FnOnce(&mut SchematicTabSession) -> R,
    ) -> Option<R> {
        let engine = self.document_state.engine.take()?;
        let Some((title, path, dirty)) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| (tab.title.clone(), tab.path.clone(), tab.dirty))
        else {
            self.document_state.engine = Some(engine);
            return None;
        };

        let mut session = SchematicTabSession::new(engine, title, path, dirty);
        let result = update(&mut session);
        let (engine, title, path, dirty) = session.into_parts();

        if let Some(tab) = self
            .document_state
            .tabs
            .get_mut(self.document_state.active_tab)
        {
            tab.title = title;
            tab.path = path;
            tab.dirty = dirty;
        }

        self.document_state.engine = Some(engine);
        Some(result)
    }

    pub(crate) fn park_active_schematic_session(&mut self) {
        let Some(engine) = self.document_state.engine.take() else {
            return;
        };

        if let Some(tab) = self
            .document_state
            .tabs
            .get_mut(self.document_state.active_tab)
        {
            tab.cached_document = Some(TabDocument::Schematic(SchematicTabSession::new(
                engine,
                tab.title.clone(),
                tab.path.clone(),
                tab.dirty,
            )));
        } else {
            self.document_state.engine = Some(engine);
        }
    }

    fn activate_active_schematic_session(&mut self) -> bool {
        let cached_document = self
            .document_state
            .tabs
            .get_mut(self.document_state.active_tab)
            .and_then(|tab| tab.cached_document.take());

        match cached_document {
            Some(TabDocument::Schematic(session)) => {
                let (engine, title, path, dirty) = session.into_parts();
                if let Some(tab) = self
                    .document_state
                    .tabs
                    .get_mut(self.document_state.active_tab)
                {
                    tab.title = title;
                    tab.path = path;
                    tab.dirty = dirty;
                }
                self.document_state.engine = Some(engine);
                true
            }
            Some(other_document) => {
                if let Some(tab) = self
                    .document_state
                    .tabs
                    .get_mut(self.document_state.active_tab)
                {
                    tab.cached_document = Some(other_document);
                }
                false
            }
            None => self.document_state.engine.is_some(),
        }
    }

    pub(crate) fn active_tab_path(&self) -> Option<PathBuf> {
        self.document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.clone())
    }

    fn sync_engine_from_schematic(&mut self, schematic: Option<SchematicSheet>) {
        self.document_state.engine = schematic
            .and_then(|sheet| signex_engine::Engine::new_with_path(sheet, self.active_tab_path()).ok());
    }

    pub(crate) fn sync_canvas_from_visible_schematic(
        &mut self,
        invalidation: signex_render::schematic::RenderInvalidation,
    ) {
        if let Some(engine) = self.document_state.engine.as_ref() {
            if let Some(cache) = self.interaction_state.canvas.render_cache.as_mut() {
                cache.update_from_sheet(engine.document(), invalidation);
            } else {
                self.interaction_state.canvas.set_render_cache(Some(
                    signex_render::schematic::SchematicRenderCache::from_sheet(engine.document()),
                ));
            }
            return;
        }

        let rebuilt_cache = self
            .active_tab_cached_schematic()
            .map(signex_render::schematic::SchematicRenderCache::from_sheet);

        if let Some(cache) = rebuilt_cache {
            self.interaction_state.canvas.set_render_cache(Some(cache));
        } else {
            self.interaction_state.canvas.set_render_cache(None);
        }
    }

    pub(crate) fn sync_pcb_canvas_from_visible_board(&mut self) {
        self.interaction_state.pcb_canvas.set_render_snapshot(
            self.active_pcb()
                .map(signex_render::pcb::PcbRenderSnapshot::from_board),
        );
    }

    pub(crate) fn open_schematic_tab(
        &mut self,
        path: PathBuf,
        title: String,
        sheet: SchematicSheet,
    ) {
        self.park_active_schematic_session();
        self.document_state.tabs.push(TabInfo {
            title,
            path,
            cached_document: None,
            dirty: false,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;

        self.apply_loaded_schematic(Some(sheet), true, true, true, true);
    }

    pub(crate) fn open_pcb_tab(&mut self, path: PathBuf, title: String, board: PcbBoard) {
        self.park_active_schematic_session();
        self.document_state.tabs.push(TabInfo {
            title,
            path,
            cached_document: Some(TabDocument::Pcb(board)),
            dirty: false,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.apply_loaded_pcb_document(true, true);
    }

    pub(crate) fn load_schematic_into_active_tab(&mut self, sheet: SchematicSheet) {
        self.apply_loaded_schematic(Some(sheet), false, false, true, false);
    }

    pub(crate) fn sync_visible_document_from_active_tab(&mut self) {
        self.interaction_state.editing_text = None;

        match self.active_tab_cached_document() {
            Some(TabDocument::Schematic(_)) => {
                if self.activate_active_schematic_session() {
                    self.apply_loaded_schematic(None, true, false, false, false);
                }
            }
            Some(TabDocument::Pcb(_)) => {
                self.apply_loaded_pcb_document(false, false);
            }
            None => {
                self.apply_loaded_empty_document(false);
            }
        }
    }

    fn clear_schematic_ui_state(&mut self) {
        self.document_state.engine = None;
        self.interaction_state.canvas.set_render_cache(None);
        self.interaction_state.canvas.selected.clear();
        self.interaction_state.canvas.wire_preview.clear();
        self.interaction_state.canvas.reset_measurement();
        self.interaction_state.canvas.drawing_mode = false;
        self.interaction_state.canvas.clear_content_cache();
        self.interaction_state.canvas.clear_overlay_cache();
        self.interaction_state.current_tool = Tool::Select;
    }

    fn apply_loaded_pcb_document(&mut self, fit_to_board: bool, refresh_panel_ctx: bool) {
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
        self.interaction_state.pcb_canvas.set_render_snapshot(None);
        self.interaction_state.pcb_canvas.clear_bg_cache();
        self.interaction_state.pcb_canvas.clear_content_cache();

        if refresh_panel_ctx {
            self.refresh_panel_ctx();
        } else {
            self.sync_panel_ctx_from_visible_document();
        }
    }

    fn apply_loaded_schematic(
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
        if self.document_state.engine.is_none() {
            return;
        }
        self.sync_canvas_from_visible_schematic(signex_render::schematic::RenderInvalidation::FULL);

        if fit_to_paper {
            self.interaction_state.canvas.fit_to_paper();
        }
        if clear_bg_cache {
            self.interaction_state.canvas.clear_bg_cache();
        }
        self.interaction_state.canvas.clear_content_cache();

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
                snapshot.paper_size.clone(),
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
    }
}