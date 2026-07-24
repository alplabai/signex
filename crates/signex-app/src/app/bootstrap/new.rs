//! `Signex::new` — initial application state + boot Task. Split from
//! `app/bootstrap.rs` as pure code motion.

use super::super::*;

use crate::canvas::SchematicCanvas;
use crate::dock::{DockArea, PanelPosition};
use crate::panels::PanelKind;

impl Signex {
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
                preferences_draft_symbol_pin_selection:
                    crate::fonts::read_symbol_pin_selection_pref(),
                preferences_keymap_editor: keymap_editor,
                preferences_keymap_status: String::new(),
                preferences_keymap_search: String::new(),
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
                netlist_incomplete_prompt: None,
                bom_preview: None,
                pending_git_commits: Vec::new(),
                inflight_git_commits: std::collections::HashSet::new(),
                pending_opens: std::collections::HashSet::new(),
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
            icon: super::bundled_window_icon(),
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
}
