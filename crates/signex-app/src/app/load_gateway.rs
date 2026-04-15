use super::*;

impl Signex {
    fn active_tab_cached_document(&self) -> Option<&TabDocument> {
        self.tabs
            .get(self.active_tab)
            .and_then(|tab| tab.cached_document.as_ref())
    }

    pub(crate) fn active_schematic(&self) -> Option<&SchematicSheet> {
        self.engine
            .as_ref()
            .map(|engine| engine.document())
            .or_else(|| self.active_tab_cached_schematic())
    }

    pub(crate) fn has_active_schematic(&self) -> bool {
        self.active_schematic().is_some()
    }

    pub(crate) fn active_render_snapshot(
        &self,
    ) -> Option<&signex_render::schematic::SchematicRenderSnapshot> {
        self.canvas.active_snapshot()
    }

    fn active_tab_cached_schematic(&self) -> Option<&SchematicSheet> {
        self.active_tab_cached_document().and_then(TabDocument::as_schematic)
    }

    fn active_tab_path(&self) -> Option<PathBuf> {
        self.tabs.get(self.active_tab).map(|tab| tab.path.clone())
    }

    fn sync_engine_from_schematic(&mut self, schematic: Option<SchematicSheet>) {
        self.engine = schematic
            .and_then(|sheet| signex_engine::Engine::new_with_path(sheet, self.active_tab_path()).ok());
    }

    pub(crate) fn sync_canvas_from_visible_schematic(
        &mut self,
        invalidation: signex_render::schematic::RenderInvalidation,
    ) {
        if let Some(engine) = self.engine.as_ref() {
            if let Some(cache) = self.canvas.render_cache.as_mut() {
                cache.update_from_sheet(engine.document(), invalidation);
            } else {
                self.canvas.set_render_cache(Some(
                    signex_render::schematic::SchematicRenderCache::from_sheet(engine.document()),
                ));
            }
            return;
        }

        let rebuilt_cache = self
            .active_tab_cached_schematic()
            .map(signex_render::schematic::SchematicRenderCache::from_sheet);

        if let Some(cache) = rebuilt_cache {
            self.canvas.set_render_cache(Some(cache));
        } else {
            self.canvas.set_render_cache(None);
        }
    }

    pub(crate) fn open_schematic_tab(
        &mut self,
        path: PathBuf,
        title: String,
        sheet: SchematicSheet,
    ) {
        self.tabs.push(TabInfo {
            title,
            path,
            cached_document: Some(TabDocument::Schematic(sheet.clone())),
            dirty: false,
        });
        self.active_tab = self.tabs.len() - 1;

        self.apply_loaded_schematic(Some(sheet), true, true, true, true);
    }

    pub(crate) fn open_pcb_tab(&mut self, path: PathBuf, title: String, board: PcbBoard) {
        self.tabs.push(TabInfo {
            title,
            path,
            cached_document: Some(TabDocument::Pcb(board)),
            dirty: false,
        });
        self.active_tab = self.tabs.len() - 1;
        self.apply_loaded_non_schematic_document(true);
    }

    pub(crate) fn load_schematic_into_active_tab(&mut self, sheet: SchematicSheet) {
        self.apply_loaded_schematic(Some(sheet), false, false, true, false);
    }

    pub(crate) fn sync_visible_document_from_active_tab(&mut self) {
        self.editing_text = None;

        match self.active_tab_cached_document() {
            Some(TabDocument::Schematic(sheet)) => {
                self.apply_loaded_schematic(Some(sheet.clone()), true, false, false, false);
            }
            Some(TabDocument::Pcb(_)) | None => {
                self.apply_loaded_non_schematic_document(false);
            }
        }
    }

    fn apply_loaded_non_schematic_document(&mut self, refresh_panel_ctx: bool) {
        self.engine = None;
        self.canvas.set_render_cache(None);
        self.canvas.selected.clear();
        self.canvas.wire_preview.clear();
        self.canvas.drawing_mode = false;
        self.canvas.clear_content_cache();
        self.canvas.clear_overlay_cache();
        self.current_tool = Tool::Select;

        if refresh_panel_ctx {
            self.refresh_panel_ctx();
        } else {
            self.sync_panel_ctx_from_visible_schematic();
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
        self.sync_engine_from_schematic(schematic);
        self.sync_canvas_from_visible_schematic(signex_render::schematic::RenderInvalidation::FULL);

        if fit_to_paper {
            self.canvas.fit_to_paper();
        }
        if clear_bg_cache {
            self.canvas.clear_bg_cache();
        }
        self.canvas.clear_content_cache();

        if commit_to_active_tab {
            self.commit_schematic();
        }

        if refresh_panel_ctx {
            self.refresh_panel_ctx();
        } else {
            self.sync_panel_ctx_from_visible_schematic();
        }
    }

    fn sync_panel_ctx_from_visible_schematic(&mut self) {
        self.panel_ctx.has_schematic = self.has_active_schematic();
        self.panel_ctx.sym_count = self
            .active_schematic()
            .map(|sheet| sheet.symbols.len())
            .unwrap_or(0);
        self.panel_ctx.wire_count = self
            .active_schematic()
            .map(|sheet| sheet.wires.len())
            .unwrap_or(0);
        self.panel_ctx.label_count = self
            .active_schematic()
            .map(|sheet| sheet.labels.len())
            .unwrap_or(0);
        self.panel_ctx.junction_count = self
            .active_schematic()
            .map(|sheet| sheet.junctions.len())
            .unwrap_or(0);
        self.panel_ctx.lib_symbol_count = self
            .active_schematic()
            .map(|sheet| sheet.lib_symbols.len())
            .unwrap_or(0);
        self.panel_ctx.lib_symbol_names = self
            .active_schematic()
            .map(|sheet| sheet.lib_symbols.keys().cloned().collect())
            .unwrap_or_default();
        self.panel_ctx.placed_symbols = self
            .active_schematic()
            .map(|sheet| {
                sheet
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
                    .collect()
            })
            .unwrap_or_default();
        self.panel_ctx.paper_size = self
            .active_schematic()
            .map(|sheet| sheet.paper_size.clone())
            .unwrap_or_else(|| "A4".to_string());
    }

    pub(crate) fn update_active_engine_path(&mut self) {
        let path = self.active_tab_path();
        if let Some(engine) = self.engine.as_mut() {
            engine.set_path(path);
        }
    }
}