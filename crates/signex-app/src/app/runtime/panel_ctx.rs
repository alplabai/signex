use super::super::*;
use super::footprint_ctx::build_footprint_editor_panel_ctx;
use super::symbol_ctx::build_symbol_editor_panel_ctx;

impl Signex {
    /// F15 — When the active tab is a Library Browser AND a row is
    /// selected in that tab's browser state, build the
    /// [`crate::panels::LibraryRowDetail`] the Properties panel
    /// renders. Returns `None` for any other active tab kind, or
    /// when the browser tab has no row selected.
    fn compute_library_row_detail(&self) -> Option<crate::panels::LibraryRowDetail> {
        let active_tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        let library_path = match &active_tab.kind {
            crate::app::TabKind::LibraryBrowser(p) => p.clone(),
            _ => return None,
        };
        let browser = self.library.library_browsers.get(&library_path)?;
        let table = browser.active_table.clone()?;
        let row_id = browser.selected_row?;
        let lib = self.library.library_at(&library_path)?;
        let row = lib
            .tables
            .get(&table)?
            .iter()
            .find(|r| signex_library::RowId::from_uuid(r.row_id) == row_id)?;

        let symbol_summary = match self.library.set.resolve_symbol(&row.symbol_ref) {
            Some(s) => format!(
                "Symbol bound — {} pin{}",
                s.pins.len(),
                if s.pins.len() == 1 { "" } else { "s" }
            ),
            None if row.symbol_ref.uuid == uuid::Uuid::nil() => "Symbol unbound".to_string(),
            None => "Symbol unresolved (UUID not in mounted libraries)".to_string(),
        };
        let footprint_summary = match &row.footprint_ref {
            Some(fp) if fp.uuid == uuid::Uuid::nil() => "Footprint unbound".to_string(),
            Some(fp) => match self.library.set.resolve_footprint(fp) {
                Some(_) => "Footprint bound".to_string(),
                None => "Footprint unresolved (UUID not in mounted libraries)".to_string(),
            },
            None => "Footprint unbound".to_string(),
        };

        Some(crate::panels::LibraryRowDetail {
            library_path,
            table,
            row_id: row.row_id,
            internal_pn: row.internal_pn.as_str().to_string(),
            class: row.class.as_str().to_string(),
            lifecycle_label: format!("{:?}", row.state),
            symbol_summary,
            footprint_summary,
        })
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

                // F24 — file existence check. Used to mark orphan
                // references (file registered on the project but
                // missing on disk) so the tree shows them as
                // `<name> (missing)` upfront.
                let exists = |filename: &str| -> bool {
                    let abs = project_dir.join(filename);
                    abs.exists()
                };
                let project_file_missing = p
                    .data
                    .schematic_root
                    .as_deref()
                    .map(|f| !exists(f))
                    .unwrap_or(false);
                let pcb_file_missing = p
                    .data
                    .pcb_file
                    .as_deref()
                    .map(|f| !exists(f))
                    .unwrap_or(false);
                // Flatten `Project::libraries` into the panel struct
                // alongside the sheet list. Each entry resolves to an
                // absolute path so the right-click menu can dispatch
                // back to the correct library; the library renders as
                // a single leaf in the project tree, so we don't
                // enumerate the library's primitive files here.
                let mut libraries: Vec<crate::panels::LibraryNodeInfo> = p
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
                        let missing = !resolved.exists();
                        // F30 — list `.snxsym` / `.snxfpt` FILES, not
                        // individual primitives. Each file can hold
                        // hundreds-to-thousands of symbols (Altium
                        // parity: one `.SchLib` ≡ one `.snxsym`), so
                        // enumerating cached_symbols would explode the
                        // tree. The user opens a `.snxsym` file to
                        // browse the symbols inside it via the SCH
                        // Library panel, not via the project tree.
                        //
                        // F34 (2026-05-03) — Add New ▸ Symbol Library
                        // now opens a Save-As dialog so the user names
                        // the file at create time and the `.snxsym`
                        // lands on disk immediately. No in-memory
                        // merging needed; the next refresh after the
                        // dialog confirm picks up the new file.
                        let (symbols, footprints) = if mounted_lib.is_some() {
                            // `.snxlib` is a manifest *file* — symbols/
                            // and footprints/ are SIBLINGS of it inside
                            // the library's working dir (the `.snxlib`'s
                            // parent). Joining `resolved` directly would
                            // build `<project>/<lib>.snxlib/symbols`,
                            // which is not a real path and silently
                            // returns empty via the `.ok().flatten()`
                            // chain — the regression from F34 where
                            // freshly-created `.snxsym` files never
                            // appeared in the project tree.
                            let lib_root = resolved
                                .parent()
                                .map(std::path::Path::to_path_buf)
                                .unwrap_or_else(|| resolved.clone());
                            let read_dir_names = |sub: &str, ext: &str| -> Vec<String> {
                                let dir = lib_root.join(sub);
                                let mut names: Vec<String> = std::fs::read_dir(&dir)
                                    .ok()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(|entry| entry.ok())
                                    .filter_map(|entry| {
                                        let path = entry.path();
                                        if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                                            path.file_stem()
                                                .and_then(|s| s.to_str())
                                                .map(|s| s.to_string())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                names.sort();
                                names
                            };
                            (
                                read_dir_names("symbols", "snxsym"),
                                read_dir_names("footprints", "snxfpt"),
                            )
                        } else {
                            (Vec::new(), Vec::new())
                        };
                        let resolved_clone = resolved.clone();
                        let is_open = open_paths.contains(&resolved_clone);
                        let is_dirty = dirty_paths.contains(&resolved_clone);
                        crate::panels::LibraryNodeInfo {
                            display_name,
                            root: resolved,
                            mounted: mounted_lib.is_some(),
                            missing,
                            symbols,
                            footprints,
                            is_open,
                            is_dirty,
                        }
                    })
                    .collect();

                // Pending (yet-to-be-materialised) libraries appear in
                // the tree as un-mounted nodes with a "(pending)"
                // suffix so the user can see what Save will create.
                // Disk write happens at project-save time via
                // `commands::materialize_pending_library`.
                for spec in p.pending_libraries.values() {
                    libraries.push(crate::panels::LibraryNodeInfo {
                        display_name: format!("{} (pending)", spec.display_name),
                        root: spec.lib_path.clone(),
                        mounted: false,
                        // Pending entries are intentionally absent on
                        // disk (they materialise at save time) — not
                        // a missing-orphan situation.
                        missing: false,
                        symbols: Vec::new(),
                        footprints: Vec::new(),
                        is_open: false,
                        is_dirty: false,
                    });
                }
                crate::panels::ProjectPanelInfo {
                    id: p.id,
                    name: p.data.name.clone(),
                    project_file: p.data.schematic_root.clone(),
                    project_file_open,
                    project_file_dirty,
                    project_file_active,
                    project_file_missing,
                    pcb_file: p.data.pcb_file.clone(),
                    pcb_file_open,
                    pcb_file_dirty,
                    pcb_file_active,
                    pcb_file_missing,
                    sheets: p
                        .data
                        .sheets
                        .iter()
                        .map(|sheet| {
                            let (is_open, is_dirty, is_active) = lookup(&sheet.filename);
                            let missing = !exists(&sheet.filename);
                            crate::panels::SheetInfo {
                                name: sheet.name.clone(),
                                filename: sheet.filename.clone(),
                                sym_count: sheet.symbols_count,
                                wire_count: sheet.wires_count,
                                label_count: sheet.labels_count,
                                is_open,
                                is_dirty,
                                is_active,
                                missing,
                            }
                        })
                        .collect(),
                    libraries,
                    is_active: Some(p.id) == active_id,
                    is_dirty: dirty_paths.contains(&p.path),
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
        let child_sheet_border_advanced_open = self
            .document_state
            .panel_ctx
            .child_sheet_border_advanced_open;
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
            component_classes: self.ui_state.component_classes.clone(),
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
            project_tree_selected: self.document_state.panel_ctx.project_tree_selected.clone(),
            library_row_detail: self.compute_library_row_detail(),
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
            footprint_filter_presets: self.interaction_state.footprint_filter_presets.clone(),
            page_format_mode,
            margin_vertical,
            margin_horizontal,
            page_origin,
            custom_paper_w_mm,
            custom_paper_h_mm,
            sheet_color,
            symbol_editor: build_symbol_editor_panel_ctx(self),
            footprint_editor: build_footprint_editor_panel_ctx(self),
            history: self.document_state.history.clone(),
        };
        self.document_state.panel_ctx.project_tree =
            crate::panels::build_project_tree(&self.document_state.panel_ctx);
    }
}
