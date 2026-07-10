use iced::Subscription;

use crate::canvas::SchematicCanvas;
use crate::dock::{DockArea, PanelPosition};
use crate::keymap::KeyStroke;
use crate::panels::PanelKind;

use super::*;

/// Load the 256×256 PNG bundled by `installer/build-icons.sh` into an
/// [`iced::window::Icon`]. When `has_bundled_icon` isn't set (i.e. the PNG
/// hasn't been generated yet) this returns `None` and the window opens with
/// the platform default icon.
fn bundled_window_icon() -> Option<iced::window::Icon> {
    #[cfg(has_bundled_icon)]
    {
        let bytes: &[u8] = include_bytes!("../../assets/brand/generated/signex-256.png");
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        iced::window::icon::from_rgba(img.into_raw(), w, h).ok()
    }
    #[cfg(not(has_bundled_icon))]
    {
        None
    }
}

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
    /// Default size of the unified Export PDF / Print Preview modal.
    /// Both `view_print_preview` (in-window) and the detached-window
    /// path read from these so resizing the modal happens in one
    /// place instead of two duplicated literals.
    pub(super) const PDF_MODAL_W: f32 = 1180.0;
    pub(super) const PDF_MODAL_H: f32 = 760.0;

    pub fn new() -> (Self, Task<Message>) {
        // Default panel layout — restored from disk if a previous
        // session persisted one, otherwise seeded with the Altium-ish
        // defaults: Projects + Components + Signal on the left,
        // Properties + Messages on the right, ERC on the bottom
        // (user's request — bottom is reserved for future log tails).
        let mut dock = match crate::fonts::read_dock_layout() {
            Some(saved) => saved,
            None => {
                let mut d = DockArea::new();
                d.add_panel(PanelPosition::Left, PanelKind::Projects);
                d.add_panel(PanelPosition::Left, PanelKind::Components);
                d.add_panel(PanelPosition::Left, PanelKind::Library);
                d.add_panel(PanelPosition::Left, PanelKind::Signal);
                d.add_panel(PanelPosition::Right, PanelKind::Properties);
                d.add_panel(PanelPosition::Right, PanelKind::Messages);
                d.add_panel(PanelPosition::Bottom, PanelKind::Erc);
                d
            }
        };
        // Silence unused-mut when read_dock_layout returns Some.
        let _ = &mut dock;

        let sch_canvas = SchematicCanvas::new();
        let pcb_canvas = crate::pcb_canvas::PcbCanvas::new();
        // Default to the 50-mil Altium grid; user-set value overrides
        // through the prefs file (UX §1.5 — last-used grid persists).
        let grid_size_mm =
            crate::fonts::read_grid_size_mm_pref().unwrap_or(crate::canvas::grid::GRID_SIZES_MM[1]);
        let standard_lib_dir = helpers::find_standard_symbols_dir();
        let mut standard_libraries = standard_lib_dir
            .as_deref()
            .map(helpers::list_standard_libraries)
            .unwrap_or_default();
        if !standard_libraries.is_empty() {
            standard_libraries.insert(0, helpers::ALL_LIBRARIES.to_string());
        }

        // Keyboard shortcut profiles: prefer the user's saved set from
        // the OS config dir, else fall back to the bundled Altium /
        // Classic built-ins (which must always parse). The active
        // profile is compiled once here into a fast lookup table.
        let keymap_profiles = crate::keymap::load_profile_set().unwrap_or_else(|_| {
            crate::keymap::ShortcutProfileSet::built_ins()
                .expect("bundled keyboard shortcut profiles must parse")
        });
        let active_keymap = keymap_profiles.compile_active();
        // Working copy for the Preferences ▸ Keyboard Shortcuts pane.
        // Re-seeded from `keymap_profiles` every time the window opens,
        // so this initial clone just keeps the pane renderable before
        // the first open.
        let keymap_editor = crate::keymap::KeymapEditorModel::new(keymap_profiles.clone());

        let mut app = Self {
            ui_state: UiState {
                // Persisted user-toggleable state (UX §1.5). Reads
                // fall back to defaults when the prefs file is absent
                // or the key missing — same as a fresh install.
                theme_id: crate::fonts::read_theme_pref(),
                unit: crate::fonts::read_unit_pref(),
                grid_visible: crate::fonts::read_grid_visible_pref(),
                snap_enabled: crate::fonts::read_snap_enabled_pref(),
                cursor_x: 0.0,
                cursor_y: 0.0,
                zoom: 100.0,
                grid_size_mm,
                visible_grid_mm: 1.27,
                snap_hotspots: true,
                ui_font_name: crate::fonts::read_ui_font_pref(),
                component_classes: crate::fonts::read_component_classes_pref(),
                preferences_draft_component_classes: crate::fonts::read_component_classes_pref(),
                keymap_profiles,
                active_keymap,
                keymap_pending_sequence: Vec::new(),
                canvas_font_name: crate::fonts::DEFAULT_CANVAS_FONT.to_string(),
                canvas_font_size: 11.0,
                canvas_font_bold: false,
                canvas_font_italic: false,
                left_width: 240.0,
                right_width: 220.0,
                bottom_height: 120.0,
                window_size: (1400.0, 900.0),
                main_window_scale: 1.0,
                panel_list_open: false,
                preferences_open: false,
                keyboard_shortcuts_open: false,
                first_run_tour_open: !crate::fonts::read_first_run_tour_dismissed(),
                find_replace: crate::find_replace::FindReplaceState::default(),
                preferences_nav: crate::preferences::PrefNav::Appearance,
                preferences_draft_theme: ThemeId::Signex,
                preferences_draft_font: String::new(),
                power_port_style: crate::fonts::read_power_port_style_pref(),
                preferences_draft_power_port_style: crate::fonts::read_power_port_style_pref(),
                label_style: crate::fonts::read_label_style_pref(),
                preferences_draft_label_style: crate::fonts::read_label_style_pref(),
                multisheet_style: crate::fonts::read_multisheet_style_pref(),
                preferences_draft_multisheet_style: crate::fonts::read_multisheet_style_pref(),
                grid_style: crate::fonts::read_grid_style_pref(),
                preferences_draft_grid_style: crate::fonts::read_grid_style_pref(),
                preferences_draft_symbol_grid_size_mm: crate::fonts::read_symbol_grid_size_mm_pref(
                ),
                preferences_draft_symbol_grid_style: crate::fonts::read_symbol_grid_style_pref(),
                preferences_keymap_editor: keymap_editor,
                preferences_keymap_status: String::new(),
                preferences_keymap_recorder: None,
                preferences_dirty: false,
                custom_theme: None,
                rename_dialog: None,
                remove_dialog: None,
                project_close_confirm: None,
                app_quit_confirm: None,
                project_options: None,
                enable_version_control: None,
                grid_properties: None,
                selection_filter_custom: None,
                erc_violations: Vec::new(),
                erc_violations_by_path: std::collections::HashMap::new(),
                erc_focus_global_index: None,
                project_netlist: None,
                erc_severity_override: crate::fonts::read_erc_severity_overrides(),
                net_colors: std::collections::HashMap::new(),
                auto_focus: false,
                annotate_dialog_open: false,
                annotate_order: crate::app::state::AnnotateOrder::AcrossThenDown,
                erc_dialog_open: false,
                annotate_reset_confirm: false,
                modal_offsets: std::collections::HashMap::new(),
                modal_dragging: None,
                tab_dragging: None,
                main_window_id: None,
                windows: std::collections::HashMap::new(),
                move_selection: crate::app::state::MoveSelectionState::default(),
                net_color_palette_open: false,
                parameter_manager_open: false,
                reorder_picker: None,
                pin_matrix_overrides: crate::fonts::read_pin_matrix_overrides(),
                annotate_locked: std::collections::HashSet::new(),
                selection_mode: crate::schematic_runtime::hit_test::SelectionMode::default(),
                pending_net_color: None,
                wire_color_overrides: std::collections::HashMap::new(),
                lasso_polygon: None,
                net_color_undo: Vec::new(),
                net_color_custom: crate::app::state::NetColorCustomState::default(),
                command_palette: crate::app::command_palette::CommandPaletteState::default(),
            },
            document_state: DocumentState {
                dock,
                tabs: vec![],
                active_tab: 0,
                engines: std::collections::HashMap::new(),
                symbol_editors: std::collections::HashMap::new(),
                footprint_editors: std::collections::HashMap::new(),
                pad_clipboard: None,
                active_path: None,
                projects: Vec::new(),
                active_project: None,
                dirty_paths: std::collections::HashSet::new(),
                next_project_id: 0,
                panel_ctx: crate::panels::PanelContext {
                    projects: Vec::new(),
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
                    theme_id: ThemeId::Signex,
                    unit: Unit::Mm,
                    grid_visible: true,
                    snap_enabled: true,
                    grid_size_mm: 1.27,
                    visible_grid_mm: 1.27,
                    snap_hotspots: true,
                    ui_font_name: crate::fonts::read_ui_font_pref(),
                    component_classes: crate::fonts::read_component_classes_pref(),
                    canvas_font_name: crate::fonts::DEFAULT_CANVAS_FONT.to_string(),
                    canvas_font_size: 11.0,
                    canvas_font_bold: false,
                    canvas_font_italic: false,
                    canvas_font_popup_open: false,

                    properties_tab: 0,
                    standard_libraries,
                    active_library: None,
                    library_symbols: vec![],
                    selected_component: None,
                    selected_pins: vec![],
                    selected_lib_symbol: None,
                    components_split: 250.0,
                    project_tree: vec![],
                    project_tree_selected: None,
                    library_row_detail: None,
                    selection_count: 0,
                    selected_uuid: None,
                    selected_kind: None,
                    selection_info: vec![],
                    drawing_edit_buf: std::collections::HashMap::new(),
                    drawing_edit_buf_for: None,
                    selected_drawing: None,
                    selected_child_sheet: None,
                    child_sheet_border_picker_open: false,
                    child_sheet_fill_picker_open: false,
                    child_sheet_border_advanced_open: false,
                    child_sheet_fill_advanced_open: false,
                    child_sheet_stroke_width_buf: None,
                    component_filter: crate::fonts::read_component_filter(),
                    collapsed_sections: std::collections::HashSet::new(),
                    pre_placement: None,
                    erc_diagnostics: Vec::new(),
                    erc_focus_index: None,
                    diagnostics_level: crate::diagnostics::configured_level_label().to_string(),
                    diagnostics: crate::diagnostics::recent_entries(),
                    selection_filters: crate::active_bar::SelectionFilter::ALL
                        .iter()
                        .copied()
                        .collect(),
                    custom_filter_presets: crate::fonts::read_custom_filter_presets(),
                    active_custom_filter_tab: 0,
                    footprint_filter_presets: crate::fonts::read_footprint_filter_presets(),
                    page_format_mode: crate::panels::PageFormatMode::default(),
                    margin_vertical: 1,
                    margin_horizontal: 1,
                    page_origin: crate::panels::PageOrigin::default(),
                    custom_paper_w_mm: 297.0,
                    custom_paper_h_mm: 210.0,
                    sheet_color: crate::panels::SheetColor::default(),
                    symbol_editor: None,
                    footprint_editor: None,
                    history: crate::panels::history::HistoryPanelState::default(),
                },
                history: crate::panels::history::HistoryPanelState::default(),
                standard_lib_dir,
                loaded_lib: std::collections::HashMap::new(),
                preview: None,
                pending_pdf_options: None,
                pending_pdf_files: None,
                pending_bom_options: None,
                export_error: None,
                bom_preview: None,
                pending_git_commits: Vec::new(),
                inflight_git_commits: std::collections::HashSet::new(),
            },
            interaction_state: InteractionState {
                current_tool: Tool::Select,
                canvas: sch_canvas,
                canvases: std::collections::HashMap::new(),
                pcb_canvas,
                dragging: None,
                drag_start_pos: None,
                drag_start_size: 0.0,
                tab_drag_origin: None,
                undo_stack: crate::undo::UndoStack::new(100),
                wire_points: Vec::new(),
                wire_drawing: false,
                arc_points: Vec::new(),
                polyline_points: Vec::new(),
                shape_anchor: None,
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
                project_tree_context_menu: None,
                grid_picker: None,
                tab_context_menu: None,
                context_submenu: None,
                pending_submenu: None,
                submenu_launcher_hovered: None,
                submenu_panel_hovered: false,
                submenu_unhovered_since: None,
                last_mouse_pos: (0.0, 0.0),
                last_tree_click: None,
                active_bar_menu: None,
                selection_filters: crate::active_bar::SelectionFilter::ALL
                    .iter()
                    .copied()
                    .collect(),
                custom_filter_presets: crate::fonts::read_custom_filter_presets(),
                active_custom_filter_tab: 0,
                footprint_filter_presets: crate::fonts::read_footprint_filter_presets(),
                selection_slots: std::array::from_fn(|_| Vec::new()),
                last_tool: std::collections::HashMap::new(),
                pending_power: None,
                pending_port: None,
                hover_symbol_uuid: None,
                hover_started_at: None,
                hover_screen_pos: None,
            },
            library: crate::library::LibraryState::default(),
        };
        // v0.9 Stage 9: load + mount globally-configured libraries
        // before the first frame so the Components Panel's Global
        // section is populated from the get-go.
        app.library.global_libraries =
            crate::panels::components_panel::global_prefs::load_and_mount_all(&mut app.library);
        crate::render_config::set_canvas_font_name(&app.ui_state.canvas_font_name);
        crate::render_config::set_canvas_font_size(app.ui_state.canvas_font_size);
        crate::render_config::set_canvas_font_style(
            app.ui_state.canvas_font_bold,
            app.ui_state.canvas_font_italic,
        );
        crate::render_config::set_power_port_style(app.ui_state.power_port_style);
        crate::render_config::set_label_style(app.ui_state.label_style);
        crate::render_config::set_multisheet_style(app.ui_state.multisheet_style);
        crate::render_config::set_grid_style(app.ui_state.grid_style);
        crate::render_config::set_symbol_grid_style(
            app.ui_state.preferences_draft_symbol_grid_style,
        );

        // Multi-window (Phase 1): open the main OS window here. Phase 2
        // will open additional windows on demand when the user drags a
        // modal off the main window, and Phase 3 will do the same for
        // undocked tabs. The returned Task produces the settled Id once
        // winit confirms the window is mapped.
        let (main_id, open_task) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(1400.0, 900.0),
            icon: bundled_window_icon(),
            // Borderless main window: the custom chrome in
            // `Signex::view_main_window_chrome` supplies wordmark +
            // menus + drag zone + search bar + min/max/close. Edge
            // resize handles inside `view_main_for` add 6 px strips
            // that call `iced::window::drag_resize` per direction so
            // the user can still drag-resize the window.
            decorations: false,
            ..Default::default()
        });
        app.ui_state.main_window_id = Some(main_id);
        let boot_task = open_task.map(|id| Message::Window(WindowMsg::MainWindowOpened(id)));
        (app, boot_task)
    }

    pub fn title(&self, _id: iced::window::Id) -> String {
        let version = env!("CARGO_PKG_VERSION");
        let dirty_count = self.document_state.dirty_paths.len();
        if dirty_count == 0 {
            format!("Signex {version}")
        } else {
            format!("• Signex {version} — {dirty_count} unsaved")
        }
    }

    pub fn theme(&self, _id: iced::window::Id) -> Option<Theme> {
        Some(self.resolve_theme())
    }

    fn resolve_theme(&self) -> Theme {
        let id = if self.ui_state.preferences_open {
            self.ui_state.preferences_draft_theme
        } else {
            self.ui_state.theme_id
        };
        Self::id_to_iced_theme(id, self.ui_state.custom_theme.as_ref())
    }

    /// Map a ThemeId to an iced::Theme with a properly tuned palette.
    fn id_to_iced_theme(
        id: ThemeId,
        custom: Option<&signex_types::theme::CustomThemeFile>,
    ) -> Theme {
        use crate::render_config::to_iced;
        match id {
            ThemeId::Custom => {
                if let Some(c) = custom {
                    let t = &c.tokens;
                    Theme::custom(
                        c.name.clone(),
                        iced::theme::Palette {
                            background: to_iced(&t.bg),
                            text: to_iced(&t.text),
                            primary: to_iced(&t.accent),
                            success: to_iced(&t.success),
                            danger: to_iced(&t.error),
                            warning: to_iced(&t.warning),
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
                    text: iced::Color::from_rgb(0.831, 0.831, 0.831),
                    primary: iced::Color::from_rgb(0.000, 0.478, 0.800),
                    success: iced::Color::from_rgb(0.416, 0.600, 0.333),
                    danger: iced::Color::from_rgb(0.957, 0.267, 0.278),
                    warning: iced::Color::from_rgb(1.000, 0.549, 0.000),
                },
            ),
            ThemeId::Signex => Theme::custom(
                "Altium Dark".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.18, 0.18, 0.19),
                    text: iced::Color::from_rgb(0.86, 0.86, 0.86),
                    primary: iced::Color::from_rgb(0.91, 0.57, 0.18),
                    success: iced::Color::from_rgb(0.34, 0.65, 0.29),
                    danger: iced::Color::from_rgb(0.96, 0.31, 0.31),
                    warning: iced::Color::from_rgb(0.91, 0.57, 0.18),
                },
            ),
            ThemeId::Alplab => Theme::custom(
                "Alp Lab".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.18, 0.18, 0.19),
                    text: iced::Color::from_rgb(0.86, 0.86, 0.86),
                    // Alp Lab cyan #0891b2 as primary accent.
                    primary: iced::Color::from_rgb(0.031, 0.569, 0.698),
                    success: iced::Color::from_rgb(0.34, 0.65, 0.29),
                    danger: iced::Color::from_rgb(0.96, 0.31, 0.31),
                    warning: iced::Color::from_rgb(0.91, 0.57, 0.18),
                },
            ),
            ThemeId::GitHubDark => Theme::custom(
                "GitHub Dark".to_string(),
                iced::theme::Palette {
                    background: iced::Color::from_rgb(0.051, 0.067, 0.090),
                    text: iced::Color::from_rgb(0.902, 0.929, 0.953),
                    primary: iced::Color::from_rgb(0.345, 0.651, 1.000),
                    success: iced::Color::from_rgb(0.247, 0.725, 0.314),
                    danger: iced::Color::from_rgb(1.000, 0.482, 0.447),
                    warning: iced::Color::from_rgb(0.824, 0.604, 0.133),
                },
            ),
            ThemeId::SolarizedLight => Theme::Light,
            ThemeId::Nord => Theme::Nord,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        use iced::keyboard;

        let kbd = keyboard::listen()
            .with((
                self.ui_state.find_replace.open,
                self.ui_state.command_palette.open,
                self.ui_state.keyboard_shortcuts_open,
                self.ui_state.first_run_tour_open,
                self.ui_state.preferences_open,
                self.ui_state.annotate_dialog_open,
                self.ui_state.erc_dialog_open,
                self.ui_state.rename_dialog.is_some(),
                self.ui_state.remove_dialog.is_some(),
                self.ui_state.enable_version_control.is_some(),
                self.library.create_options.is_some(),
                self.ui_state.preferences_keymap_recorder.is_some(),
            ))
            .map(
                |(
                    (
                        find_replace_open,
                        palette_open,
                        kbd_shortcuts_open,
                        first_run_tour_open,
                        prefs_open,
                        annotate_open,
                        erc_open,
                        rename_open,
                        remove_open,
                        enable_vc_open,
                        library_create_options_open,
                        keymap_recorder_open,
                    ),
                    event,
                )| match event {
                    // Chord recorder open (Preferences ▸ Keyboard
                    // Shortcuts): held modifiers drive the live
                    // "Ctrl+…" hint before a key lands.
                    keyboard::Event::ModifiersChanged(modifiers) if keymap_recorder_open => {
                        Message::Preferences(PreferencesMsg::Inner(
                            crate::preferences::PrefMsg::KeymapRecorderModifiersChanged(
                                crate::keymap::Modifiers::from_iced(modifiers),
                            ),
                        ))
                    }
                    keyboard::Event::KeyPressed {
                        key, modifiers: m, ..
                    } => {
                        // While the recorder is open, every raw stroke
                        // is captured for the binding under edit — it
                        // must NOT reach the live keymap resolver, or
                        // recording a shortcut would also fire it. The
                        // pending chord buffer is left untouched (it is
                        // only advanced by the resolver, which we skip).
                        if keymap_recorder_open {
                            return KeyStroke::from_iced(&key, m)
                                .map(|stroke| {
                                    Message::Preferences(PreferencesMsg::Inner(
                                        crate::preferences::PrefMsg::KeymapRecorderKeyPressed(
                                            stroke,
                                        ),
                                    ))
                                })
                                .unwrap_or(Message::Noop);
                        }
                        // Command palette captures most input while open so
                        // typing into the search field doesn't fire tool
                        // shortcuts (`p`, `w`, `l`, …). Only navigation
                        // and dismiss keys leak through.
                        if palette_open {
                            return match (key.as_ref(), m) {
                                (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                                    Message::CommandPalette(CommandPaletteMsg::Close)
                                }
                                (keyboard::Key::Named(keyboard::key::Named::ArrowDown), _) => {
                                    Message::CommandPalette(CommandPaletteMsg::MoveSelection(1))
                                }
                                (keyboard::Key::Named(keyboard::key::Named::ArrowUp), _) => {
                                    Message::CommandPalette(CommandPaletteMsg::MoveSelection(-1))
                                }
                                // Toggle: Ctrl+Shift+P while open closes.
                                (keyboard::Key::Character(c), m)
                                    if c.eq_ignore_ascii_case("p") && m.command() && m.shift() =>
                                {
                                    Message::CommandPalette(CommandPaletteMsg::Close)
                                }
                                _ => Message::Noop,
                            };
                        }
                        // v0.19 keymap migration: per-key tool / command
                        // shortcuts now come from the active profile (see
                        // `dispatch::keymap`). Only keys that can't be
                        // profile-driven stay hardcoded here: the modal-close
                        // Esc ladder and F1 (they depend on which modal is
                        // open — subscription state, not the profile), and
                        // the Ctrl/Alt+1-8 selection-memory chords (they
                        // carry the digit as data the profile can't express).
                        // Everything else is forwarded to the keymap resolver
                        // in `update`.
                        match (key.as_ref(), m) {
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if find_replace_open =>
                            {
                                Message::FindReplaceMsg(crate::find_replace::FindReplaceMsg::Close)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if kbd_shortcuts_open =>
                            {
                                Message::Overlay(OverlayMsg::CloseKeyboardShortcuts)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if first_run_tour_open =>
                            {
                                Message::Overlay(OverlayMsg::DismissFirstRunTour)
                            }
                            // Esc closes the deepest open modal first (UX §1.3).
                            // The order here goes "user-facing top → bottom":
                            // ERC, then Annotate, then Preferences. Once those
                            // are closed, Esc falls through to the tool reset.
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _) if erc_open => {
                                Message::Erc(ErcMsg::CloseDialog)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if annotate_open =>
                            {
                                Message::Annotate(AnnotateMsg::CloseDialog)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if prefs_open =>
                            {
                                Message::Preferences(PreferencesMsg::Close)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if rename_open =>
                            {
                                Message::Rename(RenameMsg::Close)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if remove_open =>
                            {
                                Message::Remove(RemoveMsg::Close)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if enable_vc_open =>
                            {
                                Message::EnableVersionControl(EnableVersionControlMsg::Close)
                            }
                            // F12 — Library Options modal Esc gap. Without
                            // this, Esc fell through to `Tool::Select`
                            // reset; users hit Create Library out of
                            // frustration thinking that was the only way
                            // out, which actually wrote the .snxlib to disk
                            // (violating the "no disk writes without user
                            // save" invariant when the user hadn't intended
                            // to confirm).
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if library_create_options_open =>
                            {
                                Message::Library(
                                    crate::library::messages::LibraryMessage::LibraryCreateOptionsCancel,
                                )
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                                // v0.15 — route through the
                                // dispatcher so Esc resets the
                                // footprint editor's tool state when
                                // a `.snxfpt` tab is active, and
                                // falls back to the schematic
                                // Tool::Select reset otherwise.
                                Message::EscapePressed
                            }
                            (keyboard::Key::Named(keyboard::key::Named::F1), _) => {
                                // F1 toggles: open if closed, close if open.
                                if kbd_shortcuts_open {
                                    Message::Overlay(OverlayMsg::CloseKeyboardShortcuts)
                                } else {
                                    Message::Menu(MenuMessage::OpenKeyboardShortcuts)
                                }
                            }
                            // Ctrl+1-8 store selection memory, Alt+1-8 recall
                            // selection memory. These carry the digit as data
                            // the profile format can't express, so they stay
                            // hardcoded. The `is_some` guard is load-bearing
                            // (#127): without it this arm matched EVERY
                            // Ctrl/Alt chord and returned Noop, which would
                            // shadow Ctrl+C/X/V/D before they reach the keymap
                            // resolver below.
                            (keyboard::Key::Character(c), m)
                                if m.command() && !m.alt() && selection_slot_from_key(c).is_some() =>
                            {
                                match selection_slot_from_key(c) {
                                    Some(slot) => Message::Selection(
                                        selection_request::SelectionRequest::StoreSlot { slot },
                                    ),
                                    _ => Message::Noop,
                                }
                            }
                            (keyboard::Key::Character(c), m)
                                if m.alt() && !m.command() && selection_slot_from_key(c).is_some() =>
                            {
                                match selection_slot_from_key(c) {
                                    Some(slot) => Message::Selection(
                                        selection_request::SelectionRequest::RecallSlot { slot },
                                    ),
                                    _ => Message::Noop,
                                }
                            }
                            // Everything else routes through the active
                            // keymap: forward the raw stroke, resolved in
                            // `update` where the multi-stroke chord buffer
                            // lives in `UiState` (sound across windows). A
                            // stroke iced can't express as a `KeyStroke`
                            // (e.g. a bare modifier press) is ignored here.
                            _ => KeyStroke::from_iced(&key, m)
                                .map(|stroke| Message::Ui(UiMsg::KeymapStroke(stroke)))
                                .unwrap_or(Message::Noop),
                        }
                    }
                    _ => Message::Noop,
                },
            );

        // Mouse events for drag-to-resize/floating-drag.
        // Subscribing to cursor move only while dragging avoids per-frame
        // app updates when idle, which noticeably hurts smoothness on macOS.
        let drag_active = self.interaction_state.dragging.is_some()
            || self.document_state.dock.tab_drag.is_some()
            || self.ui_state.modal_dragging.is_some()
            || self.ui_state.tab_dragging.is_some()
            || self
                .document_state
                .dock
                .floating
                .iter()
                .any(|fp| fp.dragging);
        let modal_drag_active = self.ui_state.modal_dragging.is_some();
        let mouse_sub = if modal_drag_active {
            // Modal drag takes priority — release ends the modal drag
            // specifically (not the generic DragEnd).
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::Ui(UiMsg::DragMove(position.x, position.y))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Message::Overlay(OverlayMsg::ModalDragEnd),
                // Window::Resized intentionally omitted — the
                // `window::resize_events()` subscription below carries
                // the window id so we can drop non-main resizes. If
                // we also forwarded the raw event here, a detached
                // modal's resize would clobber the main window's size.
                _ => Message::Noop,
            })
        } else if drag_active {
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::Ui(UiMsg::DragMove(position.x, position.y))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Message::Ui(UiMsg::DragEnd),
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) => Message::ContextMenu(ContextMenuMsg::Close),
                // Window::Resized intentionally omitted — the
                // `window::resize_events()` subscription below carries
                // the window id so we can drop non-main resizes. If
                // we also forwarded the raw event here, a detached
                // modal's resize would clobber the main window's size.
                _ => Message::Noop,
            })
        } else {
            // Always track the cursor so `last_mouse_pos` is fresh when the
            // user starts a modal drag — otherwise the first delta is huge
            // and the dialog jumps. DragMove is a no-op when no drag is
            // active (it just updates last_mouse_pos).
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::Ui(UiMsg::DragMove(position.x, position.y))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) => Message::ContextMenu(ContextMenuMsg::Close),
                // Window::Resized intentionally omitted — the
                // `window::resize_events()` subscription below carries
                // the window id so we can drop non-main resizes. If
                // we also forwarded the raw event here, a detached
                // modal's resize would clobber the main window's size.
                _ => Message::Noop,
            })
        };
        // Window-close events from winit: routed so Phase 2/3 can drop
        // detached-modal / undocked-tab entries from ui_state.windows.
        let window_close = iced::window::close_events()
            .map(|id| Message::Window(WindowMsg::SecondaryWindowClosed(id)));
        // OS close requests (native close button, Alt+F4, taskbar close).
        // In daemon mode iced does NOT auto-close on these, so we route
        // them explicitly: the main window goes through the unsaved-
        // changes guard, any other window closes. Without this, an
        // Alt+F4 on a dirty main window would otherwise be silently
        // dropped (or, if iced ever auto-closed, lose unsaved edits).
        let window_close_request = iced::window::close_requests()
            .map(|id| Message::Window(WindowMsg::WindowCloseRequested(id)));
        // Window-resize subscription. `iced::event::listen()`'s
        // Window::Resized event doesn't fire on the very first frame —
        // subscribing to `window::resize_events()` directly gets the
        // initial physical size so dropdowns position correctly without
        // a manual resize.
        // Fire a WindowResizedFor for every OS resize event, carrying
        // the window id so the dispatcher can ignore resizes of
        // detached modal / undocked-tab windows. A plain WindowResized
        // without the id would clobber `ui_state.window_size` with
        // e.g. the 420x240 size of the Move dialog, which then shifts
        // the Active-Bar dropdowns on the main window.
        let window_resize = iced::window::resize_events().map(|(id, size)| {
            Message::Window(WindowMsg::WindowResizedFor(id, size.width, size.height))
        });

        // Hover-open timer for the right-click context-menu submenus.
        // Active while ANY menu that owns submenus is open — canvas
        // right-click, project-tree right-click, or document-tab
        // right-click. The dispatcher checks `pending_submenu`'s
        // elapsed time on each tick.
        let any_menu_open = self.interaction_state.context_menu.is_some()
            || self.interaction_state.project_tree_context_menu.is_some()
            || self.interaction_state.tab_context_menu.is_some();
        let hover_tick = if any_menu_open {
            iced::time::every(std::time::Duration::from_millis(50))
                .map(|_| Message::ContextMenu(ContextMenuMsg::SubmenuTickHover))
        } else {
            Subscription::none()
        };

        // Hover-tooltip wake tick. The tooltip overlay only shows
        // after the cursor has dwelled on a placed symbol for 250 ms
        // — without a periodic re-render, the view layer would never
        // notice the threshold crossing once the cursor stopped
        // moving (no mouse events → no redraw). This fires until the
        // user moves off the symbol; once the tooltip is up, normal
        // CursorMoved events keep it tracking the cursor.
        let symbol_hover_active = self.interaction_state.hover_symbol_uuid.is_some()
            && self
                .interaction_state
                .hover_started_at
                .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(900));
        let hover_tooltip_tick = if symbol_hover_active {
            iced::time::every(std::time::Duration::from_millis(80)).map(|_| Message::Noop)
        } else {
            Subscription::none()
        };

        Subscription::batch([
            kbd,
            mouse_sub,
            window_close,
            window_close_request,
            window_resize,
            hover_tick,
            hover_tooltip_tick,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::selection_slot_from_key;

    #[test]
    fn selection_slot_only_matches_digits_one_through_eight() {
        // Digits 1-8 map to selection-memory slots 0-7.
        for (i, key) in ["1", "2", "3", "4", "5", "6", "7", "8"].iter().enumerate() {
            assert_eq!(selection_slot_from_key(key), Some(i));
        }
        // Regression guard for issue #103: the Ctrl+1-8 / Alt+1-8
        // catch-all arms are gated on `selection_slot_from_key(c).is_some()`.
        // These letters returning None is exactly what lets the
        // Ctrl+C/X/V/D and Shift+Ctrl+V/G arms below fire instead of
        // being shadowed into a no-op.
        for key in ["c", "x", "v", "d", "g", "s", "a", "z", "0", "9"] {
            assert_eq!(
                selection_slot_from_key(key),
                None,
                "{key} must not resolve to a selection slot"
            );
        }
    }
}
