//! `Signex::new` — initial application state + boot Task. Split from
//! `app/bootstrap.rs` as pure code motion.

use super::super::*;

use crate::canvas::CanvasSlot;
use crate::dock::{DockArea, PanelPosition};
use crate::panels::PanelKind;

impl Signex {
    /// [`Signex::new`] plus one `FileMsg::Opened` per command-line
    /// document (#317), so a file association opens its file through the
    /// same flow as File > Open — including its error notice.
    pub fn boot(startup_paths: Vec<std::path::PathBuf>) -> (Self, Task<Message>) {
        let (app, boot_task) = Self::new();
        Self::boot_from(app, boot_task, startup_paths)
    }

    /// Everything [`Signex::boot`] does after `new`, split out so a test can
    /// give the app a recovery folder of its own.
    pub(super) fn boot_from(
        mut app: Self,
        boot_task: Task<Message>,
        startup_paths: Vec<std::path::PathBuf>,
    ) -> (Self, Task<Message>) {
        let opens = Self::startup_open_messages(startup_paths)
            .into_iter()
            .map(Task::done);
        // Recovery copies are reconciled per document as it opens (#1164);
        // this clears what a crashed write stranded beside them. Here, not
        // in `new`, so a test app never touches the real recovery folder.
        let recovery_sweep = app.autosave_startup_sweep();
        // Before any document opens, so no open races a half-finished move.
        // Synchronous on purpose: boot runs before the event loop, so no open
        // message can be handled until it returns, which a Task could not
        // promise. The work is bounded (`pending_move::MAX_ENTRIES_PER_PASS`)
        // and touches only the recovery folder and the two document paths of
        // each entry (pure key arithmetic plus a `stat`); it never probes a
        // document's folder for its case rule. A recovery folder on a slow
        // network home directory can still delay the first frame.
        app.autosave_finish_pending_moves();
        // The installed-font probe walks the system font directories, so it
        // runs off the update thread; until it lands, named families render
        // in the bundled default (#510).
        let font_scan = Task::perform(crate::fonts::catalog::scan_in_background(), |()| {
            Message::Ui(UiMsg::InstalledFontsScanned)
        });
        // The old prefs.json rules become the user's default profile; here,
        // not in `new`, so a test app never writes a profile library.
        app.ui_state.machine_rules_loading = crate::rule_profiles::profiles_dir().is_some();
        let profile_migration = Self::migrate_default_profile_task();
        let controls_probe = Self::detect_window_controls_task();
        (
            app,
            Task::batch(
                [
                    boot_task,
                    font_scan,
                    recovery_sweep,
                    profile_migration,
                    controls_probe,
                ]
                .into_iter()
                .chain(opens),
            ),
        )
    }

    /// The open messages a launch with `paths` dispatches, in argv order.
    pub fn startup_open_messages(paths: Vec<std::path::PathBuf>) -> Vec<Message> {
        paths
            .into_iter()
            .map(|path| Message::File(FileMsg::Opened(Some(path))))
            .collect()
    }

    pub fn new() -> (Self, Task<Message>) {
        // One parse of `prefs.json` feeds every startup reader below; a
        // missing or malformed file reads as `None`, i.e. every default.
        let prefs_json = crate::fonts::load_prefs_json();
        let prefs = prefs_json.as_ref();
        // Default panel layout — restored from disk if a previous
        // session persisted one, otherwise seeded with the Altium-ish
        // defaults: Projects + Components + Signal on the left,
        // Properties + Messages on the right, ERC on the bottom
        // (user's request — bottom is reserved for future log tails).
        let mut dock = match crate::fonts::read_dock_layout_from(prefs) {
            Some(saved) => saved,
            None => {
                // Which panels open, in tab order. *Where* each one
                // goes is `PanelPosition::default_for`, so the boot
                // layout and every later "open panel X" agree on one
                // home per kind (#641).
                let mut d = DockArea::new();
                for kind in [
                    PanelKind::Projects,
                    PanelKind::Components,
                    PanelKind::Library,
                    PanelKind::Signal,
                    PanelKind::Properties,
                    PanelKind::Messages,
                    PanelKind::Erc,
                ] {
                    d.add_panel(PanelPosition::default_for(kind), kind);
                }
                d
            }
        };
        // Silence unused-mut when read_dock_layout returns Some.
        let _ = &mut dock;
        let dock_layout_sync = crate::dock::DockLayoutSync::new(
            std::sync::Arc::new(crate::fonts::PrefsDockLayoutStore::user_prefs()),
            crate::dock::DockLayoutSnapshot::capture(&dock),
        );

        let search_prefs = crate::persist::SearchPrefsSync::new(std::sync::Arc::new(
            crate::fonts::PrefsSearchStore::user_prefs(),
        ));

        let sch_canvas = CanvasSlot::new();
        let mut pcb_canvas = crate::pcb_canvas::PcbCanvas::new();
        // Read the persisted GPU-render preference ONCE — each read is a full
        // `prefs.json` parse and three consumers need the same value: the
        // widget's effective flag (so the very first PCB frame honours the
        // saved toggle before the user opens Preferences) plus the
        // `ui_state.pcb_gpu_render` saved mirror and its Preferences draft.
        let pcb_gpu_render = crate::fonts::read_pcb_gpu_render_pref_from(prefs);
        pcb_canvas.gpu_render = pcb_gpu_render;
        // #629 — same shape for the three Symbol Editor settings: one read
        // each, feeding both the committed field and its Preferences draft.
        // Reading twice would be harmless today but invites the two to be
        // seeded from different parses of a file edited mid-launch.
        let symbol_grid_size_mm = crate::fonts::read_symbol_grid_size_mm_pref_from(prefs);
        let component_classes = crate::fonts::read_component_classes_pref_from(prefs);
        let symbol_grid_style = crate::fonts::read_symbol_grid_style_pref_from(prefs);
        let symbol_pin_selection = crate::fonts::read_symbol_pin_selection_pref_from(prefs);
        let zoom_step_percent = crate::fonts::read_zoom_step_percent_pref_from(prefs);
        let ui_scale =
            crate::fonts::read_ui_scale_pref_from(prefs, &crate::fonts::prefs_file_path());
        let annotate_undo = crate::fonts::read_annotate_undo_pref_from(prefs);
        let move_keeps_wires = crate::fonts::read_move_keeps_wires_pref_from(prefs);
        let net_hover_enabled = crate::fonts::read_net_hover_pref_from(prefs);
        let drag_direction_select = crate::fonts::read_drag_direction_select_pref_from(prefs);
        let increment_step = crate::fonts::read_increment_step_pref_from(prefs);
        let paste_designators = crate::fonts::read_paste_designators_pref_from(prefs);
        let window_controls_placement =
            crate::fonts::read_window_controls_placement_pref_from(prefs);
        let autosave_enabled = crate::fonts::read_autosave_enabled_pref_from(prefs);
        // Default to the 50-mil Altium grid; user-set value overrides
        // through the prefs file (UX §1.5 — last-used grid persists).
        let default_rule_profile = crate::fonts::read_default_rule_profile_pref_from(prefs);
        let grid_size_mm = crate::fonts::read_grid_size_mm_pref_from(prefs)
            .unwrap_or(crate::canvas::grid::GRID_SIZES_MM[1]);
        // Keyboard shortcut profiles: prefer the user's saved set from
        // the OS config dir, else fall back to the bundled Altium /
        // Classic built-ins (which must always parse). The active
        // profile is compiled once here into a fast lookup table.
        //
        // An `Err` here never means "no shortcuts file" — `load_profile_set`
        // returns `Ok` when the path does not exist. It always means a file
        // IS there and could not be honoured, so the fallback set is missing
        // custom profiles the user still has on disk. Booting with working
        // keys is right, but the reason has to survive: it raises the
        // Preferences banner and arms the backup-before-save guard that stops
        // the next Apply from writing the fallback over their file (#595).
        let (keymap_profiles, keymap_load_error) = match crate::keymap::load_profile_set() {
            Ok(set) => (set, None),
            Err(error) => (
                crate::keymap::ShortcutProfileSet::built_ins()
                    .expect("bundled keyboard shortcut profiles must parse"),
                Some(error.to_string()),
            ),
        };
        // Reported at `Error` level, once: the default filter is
        // `LevelFilter::Info` (`crate::diagnostics`), so `warn!` / `debug!`
        // would never reach the user's Messages panel — and this is one step
        // away from losing every shortcut they have customised.
        if let Some(error) = keymap_load_error.as_deref() {
            tracing::error!(
                target: "signex::keymap",
                path = %crate::keymap::config_path()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "<no config directory>".to_string()),
                error = error,
                "saved keyboard shortcuts could not be loaded; the bundled built-in \
                 profiles are in use for this session and the file on disk was left \
                 untouched — repair or delete it to load your own profiles again"
            );
        }
        // The prefs file's own health (#602). Every `read_*_pref` above
        // has just swallowed this same failure with `.ok()` and handed
        // back its default, so by this line the whole UI is already
        // showing factory settings — and `update_prefs_json` will refuse
        // every write for the rest of the session to keep the bytes
        // repairable. Neither half is visible without asking, so ask
        // once and carry the answer to the Preferences banner.
        //
        // `main.rs` installs the logger and takes the first prefs read
        // (which is what runs `migrate_legacy_prefs`) before `Signex::new`
        // is called, so this probe sees the post-migration file and its
        // log line is not emitted into a missing subscriber.
        let prefs_load_error = crate::fonts::check_prefs_file()
            .err()
            .map(|error| error.to_string());
        // `Error` level for the same reason as the keymap report above:
        // the default filter is `LevelFilter::Info` (`crate::diagnostics`),
        // so `warn!` / `debug!` would never reach the user's Messages
        // panel — and this is every preference they have, not saving.
        if let Some(error) = prefs_load_error.as_deref() {
            tracing::error!(
                target: "signex::prefs",
                path = %crate::fonts::prefs_file_path().display(),
                error = error,
                "the preferences file could not be loaded; every preference is showing \
                 its default and no change will be saved while it stays that way — the \
                 file itself was left untouched, so repair it by hand or use \
                 Preferences to start a fresh one beside it"
            );
        }
        let active_keymap = std::sync::Arc::new(keymap_profiles.compile_active());
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
                theme_id: crate::fonts::read_theme_pref_from(prefs),
                unit: crate::fonts::read_unit_pref_from(prefs),
                grid_visible: crate::fonts::read_grid_visible_pref_from(prefs),
                snap_enabled: crate::fonts::read_snap_enabled_pref_from(prefs),
                cursor_x: 0.0,
                cursor_y: 0.0,
                cursor_document: None,
                zoom: 100.0,
                grid_size_mm,
                visible_grid_mm: 1.27,
                snap_hotspots: true,
                ui_font_name: crate::fonts::read_ui_font_pref_from(prefs),
                component_classes: component_classes.clone(),
                preferences_draft_component_classes: component_classes.clone(),
                keymap_profiles,
                active_keymap,
                keymap_pending_sequence: Vec::new(),
                keymap_pending_target: None,
                keymap_chord_wait: None,
                keymap_chord_generation: 0,
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
                shortcut_sections: None,
                about_open: false,
                recent_projects: crate::recent_projects::RecentProjects::from_paths(
                    crate::fonts::read_recent_projects_from(prefs),
                ),
                first_run_tour_open: !crate::fonts::read_first_run_tour_dismissed_from(prefs),
                find_replace: crate::find_replace::FindReplaceState::default(),
                preferences_nav: crate::preferences::PrefNav::Appearance,
                // #631 — seeded from the saved theme, not hardcoded. This
                // was harmless while nothing read the draft before the
                // dialog first opened (`seed_preferences_drafts_from_live`
                // fixed it there); the canvas now renders from it, so a
                // user whose saved theme is not Signex would have opened
                // to the wrong canvas colours.
                preferences_draft_theme: crate::fonts::read_theme_pref_from(prefs),
                preferences_draft_font: String::new(),
                grid_style: crate::fonts::read_grid_style_pref_from(prefs),
                preferences_draft_grid_style: crate::fonts::read_grid_style_pref_from(prefs),
                pcb_gpu_render,
                preferences_draft_pcb_gpu_render: pcb_gpu_render,
                ui_scale,
                preferences_draft_ui_scale_percent: crate::ui_scale::scale_to_percent(ui_scale),
                zoom_step_percent,
                preferences_draft_zoom_step_percent: zoom_step_percent,
                annotate_undo,
                preferences_draft_annotate_undo: annotate_undo,
                move_keeps_wires,
                preferences_draft_move_keeps_wires: move_keeps_wires,
                net_hover_enabled,
                preferences_draft_net_hover_enabled: net_hover_enabled,
                drag_direction_select,
                preferences_draft_drag_direction_select: drag_direction_select,
                increment_step,
                preferences_draft_increment_step: increment_step,
                paste_designators,
                preferences_draft_paste_designators: paste_designators,
                window_controls_placement,
                preferences_draft_window_controls_placement: window_controls_placement,
                window_controls_placement_at_boot: window_controls_placement,
                desktop_button_layout: None,
                main_window_mode: iced::window::Mode::Windowed,
                focused_window: None,
                main_window_opened: false,
                lock_unfocused: false,
                modal_window_sizes: Default::default(),
                quit_confirm_refreshed: false,
                edit_row_window_vanished: Default::default(),
                recovery_window_vanished: 0,
                library_modal_owners: Default::default(),
                main_window_monitor_size: None,
                autosave_enabled,
                preferences_draft_autosave_enabled: autosave_enabled,
                symbol_grid_size_mm,
                preferences_draft_symbol_grid_size_mm: symbol_grid_size_mm,
                symbol_grid_style,
                preferences_draft_symbol_grid_style: symbol_grid_style,
                symbol_pin_selection,
                preferences_draft_symbol_pin_selection: symbol_pin_selection,
                preferences_theme_status: String::new(),
                preferences_keymap_editor: keymap_editor,
                preferences_keymap_status: String::new(),
                keymap_load_error,
                // #603 — probed independently of `keymap_load_error`,
                // because the backup outlives the failure that produced
                // it and must stay reachable after a successful save
                // clears the banner.
                keymap_backup: crate::keymap::existing_backup_profiles_path()
                    .map(|bak| bak.display().to_string()),
                prefs_load_error,
                preferences_prefs_status: String::new(),
                preferences_keymap_search: String::new(),
                preferences_keymap_recorder: None,
                preferences_dirty: false,
                preferences_dirty_sticky: false,
                custom_theme: None,
                rename_dialog: None,
                remove_dialog: None,
                project_close_confirm: None,
                create_child_sheet_confirm: None,
                pending_create_sheet: None,
                app_quit_confirm: None,
                enable_version_control: None,
                grid_properties: None,
                find_similar: None,
                find_similar_generation: 0,
                paste_array: None,
                selection_filter_custom: None,
                erc_violations: Vec::new(),
                erc_violations_by_path: std::collections::HashMap::new(),
                erc_waived_hidden: std::collections::BTreeMap::new(),
                erc_focus_global_index: None,
                erc_run_generation: 0,
                erc_run_rule_set: None,
                pending_profile_update: None,
                erc_running: false,
                // The machine's rules live in the `User Default` profile; the
                // boot task loads them (`ErcMsg::UserDefaultLoaded`).
                erc_severity_override: std::collections::HashMap::new(),
                profile_manager: Default::default(),
                default_rule_profile,
                machine_rules_loading: false,
                machine_rules_problem: None,
                machine_rules_locked: None,
                save_profile_dialog: None,
                profile_writes: Default::default(),
                net_colors: std::collections::HashMap::new(),
                auto_focus: false,
                annotate_dialog_open: false,
                annotate_order: crate::app::state::AnnotateOrder::default(),
                erc_dialog_open: false,
                annotate_reset_confirm: false,
                modal_offsets: std::collections::HashMap::new(),
                modal_dragging: None,
                tab_dragging: None,
                main_window_id: None,
                windows: std::collections::HashMap::new(),
                passive_calculator: signex_widgets::passive_calculator::CalculatorControl::default(
                ),
                passive_calculator_open: false,
                move_selection: crate::app::state::MoveSelectionState::default(),
                net_color_palette_open: false,
                parameter_manager_open: false,
                reorder_picker: None,
                move_pick: None,
                pin_matrix_overrides: std::collections::HashMap::new(),
                annotate_locked: std::collections::HashSet::new(),
                annotate_preview: Default::default(),
                reprefix_generation: 0,
                reprefix_confirm: None,
                reprefix_window_close_pending: false,
                selection_mode: crate::schematic_runtime::hit_test::SelectionMode::default(),
                pending_net_color: None,
                wire_color_overrides: std::collections::HashMap::new(),
                net_class_styles: Default::default(),
                lasso_polygon: None,
                net_color_undo: Default::default(),
                net_color_revision: Default::default(),
                net_color_redo: Default::default(),
                net_color_custom: crate::app::state::NetColorCustomState::default(),
                command_palette: crate::app::command_palette::CommandPaletteState::default(),
            },
            document_state: DocumentState {
                dock,
                dock_layout_sync,
                search_prefs,
                document_saves: crate::persist::DocumentSaves::default(),
                autosave: crate::autosave::AutosaveState::new(
                    crate::autosave::default_recovery_dir(),
                ),
                project_fs: Default::default(),
                navigator_job: Default::default(),
                ownership: Default::default(),
                erc_fallback_noted: Default::default(),
                profile_drift_noted: Default::default(),
                read_only_refusal_noted: Default::default(),
                rules_history: Default::default(),
                missing_fonts: Default::default(),
                tabs: vec![],
                active_tab: 0,
                engines: std::collections::HashMap::new(),
                symbol_editors: std::collections::HashMap::new(),
                footprint_editors: std::collections::HashMap::new(),
                pad_clipboard: None,
                active_path: None,
                projects: Vec::new(),
                active_project: None,
                instance_note_scopes: std::collections::HashSet::new(),
                instance_note_due: false,
                recovery_check_due: Vec::new(),
                dirty_paths: std::collections::HashSet::new(),
                net_class_history: std::collections::HashMap::new(),
                project_tabs: std::collections::HashMap::new(),
                net_class_picks: std::collections::HashMap::new(),
                next_project_id: 0,
                next_undo_group: 0,
                documents_epoch: 0,
                designator_cache: Default::default(),
                next_pcb_revision: 0,
                pcb_revisions: std::collections::HashMap::new(),
                undo_groups: std::collections::HashMap::new(),
                panel_ctx: crate::panels::PanelContext {
                    projects: Vec::new(),
                    sym_count: 0,
                    wire_count: 0,
                    label_count: 0,
                    junction_count: 0,
                    has_schematic: false,
                    has_pcb: false,
                    paper_size: "A4".to_string(),
                    lib_symbol_count: 0,
                    lib_symbol_names: vec![],
                    placed_symbols: vec![],
                    symbol_lists_key: None,
                    tokens: signex_types::theme::theme_tokens(ThemeId::Signex),
                    theme_id: ThemeId::Signex,
                    unit: Unit::Mm,
                    grid_visible: true,
                    snap_enabled: true,
                    grid_size_mm: 1.27,
                    visible_grid_mm: 1.27,
                    // Overwritten from `ui_state.symbol_grid_style` once
                    // the app value is assembled, below.
                    symbol_grid_style: crate::render_config::GridStyle::Dots,
                    snap_hotspots: true,
                    ui_font_name: crate::fonts::read_ui_font_pref_from(prefs),
                    component_classes: crate::fonts::read_component_classes_pref_from(prefs),
                    canvas_font_name: crate::fonts::DEFAULT_CANVAS_FONT.to_string(),
                    canvas_font_size: 11.0,
                    canvas_font_bold: false,
                    canvas_font_italic: false,
                    canvas_font_popup_open: false,

                    properties_tab: 0,
                    net_classes: Default::default(),
                    project_tree: vec![],
                    project_tree_builds: 0,
                    project_tree_selected: None,
                    navigator: crate::panels::NavigatorModel::default(),
                    library_row_detail: None,
                    selection_count: 0,
                    selected_uuid: None,
                    selected_kind: None,
                    selection_info: vec![],
                    multi_groups: Default::default(),
                    multi_draft: None,
                    symbol_choices: vec![],
                    drawing_edit_buf: std::collections::HashMap::new(),
                    drawing_edit_buf_for: None,
                    selected_drawing: None,
                    selected_drawing_placement: Default::default(),
                    selected_child_sheet: None,
                    child_sheet_border_picker_open: false,
                    child_sheet_fill_picker_open: false,
                    child_sheet_border_advanced_open: false,
                    child_sheet_fill_advanced_open: false,
                    child_sheet_stroke_width_buf: None,
                    child_sheet_name_buf: None,
                    child_sheet_file_buf: None,
                    title_block: std::collections::BTreeMap::new(),
                    project_parameters: std::collections::BTreeMap::new(),
                    title_block_drafts: Vec::new(),
                    paper_edge_drafts: Vec::new(),
                    numeric_draft: None,
                    title_block_sheet: None,
                    collapsed_sections: std::collections::HashSet::new(),
                    pre_placement: None,
                    erc_diagnostics: Vec::new(),
                    erc_waivers: Vec::new(),
                    erc_focus_index: None,
                    erc_running: false,
                    diagnostics_level: crate::diagnostics::configured_level_label().to_string(),
                    diagnostics: crate::diagnostics::recent_entries(),
                    selection_filters: crate::active_bar::SelectionFilter::ALL
                        .iter()
                        .copied()
                        .collect(),
                    custom_filter_presets: crate::fonts::read_custom_filter_presets_from(prefs),
                    active_custom_filter_tab: 0,
                    footprint_filter_presets: crate::fonts::read_footprint_filter_presets_from(
                        prefs,
                    ),
                    page_format_mode: crate::panels::PageFormatMode::default(),
                    sheet_frame: Default::default(),
                    page_origin: crate::panels::PageOrigin::default(),
                    sheet_color: crate::panels::SheetColor::default(),
                    symbol_editor: None,
                    footprint_editor: None,
                    history: crate::panels::history::HistoryPanelState::default(),
                    document_font_family: None,
                },
                history: crate::panels::history::HistoryPanelState::default(),
                preview: None,
                page_setup_memory: Default::default(),
                tab_placements: Default::default(),
                net_nav: Default::default(),
                raster_queue: Default::default(),
                error_notice: None,
                netlist_incomplete_prompt: None,
                bom_preview: None,
                pending_git_commits: Vec::new(),
                inflight_git_commits: std::collections::HashSet::new(),
                unbound_git_commits: std::collections::HashSet::new(),
                git_commit_sequences: std::collections::HashMap::new(),
                active_git_repos: std::collections::HashSet::new(),
                paused_git_projects: std::collections::HashSet::new(),
                pending_opens: Default::default(),
                pending_restores: Default::default(),
                pending_path_ops: crate::app::path_ops::PendingPathOps::default(),
                dir_case: Default::default(),
                altium_import_running: None,
                project_folder_renames: std::collections::HashSet::new(),
                project_writes: Vec::new(),
                moved_project_dirs: Vec::new(),
                deferred_git_commits: Vec::new(),
            },
            interaction_state: InteractionState {
                current_tool: Tool::Select,
                canvas: sch_canvas,
                canvases: std::collections::HashMap::new(),
                pcb_canvas,
                pcb_canvases: std::collections::HashMap::new(),
                dragging: None,
                drag_start_pos: None,
                drag_start_size: 0.0,
                tab_drag_origin: None,
                wire_points: Vec::new(),
                wire_drawing: false,
                arc_points: Vec::new(),
                polyline_points: Vec::new(),
                shape_anchor: None,
                clipboard: None,
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
                custom_filter_presets: crate::fonts::read_custom_filter_presets_from(prefs),
                active_custom_filter_tab: 0,
                footprint_filter_presets: crate::fonts::read_footprint_filter_presets_from(prefs),
                selection_slots: std::array::from_fn(|_| Vec::new()),
                last_tool: std::collections::HashMap::new(),
                pending_power: None,
                placement: None,
                last_placement: None,
                paste_counters: None,
                placement_epoch: 0,
                pending_port: None,
                bus_entry_size: signex_types::schematic::SCHEMATIC_BUS_ENTRY_DEFAULT_SIZE,
                hover_symbol_uuid: None,
                hover_started_at: None,
                hover_screen_pos: None,
            },
            library: crate::library::LibraryState {
                // The values parsed above, not a second read of `prefs.json`.
                new_library_defaults: crate::library::NewLibraryDefaults {
                    grid_size_mm: symbol_grid_size_mm,
                    pin_selection: symbol_pin_selection,
                    component_classes,
                },
                ..Default::default()
            },
            native_menu: Default::default(),
        };
        // The PCB canvas starts from the loaded grid state, not its own
        // defaults, so the first frame honours a saved "grid off" (#682).
        app.interaction_state.pcb_canvas.set_grid(
            app.ui_state.grid_visible,
            app.ui_state.visible_grid_mm as f64,
        );
        // v0.9 Stage 9: the globally-configured libraries and the bundled
        // stock library (#328) are mounted off the UI thread (#1470): the
        // config list and the stock path probe are cheap, but every manifest
        // and table read runs in a Task, so the first frame is not blocked.
        // Each lands as `LibraryMessage::MountFinished`; a missing library
        // warns and stays empty.
        app.library.global_libraries = crate::panels::components_panel::global_prefs::load();
        let configured: Vec<std::path::PathBuf> = app
            .library
            .global_libraries
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        // The stock library registers first, so a global entry or project
        // library naming the same file finds it already pending and read-only.
        let stock_path = crate::panels::components_panel::stock::register_stock(&mut app.library);
        let global_paths: Vec<std::path::PathBuf> = configured
            .into_iter()
            .filter(|path| {
                app.library
                    .request_mount(path, crate::library::mount::MountIntent::Silent)
                    == crate::library::mount::MountRequest::Spawn
            })
            .collect();
        let mount_task = iced::Task::batch([crate::library::mount::spawn_pending_mounts(
            stock_path.into_iter().chain(global_paths).collect(),
        )]);
        // #630 — seed the two render surfaces that consume a grid style
        // from the saved preferences. Everything else the old
        // `render_config` global carried (canvas font/size/style) had no
        // reader in any draw path, so there is nothing left to seed.
        app.document_state.panel_ctx.symbol_grid_style = app.ui_state.symbol_grid_style;

        // Multi-window (Phase 1): open the main OS window here. Phase 2
        // will open additional windows on demand when the user drags a
        // modal off the main window, and Phase 3 will do the same for
        // undocked tabs. The returned Task produces the settled Id once
        // winit confirms the window is mapped.
        let (main_id, open_task) = iced::window::open(main_window_settings(
            crate::chrome::controls::resolve(
                crate::chrome::controls::ChromePlatform::current(),
                window_controls_placement,
                None,
            )
            .is_native(),
        ));
        app.ui_state.main_window_id = Some(main_id);
        let boot_task = iced::Task::batch([
            open_task.map(|id| Message::Window(WindowMsg::MainWindowOpened(id))),
            mount_task,
        ]);
        (app, boot_task)
    }
}

/// Settings for the main window. Every case is borderless (the custom strip
/// replaces the title bar) except macOS with native traffic lights, where the
/// OS keeps its title bar, made transparent so the strip draws behind it.
fn main_window_settings(native_decorations: bool) -> iced::window::Settings {
    iced::window::Settings {
        size: iced::Size::new(1400.0, 900.0),
        icon: super::bundled_window_icon(),
        // Without native decorations the strip draws the buttons, and the
        // 6 px strips in `resize_edges_overlay` provide edge resizing.
        decorations: native_decorations,
        // Without this winit closes the window on its own and the close
        // request never reaches the unsaved-changes guard.
        exit_on_close_request: false,
        platform_specific: native_title_bar(native_decorations),
        ..Default::default()
    }
}

#[cfg(target_os = "macos")]
fn native_title_bar(native: bool) -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific {
        title_hidden: native,
        titlebar_transparent: native,
        fullsize_content_view: native,
    }
}

#[cfg(not(target_os = "macos"))]
fn native_title_bar(_native: bool) -> iced::window::settings::PlatformSpecific {
    iced::window::settings::PlatformSpecific::default()
}

#[cfg(test)]
mod tests {
    use super::main_window_settings;
    use crate::app::Signex;

    #[test]
    fn the_main_window_routes_close_requests_through_the_app() {
        for native in [true, false] {
            let s = main_window_settings(native);
            assert!(!s.exit_on_close_request, "native={native}");
            assert_eq!(s.decorations, native);
        }
    }

    /// #1470 — startup mounts nothing before the first frame: the stock and
    /// global libraries come in as Tasks, so `new()` returns with every
    /// library still unmounted (and no install-directory probe ran).
    #[test]
    fn bootstrap_returns_before_any_library_is_mounted() {
        let (app, _boot) = Signex::new();
        assert!(app.library.open_libraries.is_empty());
        assert!(app.library.stock_library.is_none());
    }
}
