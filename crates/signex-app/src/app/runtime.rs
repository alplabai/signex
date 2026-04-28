use super::*;

impl Signex {
    pub(crate) fn finish_update(&mut self) -> Task<Message> {
        self.document_state.panel_ctx.unit = self.ui_state.unit;
        self.document_state.panel_ctx.grid_visible = self.ui_state.grid_visible;
        self.document_state.panel_ctx.snap_enabled = self.ui_state.snap_enabled;
        self.document_state.panel_ctx.grid_size_mm = self.ui_state.grid_size_mm;
        self.document_state.panel_ctx.visible_grid_mm = self.ui_state.visible_grid_mm;
        self.document_state.panel_ctx.snap_hotspots = self.ui_state.snap_hotspots;
        self.sync_diagnostics_panel_ctx();

        Task::none()
    }

    pub(crate) fn refresh_panel_ctx(&mut self) {
        // Build per-project panel info from every loaded project in the
        // workspace. The `project_name` / `project_file` / `pcb_file` /
        // `sheets` legacy singletons mirror whichever entry is `is_active`
        // — kept around for the few panels that haven't migrated to read
        // `panel_ctx.projects` directly yet.
        let active_id = self.document_state.active_project;
        // Open-state lookup is "is there a tab pointing at this path?"
        // Dirty-state lookup is the project-scoped `dirty_paths` set,
        // which persists across tab close (Altium parity: closing a
        // tab keeps the file dirty until the project is saved or the
        // edits are explicitly discarded).
        let open_paths: std::collections::HashSet<std::path::PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|tab| tab.path.clone())
            .collect();
        let dirty_paths = self.document_state.dirty_paths.clone();
        // Active-tab path drives the per-row highlight in the tree —
        // matches Altium's "you are here" cue. None when no tabs open.
        let active_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.clone());
        let projects_panel: Vec<crate::panels::ProjectPanelInfo> = self
            .document_state
            .projects
            .iter()
            .map(|p| {
                let project_dir = std::path::Path::new(&p.data.dir);
                let active_path_ref = active_path.as_ref();
                let lookup = |filename: &str| -> (bool, bool, bool) {
                    let abs = project_dir.join(filename);
                    let is_open = open_paths.contains(&abs);
                    let is_dirty = dirty_paths.contains(&abs);
                    let is_active = active_path_ref == Some(&abs);
                    (is_open, is_dirty, is_active)
                };
                let (project_file_open, project_file_dirty, project_file_active) = p
                    .data
                    .schematic_root
                    .as_deref()
                    .map(lookup)
                    .unwrap_or((false, false, false));
                let (pcb_file_open, pcb_file_dirty, pcb_file_active) = p
                    .data
                    .pcb_file
                    .as_deref()
                    .map(lookup)
                    .unwrap_or((false, false, false));
                // Flatten `Project::libraries` into the panel struct
                // alongside the sheet list. Each entry resolves to an
                // absolute path so the right-click menu can dispatch
                // back to the correct library; cached components
                // come from `LibraryState::library_at` when the
                // library is currently mounted.
                let libraries: Vec<crate::panels::LibraryNodeInfo> = p
                    .data
                    .libraries
                    .iter()
                    .map(|entry| {
                        let resolved = p.data.resolve_library_path(entry);
                        let mounted_lib = self.library.library_at(&resolved);
                        let display_name = match mounted_lib {
                            Some(lib) => lib.display_name.clone(),
                            None => entry
                                .path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .map(str::to_string)
                                .unwrap_or_else(|| entry.path.display().to_string()),
                        };
                        let (symbols, footprints, sims) = scan_library_primitives(&resolved);
                        crate::panels::LibraryNodeInfo {
                            display_name,
                            root: resolved,
                            symbols,
                            footprints,
                            sims,
                            mounted: mounted_lib.is_some(),
                        }
                    })
                    .collect();
                crate::panels::ProjectPanelInfo {
                    id: p.id,
                    name: p.data.name.clone(),
                    project_file: p.data.schematic_root.clone(),
                    project_file_open,
                    project_file_dirty,
                    project_file_active,
                    pcb_file: p.data.pcb_file.clone(),
                    pcb_file_open,
                    pcb_file_dirty,
                    pcb_file_active,
                    sheets: p
                        .data
                        .sheets
                        .iter()
                        .map(|sheet| {
                            let (is_open, is_dirty, is_active) = lookup(&sheet.filename);
                            crate::panels::SheetInfo {
                                name: sheet.name.clone(),
                                filename: sheet.filename.clone(),
                                sym_count: sheet.symbols_count,
                                wire_count: sheet.wires_count,
                                label_count: sheet.labels_count,
                                is_open,
                                is_dirty,
                                is_active,
                            }
                        })
                        .collect(),
                    libraries,
                    is_active: Some(p.id) == active_id,
                }
            })
            .collect();

        let active_schematic_snapshot = self.active_render_snapshot();
        let active_pcb_snapshot = self.active_pcb_snapshot();

        let canvas_font_popup_open = self.document_state.panel_ctx.canvas_font_popup_open;
        let properties_tab = self.document_state.panel_ctx.properties_tab;
        let standard_libraries = self.document_state.panel_ctx.standard_libraries.clone();
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
        let drawing_edit_buf = self.document_state.panel_ctx.drawing_edit_buf.clone();
        let drawing_edit_buf_for = self.document_state.panel_ctx.drawing_edit_buf_for;
        let selected_drawing = self.document_state.panel_ctx.selected_drawing.clone();
        let selected_child_sheet = self.document_state.panel_ctx.selected_child_sheet.clone();
        let child_sheet_border_picker_open =
            self.document_state.panel_ctx.child_sheet_border_picker_open;
        let child_sheet_fill_picker_open =
            self.document_state.panel_ctx.child_sheet_fill_picker_open;
        let child_sheet_border_advanced_open =
            self.document_state.panel_ctx.child_sheet_border_advanced_open;
        let child_sheet_fill_advanced_open =
            self.document_state.panel_ctx.child_sheet_fill_advanced_open;
        let child_sheet_stroke_width_buf = self
            .document_state
            .panel_ctx
            .child_sheet_stroke_width_buf
            .clone();
        let component_filter = self.document_state.panel_ctx.component_filter.clone();
        let collapsed_sections = self.document_state.panel_ctx.collapsed_sections.clone();
        let pre_placement = self.document_state.panel_ctx.pre_placement.clone();
        let page_format_mode = self.document_state.panel_ctx.page_format_mode;
        let margin_vertical = self.document_state.panel_ctx.margin_vertical;
        let margin_horizontal = self.document_state.panel_ctx.margin_horizontal;
        let page_origin = self.document_state.panel_ctx.page_origin;
        let custom_paper_w_mm = self.document_state.panel_ctx.custom_paper_w_mm;
        let custom_paper_h_mm = self.document_state.panel_ctx.custom_paper_h_mm;
        let sheet_color = self.document_state.panel_ctx.sheet_color;
        let erc_diagnostics = self.build_erc_diagnostic_entries();

        self.document_state.panel_ctx = crate::panels::PanelContext {
            projects: projects_panel,
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
                .map(|snapshot| {
                    snapshot
                        .child_sheets
                        .iter()
                        .map(|child| child.name.clone())
                        .collect()
                })
                .unwrap_or_default(),
            has_schematic: self.has_active_schematic(),
            has_pcb: self.has_active_pcb(),
            paper_size: active_schematic_snapshot
                .map(|snapshot| snapshot.paper_size.clone())
                .or_else(|| {
                    active_pcb_snapshot
                        .map(|snapshot| format!("PCB • {} layers", snapshot.layers.len()))
                })
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
            theme_id: self.ui_state.theme_id,
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
            standard_libraries,
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
            drawing_edit_buf,
            drawing_edit_buf_for,
            selected_drawing,
            selected_child_sheet,
            child_sheet_border_picker_open,
            child_sheet_fill_picker_open,
            child_sheet_border_advanced_open,
            child_sheet_fill_advanced_open,
            child_sheet_stroke_width_buf,
            component_filter,
            collapsed_sections,
            pre_placement,
            erc_diagnostics,
            erc_focus_index: self.ui_state.erc_focus_global_index,
            diagnostics_level: crate::diagnostics::configured_level_label().to_string(),
            diagnostics: crate::diagnostics::recent_entries(),
            selection_filters: self.interaction_state.selection_filters.clone(),
            custom_filter_presets: self.interaction_state.custom_filter_presets.clone(),
            active_custom_filter_tab: self.interaction_state.active_custom_filter_tab,
            page_format_mode,
            margin_vertical,
            margin_horizontal,
            page_origin,
            custom_paper_w_mm,
            custom_paper_h_mm,
            sheet_color,
            symbol_editor: build_symbol_editor_panel_ctx(self),
        };
        self.document_state.panel_ctx.project_tree =
            crate::panels::build_project_tree(&self.document_state.panel_ctx);
    }

    fn sync_diagnostics_panel_ctx(&mut self) {
        self.document_state.panel_ctx.diagnostics_level =
            crate::diagnostics::configured_level_label().to_string();
        self.document_state.panel_ctx.diagnostics = crate::diagnostics::recent_entries();
    }

    pub(crate) fn sync_active_tab(&mut self) {
        // Follow the focused tab into its project: the Projects-panel
        // accent and active_project-scoped handlers (ERC / annotate /
        // save-all) should track the user's tab focus, not the most
        // recently opened project. Tabs with no `project_id` (loose
        // schematics opened without a `.standard_pro`) leave the pointer
        // alone so the panel keeps showing whichever project was last
        // active. (#54 phase 2.4)
        if let Some(pid) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| t.project_id)
        {
            self.document_state.active_project = Some(pid);
        }

        self.sync_visible_document_from_active_tab();
        // ERC results are cached per-sheet. On tab switch, repoint the visible
        // list/markers at the newly active sheet instead of dropping results.
        let active_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.path.clone());
        self.refresh_active_erc_from_cache(active_path.as_ref());
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
        // Always rebuild the panel context so the active-row highlight
        // and active-project accent track the focused tab even when
        // sync_visible_document_from_active_tab took the empty-doc
        // branch (which suppresses the implicit refresh).
        self.refresh_panel_ctx();
    }

    /// Refresh `panel_ctx` selection fields from the active canvas.
    ///
    /// NOTE: `panel_ctx` is shared across every window — the dock
    /// panels, Properties panel, and status bar all read these
    /// fields. When an undocked window handles a canvas event via
    /// the swap trick, "active canvas" refers to the undocked
    /// window's canvas for the duration of the event, so this
    /// function writes THAT window's selection into the shared
    /// panel_ctx. End result: main-window panels reflect the
    /// most-recently-interacted-with window's selection. This is
    /// intentional "last-touched wins" behaviour.
    pub(crate) fn update_selection_info(&mut self) {
        // AutoFocus dims every item not in the current selection, so any
        // selection change must invalidate the cached content layer to
        // reflect the new focus set.
        if self.ui_state.auto_focus {
            self.interaction_state
                .active_canvas_mut()
                .clear_content_cache();
        }
        let selected = &self.interaction_state.active_canvas_mut().selected;
        self.document_state.panel_ctx.selection_count = selected.len();
        self.document_state.panel_ctx.selection_info.clear();
        self.document_state.panel_ctx.selected_uuid = None;
        self.document_state.panel_ctx.selected_kind = None;
        self.document_state.panel_ctx.selected_drawing = None;
        self.document_state.panel_ctx.selected_child_sheet = None;

        if selected.len() != 1 {
            if !selected.is_empty() {
                self.document_state
                    .panel_ctx
                    .selection_info
                    .push(("Selected".into(), format!("{} items", selected.len())));
            }
            return;
        }

        // Borrow `engines` + `panel_ctx` as disjoint fields so the
        // compiler can split the mutation below. Going through
        // `active_engine()` would keep the whole `DocumentState`
        // borrowed for the duration of the block.
        let active_path = self.document_state.active_path.clone();
        if let Some(path) = active_path
            && let Some(engine) = self.document_state.engines.get(&path)
            && let Some(details) = engine.describe_single_selection(selected)
        {
            self.document_state.panel_ctx.selected_uuid = Some(details.selected_uuid);
            self.document_state.panel_ctx.selected_kind = Some(details.selected_kind);
            self.document_state.panel_ctx.selection_info = details.info;
            // Cache the live SchDrawing for the Properties preview
            // widget — only when the single selection is a drawing.
            if matches!(
                details.selected_kind,
                signex_types::schematic::SelectedKind::Drawing
            ) {
                use signex_types::schematic::SchDrawing;
                self.document_state.panel_ctx.selected_drawing = engine
                    .document()
                    .drawings
                    .iter()
                    .find(|d| {
                        let u = match d {
                            SchDrawing::Line { uuid, .. }
                            | SchDrawing::Rect { uuid, .. }
                            | SchDrawing::Circle { uuid, .. }
                            | SchDrawing::Arc { uuid, .. }
                            | SchDrawing::Polyline { uuid, .. } => *uuid,
                        };
                        u == details.selected_uuid
                    })
                    .cloned();
            }
            if matches!(
                details.selected_kind,
                signex_types::schematic::SelectedKind::ChildSheet
            ) {
                self.document_state.panel_ctx.selected_child_sheet = engine
                    .document()
                    .child_sheets
                    .iter()
                    .find(|cs| cs.uuid == details.selected_uuid)
                    .cloned();
            }
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
        self.interaction_state.active_canvas_mut().set_theme_colors(
            signex_render::colors::to_iced(&colors.background),
            signex_render::colors::to_iced(&colors.grid),
            signex_render::colors::to_iced(&colors.paper),
        );
        self.interaction_state.pcb_canvas.set_theme_colors(
            signex_render::colors::to_iced(&colors.background),
            signex_render::colors::to_iced(&colors.grid),
        );
        self.interaction_state.active_canvas_mut().canvas_colors = colors;
        self.interaction_state.pcb_canvas.canvas_colors = colors;
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.interaction_state.pcb_canvas.clear_content_cache();
    }
}

/// Scan a library directory for standalone primitive files. Returns
/// `(symbols, footprints, sims)` triples — each `(stem, absolute_path)`.
/// Missing subdirectories are silently treated as empty so a fresh
/// library doesn't error; non-UTF-8 filenames and dotfiles are skipped.
///
/// Order is filename-stem-sorted so the project tree stays stable
/// across sessions (read_dir order is platform-dependent on Windows).
/// Project the active `.snxsym` editor's data into a panel-side
/// snapshot. Called from `refresh_panel_ctx` so the right-dock
/// Properties panel and the SCH-Library left-dock panel can render
/// context-aware content while the active tab is a Symbol editor.
/// Returns `None` for any other tab kind.
fn build_symbol_editor_panel_ctx(
    app: &super::Signex,
) -> Option<crate::panels::SymbolEditorPanelContext> {
    use crate::library::editor::symbol::state as sym_state;
    use crate::panels::{
        GraphicKindSummary, GraphicSummary, SymbolEditorPanelContext, SymbolEditorSelection,
        SymbolFileEntry, SymbolPinDetails, SymbolPinSummary,
    };

    let active = app.document_state.tabs.get(app.document_state.active_tab)?;
    let path = match &active.kind {
        crate::app::TabKind::SymbolEditor(p) => p.clone(),
        _ => return None,
    };
    let editor = app.document_state.symbol_editors.get(&path)?;
    let sym = editor.primitive();

    let pins: Vec<SymbolPinSummary> = sym
        .pins
        .iter()
        .enumerate()
        .map(|(idx, pin)| SymbolPinSummary {
            idx,
            number: pin.number.clone(),
            name: pin.name.clone(),
            electrical: format!("{:?}", pin.electrical),
            position: pin.position,
            orientation: format!("{:?}", pin.orientation),
            length: pin.length,
            details: SymbolPinDetails {
                description: pin.description.clone(),
                function: pin.function.clone(),
                pin_package_length: pin.pin_package_length,
                propagation_delay_ns: pin.propagation_delay_ns,
                designator_visible: pin.designator_visible,
                name_visible: pin.name_visible,
                inside_symbol: pin.inside_symbol,
                inside_edge_symbol: pin.inside_edge_symbol,
                outside_edge_symbol: pin.outside_edge_symbol,
                outside_symbol: pin.outside_symbol,
                hidden: pin.hidden,
                locked: pin.locked,
                part_number: pin.part_number,
            },
        })
        .collect();

    let symbols_in_file: Vec<SymbolFileEntry> = editor
        .file
        .symbols
        .iter()
        .enumerate()
        .map(|(idx, s)| SymbolFileEntry {
            idx,
            name: s.name.clone(),
            uuid: s.uuid,
            pin_count: s.pins.len(),
        })
        .collect();

    let graphics: Vec<GraphicSummary> = sym
        .graphics
        .iter()
        .enumerate()
        .map(|(idx, g)| GraphicSummary {
            idx,
            kind: graphic_kind_to_summary(&g.kind),
            stroke_width: g.stroke_width,
        })
        .collect();

    let selected = match editor.selected {
        Some(sym_state::SymbolSelection::Pin(idx)) => pins
            .get(idx)
            .cloned()
            .map(SymbolEditorSelection::Pin)
            .unwrap_or(SymbolEditorSelection::None),
        Some(sym_state::SymbolSelection::Field(sym_state::FieldKey::Reference)) => {
            SymbolEditorSelection::FieldReference
        }
        Some(sym_state::SymbolSelection::Field(sym_state::FieldKey::Value)) => {
            SymbolEditorSelection::FieldValue
        }
        Some(sym_state::SymbolSelection::Graphic(idx)) => sym
            .graphics
            .get(idx)
            .map(|g| {
                SymbolEditorSelection::Graphic(GraphicSummary {
                    idx,
                    kind: graphic_kind_to_summary(&g.kind),
                    stroke_width: g.stroke_width,
                })
            })
            .unwrap_or(SymbolEditorSelection::None),
        None => SymbolEditorSelection::None,
    };

    let active_max_part = sym_state::max_part_number(sym);
    let active_has_part_zero = sym.pins.iter().any(|p| p.part_number == 0);

    Some(SymbolEditorPanelContext {
        path,
        symbol_name: sym.name.clone(),
        symbol_uuid: sym.uuid,
        pins,
        graphics,
        selected,
        symbols_in_file,
        active_idx: editor.active_idx,
        active_part: editor.active_part,
        active_max_part,
        active_has_part_zero,
    })
}

/// Project a `SymbolGraphicKind` into a [`GraphicKindSummary`] so the
/// Properties panel can render per-shape fields without depending on
/// the library type.
fn graphic_kind_to_summary(
    kind: &signex_library::SymbolGraphicKind,
) -> crate::panels::GraphicKindSummary {
    use crate::panels::GraphicKindSummary;
    use signex_library::SymbolGraphicKind;
    match kind {
        SymbolGraphicKind::Rectangle { from, to } => GraphicKindSummary::Rectangle {
            from: *from,
            to: *to,
        },
        SymbolGraphicKind::Line { from, to } => GraphicKindSummary::Line {
            from: *from,
            to: *to,
        },
        SymbolGraphicKind::Circle { center, radius } => GraphicKindSummary::Circle {
            center: *center,
            radius: *radius,
        },
        SymbolGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => GraphicKindSummary::Arc {
            center: *center,
            radius: *radius,
            start_deg: *start_deg,
            end_deg: *end_deg,
        },
        SymbolGraphicKind::Text {
            position,
            content,
            size,
        } => GraphicKindSummary::Text {
            position: *position,
            content: content.clone(),
            size: *size,
        },
    }
}

fn scan_library_primitives(
    root: &std::path::Path,
) -> (
    Vec<(String, std::path::PathBuf)>,
    Vec<(String, std::path::PathBuf)>,
    Vec<(String, std::path::PathBuf)>,
) {
    let symbols = scan_dir(&root.join("symbols"), "snxsym");
    let footprints = scan_dir(&root.join("footprints"), "snxfpt");
    let sims = scan_dir(&root.join("sims"), "snxsim");
    (symbols, footprints, sims)
}

fn scan_dir(dir: &std::path::Path, want_ext: &str) -> Vec<(String, std::path::PathBuf)> {
    let read_iter = match std::fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return Vec::new(),
    };
    let mut out: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in read_iter.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        // Skip dotfiles, swap files, backup files.
        if name.starts_with('.')
            || name.ends_with(".swp")
            || name.ends_with(".bak")
            || name.ends_with('~')
        {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if ext != want_ext {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        out.push((stem, path));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}
