use iced::Task;

use crate::dock::{DockMessage, PanelPosition};

use super::super::*;

impl Signex {
    pub(crate) fn handle_dock_message(&mut self, msg: DockMessage) -> Task<Message> {
        use signex_widgets::tree_view::{TreeIcon, TreeMsg, get_node};

        match &msg {
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetUnit(unit)) => {
                self.ui_state.unit = *unit;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleGrid) => {
                self.ui_state.grid_visible = !self.ui_state.grid_visible;
                self.interaction_state.canvas.grid_visible = self.ui_state.grid_visible;
                self.interaction_state.pcb_canvas.grid_visible = self.ui_state.grid_visible;
                self.interaction_state.canvas.clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSnap) => {
                self.ui_state.snap_enabled = !self.ui_state.snap_enabled;
                self.interaction_state.canvas.snap_enabled = self.ui_state.snap_enabled;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::PropertiesTab(idx)) => {
                self.document_state.panel_ctx.properties_tab = *idx;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SelectLibrary(name)) => {
                let name = name.clone();
                if let Some(dir) = &self.document_state.kicad_lib_dir {
                    let mut symbols = std::collections::HashMap::new();
                    let mut syms = Vec::new();
                    let libraries: Vec<String> = if name == helpers::ALL_LIBRARIES {
                        self.document_state
                            .panel_ctx
                            .kicad_libraries
                            .iter()
                            .filter(|entry| entry.as_str() != helpers::ALL_LIBRARIES)
                            .cloned()
                            .collect()
                    } else {
                        vec![name.clone()]
                    };

                    for library_name in libraries {
                        let path = dir.join(format!("{library_name}.kicad_sym"));
                        match std::fs::read_to_string(&path) {
                            Ok(content) => match kicad_parser::parse_symbol_lib(&content) {
                                Ok(parsed) => {
                                    for (lib_id, lib_symbol) in parsed {
                                        syms.push(crate::panels::LibrarySymbolEntry {
                                            symbol_name: lib_id
                                                .rsplit(':')
                                                .next()
                                                .unwrap_or(&lib_id)
                                                .to_string(),
                                            library_name: library_name.clone(),
                                            pin_count: lib_symbol.pins.len(),
                                            lib_id: lib_id.clone(),
                                        });
                                        symbols.insert(lib_id, lib_symbol);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse library '{}': {e}", library_name);
                                }
                            },
                            Err(e) => eprintln!("Failed to read {}: {e}", path.display()),
                        }
                    }

                    syms.sort_by(|a, b| {
                        a.symbol_name
                            .cmp(&b.symbol_name)
                            .then_with(|| a.library_name.cmp(&b.library_name))
                    });

                    self.document_state.panel_ctx.library_symbols = syms;
                    self.document_state.panel_ctx.active_library = Some(name);
                    self.document_state.panel_ctx.selected_component = None;
                    self.document_state.panel_ctx.selected_pins.clear();
                    self.document_state.panel_ctx.selected_lib_symbol = None;
                    self.document_state.loaded_lib = symbols;
                }
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ComponentFilter(filter)) => {
                self.document_state.panel_ctx.component_filter = filter.clone();
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSection(key)) => {
                let key = key.clone();
                if !self.document_state.panel_ctx.collapsed_sections.remove(&key) {
                    self.document_state.panel_ctx.collapsed_sections.insert(key);
                }
            }
            crate::dock::DockMessage::Panel(
                crate::panels::PanelMsg::EditSymbolDesignator(uuid, new_val),
            ) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::SymbolReference(*uuid),
                        value: new_val.clone(),
                    },
                    false,
                    false,
                );
            }
            crate::dock::DockMessage::Panel(
                crate::panels::PanelMsg::EditSymbolValue(uuid, new_val),
            ) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::SymbolValue(*uuid),
                        value: new_val.clone(),
                    },
                    false,
                    false,
                );
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SelectComponent(lib_id)) => {
                let lib_id = lib_id.clone();
                if let Some(sym) = self.document_state.loaded_lib.get(&lib_id) {
                    let library_name = lib_id
                        .split(':')
                        .next()
                        .unwrap_or(helpers::ALL_LIBRARIES)
                        .to_string();
                    self.document_state.panel_ctx.selected_component = Some(lib_id);
                    self.document_state.panel_ctx.selected_pins = sym
                        .pins
                        .iter()
                        .map(|lp| {
                            (
                                lp.pin.number.clone(),
                                lp.pin.name.clone(),
                                format!("{:?}", lp.pin.pin_type),
                            )
                        })
                        .collect();
                    self.document_state.panel_ctx.selected_lib_symbol = Some(sym.clone());
                    if self.document_state.panel_ctx.active_library.is_none() {
                        self.document_state.panel_ctx.active_library = Some(library_name);
                    }
                }
            }
            crate::dock::DockMessage::Panel(
                crate::panels::PanelMsg::EditSymbolFootprint(uuid, new_val),
            ) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateSymbolFootprint {
                        symbol_id: *uuid,
                        footprint: new_val.clone(),
                    },
                    false,
                    false,
                );
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSymbolMirrorX(uuid)) => {
                self.apply_engine_command(
                    signex_engine::Command::MirrorSelection {
                        items: vec![signex_types::schematic::SelectedItem::new(
                            *uuid,
                            signex_types::schematic::SelectedKind::Symbol,
                        )],
                        axis: signex_engine::MirrorAxis::Vertical,
                    },
                    true,
                    true,
                );
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSymbolMirrorY(uuid)) => {
                self.apply_engine_command(
                    signex_engine::Command::MirrorSelection {
                        items: vec![signex_types::schematic::SelectedItem::new(
                            *uuid,
                            signex_types::schematic::SelectedKind::Symbol,
                        )],
                        axis: signex_engine::MirrorAxis::Horizontal,
                    },
                    true,
                    true,
                );
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSymbolLocked(uuid)) => {
                let _ = *uuid;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSymbolDnp(uuid)) => {
                let _ = *uuid;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::EditLabelText(uuid, new_text)) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::Label(*uuid),
                        value: new_text.clone(),
                    },
                    false,
                    false,
                );
            }
            crate::dock::DockMessage::Panel(
                crate::panels::PanelMsg::EditTextNoteText(uuid, new_text),
            ) => {
                self.apply_engine_command(
                    signex_engine::Command::UpdateText {
                        target: signex_engine::TextTarget::TextNote(*uuid),
                        value: new_text.clone(),
                    },
                    false,
                    false,
                );
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetPrePlacementText(text)) => {
                if let Some(ref mut pp) = self.document_state.panel_ctx.pre_placement {
                    pp.label_text = text.clone();
                }
            }
            crate::dock::DockMessage::Panel(
                crate::panels::PanelMsg::SetPrePlacementDesignator(text),
            ) => {
                if let Some(ref mut pp) = self.document_state.panel_ctx.pre_placement {
                    pp.designator = text.clone();
                }
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetPrePlacementRotation(rot)) => {
                if let Some(ref mut pp) = self.document_state.panel_ctx.pre_placement {
                    pp.rotation = *rot;
                }
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ConfirmPrePlacement) => {
                self.document_state.panel_ctx.pre_placement = None;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetGridSize(size)) => {
                self.ui_state.grid_size_mm = *size;
                self.document_state.panel_ctx.grid_size_mm = *size;
                self.interaction_state.canvas.snap_grid_mm = *size as f64;
                self.interaction_state.canvas.clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetVisibleGridSize(size)) => {
                self.ui_state.visible_grid_mm = *size;
                self.document_state.panel_ctx.visible_grid_mm = *size;
                self.interaction_state.canvas.visible_grid_mm = *size as f64;
                self.interaction_state.pcb_canvas.visible_grid_mm = *size as f64;
                self.interaction_state.canvas.clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::ToggleSnapHotspots) => {
                self.ui_state.snap_hotspots = !self.ui_state.snap_hotspots;
                self.document_state.panel_ctx.snap_hotspots = self.ui_state.snap_hotspots;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetUiFont(name)) => {
                self.ui_state.ui_font_name = name.clone();
                self.document_state.panel_ctx.ui_font_name = name.clone();
                crate::fonts::write_ui_font_pref(name);
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetCanvasFont(name)) => {
                self.ui_state.canvas_font_name = name.clone();
                self.document_state.panel_ctx.canvas_font_name = name.clone();
                signex_render::set_canvas_font_name(name);
                signex_render::set_canvas_font_style(
                    self.ui_state.canvas_font_bold,
                    self.ui_state.canvas_font_italic,
                );
                self.interaction_state.canvas.clear_content_cache();
                self.interaction_state.canvas.clear_overlay_cache();
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetCanvasFontSize(size)) => {
                self.ui_state.canvas_font_size = *size;
                self.document_state.panel_ctx.canvas_font_size = *size;
                signex_render::set_canvas_font_size(*size);
                self.interaction_state.canvas.clear_content_cache();
                self.interaction_state.canvas.clear_overlay_cache();
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetCanvasFontBold(bold)) => {
                self.ui_state.canvas_font_bold = *bold;
                self.document_state.panel_ctx.canvas_font_bold = *bold;
                signex_render::set_canvas_font_style(
                    self.ui_state.canvas_font_bold,
                    self.ui_state.canvas_font_italic,
                );
                self.interaction_state.canvas.clear_content_cache();
                self.interaction_state.canvas.clear_overlay_cache();
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetCanvasFontItalic(italic)) => {
                self.ui_state.canvas_font_italic = *italic;
                self.document_state.panel_ctx.canvas_font_italic = *italic;
                signex_render::set_canvas_font_style(
                    self.ui_state.canvas_font_bold,
                    self.ui_state.canvas_font_italic,
                );
                self.interaction_state.canvas.clear_content_cache();
                self.interaction_state.canvas.clear_overlay_cache();
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::OpenCanvasFontPopup) => {
                self.document_state.panel_ctx.canvas_font_popup_open = true;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::CloseCanvasFontPopup) => {
                self.document_state.panel_ctx.canvas_font_popup_open = false;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetMarginVertical(zones)) => {
                let _ = zones;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::SetMarginHorizontal(zones)) => {
                let _ = zones;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::DragComponentsSplit) => {
                self.interaction_state.dragging = Some(DragTarget::ComponentsSplit);
                self.interaction_state.drag_start_pos = None;
                self.interaction_state.drag_start_size = self.document_state.panel_ctx.components_split;
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::Tree(TreeMsg::Toggle(path))) => {
                let path = path.clone();
                signex_widgets::tree_view::toggle(&mut self.document_state.panel_ctx.project_tree, &path);
            }
            crate::dock::DockMessage::Panel(crate::panels::PanelMsg::Tree(TreeMsg::Select(path))) => {
                if let Some(node) = get_node(self.document_state.panel_ctx.project_tree.as_slice(), &path)
                    && matches!(node.icon, TreeIcon::Schematic | TreeIcon::Pcb)
                {
                    let filename = node.label.clone();
                    if let Some(dir) = self.document_state.project_path.as_ref().and_then(|p| p.parent()) {
                        let file_path = dir.join(&filename);
                        if file_path.exists() {
                            if let Some(idx) = self.document_state.tabs.iter().position(|t| t.path == file_path) {
                                if idx != self.document_state.active_tab {
                                    self.park_active_schematic_session();
                                    self.document_state.active_tab = idx;
                                    self.sync_active_tab();
                                }
                            } else if filename.ends_with(".kicad_sch")
                                || filename.ends_with(".snxsch")
                            {
                                match kicad_parser::parse_schematic_file(&file_path) {
                                    Ok(sheet) => {
                                        self.open_schematic_tab(
                                            file_path,
                                            filename.replace(".kicad_sch", ""),
                                            sheet,
                                        );
                                    }
                                    Err(e) => eprintln!("Failed to parse {filename}: {e}"),
                                }
                            } else if filename.ends_with(".kicad_pcb")
                                || filename.ends_with(".snxpcb")
                            {
                                match kicad_parser::parse_pcb_file(&file_path) {
                                    Ok(board) => {
                                        let title = filename
                                            .trim_end_matches(".kicad_pcb")
                                            .trim_end_matches(".snxpcb")
                                            .to_string();
                                        self.open_pcb_tab(file_path, title, board);
                                    }
                                    Err(e) => eprintln!("Failed to parse {filename}: {e}"),
                                }
                            } else {
                                eprintln!("Unsupported project tree document: {filename}");
                            }
                        }
                    }
                }
            }
            crate::dock::DockMessage::TabDragStart(..) => {
                self.interaction_state.tab_drag_origin = Some(self.interaction_state.last_mouse_pos);
            }
            crate::dock::DockMessage::FloatingDragEnd(idx) => {
                let idx = *idx;
                if let Some(fp) = self.document_state.dock.floating.get(idx) {
                    let (ww, wh) = self.ui_state.window_size;
                    let zone = 120.0;
                    let cx = fp.x + fp.width / 2.0;
                    let cy = fp.y + fp.height / 4.0;
                    let target = if cx < zone {
                        Some(PanelPosition::Left)
                    } else if cx > ww - zone {
                        Some(PanelPosition::Right)
                    } else if cy > wh - zone {
                        Some(PanelPosition::Bottom)
                    } else {
                        None
                    };
                    eprintln!(
                        "[dock-back] fp=({:.0},{:.0}) win=({ww:.0},{wh:.0}) target={target:?}",
                        fp.x, fp.y
                    );
                    if let Some(pos) = target {
                        self.document_state.dock.update(DockMessage::DockFloatingTo(idx, pos));
                        return Task::none();
                    }
                }
            }
            _ => {}
        }

        self.document_state.dock.update(msg);
        Task::none()
    }
}