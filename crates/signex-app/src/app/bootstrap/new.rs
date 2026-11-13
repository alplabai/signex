//! `Signex::new` — initial application state + boot Task. Split from
//! `app/bootstrap.rs` as pure code motion.

use super::super::*;

use crate::canvas::CanvasSlot;

impl Signex {
    /// [`Signex::new`] plus one `FileMsg::Opened` per command-line
    /// document (#317), so a file association opens its file through the
    /// same flow as File > Open — including its error notice.
    pub fn boot(startup_paths: Vec<std::path::PathBuf>) -> (Self, Task<Message>) {
        let (mut app, boot_task) = Self::new();
        // The window the person works in lists its open projects for the agent
        // registry (#1924); an app made any other way (a test, a headless run)
        // does not.
        app.agent.lists_projects = true;
        // And journals what agents do in its projects (#2253).
        app.agent.journals_on = true;
        // A test run that boots the real window lists into a folder of its own,
        // as the config files go to a redirect (see `config_root`).
        if crate::config_root::is_test_redirect_active() {
            app.agent.use_test_runtime_dir();
        }
        Self::boot_from(app, boot_task, startup_paths)
    }

    /// Everything [`Signex::boot`] does after `new`, split out so a test can
    /// give the app a recovery folder of its own.
    pub(super) fn boot_from(
        app: Self,
        boot_task: Task<Message>,
        startup_paths: Vec<std::path::PathBuf>,
    ) -> (Self, Task<Message>) {
        Self::boot_from_on(
            app,
            boot_task,
            startup_paths,
            crate::keymap::Platform::current(),
        )
    }

    /// [`Self::boot_from`] for an explicit OS, so a test can see what a Mac
    /// launch adds without a Mac.
    pub(in crate::app) fn boot_from_on(
        mut app: Self,
        boot_task: Task<Message>,
        startup_paths: Vec<std::path::PathBuf>,
        platform: crate::keymap::Platform,
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
        // The one-time macOS shortcut question opens a native dialog and
        // persists a flag, so only the window the person works in asks it:
        // not `new` (a headless run boots from it).
        let keymap_prompt = crate::app::handlers::keymap_mod_prompt::keymap_mod_prompt_boot(
            &app.ui_state.keymap_profiles,
            crate::fonts::load_prefs_json().as_ref(),
            platform,
        );
        (
            app,
            Task::batch(
                [
                    boot_task,
                    font_scan,
                    recovery_sweep,
                    profile_migration,
                    controls_probe,
                    keymap_prompt,
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
        let (first_run_tour_open, tour_boot) = first_run_tour_boot(prefs);
        // Default panel layout — restored from disk if a previous session
        // persisted one, otherwise the Graphite first-run layout: the short
        // Messages dock. A saved layout is never replaced.
        let saved_dock = crate::fonts::read_dock_layout_from(prefs);
        let graphite_first_run = saved_dock.is_none();
        let dock = match saved_dock {
            Some(saved) => saved,
            None => crate::dock::graphite_first_run(),
        };
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
        let antialiasing = crate::fonts::read_antialiasing_pref_from(prefs);
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
        let undo_depth =
            crate::undo_depth::read_undo_depth_pref_from(prefs, &crate::fonts::prefs_file_path());
        let annotate_undo = crate::fonts::read_annotate_undo_pref_from(prefs);
        let move_keeps_wires = crate::fonts::read_move_keeps_wires_pref_from(prefs);
        let net_hover_enabled = crate::fonts::read_net_hover_pref_from(prefs);
        let trackpad_pan = crate::fonts::read_trackpad_pan_pref_from(prefs);
        let wheel_mode = crate::fonts::read_wheel_mode_pref_from(prefs);
        let i18n = std::sync::Arc::new(signex_i18n::I18n::new(
            &crate::fonts::read_language_pref_from(prefs),
        ));
        crate::app::handlers::preferences::language::log_dropped_language(&i18n);
        let remember_window_geometry = crate::fonts::read_remember_window_geometry_pref_from(prefs);
        let drag_direction_select = crate::fonts::read_drag_direction_select_pref_from(prefs);
        let increment_step = crate::fonts::read_increment_step_pref_from(prefs);
        let paste_designators = crate::fonts::read_paste_designators_pref_from(prefs);
        // Never rewrite prefs under a headless config root (#1926); the read
        // above already treats the old value as migrated for this session.
        if !crate::config_root::is_override_active()
            && crate::fonts::paste_designators_needs_migration(prefs)
        {
            match crate::fonts::migrate_paste_designators_pref() {
                crate::fonts::PasteDesignatorsMigration::Migrated => crate::diagnostics::log_info(
                    "Designators on paste: your saved \"Keep\" was the old default, so it is now \"Keep unless taken\" (a pasted part keeps its designator unless the project already uses it). Choose Keep in Preferences to turn this off.",
                ),
                crate::fonts::PasteDesignatorsMigration::WriteFailed => {
                    crate::diagnostics::log_warning(
                        "Designators on paste: your saved \"Keep\" was the old default and is treated as \"Keep unless taken\" this session, but the preferences file could not be updated, so this is repeated at the next start. Choose Keep in Preferences to turn this off.",
                    )
                }
                crate::fonts::PasteDesignatorsMigration::NotNeeded => {}
            }
        }
        let window_controls_placement =
            crate::fonts::read_window_controls_placement_pref_from(prefs);
        let autosave_enabled = crate::fonts::read_autosave_enabled_pref_from(prefs);
        let agent_connections_enabled = crate::fonts::read_agent_connections_pref_from(prefs);
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

        let drafts = crate::panels::PanelDrafts::default();
        // Only a preference that is on reads the file; the record is checked
        // against the real monitors once the window exists.
        let restored_geometry = remember_window_geometry
            .then(crate::app::window_geometry::load)
            .flatten();
        let mut app = Self {
            ui_state: UiState {
                io: crate::io_ports::IoPorts::default(),
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
                grid_size_mm,
                visible_grid_mm: 1.27,
                snap_hotspots: true,
                ui_font_name: crate::fonts::read_ui_font_pref_from(prefs),
                component_classes: component_classes.clone(),
                preferences_draft_component_classes: component_classes.clone(),
                keymap_profiles,
                active_keymap,
                pcb_layer_views: Default::default(),
                pcb_view_angle_entry: None,
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
                bottom_height: if graphite_first_run {
                    crate::dock::GRAPHITE_BOTTOM_HEIGHT
                } else {
                    crate::app::state::DEFAULT_BOTTOM_PANEL_HEIGHT
                },
                window_size: (1400.0, 900.0),
                window_os_scales: std::collections::HashMap::new(),
                undocked_window_sizes: std::collections::HashMap::new(),
                panel_list_open: false,
                preferences_open: false,
                keyboard_shortcuts_open: false,
                shortcut_sections: None,
                about_open: false,
                recent_projects: crate::recent_projects::RecentProjects::from_paths(
                    crate::fonts::read_recent_projects_from(prefs),
                ),
                pinned_projects: crate::pinned_projects::PinnedProjects::from_paths(
                    crate::fonts::read_pinned_projects_from(prefs),
                ),
                first_run_tour_open,
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
                antialiasing,
                preferences_draft_antialiasing: antialiasing,
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
                trackpad_pan,
                preferences_draft_trackpad_pan: trackpad_pan,
                wheel_mode,
                preferences_draft_wheel_mode: wheel_mode,
                i18n: std::sync::Arc::clone(&i18n),
                remember_window_geometry,
                preferences_draft_remember_window_geometry: remember_window_geometry,
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
                borderless_dialog_windows: Default::default(),
                main_window_monitor_size: None,
                autosave_enabled,
                preferences_draft_autosave_enabled: autosave_enabled,
                agent_connections_enabled,
                preferences_draft_agent_connections_enabled: agent_connections_enabled,
                symbol_grid_size_mm,
                preferences_draft_symbol_grid_size_mm: symbol_grid_size_mm,
                symbol_grid_style,
                preferences_draft_symbol_grid_style: symbol_grid_style,
                symbol_pin_selection,
                preferences_draft_symbol_pin_selection: symbol_pin_selection,
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
                preferences_nav_search: String::new(),
                preferences_keymap_recorder: None,
                preferences_dirty: false,
                rename_dialog: None,
                remove_dialog: None,
                project_close_confirm: None,
                create_child_sheet_confirm: None,
                pending_create_sheet: None,
                app_quit_confirm: None,
                enable_version_control: None,
                grid_properties: None,
                new_project: None,
                document_setup: None,
                setup_profiles: crate::fonts::read_setup_profiles_from(prefs),
                removed_profile_names: Default::default(),
                missing_profiles_warned: Default::default(),
                update_pcb: None,
                sheet_numbering: None,
                sheet_numbering_reads: 0,
                board_reannotate: None,
                board_reannotate_options: Default::default(),
                back_annotate: None,
                find_similar: None,
                find_similar_generation: 0,
                assign_footprints: None,
                assign_footprints_reads: 0,
                net_inspector: Default::default(),
                paste_array: None,
                paste_array_opens: 0,
                selection_filter_custom: None,
                erc_violations: Vec::new(),
                erc_violations_by_path: std::collections::HashMap::new(),
                erc_waived_hidden: std::collections::BTreeMap::new(),
                erc_waived_findings: Vec::new(),
                erc_lapsed_waivers: std::collections::BTreeSet::new(),
                erc_page_frames: Default::default(),
                erc_waive_draft: None,
                erc_focus_global_index: None,
                erc_epoch: 0,
                erc_run_epoch: 0,
                erc_run_generation: 0,
                erc_run_rule_set: None,
                pending_profile_update: None,
                erc_running: false,
                // The machine's rules live in the `User Default` profile; the
                // boot task loads them (`ErcMsg::UserDefaultLoaded`).
                erc_severity_override: std::collections::HashMap::new(),
                machine_drc_rules: Default::default(),
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
                window_geometry:
                    crate::app::handlers::window_geometry_events::GeometryTracker::with_restore(
                        restored_geometry.clone(),
                    ),
                windows: std::collections::HashMap::new(),
                passive_calculator: signex_widgets::passive_calculator::CalculatorControl::default(
                ),
                passive_calculator_open: false,
                move_selection: crate::app::state::MoveSelectionState::default(),
                net_color_palette_open: false,
                parameter_manager_open: false,
                parameter_table: crate::app::state::ParameterTableState::default(),
                reorder_picker: None,
                move_pick: None,
                pin_matrix_overrides: std::collections::HashMap::new(),
                annotate_locked: std::collections::HashSet::new(),
                annotate_preview: Default::default(),
                annotate_runs: Default::default(),
                sheet_place: Default::default(),
                pending_sheet_opens: Default::default(),
                sheet_open_seq: 0,
                reprefix_generation: 0,
                reprefix_confirm: None,
                reprefix_window_close_pending: false,
                selection_mode: crate::schematic_runtime::hit_test::SelectionMode::default(),
                pending_net_color: None,
                net_class_styles: Default::default(),
                lasso_polygon: None,
                net_color_custom: crate::app::state::NetColorCustomState::default(),
                command_palette: crate::app::command_palette::CommandPaletteState::default(),
            },
            document_state: DocumentState {
                drafts: drafts.clone(),
                dock,
                dock_layout_sync,
                search_prefs,
                document_saves: crate::persist::DocumentSaves::default(),
                parked_save_all_settles: Default::default(),
                document_ids: Default::default(),
                autosave: crate::autosave::AutosaveState::new(
                    crate::autosave::default_recovery_dir(),
                ),
                project_fs: Default::default(),
                disk_poll: Default::default(),
                navigator_job: Default::default(),
                window_hierarchies: Default::default(),
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
                sheet_edit_epoch: 0,
                symbol_editors_opened: 0,
                undo_depth,
                dirty_docs: Default::default(),
                net_class_history: std::collections::HashMap::new(),
                project_tabs: std::collections::HashMap::new(),
                next_project_id: 0,
                next_undo_group: 0,
                documents_epoch: 0,
                designator_cache: Default::default(),
                next_pcb_revision: 0,
                pcb_revisions: std::collections::HashMap::new(),
                parked_pcb_boards: std::collections::HashMap::new(),
                undo_groups: std::collections::HashMap::new(),
                agent_edits: Default::default(),
                check_runs: Default::default(),
                window_panel_ctx: std::collections::HashMap::new(),
                window_panel_ctx_keys: std::collections::HashMap::new(),
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
                    tokens: signex_types::theme::theme_tokens(ThemeId::DEFAULT),
                    theme_id: ThemeId::DEFAULT,
                    i18n,
                    canvas_colors: signex_types::theme::canvas_colors(ThemeId::DEFAULT),
                    device_px_per_logical_px:
                        signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
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
                    signal_mode: Default::default(),
                    projects_view: Default::default(),
                    net_classes: Default::default(),
                    pcb_layers: Default::default(),
                    project_tree: vec![],
                    project_tree_builds: 0,
                    project_tree_selected: None,
                    navigator: crate::panels::NavigatorModel::default(),
                    library_row_detail: None,
                    component_sheet: None,
                    pcb_nets: None,
                    net_inspector: Default::default(),
                    pcb_board: None,
                    selection_count: 0,
                    selected_uuid: None,
                    selected_kind: None,
                    selection_info: vec![],
                    multi_groups: Default::default(),
                    multi_draft: None,
                    symbol_choices: vec![],
                    footprint_choices: vec![],
                    footprint_card: None,
                    drawing_edit_buf: std::collections::HashMap::new(),
                    drawing_edit_buf_for: None,
                    drawing_edit_buf_page: None,
                    selected_drawing: None,
                    selected_drawing_placement: Default::default(),
                    selected_child_sheet: None,
                    child_sheet_border_picker_open: false,
                    wire_color_picker: Default::default(),
                    selected_wire_style: None,
                    child_sheet_fill_picker_open: false,
                    child_sheet_border_advanced_open: false,
                    child_sheet_fill_advanced_open: false,
                    child_sheet_stroke_width_buf: None,
                    child_sheet_name_buf: None,
                    child_sheet_file_buf: None,
                    title_block: std::collections::BTreeMap::new(),
                    project_parameters: std::collections::BTreeMap::new(),
                    drafts: drafts.clone(),
                    numeric_draft: None,
                    title_block_sheet: None,
                    collapsed_sections: std::collections::HashSet::new(),
                    pre_placement: None,
                    erc_diagnostics: Vec::new(),
                    erc_waivers: Vec::new(),
                    erc_violation_waivers: Vec::new(),
                    erc_waive_draft: None,
                    erc_focus_index: None,
                    erc_running: false,
                    erc_results_stale: false,
                    diagnostics_level: crate::diagnostics::configured_level_label().to_string(),
                    diagnostics: crate::diagnostics::recent_entries(),
                    pcb_drc: None,
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
                    sheet_color: Default::default(),
                    symbol_editor: None,
                    footprint_editor: None,
                    history: crate::panels::history::HistoryPanelState::default(),
                    document_font_family: None,
                },
                history: crate::panels::history::HistoryPanelState::default(),
                window_history: std::collections::HashMap::new(),
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
                pcb_selection: Default::default(),
                pcb_selections: std::collections::HashMap::new(),
                pcb_carry: None,
                pcb_ratsnest: Default::default(),
                pcb_drc: Default::default(),
                cross_select: Default::default(),
                board_sketch: None,
                pcb_footprint_draft: None,
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
                clipboard_text: None,
                draw_mode: DrawMode::default(),
                editing_text: None,
                context_menu: None,
                project_tree_context_menu: None,
                grid_picker: None,
                tab_context_menu: None,
                start_row_menu: None,
                context_submenu: None,
                pending_submenu: None,
                submenu_launcher_hovered: None,
                submenu_panel_hovered: false,
                submenu_unhovered_since: None,
                last_mouse_pos: (0.0, 0.0),
                last_tree_click: Default::default(),
                active_bar_menu: None,
                active_bar_menu_window: None,
                acting_window: None,
                pointer_window: None,
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
                clicked_pin: None,
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
                stock_copy_target: crate::fonts::read_stock_copy_target_pref_from(prefs),
                ..Default::default()
            },
            native_menu: Default::default(),
            agent: crate::app::agent::AgentState::new(),
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
        let mut main_settings = main_window_settings(
            crate::chrome::controls::resolve(
                crate::chrome::controls::ChromePlatform::current(),
                window_controls_placement,
                None,
            )
            .is_native(),
        );
        if let Some(saved) = &restored_geometry {
            crate::app::window_geometry::apply_to_settings(&mut main_settings, saved);
        }
        let (main_id, open_task) = iced::window::open(main_settings);
        app.ui_state.main_window_id = Some(main_id);
        app.note_unusable_setup_profiles(crate::fonts::unusable_setup_profiles(prefs));
        let boot_task = iced::Task::batch([
            open_task.map(|id| Message::Window(WindowMsg::MainWindowOpened(id))),
            mount_task,
            tour_boot,
            app.check_pinned_projects(),
        ]);
        // The panel context above was seeded with Signex tokens; point it at the
        // saved theme before the first frame, so a Graphite first launch does not
        // draw one frame (and every dialog window) in the old palette.
        app.sync_theme();
        (app, boot_task)
    }
}

/// Whether this launch shows the first-run tour card, and the write that
/// makes it the only launch that does: the card is shown once, so the pref is
/// set as soon as it opens, even if the user never closes it (#1994).
pub(in crate::app) fn first_run_tour_boot(
    prefs: Option<&serde_json::Value>,
) -> (bool, iced::Task<Message>) {
    if crate::fonts::read_first_run_tour_dismissed_from(prefs) {
        return (false, iced::Task::none());
    }
    // Built when the runtime runs the task, not here: the write takes a slot
    // of the in-flight preference-write count when it is made, and a boot that
    // only builds the app must not leave that count raised.
    (
        true,
        // `Task::done` reports zero units in the fork; `future` reports one.
        iced::Task::future(std::future::ready(()))
            .then(|()| crate::app::dispatch::dismiss_first_run_tour_task()),
    )
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

    /// The first frame draws with the saved theme's tokens, not the seed's.
    #[test]
    fn the_first_frame_uses_the_saved_themes_tokens() {
        let (app, _boot) = Signex::new();
        let saved = app.ui_state.theme_id;
        assert_eq!(app.document_state.panel_ctx.theme_id, saved);
        assert_eq!(
            app.document_state.panel_ctx.tokens,
            signex_types::theme::theme_tokens(saved),
        );
    }

    /// Picking a theme in Preferences previews it live through the real
    /// message path (no `theme_id` poke): the panel tokens follow the draft
    /// and fall back when Preferences closes without saving.
    #[test]
    fn a_draft_theme_previews_through_the_preferences_messages() {
        use crate::app::{Message, PreferencesMsg};
        use crate::preferences::PrefMsg;
        let (mut app, _boot) = Signex::new();
        let saved = app.ui_state.theme_id;
        let _ = app.update(Message::Preferences(PreferencesMsg::Open));
        for draft in [
            saved.light_dark_counterpart(),
            saved,
            saved.light_dark_counterpart(),
        ] {
            let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
                PrefMsg::DraftTheme(draft),
            )));
            assert_eq!(app.document_state.panel_ctx.theme_id, draft);
            assert_eq!(
                app.document_state.panel_ctx.tokens,
                signex_types::theme::theme_tokens(draft),
                "{draft:?}"
            );
        }
        assert_eq!(app.ui_state.theme_id, saved, "the saved theme is untouched");
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
