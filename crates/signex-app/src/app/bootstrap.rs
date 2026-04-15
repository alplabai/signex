use super::*;

fn selection_slot_from_key(key: &str) -> Option<usize> {
    match key {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        "4" => Some(3),
        "5" => Some(4),
        "6" => Some(5),
        "7" => Some(6),
        "8" => Some(7),
        _ => None,
    }
}

impl Signex {
    pub(super) const CONTEXT_MENU_WIDTH: f32 = 248.0;

    pub fn new() -> (Self, Task<Message>) {
        let mut dock = DockArea::new();
        dock.add_panel(PanelPosition::Left, PanelKind::Projects);
        dock.add_panel(PanelPosition::Left, PanelKind::Components);
        dock.add_panel(PanelPosition::Right, PanelKind::Properties);
        dock.add_panel(PanelPosition::Bottom, PanelKind::Messages);
        dock.add_panel(PanelPosition::Bottom, PanelKind::Signal);

        let sch_canvas = SchematicCanvas::new();
        let pcb_canvas = crate::pcb_canvas::PcbCanvas::new();
        let grid_size_mm = crate::canvas::grid::GRID_SIZES_MM[2]; // 2.54mm
        let kicad_lib_dir = helpers::find_kicad_symbols_dir();
        let kicad_libraries = kicad_lib_dir
            .as_deref()
            .map(helpers::list_kicad_libraries)
            .unwrap_or_default();

        let app = Self {
            theme_id: ThemeId::Signex,
            unit: Unit::Mm,
            grid_visible: true,
            snap_enabled: true,
            cursor_x: 0.0,
            cursor_y: 0.0,
            zoom: 100.0,
            dock,
            tabs: vec![],
            active_tab: 0,
            current_tool: Tool::Select,
            canvas: sch_canvas,
            pcb_canvas,
            grid_size_mm,
            visible_grid_mm: 2.54,
            snap_hotspots: true,
            ui_font_name: crate::fonts::read_ui_font_pref(),
            canvas_font_name: crate::fonts::DEFAULT_CANVAS_FONT.to_string(),
            canvas_font_size: 11.0,
            canvas_font_bold: false,
            canvas_font_italic: false,
            engine: None,
            project_path: None,
            project_data: None,
            panel_ctx: crate::panels::PanelContext {
                project_name: None,
                project_file: None,
                pcb_file: None,
                sheets: vec![],
                sym_count: 0,
                wire_count: 0,
                label_count: 0,
                junction_count: 0,
                child_sheets: vec![],
                has_schematic: false,
                has_pcb: false,
                paper_size: "A4".to_string(),
                lib_symbol_count: 0,
                lib_symbol_names: vec![],
                placed_symbols: vec![],
                tokens: signex_types::theme::theme_tokens(ThemeId::Signex),
                unit: Unit::Mm,
                grid_visible: true,
                snap_enabled: true,
                grid_size_mm: 2.54,
                visible_grid_mm: 2.54,
                snap_hotspots: true,
                ui_font_name: crate::fonts::read_ui_font_pref(),
                canvas_font_name: crate::fonts::DEFAULT_CANVAS_FONT.to_string(),
                canvas_font_size: 11.0,
                canvas_font_bold: false,
                canvas_font_italic: false,
                canvas_font_popup_open: false,
                properties_tab: 0,
                kicad_libraries,
                active_library: None,
                library_symbols: vec![],
                selected_component: None,
                selected_pins: vec![],
                selected_lib_symbol: None,
                components_split: 250.0,
                project_tree: vec![],
                selection_count: 0,
                selected_uuid: None,
                selected_kind: None,
                selection_info: vec![],
                component_filter: String::new(),
                collapsed_sections: std::collections::HashSet::new(),
                pre_placement: None,
            },
            left_width: 240.0,
            right_width: 220.0,
            bottom_height: 120.0,
            // active_menu removed — iced_aw MenuBar manages overlay state
            kicad_lib_dir,
            loaded_lib: std::collections::HashMap::new(),
            dragging: None,
            drag_start_pos: None,
            drag_start_size: 0.0,
            tab_drag_origin: None,
            window_size: (1400.0, 900.0),
            undo_stack: crate::undo::UndoStack::new(100),
            wire_points: Vec::new(),
            wire_drawing: false,
            clipboard_wires: Vec::new(),
            clipboard_buses: Vec::new(),
            clipboard_labels: Vec::new(),
            clipboard_symbols: Vec::new(),
            clipboard_junctions: Vec::new(),
            clipboard_no_connects: Vec::new(),
            clipboard_text_notes: Vec::new(),
            draw_mode: DrawMode::default(),
            editing_text: None,
            context_menu: None,
            last_mouse_pos: (0.0, 0.0),
            active_bar_menu: None,
            selection_filters: crate::active_bar::SelectionFilter::ALL.iter().copied().collect(),
            selection_slots: std::array::from_fn(|_| Vec::new()),
            last_tool: std::collections::HashMap::new(),
            pending_power: None,
            pending_port: None,
            panel_list_open: false,
            preferences_open: false,
            find_replace: crate::find_replace::FindReplaceState::default(),
            preferences_nav: crate::preferences::PrefNav::Appearance,
            preferences_draft_theme: ThemeId::Signex,
            preferences_draft_font: String::new(),
            power_port_style: crate::fonts::read_power_port_style_pref(),
            preferences_draft_power_port_style: crate::fonts::read_power_port_style_pref(),
            preferences_dirty: false,
            custom_theme: None,
        };
        signex_render::set_canvas_font_name(&app.canvas_font_name);
        signex_render::set_canvas_font_size(app.canvas_font_size);
        signex_render::set_canvas_font_style(app.canvas_font_bold, app.canvas_font_italic);
        signex_render::set_power_port_style(app.power_port_style);
        (app, Task::none())
    }

    pub fn title(&self) -> String {
        "Signex".to_string()
    }

    pub fn theme(&self) -> Theme {
        // While Preferences dialog is open, live-preview the draft theme.
        let id = if self.preferences_open {
            self.preferences_draft_theme
        } else {
            self.theme_id
        };
        Self::id_to_iced_theme(id, self.custom_theme.as_ref())
    }

    /// Map a ThemeId to an iced::Theme with a properly tuned palette.
    fn id_to_iced_theme(
        id: ThemeId,
        custom: Option<&signex_types::theme::CustomThemeFile>,
    ) -> Theme {
        use signex_render::colors::to_iced;
        match id {
            ThemeId::Custom => {
                if let Some(c) = custom {
                    let t = &c.tokens;
                    Theme::custom(
                        c.name.clone(),
                        iced::theme::Palette {
                            background: to_iced(&t.bg),
                            text:       to_iced(&t.text),
                            primary:    to_iced(&t.accent),
                            success:    to_iced(&t.success),
                            danger:     to_iced(&t.error),
                            warning:    to_iced(&t.warning),
                        },
                    )
                } else {
                    Theme::CatppuccinMocha
                }
            }
            ThemeId::CatppuccinMocha => Theme::CatppuccinMocha,
            ThemeId::VsCodeDark => Theme::custom(
                "VS Code Dark".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.118, 0.118, 0.118),
                    text:       iced::Color::from_rgb(0.831, 0.831, 0.831),
                    primary:    iced::Color::from_rgb(0.000, 0.478, 0.800),
                    success:    iced::Color::from_rgb(0.416, 0.600, 0.333),
                    danger:     iced::Color::from_rgb(0.957, 0.267, 0.278),
                    warning:    iced::Color::from_rgb(1.000, 0.549, 0.000),
                },
            ),
            ThemeId::Signex => Theme::custom(
                "Altium Dark".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.18, 0.18, 0.19),
                    text:       iced::Color::from_rgb(0.86, 0.86, 0.86),
                    primary:    iced::Color::from_rgb(0.91, 0.57, 0.18),
                    success:    iced::Color::from_rgb(0.34, 0.65, 0.29),
                    danger:     iced::Color::from_rgb(0.96, 0.31, 0.31),
                    warning:    iced::Color::from_rgb(0.91, 0.57, 0.18),
                },
            ),
            ThemeId::GitHubDark => Theme::custom(
                "GitHub Dark".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.051, 0.067, 0.090),
                    text:       iced::Color::from_rgb(0.902, 0.929, 0.953),
                    primary:    iced::Color::from_rgb(0.345, 0.651, 1.000),
                    success:    iced::Color::from_rgb(0.247, 0.725, 0.314),
                    danger:     iced::Color::from_rgb(1.000, 0.482, 0.447),
                    warning:    iced::Color::from_rgb(0.824, 0.604, 0.133),
                },
            ),
            ThemeId::SolarizedLight => Theme::Light,
            ThemeId::Nord => Theme::Nord,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        use iced::keyboard;

        let find_replace_open = self.find_replace.open;
        let kbd = keyboard::listen().map(move |event| match event {
            keyboard::Event::KeyPressed {
                key, modifiers: m, ..
            } => match (key.as_ref(), m) {
                (keyboard::Key::Character(c), m) if c == "q" && m.command() => Message::UnitCycled,
                (keyboard::Key::Character(c), m) if c == "g" && !m.command() && !m.shift() => {
                    Message::GridCycle
                }
                (keyboard::Key::Character(c), m) if c == "w" && !m.command() => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Wire))
                }
                (keyboard::Key::Character(c), m) if c == "b" && !m.command() => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Bus))
                }
                (keyboard::Key::Character(c), m) if c == "l" && !m.command() => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Label))
                }
                (keyboard::Key::Character(c), m) if c == "p" && !m.command() => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Component))
                }
                // Ctrl+, open Preferences
                (keyboard::Key::Character(c), m) if c == "," && m.command() => {
                    Message::OpenPreferences
                }
                (keyboard::Key::Character(c), m) if c == "f" && m.command() => Message::OpenFind,
                (keyboard::Key::Character(c), m) if c == "h" && m.command() => Message::OpenReplace,
                (keyboard::Key::Named(keyboard::key::Named::Escape), _) if find_replace_open => {
                    Message::FindReplaceMsg(crate::find_replace::FindReplaceMsg::Close)
                }
                (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Select))
                }
                (keyboard::Key::Named(keyboard::key::Named::Home), _) => {
                    Message::CanvasEvent(CanvasEvent::FitAll)
                }
                // Delete selected
                (keyboard::Key::Named(keyboard::key::Named::Delete), _) => Message::DeleteSelected,
                // Undo/Redo
                (keyboard::Key::Character(c), m) if c == "z" && m.command() && !m.shift() => {
                    Message::Undo
                }
                (keyboard::Key::Character(c), m) if c == "y" && m.command() => Message::Redo,
                (keyboard::Key::Character(c), m) if c == "z" && m.command() && m.shift() => {
                    Message::Redo
                }
                // Shift+Space: cycle draw mode (90-degree -> 45-degree -> Free)
                (keyboard::Key::Named(keyboard::key::Named::Space), m) if m.shift() => {
                    Message::CycleDrawMode
                }
                // Space: rotate selected symbol (Altium convention)
                (keyboard::Key::Named(keyboard::key::Named::Space), _) => Message::RotateSelected,
                // Mirror: X key = horizontal flip (left-right) = KiCad mirror_y
                //         Y key = vertical flip (top-bottom) = KiCad mirror_x
                (keyboard::Key::Character(c), m) if c == "x" && !m.command() => {
                    Message::MirrorSelectedY // X key = horizontal flip = toggle mirror_y
                }
                (keyboard::Key::Character(c), m) if c == "y" && !m.command() => {
                    Message::MirrorSelectedX // Y key = vertical flip = toggle mirror_x
                }
                // Ctrl+S save
                (keyboard::Key::Character(c), m) if c == "s" && m.command() => Message::SaveFile,
                // Ctrl+A select all
                (keyboard::Key::Character(c), m) if c == "a" && m.command() => {
                    Message::Selection(selection_message::SelectionMessage::SelectAll)
                }
                // Ctrl+M measure tool
                (keyboard::Key::Character(c), m) if c == "m" && m.command() => {
                    Message::Tool(ToolMessage::SelectTool(Tool::Measure))
                }
                // Ctrl+1-8 store selection memory, Alt+1-8 recall selection memory
                (keyboard::Key::Character(c), m)
                    if m.command() && !m.alt() =>
                {
                    match selection_slot_from_key(c.as_ref()) {
                        Some(slot) => {
                            Message::Selection(selection_message::SelectionMessage::StoreSlot {
                                slot,
                            })
                        }
                        _ => Message::Noop,
                    }
                }
                (keyboard::Key::Character(c), m)
                    if m.alt() && !m.command() =>
                {
                    match selection_slot_from_key(c.as_ref()) {
                        Some(slot) => {
                            Message::Selection(selection_message::SelectionMessage::RecallSlot {
                                slot,
                            })
                        }
                        _ => Message::Noop,
                    }
                }
                // Ctrl+C copy, Ctrl+X cut
                (keyboard::Key::Character(c), m) if c == "c" && m.command() => Message::Copy,
                (keyboard::Key::Character(c), m) if c == "x" && m.command() => Message::Cut,
                // Shift+Ctrl+V smart paste
                (keyboard::Key::Character(c), m) if c == "v" && m.command() && m.shift() => {
                    Message::SmartPaste
                }
                // Ctrl+V paste
                (keyboard::Key::Character(c), m) if c == "v" && m.command() => Message::Paste,
                // Ctrl+D duplicate
                (keyboard::Key::Character(c), m) if c == "d" && m.command() => {
                    Message::Duplicate
                }
                // Shift+Ctrl+G -- toggle grid visibility
                (keyboard::Key::Character(c), m) if c == "g" && m.command() && m.shift() => {
                    Message::GridToggle
                }
                // Tab -- pre-placement properties (only during active tool)
                (keyboard::Key::Named(keyboard::key::Named::Tab), _) => {
                    Message::PrePlacementTab
                }
                _ => Message::Noop,
            },
            _ => Message::Noop,
        });

        // Mouse events for drag-to-resize/floating-drag.
        // Subscribing to cursor move only while dragging avoids per-frame
        // app updates when idle, which noticeably hurts smoothness on macOS.
        let drag_active = self.dragging.is_some()
            || self.dock.tab_drag.is_some()
            || self.dock.floating.iter().any(|fp| fp.dragging);
        let mouse_sub = if drag_active {
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::DragMove(position.x, position.y)
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                    Message::DragEnd
                }
                // Any click dismisses context menu
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                    Message::CloseContextMenu
                }
                iced::Event::Window(iced::window::Event::Resized(size)) => {
                    Message::WindowResized(size.width, size.height)
                }
                _ => Message::Noop,
            })
        } else {
            iced::event::listen().map(|event| match event {
                // Any click dismisses context menu
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                    Message::CloseContextMenu
                }
                iced::Event::Window(iced::window::Event::Resized(size)) => {
                    Message::WindowResized(size.width, size.height)
                }
                _ => Message::Noop,
            })
        };
        Subscription::batch([kbd, mouse_sub])
    }
}
