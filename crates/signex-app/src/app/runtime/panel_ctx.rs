use super::super::*;
use super::footprint_ctx::build_footprint_editor_panel_ctx;
use super::symbol_ctx::build_symbol_editor_panel_ctx;

/// `(lib_symbol_names, placed_symbols)` as held by the panel context.
type SymbolLists = (Vec<String>, Vec<(String, String, String, String)>);

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

        // A read failure and an absent UUID are different diagnoses.
        // Reporting "UUID not in mounted libraries" for a corrupt
        // `.snxsym` or a library server returning 500 sends the user
        // off to re-bind a perfectly good UUID, so the error arm gets
        // its own wording and a Messages-panel record.
        let symbol_summary = match self.library.set.resolve_symbol(&row.symbol_ref) {
            Ok(Some(s)) => format!(
                "Symbol bound — {} pin{}",
                s.pins.len(),
                if s.pins.len() == 1 { "" } else { "s" }
            ),
            Ok(None) if row.is_symbol_unbound() => "No symbol bound".to_string(),
            Ok(None) => "Symbol unresolved (UUID not in mounted libraries)".to_string(),
            Err(e) => {
                tracing::error!(
                    target: "signex::library",
                    error = %e,
                    library_id = %row.symbol_ref.library_id,
                    symbol_uuid = %row.symbol_ref.uuid,
                    internal_pn = %row.internal_pn,
                    "symbol could not be read from its library — the binding is not known to be wrong"
                );
                format!("Symbol could not be read from its library — {e}")
            }
        };
        let symbol_summary = match row.alternate_symbols.len() {
            0 => symbol_summary,
            1 => format!("{symbol_summary} (+1 alternate symbol)"),
            n => format!("{symbol_summary} (+{n} alternate symbols)"),
        };
        let footprint_summary = match &row.footprint_ref {
            Some(_) if row.is_footprint_unbound() => "No footprint bound".to_string(),
            Some(fp) => match self.library.set.resolve_footprint(fp) {
                Ok(Some(_)) => "Footprint bound".to_string(),
                Ok(None) => "Footprint unresolved (UUID not in mounted libraries)".to_string(),
                Err(e) => {
                    tracing::error!(
                        target: "signex::library",
                        error = %e,
                        library_id = %fp.library_id,
                        footprint_uuid = %fp.uuid,
                        internal_pn = %row.internal_pn,
                        "footprint could not be read from its library — the binding is not known to be wrong"
                    );
                    format!("Footprint could not be read from its library — {e}")
                }
            },
            None => "No footprint bound".to_string(),
        };

        let read_only = self.library.is_read_only(&library_path);
        Some(crate::panels::LibraryRowDetail {
            library_path,
            table,
            row_id: row.row_id,
            internal_pn: row.internal_pn.as_str().to_string(),
            class: row.class.as_str().to_string(),
            lifecycle_label: format!("{:?}", row.state),
            symbol_unbound: row.is_symbol_unbound(),
            footprint_unbound: row.is_footprint_unbound(),
            symbol_summary,
            footprint_summary,
            read_only,
        })
    }

    /// Every path and directory the project tree reads from the disk. Pure:
    /// the paths come from project data, so building it does no IO.
    fn project_fs_request(&self) -> crate::app::state::project_fs::ProjectFsRequest {
        use crate::app::state::project_fs::git_marker;
        let mut request = crate::app::state::project_fs::ProjectFsRequest::default();
        for p in &self.document_state.projects {
            let dir = std::path::Path::new(&p.data.dir);
            let files = p
                .data
                .schematic_root
                .iter()
                .chain(p.data.pcb_file.iter())
                .chain(p.data.sheets.iter().map(|s| &s.filename));
            request.exists.extend(files.map(|f| dir.join(f)));
            if let Some(project_dir) = p.path.parent() {
                request.exists.push(git_marker(project_dir));
            }
            for entry in &p.data.libraries {
                let resolved = p.data.resolve_library_path(entry);
                if let Some(lib_dir) = resolved.parent() {
                    request.exists.push(git_marker(lib_dir));
                }
                request.exists.push(resolved);
            }
        }
        request
    }

    /// The Net Classes section's state: the panel's own selection and drafts
    /// carried over, the table and history flags read from the active project.
    fn net_class_panel_state(&self) -> crate::panels::NetClassPanel {
        let document = &self.document_state;
        let mut panel = document.panel_ctx.net_classes.clone();
        let project = document
            .active_project
            .and_then(|id| document.projects.iter().find(|p| p.id == id));
        match project {
            Some(p) => {
                let history = document.net_class_history.get(&p.id);
                if panel.project != Some(p.id) {
                    panel.project = Some(p.id);
                    panel.selected = 0;
                    panel.draft = p
                        .data
                        .net_classes
                        .first()
                        .map(crate::panels::NetClassDraft::from_class)
                        .unwrap_or_default();
                    panel.notice = None;
                }
                panel.available = true;
                panel.classes = p.data.net_classes.clone();
                panel.can_undo = history.is_some_and(|h| h.can_undo());
                panel.can_redo = history.is_some_and(|h| h.can_redo());
                let selected = panel.selected.min(panel.classes.len().saturating_sub(1));
                if selected != panel.selected {
                    panel.selected = selected;
                    panel.draft = panel
                        .classes
                        .get(selected)
                        .map(crate::panels::NetClassDraft::from_class)
                        .unwrap_or_default();
                }
            }
            None => panel = Default::default(),
        }
        panel.label = self.selected_label_class_pick();
        panel
    }

    /// Fill `lib_symbol_names` / `placed_symbols`: the lists kept from the
    /// previous refresh when the engine is unchanged, else rebuilt (#1383).
    fn fill_symbol_lists(&mut self, kept: Option<SymbolLists>) {
        let (names, placed) = match kept {
            Some(lists) => lists,
            None => {
                if let Some(snapshot) = self.active_render_snapshot() {
                    (
                        snapshot.lib_symbols.keys().cloned().collect(),
                        snapshot
                            .symbols
                            .iter()
                            .map(|s| {
                                (
                                    s.reference.clone(),
                                    s.value.clone(),
                                    s.footprint.clone(),
                                    s.lib_id.clone(),
                                )
                            })
                            .collect(),
                    )
                } else {
                    let placed = self
                        .active_pcb_snapshot()
                        .map(|pcb| {
                            pcb.footprints
                                .iter()
                                .map(|f| {
                                    (
                                        f.reference.clone(),
                                        f.value.clone(),
                                        f.footprint_id.clone(),
                                        f.layer.clone(),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    (Vec::new(), placed)
                }
            }
        };
        let ctx = &mut self.document_state.panel_ctx;
        ctx.lib_symbol_names = names;
        ctx.placed_symbols = placed;
    }

    /// Who owns the numeric box `key` and the value it stores now, from the
    /// same projection the Properties panel renders; `None` when the box is
    /// not on screen (other selection, other tab, no placement).
    pub(crate) fn numeric_field_state(
        &self,
        key: crate::panels::NumericFieldKey,
    ) -> Option<(crate::panels::NumericOwner, f64)> {
        let active_tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| &tab.path);
        crate::panels::numeric_field_state(
            key,
            active_tab,
            build_symbol_editor_panel_ctx(self).as_ref(),
            self.document_state.panel_ctx.pre_placement.as_ref(),
        )
    }

    pub(crate) fn refresh_panel_ctx(&mut self) {
        self.drop_pending_profile_update_for_other_project();
        let fs_request = self.project_fs_request();
        self.document_state.project_fs.want(fs_request);
        // The dirty dots below read `dirty_paths`; make sure footprint
        // edits are in it first (#666).
        self.sync_editor_dirty_paths();
        let previous_navigator = std::mem::take(&mut self.document_state.panel_ctx.navigator);
        let navigator = self.next_navigator_model(previous_navigator);
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
                // Read from the cache, never the disk (#111): the off-thread
                // probe keeps it current.
                let fs_facts = &self.document_state.project_fs.facts;
                let exists =
                    |filename: &str| -> bool { !fs_facts.is_missing(&project_dir.join(filename)) };
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
                        let missing = fs_facts.is_missing(&resolved);
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
                        let resolved_clone = resolved.clone();
                        let is_open = open_paths.contains(&resolved_clone);
                        let is_dirty = dirty_paths.contains(&resolved_clone);
                        crate::panels::LibraryNodeInfo {
                            display_name,
                            root: resolved,
                            mounted: mounted_lib.is_some(),
                            missing,
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

        // The two symbol lists are O(symbols) string clones; reuse the
        // previous ones while the active engine is unchanged (#1383).
        // Keyed on the render cache the lists are built from, not the engine:
        // the cache can lag an engine edit. Revision 0 means "unknown".
        let symbol_lists_key = self.document_state.active_engine().and_then(|engine| {
            let cache = self
                .interaction_state
                .active_canvas()
                .active_render_cache()?;
            (cache.revision() != 0).then(|| (engine.instance_id(), cache.revision()))
        });
        let reuse_symbol_lists = symbol_lists_key.is_some()
            && symbol_lists_key == self.document_state.panel_ctx.symbol_lists_key;
        let kept_symbol_lists = reuse_symbol_lists.then(|| {
            let ctx = &mut self.document_state.panel_ctx;
            (
                std::mem::take(&mut ctx.lib_symbol_names),
                std::mem::take(&mut ctx.placed_symbols),
            )
        });

        // The tree is a pure function of `projects`; keep the previous one
        // (and its expand/collapse state) while they are unchanged (#1383).
        let (kept_tree, previous_projects, project_tree_builds) = {
            let ctx = &mut self.document_state.panel_ctx;
            (
                std::mem::take(&mut ctx.project_tree),
                std::mem::take(&mut ctx.projects),
                ctx.project_tree_builds,
            )
        };
        let tree_unchanged = previous_projects == projects_panel
            && (project_tree_builds > 0 || projects_panel.is_empty());
        let kept_project_tree = if tree_unchanged { kept_tree } else { vec![] };
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
        let symbol_choices = self.document_state.panel_ctx.symbol_choices.clone();
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
        let child_sheet_name_buf = self.document_state.panel_ctx.child_sheet_name_buf.clone();
        let child_sheet_file_buf = self.document_state.panel_ctx.child_sheet_file_buf.clone();
        let title_block: std::collections::BTreeMap<String, String> = active_schematic_snapshot
            .map(|snapshot| {
                snapshot
                    .title_block
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let project_parameters = self
            .document_state
            .active_path
            .as_deref()
            .and_then(|path| self.document_state.document_project(path))
            .map(|project| project.data.parameters.clone())
            .unwrap_or_default();
        // Drafts stay with the sheet they were typed on (shown and committed
        // only there); a closed sheet's drafts are dropped.
        let title_block_drafts: Vec<crate::panels::TitleBlockDraft> = self
            .document_state
            .panel_ctx
            .title_block_drafts
            .iter()
            .filter(|draft| self.document_state.engines.contains_key(&draft.sheet))
            .cloned()
            .collect();
        let paper_edge_drafts: Vec<crate::panels::PaperEdgeDraft> = self
            .document_state
            .panel_ctx
            .paper_edge_drafts
            .iter()
            .filter(|draft| self.document_state.engines.contains_key(&draft.sheet))
            .cloned()
            .collect();
        let numeric_draft = self
            .document_state
            .panel_ctx
            .numeric_draft
            .clone()
            .filter(|draft| draft.is_live(self.numeric_field_state(draft.key)));
        let title_block_sheet = self.document_state.active_path.clone();
        let component_filter = self.document_state.panel_ctx.component_filter.clone();
        let collapsed_sections = self.document_state.panel_ctx.collapsed_sections.clone();
        let pre_placement = self.document_state.panel_ctx.pre_placement.clone();
        let page_format_mode = self.document_state.panel_ctx.page_format_mode;
        let page_origin = self.document_state.panel_ctx.page_origin;
        let sheet_color = self.document_state.panel_ctx.sheet_color;
        let erc_diagnostics = self.build_erc_diagnostic_entries();

        let net_classes = self.net_class_panel_state();

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
            has_schematic: self.has_active_schematic(),
            has_pcb: self.has_active_pcb(),
            paper_size: active_schematic_snapshot
                .map(|snapshot| {
                    crate::app::load_gateway::effective_paper_size(&snapshot.paper_size)
                })
                .or_else(|| {
                    active_pcb_snapshot
                        .map(|snapshot| format!("PCB • {} layers", snapshot.layers.len()))
                })
                .unwrap_or_else(|| "A4".to_string()),
            document_font_family: active_schematic_snapshot
                .and_then(|snapshot| snapshot.font_family.clone()),
            lib_symbol_count: active_schematic_snapshot
                .map(|snapshot| snapshot.lib_symbols.len())
                .unwrap_or(0),
            lib_symbol_names: Vec::new(),
            placed_symbols: Vec::new(),
            symbol_lists_key,
            tokens: self.tokens_for(self.ui_state.theme_id),
            theme_id: self.ui_state.theme_id,
            unit: self.ui_state.unit,
            grid_visible: self.ui_state.grid_visible,
            snap_enabled: self.ui_state.snap_enabled,
            grid_size_mm: self.ui_state.grid_size_mm,
            visible_grid_mm: self.ui_state.visible_grid_mm,
            // Preserve the effective (possibly live-preview) style — a
            // panel_ctx rebuild mid-Preferences must not snap the Symbol
            // Editor grid back to the committed value. Seeded from
            // `ui_state.symbol_grid_style` at boot; every later write
            // goes through the Preferences handler.
            symbol_grid_style: self.document_state.panel_ctx.symbol_grid_style,
            snap_hotspots: self.ui_state.snap_hotspots,
            ui_font_name: self.ui_state.ui_font_name.clone(),
            component_classes: self.ui_state.component_classes.clone(),
            canvas_font_name: self.ui_state.canvas_font_name.clone(),
            canvas_font_size: self.ui_state.canvas_font_size,
            canvas_font_bold: self.ui_state.canvas_font_bold,
            canvas_font_italic: self.ui_state.canvas_font_italic,
            canvas_font_popup_open,
            properties_tab,
            net_classes,
            standard_libraries,
            active_library,
            library_symbols,
            selected_component,
            selected_pins,
            selected_lib_symbol,
            components_split,
            project_tree: kept_project_tree,
            project_tree_builds,
            project_tree_selected: self.document_state.panel_ctx.project_tree_selected.clone(),
            navigator,
            library_row_detail: self.compute_library_row_detail(),
            selection_count,
            selected_uuid,
            selected_kind,
            selection_info,
            symbol_choices,
            drawing_edit_buf,
            drawing_edit_buf_for,
            selected_drawing,
            selected_child_sheet,
            child_sheet_border_picker_open,
            child_sheet_fill_picker_open,
            child_sheet_border_advanced_open,
            child_sheet_fill_advanced_open,
            child_sheet_stroke_width_buf,
            child_sheet_name_buf,
            child_sheet_file_buf,
            title_block,
            project_parameters,
            title_block_drafts,
            paper_edge_drafts,
            numeric_draft,
            title_block_sheet,
            component_filter,
            collapsed_sections,
            pre_placement,
            erc_diagnostics,
            erc_focus_index: self.ui_state.erc_focus_global_index,
            erc_running: self.ui_state.erc_running,
            diagnostics_level: crate::diagnostics::configured_level_label().to_string(),
            diagnostics: crate::diagnostics::recent_entries(),
            selection_filters: self.interaction_state.selection_filters.clone(),
            custom_filter_presets: self.interaction_state.custom_filter_presets.clone(),
            active_custom_filter_tab: self.interaction_state.active_custom_filter_tab,
            footprint_filter_presets: self.interaction_state.footprint_filter_presets.clone(),
            page_format_mode,
            sheet_frame: active_schematic_snapshot
                .map(|snapshot| snapshot.frame)
                .unwrap_or_default(),
            page_origin,
            sheet_color,
            symbol_editor: build_symbol_editor_panel_ctx(self),
            footprint_editor: build_footprint_editor_panel_ctx(self),
            history: self.document_state.history.clone(),
        };
        self.fill_symbol_lists(kept_symbol_lists);
        if !tree_unchanged {
            let ctx = &mut self.document_state.panel_ctx;
            ctx.project_tree = crate::panels::build_project_tree(ctx);
            ctx.project_tree_builds = ctx.project_tree_builds.wrapping_add(1);
        }
        // Sheet N of M and the project's title block can change with it.
        self.refresh_canvas_sheet_frame();
        self.refresh_canvas_designators();
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData};

    use crate::app::Signex;
    use crate::app::state::LoadedProject;
    use crate::app::state::project_fs::git_marker;

    /// #1383: an unchanged engine keeps the symbol lists (no per-edit
    /// O(symbols) re-clone); an edit rebuilds them.
    #[test]
    fn symbol_lists_are_reused_until_the_engine_revision_moves() {
        let (mut app, _task) = Signex::new();
        app.open_schematic_tab(
            PathBuf::from("snx-1383.snxsch"),
            "snx-1383".to_string(),
            signex_types::schematic::SchematicSheet::default(),
        );
        app.refresh_panel_ctx();
        let key = app.document_state.panel_ctx.symbol_lists_key;
        assert!(key.is_some(), "schematic engine must key the lists");

        let sentinel = (
            "Z1".to_string(),
            String::new(),
            String::new(),
            String::new(),
        );
        app.document_state.panel_ctx.placed_symbols = vec![sentinel.clone()];
        app.refresh_panel_ctx();
        assert_eq!(app.document_state.panel_ctx.placed_symbols, vec![sentinel]);

        app.document_state.panel_ctx.symbol_lists_key = Some((0, 0));
        app.refresh_panel_ctx();
        assert!(app.document_state.panel_ctx.placed_symbols.is_empty());
        assert_eq!(app.document_state.panel_ctx.symbol_lists_key, key);
    }

    /// #1383: the project tree is rebuilt only when what it shows changes,
    /// and a changed sheet state does rebuild it.
    #[test]
    fn project_tree_is_reused_until_the_projects_change() {
        let (mut app, _task) = Signex::new();
        let id = app.document_state.mint_project_id();
        app.document_state.projects.push(LoadedProject {
            id,
            path: PathBuf::from("snx-1383-tree/p.snxprj"),
            data: ProjectData {
                name: "p".to_string(),
                description: None,
                dir: "snx-1383-tree".to_string(),
                schematic_root: Some("a.snxsch".to_string()),
                pcb_file: None,
                sheets: Vec::new(),
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
        app.refresh_panel_ctx();
        let builds = app.document_state.panel_ctx.project_tree_builds;
        assert!(builds > 0 && !app.document_state.panel_ctx.project_tree.is_empty());

        app.refresh_panel_ctx();
        app.refresh_panel_ctx();
        assert_eq!(app.document_state.panel_ctx.project_tree_builds, builds);

        app.document_state.projects[0].data.pcb_file = Some("b.snxpcb".into());
        app.refresh_panel_ctx();
        assert_eq!(app.document_state.panel_ctx.project_tree_builds, builds + 1);
    }

    /// #1383: a real engine edit moves the key, so the panel lists show the
    /// placed symbol instead of the reused (stale) ones.
    #[test]
    fn symbol_lists_follow_a_real_edit() {
        let (mut app, _task) = Signex::new();
        app.open_schematic_tab(
            PathBuf::from("snx-1383-edit.snxsch"),
            "snx-1383-edit".to_string(),
            signex_types::schematic::SchematicSheet::default(),
        );
        app.refresh_panel_ctx();
        assert!(app.document_state.panel_ctx.placed_symbols.is_empty());

        app.document_state.panel_ctx.selected_component = Some("Device:R".to_string());
        assert!(app.place_selected_component(10.0, 10.0));
        app.refresh_panel_ctx();
        assert_eq!(app.document_state.panel_ctx.placed_symbols.len(), 1);
    }

    /// #1098: the probe request must name `<dir>/.git` for the project dir
    /// and each library dir, or the Enable Version Control row never appears.
    #[test]
    fn fs_request_names_the_git_marker_of_project_and_library_dirs() {
        let dir = Path::new("snx-1098-proj");
        let (mut app, _task) = Signex::new();
        let id = app.document_state.mint_project_id();
        app.document_state.projects.push(LoadedProject {
            id,
            path: dir.join("P.snxprj"),
            data: ProjectData {
                description: None,
                name: "P".to_string(),
                dir: dir.to_string_lossy().into_owned(),
                schematic_root: None,
                pcb_file: None,
                sheets: Vec::new(),
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: vec![LibraryEntry {
                    path: PathBuf::from("lib/L.snxlib"),
                    kind: LibraryEntryKind::ProjectLocal,
                    library_id: None,
                    unknown: Default::default(),
                }],
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });

        let request = app.project_fs_request();
        assert!(request.exists.contains(&git_marker(dir)));
        assert!(request.exists.contains(&git_marker(&dir.join("lib"))));
    }
}
