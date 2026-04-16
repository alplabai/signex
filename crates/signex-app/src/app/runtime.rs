use super::*;

impl Signex {
    pub(crate) fn finish_update(&mut self) -> Task<Message> {
        if self.interaction_state.current_tool != Tool::Measure
            && (self.interaction_state.canvas.measure_start.is_some()
                || self.interaction_state.canvas.measure_end.is_some())
        {
            self.clear_measurement();
        }

        self.document_state.panel_ctx.unit = self.ui_state.unit;
        self.document_state.panel_ctx.grid_visible = self.ui_state.grid_visible;
        self.document_state.panel_ctx.snap_enabled = self.ui_state.snap_enabled;
        self.document_state.panel_ctx.grid_size_mm = self.ui_state.grid_size_mm;
        self.document_state.panel_ctx.visible_grid_mm = self.ui_state.visible_grid_mm;
        self.document_state.panel_ctx.snap_hotspots = self.ui_state.snap_hotspots;

        Task::none()
    }

    pub(crate) fn refresh_panel_ctx(&mut self) {
        let sheets: Vec<crate::panels::SheetInfo> = self
            .document_state
            .project_data
            .as_ref()
            .map(|proj| {
                proj.sheets
                    .iter()
                    .map(|sheet| crate::panels::SheetInfo {
                        name: sheet.name.clone(),
                        filename: sheet.filename.clone(),
                        sym_count: sheet.symbols_count,
                        wire_count: sheet.wires_count,
                        label_count: sheet.labels_count,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let project_name = self
            .document_state
            .project_data
            .as_ref()
            .map(|project| project.name.clone())
            .or_else(|| {
                self.document_state
                    .project_path
                    .as_ref()
                    .and_then(|path| path.file_stem().map(|stem| stem.to_string_lossy().to_string()))
            });

        let active_schematic_snapshot = self.active_render_snapshot();
        let active_pcb_snapshot = self.active_pcb_snapshot();

        let canvas_font_popup_open = self.document_state.panel_ctx.canvas_font_popup_open;
        let properties_tab = self.document_state.panel_ctx.properties_tab;
        let kicad_libraries = self.document_state.panel_ctx.kicad_libraries.clone();
        let active_library = self.document_state.panel_ctx.active_library.clone();
        let library_symbols = self.document_state.panel_ctx.library_symbols.clone();
        let selected_component = self.document_state.panel_ctx.selected_component.clone();
        let selected_pins = self.document_state.panel_ctx.selected_pins.clone();
        let selected_lib_symbol = self.document_state.panel_ctx.selected_lib_symbol.clone();
        let components_split = self.document_state.panel_ctx.components_split;
        let selection_count = self.document_state.panel_ctx.selection_count;
        let selected_uuid = self.document_state.panel_ctx.selected_uuid;
        let selected_kind = self.document_state.panel_ctx.selected_kind;
        let selection_info = self.document_state.panel_ctx.selection_info.clone();
        let component_filter = self.document_state.panel_ctx.component_filter.clone();
        let collapsed_sections = self.document_state.panel_ctx.collapsed_sections.clone();
        let pre_placement = self.document_state.panel_ctx.pre_placement.clone();

        self.document_state.panel_ctx = crate::panels::PanelContext {
            project_name,
            project_file: self
                .document_state
                .project_data
                .as_ref()
                .and_then(|project| project.schematic_root.clone())
                .or_else(|| {
                    self.document_state
                        .project_path
                        .as_ref()
                        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().to_string()))
                }),
            pcb_file: self
                .document_state
                .project_data
                .as_ref()
                .and_then(|project| project.pcb_file.clone()),
            sheets,
            sym_count: active_schematic_snapshot
                .map(|snapshot| snapshot.symbols.len())
                .or_else(|| active_pcb_snapshot.map(|snapshot| snapshot.footprints.len()))
                .unwrap_or(0),
            wire_count: active_schematic_snapshot
                .map(|snapshot| snapshot.wires.len())
                .or_else(|| active_pcb_snapshot.map(|snapshot| snapshot.segments.len()))
                .unwrap_or(0),
            label_count: active_schematic_snapshot
                .map(|snapshot| snapshot.labels.len())
                .or_else(|| active_pcb_snapshot.map(|snapshot| snapshot.texts.len()))
                .unwrap_or(0),
            junction_count: active_schematic_snapshot
                .map(|snapshot| snapshot.junctions.len())
                .or_else(|| active_pcb_snapshot.map(|snapshot| snapshot.vias.len()))
                .unwrap_or(0),
            child_sheets: active_schematic_snapshot
                .map(|snapshot| snapshot.child_sheets.iter().map(|child| child.name.clone()).collect())
                .unwrap_or_default(),
            has_schematic: self.has_active_schematic(),
            has_pcb: self.has_active_pcb(),
            paper_size: active_schematic_snapshot
                .map(|snapshot| snapshot.paper_size.clone())
                .or_else(|| active_pcb_snapshot.map(|snapshot| format!("PCB • {} layers", snapshot.layers.len())))
                .unwrap_or_else(|| "A4".to_string()),
            lib_symbol_count: active_schematic_snapshot
                .map(|snapshot| snapshot.lib_symbols.len())
                .unwrap_or(0),
            lib_symbol_names: active_schematic_snapshot
                .map(|snapshot| snapshot.lib_symbols.keys().cloned().collect())
                .unwrap_or_default(),
            placed_symbols: if let Some(snapshot) = active_schematic_snapshot {
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
                    .collect()
            } else {
                active_pcb_snapshot
                    .map(|snapshot| {
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
                            .collect()
                    })
                    .unwrap_or_default()
            },
            tokens: signex_types::theme::theme_tokens(self.ui_state.theme_id),
            unit: self.ui_state.unit,
            grid_visible: self.ui_state.grid_visible,
            snap_enabled: self.ui_state.snap_enabled,
            grid_size_mm: self.ui_state.grid_size_mm,
            visible_grid_mm: self.ui_state.visible_grid_mm,
            snap_hotspots: self.ui_state.snap_hotspots,
            ui_font_name: self.ui_state.ui_font_name.clone(),
            canvas_font_name: self.ui_state.canvas_font_name.clone(),
            canvas_font_size: self.ui_state.canvas_font_size,
            canvas_font_bold: self.ui_state.canvas_font_bold,
            canvas_font_italic: self.ui_state.canvas_font_italic,
            canvas_font_popup_open,
            properties_tab,
            kicad_libraries,
            active_library,
            library_symbols,
            selected_component,
            selected_pins,
            selected_lib_symbol,
            components_split,
            project_tree: vec![],
            selection_count,
            selected_uuid,
            selected_kind,
            selection_info,
            component_filter,
            collapsed_sections,
            pre_placement,
        };
        self.document_state.panel_ctx.project_tree =
            crate::panels::build_project_tree(&self.document_state.panel_ctx);
    }

    pub(crate) fn sync_active_tab(&mut self) {
        self.sync_visible_document_from_active_tab();
    }

    pub(crate) fn update_selection_info(&mut self) {
        let selected = &self.interaction_state.canvas.selected;
        self.document_state.panel_ctx.selection_count = selected.len();
        self.document_state.panel_ctx.selection_info.clear();
        self.document_state.panel_ctx.selected_uuid = None;
        self.document_state.panel_ctx.selected_kind = None;

        if selected.len() != 1 {
            if !selected.is_empty() {
                self.document_state
                    .panel_ctx
                    .selection_info
                    .push(("Selected".into(), format!("{} items", selected.len())));
            }
            return;
        }

        if let Some(engine) = self.document_state.engine.as_ref()
            && let Some(details) = engine.describe_single_selection(selected)
        {
            self.document_state.panel_ctx.selected_uuid = Some(details.selected_uuid);
            self.document_state.panel_ctx.selected_kind = Some(details.selected_kind);
            self.document_state.panel_ctx.selection_info = details.info;
        }
    }

    pub(crate) fn update_canvas_theme(&mut self) {
        let colors = if self.ui_state.theme_id == ThemeId::Custom {
            self.ui_state
                .custom_theme
                .as_ref()
                .map(|custom_theme| custom_theme.canvas)
                .unwrap_or_else(|| signex_types::theme::canvas_colors(ThemeId::Signex))
        } else {
            signex_types::theme::canvas_colors(self.ui_state.theme_id)
        };
        self.interaction_state.canvas.set_theme_colors(
            signex_render::colors::to_iced(&colors.background),
            signex_render::colors::to_iced(&colors.grid),
            signex_render::colors::to_iced(&colors.paper),
        );
        self.interaction_state.pcb_canvas.set_theme_colors(
            signex_render::colors::to_iced(&colors.background),
            signex_render::colors::to_iced(&colors.grid),
        );
        self.interaction_state.canvas.canvas_colors = colors;
        self.interaction_state.pcb_canvas.canvas_colors = colors;
        self.interaction_state.canvas.clear_content_cache();
        self.interaction_state.pcb_canvas.clear_content_cache();
    }
}