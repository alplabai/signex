use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_tool_message(&mut self, message: ToolMessage) -> Task<Message> {
        match message {
            ToolMessage::PrePlacementTab => {
                // v0.16.1 — footprint-pad placement pause/resume.
                // When the active tab is a footprint editor and the
                // user has armed Pads-mode PlacePad, route TAB to a
                // dedicated toggle that gates the canvas's empty-
                // canvas click. This sits BEFORE the schematic flow
                // so it doesn't fight with `current_tool` (which
                // tracks schematic tools, not footprint pads).
                if let Some(active_tab) =
                    self.document_state.tabs.get(self.document_state.active_tab)
                    && let Some(path) = active_tab.kind.as_footprint_editor()
                {
                    let path = path.clone();
                    return self.update(crate::app::contracts::Message::Library(
                        crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                            path,
                            msg: crate::library::messages::PrimitiveEdit::Footprint(
                                crate::library::messages::FootprintEditorMsg::TogglePlacementPause,
                            ),
                        },
                    ));
                }
                // Pasted content has no pre-placement properties: the Tab form
                // belongs to a part on the cursor and the legacy tools.
                if self
                    .interaction_state
                    .placement
                    .as_ref()
                    .is_some_and(|s| s.kind.is_paste())
                {
                    return self.finish_update();
                }
                if self.interaction_state.current_tool != Tool::Select {
                    use crate::panels::PrePlacementKind;
                    let session_form = self.placement_form();
                    use signex_types::schematic::LabelType;
                    // Figure out the exact placement flavor so the form
                    // shows fields relevant to what's being dropped. A
                    // Tool::Label with `pending_port` = Global is NOT the
                    // same as a plain net label; a Tool::Component with
                    // `pending_power` is a power port, not a regular part.
                    let (kind, tool_name, default_label_text, default_designator) =
                        match self.interaction_state.current_tool {
                            Tool::Wire => (
                                PrePlacementKind::Wire,
                                "Wire".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::Bus => (
                                PrePlacementKind::Bus,
                                "Bus".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::BusEntry => (
                                PrePlacementKind::BusEntry,
                                "Bus Entry".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::NoConnect => (
                                PrePlacementKind::NoConnect,
                                "No Connect".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::Junction => (
                                PrePlacementKind::Other,
                                "Junction".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::NoErcDirective => (
                                PrePlacementKind::Other,
                                "No ERC".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::Text => (
                                PrePlacementKind::TextNote,
                                "Text".to_string(),
                                "Text".to_string(),
                                String::new(),
                            ),
                            Tool::Component => {
                                // Power port — armed via Active Bar — lives under
                                // Tool::Component but has `pending_power` set.
                                if let Some((net, _)) =
                                    self.interaction_state.pending_power.as_ref().cloned()
                                {
                                    (
                                        PrePlacementKind::PowerPort,
                                        format!("Power Port ({})", net),
                                        net,
                                        String::new(),
                                    )
                                } else {
                                    // A part on the cursor is the truth for its
                                    // own form.
                                    let (label, designator) = session_form
                                        .as_ref()
                                        .map(|f| (f.value.clone(), f.designator.clone()))
                                        .unwrap_or_else(|| ("NET".to_string(), String::new()));
                                    (
                                        PrePlacementKind::Component,
                                        "Component".to_string(),
                                        label,
                                        designator,
                                    )
                                }
                            }
                            Tool::Label => match self.interaction_state.pending_port.as_ref() {
                                Some((LabelType::Global, _)) => (
                                    PrePlacementKind::GlobalPort,
                                    "Global Port".to_string(),
                                    "PORT".to_string(),
                                    String::new(),
                                ),
                                Some((LabelType::Hierarchical, _)) => (
                                    PrePlacementKind::HierPort,
                                    "Hierarchical Port".to_string(),
                                    "SHEET".to_string(),
                                    String::new(),
                                ),
                                _ => (
                                    PrePlacementKind::NetLabel,
                                    "Net Label".to_string(),
                                    "NET".to_string(),
                                    String::new(),
                                ),
                            },
                            Tool::Line => (
                                PrePlacementKind::Line,
                                "Line".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::Rectangle => (
                                PrePlacementKind::Rectangle,
                                "Rectangle".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::Circle => (
                                PrePlacementKind::Circle,
                                "Full Circle".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::Arc => (
                                PrePlacementKind::Arc,
                                "Arc".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            Tool::Polyline => (
                                PrePlacementKind::Polygon,
                                "Polygon".to_string(),
                                String::new(),
                                String::new(),
                            ),
                            _ => (
                                PrePlacementKind::Other,
                                format!("{}", self.interaction_state.current_tool),
                                String::new(),
                                String::new(),
                            ),
                        };
                    // The session's own values win over a form left from an
                    // earlier Tab: Space / X / Y and the form edits all live
                    // in the session.
                    let on_cursor_part = session_form
                        .as_ref()
                        .filter(|_| matches!(kind, PrePlacementKind::Component));
                    let label_text = on_cursor_part
                        .map(|f| f.value.clone())
                        .or_else(|| {
                            self.document_state
                                .panel_ctx
                                .pre_placement
                                .as_ref()
                                .map(|pp| pp.label_text.clone())
                                .filter(|text| !text.is_empty())
                        })
                        .unwrap_or(default_label_text);
                    let designator = on_cursor_part
                        .map(|f| f.designator.clone())
                        .or_else(|| {
                            self.document_state
                                .panel_ctx
                                .pre_placement
                                .as_ref()
                                .map(|pp| pp.designator.clone())
                                .filter(|value| !value.is_empty())
                        })
                        .unwrap_or(default_designator);
                    let defaults = crate::panels::PrePlacementData::new(
                        String::new(),
                        PrePlacementKind::Other,
                        String::new(),
                        0.0,
                        0.0,
                    );
                    let prev = self.document_state.panel_ctx.pre_placement.as_ref();
                    let prev_rotation = on_cursor_part
                        .map(|f| f.rotation)
                        .or_else(|| prev.map(|pp| pp.rotation))
                        .unwrap_or(defaults.rotation);
                    let prev_font = prev.map_or(defaults.font, |pp| pp.font.clone());
                    let prev_font_size = prev.map_or(defaults.font_size_pt, |pp| pp.font_size_pt);
                    let prev_jh = prev.map_or(defaults.justify_h, |pp| pp.justify_h);
                    let prev_jv = prev.map_or(defaults.justify_v, |pp| pp.justify_v);
                    let prev_shape_width =
                        prev.map_or(defaults.shape_width_mm, |pp| pp.shape_width_mm);
                    let prev_shape_fill = prev.map_or(defaults.shape_fill, |pp| pp.shape_fill);
                    self.document_state.panel_ctx.pre_placement =
                        Some(crate::panels::PrePlacementData {
                            tool_name,
                            kind,
                            label_text,
                            designator,
                            rotation: prev_rotation,
                            font: prev_font,
                            font_size_pt: prev_font_size,
                            justify_h: prev_jh,
                            justify_v: prev_jv,
                            bold: false,
                            italic: false,
                            underline: false,
                            cursor_x_mm: self.ui_state.cursor_x,
                            cursor_y_mm: self.ui_state.cursor_y,
                            shape_width_mm: prev_shape_width,
                            shape_fill: prev_shape_fill,
                        });
                    // TAB opens the pre-placement form in Properties, so
                    // the panel has to end up in front. `add_panel`
                    // returned early whenever Properties was already
                    // docked behind another tab, and the form the user
                    // just asked for stayed hidden (#641).
                    let show_properties = self.show_panel(crate::panels::PanelKind::Properties);
                    // Freeze ghost + suppress canvas clicks until OK. TAB
                    // does NOT place the object — it pauses placement and
                    // the pre-placement form edits the properties used by
                    // the next click. Placement resumes when the user hits
                    // Resume or Enter.
                    self.interaction_state.active_canvas_mut().placement_paused = true;
                    return Task::batch([show_properties, self.finish_update()]);
                }
                self.finish_update()
            }
            ToolMessage::ResumePlacement => {
                // Big on-canvas "Resume" button (Altium-style pause overlay).
                // Keep `pre_placement` alive so the next click consumes the
                // values the user just edited — only un-pause the canvas and
                // close any transient selection the dock picked up.
                self.interaction_state.active_canvas_mut().placement_paused = false;
                self.interaction_state.active_canvas_mut().selected.clear();
                // The overlay layer draws the selection highlight and now
                // also the ERC markers; without a rebuild the old highlight
                // stays painted beside the markers (#1403).
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
                self.update_selection_info();
                // v0.13 — also resume the footprint editor's placement
                // pause when the active tab is a footprint editor.
                let resume_task = if let Some(active_tab) =
                    self.document_state.tabs.get(self.document_state.active_tab)
                    && let Some(path) = active_tab.kind.as_footprint_editor()
                {
                    let path = path.clone();
                    self.update(crate::app::contracts::Message::Library(
                        crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                            path,
                            msg: crate::library::messages::PrimitiveEdit::Footprint(
                                crate::library::messages::FootprintEditorMsg::TogglePlacementPause,
                            ),
                        },
                    ))
                } else {
                    Task::none()
                };
                Task::batch([resume_task, self.finish_update()])
            }
            ToolMessage::CycleDrawMode => {
                self.interaction_state.draw_mode = self.interaction_state.draw_mode.next();
                self.finish_update()
            }
            ToolMessage::RemoveLastVertex => {
                // Segments commit per click, so the only uncommitted vertex
                // is the wire's start point: popping it ends the draw but
                // keeps the Wire tool armed. Outside a draw it is a delete.
                if self.interaction_state.wire_drawing {
                    self.interaction_state.wire_drawing = false;
                    self.interaction_state.wire_points.clear();
                    let canvas = self.interaction_state.active_canvas_mut();
                    canvas.wire_preview.clear();
                    canvas.drawing_mode = false;
                    return self.finish_update();
                }
                // A polyline buffers every click until Enter / double-click,
                // so Backspace drops the last buffered point.
                if self.interaction_state.polyline_points.pop().is_some() {
                    let points = self.interaction_state.polyline_points.clone();
                    let canvas = self.interaction_state.active_canvas_mut();
                    canvas.polyline_points = points;
                    canvas.clear_overlay_cache();
                    return self.finish_update();
                }
                self.update(Message::Edit(EditMsg::DeleteSelected))
            }
            ToolMessage::CancelDrawing => {
                if self.interaction_state.wire_drawing {
                    self.interaction_state.wire_drawing = false;
                    self.interaction_state.wire_points.clear();
                    self.interaction_state
                        .active_canvas_mut()
                        .wire_preview
                        .clear();
                    self.interaction_state.active_canvas_mut().drawing_mode = false;
                    self.interaction_state.current_tool = Tool::Select;
                    self.interaction_state.active_canvas_mut().tool_preview = None;
                } else if self.interaction_state.current_tool == Tool::Sheet {
                    // Drops the half-placed corner and the ghost, back to Select.
                    return self.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
                }
                self.finish_update()
            }
            ToolMessage::SelectTool(tool) => {
                // Any tool pick ends a part on the cursor and cancels a
                // library read still in flight.
                self.drop_placement_session();
                // An armed Move belongs to the Select tool: any tool pick
                // (re-selecting Select included) cancels it and any float.
                self.cancel_move_pick();
                // Buffers belong to the tool that started them (#102).
                // Re-selecting the active tool mid-gesture keeps the gesture
                // on purpose: only a switch to a different tool drops the buffers.
                if tool != self.interaction_state.current_tool {
                    self.clear_drawing_buffers();
                }
                self.interaction_state.current_tool = tool;
                // Re-label the floating cursor tag so the user sees which
                // placement mode they're in. Altium shows the tool name near
                // the crosshair until the first click.
                self.interaction_state.active_canvas_mut().tool_preview = match tool {
                    Tool::Wire => Some("Wire".to_string()),
                    Tool::Bus => Some("Bus".to_string()),
                    Tool::BusEntry => Some("Bus Entry".to_string()),
                    Tool::Text => Some("Text".to_string()),
                    Tool::NoConnect => Some("No Connect".to_string()),
                    Tool::Junction => Some("Junction".to_string()),
                    Tool::NoErcDirective => Some("No ERC".to_string()),
                    Tool::Label => Some("Label".to_string()),
                    Tool::Component => Some("Component".to_string()),
                    Tool::Line => Some("Line".to_string()),
                    Tool::Rectangle => Some("Rectangle".to_string()),
                    Tool::Circle => Some("Circle".to_string()),
                    Tool::Sheet => Some("Sheet".to_string()),
                    Tool::SheetEntry => Some("Sheet Entry".to_string()),
                    Tool::Measure => Some("Measure".to_string()),
                    Tool::BreakWire => Some("Break Wire".to_string()),
                    _ => None,
                };
                // Clear every cursor-following ghost so a leftover from a
                // previous tool can't linger on the canvas. Placement
                // presets (Label/Power port/etc.) re-arm their specific
                // ghost after the dispatch.
                self.clear_ghost_previews();
                // A port the previous pick armed is not this tool's: a
                // placement that wants one sets it after this arm (#1920).
                self.interaction_state.pending_port = None;
                // Arm a default ghost text-note preview for the Text tool so
                // the user sees a sample "Text" glyph at the cursor. Other
                // tools with ghosts arm them from their placement preset.
                if tool == Tool::Text {
                    self.interaction_state.active_canvas_mut().ghost_text =
                        Some(signex_types::schematic::TextNote {
                            uuid: uuid::Uuid::now_v7(),
                            text: "Text".to_string(),
                            position: signex_types::schematic::Point::new(0.0, 0.0),
                            rotation: 0.0,
                            font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
                            justify_h: signex_types::schematic::HAlign::Left,
                            justify_v: signex_types::schematic::VAlign::default(),
                            font_family: None,
                            locked: false,
                        });
                }
                if tool == Tool::BusEntry {
                    self.interaction_state.active_canvas_mut().ghost_bus_entry =
                        Some(self.interaction_state.bus_entry_size);
                }
                if tool == Tool::Select {
                    self.clear_transient_schematic_tool_state();
                }
                self.finish_update()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::contracts::Message;

    /// #1403 - resuming placement drops the selection, so the overlay layer
    /// that paints the highlight must be rebuilt rather than left stale.
    #[test]
    fn resume_placement_rebuilds_the_overlay() {
        let (mut app, _t) = Signex::new();
        let id = signex_types::schematic::SelectedItem::new(
            uuid::Uuid::from_u128(7),
            signex_types::schematic::SelectedKind::Wire,
        );
        app.interaction_state.active_canvas_mut().selected = vec![id];
        let before = app.interaction_state.active_canvas().overlay_cache_clears;

        let _ = app.update(Message::Tool(ToolMessage::ResumePlacement));

        let canvas = app.interaction_state.active_canvas();
        assert!(canvas.selected.is_empty());
        assert!(canvas.overlay_cache_clears > before, "stale overlay left");
    }

    /// A schematic with one empty sheet, as the canvas click handlers need.
    fn app_with_empty_sheet() -> Signex {
        let path = std::path::PathBuf::from("tool-pick.snxsch");
        let sheet = signex_types::schematic::SchematicSheet {
            paper_size: "A4".to_string(),
            ..Default::default()
        };
        let (mut app, _task) = Signex::new();
        app.ui_state.first_run_tour_open = false;
        app.document_state.engines.insert(
            path.clone(),
            signex_engine::Engine::new(sheet).expect("engine"),
        );
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "tool-pick".to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = 0;
        app.document_state.active_path = Some(path);
        app
    }

    /// The label types the sheet holds after one click at (10, 10).
    fn label_types_after_click(app: &mut Signex) -> Vec<signex_types::schematic::LabelType> {
        let _ = app.update(Message::CanvasEvent(crate::app::CanvasEvent::Clicked {
            world_x: 10.0,
            world_y: 10.0,
        }));
        app.document_state
            .active_engine()
            .expect("engine")
            .document()
            .labels
            .iter()
            .map(|label| label.label_type)
            .collect()
    }

    /// #1920 - a Port (Global) armed from the Active Bar, then Place Net
    /// Label from the palette, the keyboard or the Place menu, must place a
    /// net label: the port the first pick left behind is not the new tool's.
    #[test]
    fn a_label_tool_pick_after_a_port_places_a_net_label() {
        use signex_types::schematic::LabelType;
        let arm_port = |app: &mut Signex| {
            let _ = app.update(Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
                crate::active_bar::ActiveBarAction::PlacePort,
            )));
            assert!(app.interaction_state.pending_port.is_some(), "port armed");
        };

        let mut by_command = app_with_empty_sheet();
        arm_port(&mut by_command);
        let id = crate::keymap::AppCommandId::new("place_net_label").unwrap();
        let _ = by_command.dispatch_command(&id, crate::app::command::CommandArgs::none(), None);
        assert_eq!(label_types_after_click(&mut by_command), [LabelType::Net]);

        let mut by_menu = app_with_empty_sheet();
        arm_port(&mut by_menu);
        let _ = by_menu.update(Message::Menu(crate::menu_bar::MenuMessage::command(
            "place_net_label",
        )));
        assert_eq!(label_types_after_click(&mut by_menu), [LabelType::Net]);
    }

    /// The `SelectTool` arm itself drops a port the previous pick armed
    /// (#1920), independent of any caller that also clears it.
    #[test]
    fn selecting_a_tool_drops_a_pending_port() {
        let mut app = app_with_empty_sheet();
        let _ = app.update(Message::ActiveBar(crate::active_bar::ActiveBarMsg::Action(
            crate::active_bar::ActiveBarAction::PlacePort,
        )));
        assert!(app.interaction_state.pending_port.is_some(), "port armed");
        let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
        assert!(app.interaction_state.pending_port.is_none());
    }
}
