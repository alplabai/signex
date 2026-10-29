//! Tests of the detached-dialog sync (docs/DETACHED_MODALS.md).

use super::*;
use crate::app::{
    EnableVersionControlState, GridPropertiesState, InputMsg, RemoveDialogState, RenameDialogState,
    SelectionFilterCustomState, VersionControlScope, WindowMsg,
};
use crate::layout_probe::{LaidOutText, lay_out_texts};

fn test_app() -> (Signex, iced::window::Id) {
    let (mut app, _task) = Signex::new();
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.main_window_opened = true;
    app.ui_state.first_run_tour_open = false;
    (app, main)
}

/// Put `modal` in its open state, the way its open site does.
fn open(app: &mut Signex, modal: ModalId) {
    let ui = &mut app.ui_state;
    match modal {
        ModalId::FindReplace => ui.find_replace.open = true,
        ModalId::RenameDialog => {
            ui.rename_dialog = Some(RenameDialogState {
                target_path: "p.snxsch".into(),
                tree_path: vec![0],
                project_id: None,
                buffer: "p".into(),
                error: None,
                is_project_rename: false,
            })
        }
        ModalId::RemoveDialog => {
            ui.remove_dialog = Some(RemoveDialogState {
                target_path: "p.snxsch".into(),
                tree_path: vec![0],
                project_id: None,
                display_name: "p".into(),
                is_outside_project: false,
                unsaved_prompt: false,
            })
        }
        ModalId::EnableVersionControl => {
            ui.enable_version_control = Some(EnableVersionControlState {
                scope: VersionControlScope::Project,
                project_path: "p.snxprj".into(),
                project_dir: ".".into(),
                project_name: "p".into(),
                items: Vec::new(),
                use_lfs: false,
                intro_text: String::new(),
                error: None,
                working: false,
            })
        }
        ModalId::GridProperties => {
            ui.grid_properties = Some(GridPropertiesState {
                step_mm: "1".into(),
                fine_display: Default::default(),
                coarse_display: Default::default(),
                multiplier: 5,
            })
        }
        ModalId::SelectionFilterCustom => {
            ui.selection_filter_custom = Some(SelectionFilterCustomState {
                pads: true,
                tracks: true,
                arcs: true,
                pours: true,
                bodies_3d: true,
                keepouts: true,
                cutouts: true,
                texts: true,
                vias: true,
                regions: true,
                fills: true,
                other: true,
            })
        }
        ModalId::About => ui.about_open = true,
        ModalId::KeyboardShortcuts => ui.keyboard_shortcuts_open = true,
        ModalId::PassiveCalculator => ui.passive_calculator_open = true,
        ModalId::FirstRunTour => ui.first_run_tour_open = true,
        ModalId::ErrorNotice => {
            app.document_state.error_notice =
                Some(crate::app::state::ErrorNotice::create("it failed"))
        }
        ModalId::AutosaveRecovery => {
            app.document_state
                .autosave
                .offer(crate::autosave::RecoveryOffer {
                    path: "p.snxsch".into(),
                    age_secs: 90,
                    older_than_file: false,
                })
        }
        ModalId::ProjectCloseConfirm => {
            let project_id = app.document_state.mint_project_id();
            app.ui_state.project_close_confirm = Some(crate::app::ProjectCloseConfirmState {
                project_id,
                project_name: "p".into(),
                dirty_paths: vec!["/p/a.snxsch".into()],
                dirty_rows: Vec::new(),
                kept_paths: Vec::new(),
                sheet_refs: Default::default(),
            })
        }
        ModalId::AppQuitConfirm => {
            app.ui_state.app_quit_confirm = Some(crate::app::AppQuitConfirmState {
                dirty_paths: vec!["/p/a.snxsch".into()],
                dirty_rows: Vec::new(),
            })
        }
        ModalId::LibraryPicker => app.library.picker = Some(Default::default()),
        ModalId::LibraryCreateOptions => {
            app.library.create_options = Some(crate::library::state::LibraryCreateOptionsState {
                project_path: "p.snxprj".into(),
                lib_path: "l.snxlib".into(),
                enable_git: false,
                use_lfs: false,
            })
        }
        ModalId::LibraryCloseConfirm => {
            app.library.close_library_confirm =
                Some(crate::library::state::CloseLibraryConfirmState {
                    library_path: "l.snxlib".into(),
                    library_name: "l".into(),
                    dirty_editors: Vec::new(),
                })
        }
        ModalId::LibraryRecovery => {
            app.library.recovery = Some(crate::library::recovery::RecoveryDialog::LibraryMissing {
                path: "l.snxlib".into(),
            })
        }
        ModalId::NetColorCustom => app.ui_state.net_color_custom.show = true,
        ModalId::LibraryPrimitivePicker => {
            app.library.primitive_picker = Some(crate::library::state::PrimitivePickerState {
                kind: signex_library::PrimitiveKind::Symbol,
                target: crate::library::state::PrimitivePickerTarget::BrowserRow(
                    crate::library::state::EditorAddress::new(
                        "l.snxlib".into(),
                        "t".into(),
                        signex_library::RowId::new(),
                    ),
                ),
                filter: String::new(),
                error: None,
                scroll_y: 0.0,
            })
        }
        ModalId::LibraryDocumentOptions => {
            app.library.document_options = Some(crate::library::state::DocumentOptionsModalState {
                library_path: "l.snxlib".into(),
                library_name: "l".into(),
                draft: Default::default(),
            })
        }
        ModalId::LibraryUpdates => {
            app.library.library_updates =
                Some(crate::library::updates_dialog::LibraryUpdatesState::new(
                    "p.snxsch".into(),
                    Vec::new(),
                ))
        }
        ModalId::NetlistIncompletePrompt => panic!("needs an export context"),
        ModalId::LibraryEditRow | ModalId::LibraryDeleteConfirm => {
            panic!("per-browser: see the browser tests")
        }
        ModalId::CreateChildSheet => {
            ui.create_child_sheet_confirm =
                Some(crate::app::contracts::CreateChildSheetConfirmState {
                    symbol: uuid::Uuid::nil(),
                    name: "s".into(),
                    parent: "p.snxsch".into(),
                    project_dir: ".".into(),
                    path_text: "s.snxsch".into(),
                    error: None,
                    exists: false,
                })
        }
        other => panic!("{other:?} is not followed"),
    }
}

/// Every followed dialog the fixtures can open (the netlist prompt needs a
/// whole export context and has its own test).
fn testable() -> impl Iterator<Item = ModalId> {
    FOLLOWED
        .into_iter()
        .filter(|m| *m != ModalId::NetlistIncompletePrompt)
}

fn window_of(app: &Signex, modal: ModalId) -> Option<iced::window::Id> {
    app.ui_state.windows.iter().find_map(|(id, kind)| {
        matches!(kind, WindowKind::DetachedModal(m) if *m == modal).then_some(*id)
    })
}

#[test]
fn opening_each_followed_dialog_gives_it_a_window_and_closing_removes_it() {
    for modal in testable() {
        let (mut app, _main) = test_app();
        open(&mut app, modal);
        assert!(
            window_of(&app, modal).is_none(),
            "{modal:?} before the sync"
        );
        let _ = app.sync_detached_modals();
        assert!(window_of(&app, modal).is_some(), "{modal:?} gets a window");
        // Nothing new on a second pass.
        let _ = app.sync_detached_modals();
        assert_eq!(app.ui_state.windows.len(), 1, "{modal:?} opened twice");

        // The dialog's own close message (what the OS close button and
        // the in-dialog close send) clears the state and the window.
        let id = window_of(&app, modal).expect("window");
        let close = app
            .window_close_message(id, modal)
            .expect("a close message");
        let _ = app.update(close);
        assert_eq!(app.followed_modal_open(modal), Some(false), "{modal:?}");
        assert!(window_of(&app, modal).is_none(), "{modal:?} window stays");
    }
}

#[test]
fn the_os_close_request_and_esc_close_each_followed_dialog() {
    for modal in testable() {
        for via_esc in [false, true] {
            let (mut app, _main) = test_app();
            open(&mut app, modal);
            let _ = app.sync_detached_modals();
            let id = window_of(&app, modal).expect("window");
            let message = if via_esc {
                Message::Input(InputMsg::EscapePressed { window: Some(id) })
            } else {
                Message::Window(WindowMsg::WindowCloseRequested(id))
            };
            let _ = app.update(message);
            assert_eq!(
                app.followed_modal_open(modal),
                Some(false),
                "{modal:?} (esc: {via_esc})"
            );
            assert!(
                window_of(&app, modal).is_none(),
                "{modal:?} (esc: {via_esc})"
            );
        }
    }
}

#[test]
fn a_window_that_closes_underneath_clears_the_state() {
    for modal in testable() {
        let (mut app, _main) = test_app();
        open(&mut app, modal);
        let _ = app.sync_detached_modals();
        let id = window_of(&app, modal).expect("window");
        let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(id)));
        // A recovery offer is only ever answered by the user: its window
        // vanishing keeps it (the sync shows it again).
        let expected = matches!(modal, ModalId::AutosaveRecovery | ModalId::LibraryRecovery);
        assert_eq!(app.followed_modal_open(modal), Some(expected), "{modal:?}");
    }
}

fn main_texts(app: &Signex, main: iced::window::Id) -> Vec<LaidOutText> {
    lay_out_texts(app.view(main), iced::Size::new(1400.0, 900.0))
}

#[test]
fn a_detached_dialog_does_not_paint_in_the_main_window() {
    // (dialog, a text only its body draws)
    let probes = [
        (ModalId::About, "Copy version info"),
        (
            ModalId::FindReplace,
            "Targets: labels, notes, designators, values",
        ),
    ];
    for (modal, probe) in probes {
        let (mut app, main) = test_app();
        open(&mut app, modal);
        assert!(
            main_texts(&app, main).iter().any(|t| t.text == probe),
            "{modal:?} paints in the window before it is detached"
        );
        let _ = app.sync_detached_modals();
        assert!(
            !main_texts(&app, main).iter().any(|t| t.text == probe),
            "{modal:?} still paints in the main window"
        );
        // Its own window has the body.
        let id = window_of(&app, modal).expect("window");
        let texts = lay_out_texts(app.view(id), iced::Size::new(800.0, 700.0));
        assert!(texts.iter().any(|t| t.text == probe), "{modal:?} body");
    }
}

#[test]
fn f1_still_toggles_the_shortcuts_sheet_from_either_window() {
    let (mut app, main) = test_app();
    let f1 = iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::F1),
        modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::F1),
        physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::F1),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::empty(),
        text: None,
        repeat: false,
    };
    let _ = app.handle_key_input(main, f1.clone());
    assert!(app.ui_state.keyboard_shortcuts_open);
    let _ = app.sync_detached_modals();
    let sheet = window_of(&app, ModalId::KeyboardShortcuts).expect("window");
    // F1 typed in the sheet's own window closes it.
    let _ = app.handle_key_input(sheet, f1.clone());
    assert!(!app.ui_state.keyboard_shortcuts_open);
    // And from the main window it opens again.
    let _ = app.handle_key_input(main, f1);
    assert!(app.ui_state.keyboard_shortcuts_open);
}

fn key(c: &str) -> iced::keyboard::Event {
    iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Character(c.into()),
        modified_key: iced::keyboard::Key::Character(c.into()),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::empty(),
        text: Some(c.into()),
        repeat: false,
    }
}

#[test]
fn a_detached_confirm_locks_the_main_window_and_a_tool_window_does_not() {
    // A tool window: keys still reach the keymap in the main window.
    let (mut app, main) = test_app();
    open(&mut app, ModalId::FindReplace);
    let _ = app.sync_detached_modals();
    assert!(app.locking_confirm_window().is_none());
    assert!(!app.has_locking_confirm_modal());
    assert!(
        app.route_key_for_test(main, &key("w")).is_some(),
        "tool window"
    );

    // A confirm: the main window takes no key, Esc still resolves.
    let (mut app, main) = test_app();
    open(&mut app, ModalId::RenameDialog);
    let _ = app.sync_detached_modals();
    let confirm = window_of(&app, ModalId::RenameDialog).expect("window");
    assert_eq!(app.locking_confirm_window(), Some(confirm));
    assert!(app.has_locking_confirm_modal(), "the native menu locks too");
    assert!(
        app.route_key_for_test(main, &key("w")).is_none(),
        "main is locked"
    );
    let esc = iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
        modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
        physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Escape),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::empty(),
        text: None,
        repeat: false,
    };
    assert!(
        app.route_key_for_test(main, &esc).is_some(),
        "Esc dismisses"
    );
    // The dialog's own window keeps working.
    assert!(app.route_key_for_test(confirm, &key("w")).is_some());

    // Remove locks the same way.
    let (mut app, _main) = test_app();
    open(&mut app, ModalId::RemoveDialog);
    let _ = app.sync_detached_modals();
    assert!(app.locking_confirm_window().is_some());
}

#[test]
fn clicking_the_locked_window_raises_the_confirm() {
    let (mut app, _main) = test_app();
    open(&mut app, ModalId::RenameDialog);
    let _ = app.sync_detached_modals();
    // A task, not Task::none(): it asks the OS to focus the dialog.
    let task = app.update(Message::Window(WindowMsg::LockedWindowClicked));
    assert!(task.units() > 0, "no gain_focus task");
}

#[test]
fn a_quit_confirm_stays_answerable_while_a_detached_confirm_is_open() {
    let (mut app, main) = test_app();
    open(&mut app, ModalId::RenameDialog);
    let _ = app.sync_detached_modals();
    assert!(app.locking_confirm_window().is_some());
    app.document_state.dirty_paths.insert("/p/a.snxsch".into());
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    assert!(
        app.ui_state.app_quit_confirm.is_some(),
        "the quit confirm opened"
    );
    // It has its own window, so the lock layer cannot hide it...
    let window = app
        .detached_window_of(ModalId::AppQuitConfirm)
        .expect("its window");
    let texts = lay_out_texts(app.view(window), iced::Size::new(540.0, 380.0));
    assert!(
        texts.iter().any(|t| t.text.contains("unsaved document")),
        "the quit confirm shows its own window"
    );
    // ...and Esc, which the lock lets through, cancels it.
    let _ = app.update(Message::Input(InputMsg::EscapePressed {
        window: Some(main),
    }));
    assert!(app.ui_state.app_quit_confirm.is_none());
}

#[test]
fn closing_enable_version_control_while_git_runs_keeps_the_dialog() {
    let (mut app, _main) = test_app();
    open(&mut app, ModalId::EnableVersionControl);
    app.ui_state
        .enable_version_control
        .as_mut()
        .expect("open")
        .working = true;
    let _ = app.sync_detached_modals();
    let id = window_of(&app, ModalId::EnableVersionControl).expect("window");
    let _ = app.update(Message::Window(WindowMsg::WindowCloseRequested(id)));
    assert!(
        app.ui_state.enable_version_control.is_some(),
        "state is not dropped"
    );
    assert!(
        window_of(&app, ModalId::EnableVersionControl).is_some(),
        "window stays"
    );
}

#[test]
fn the_lock_asks_the_runtime_to_release_text_focus_once() {
    let (mut app, _main) = test_app();
    open(&mut app, ModalId::RenameDialog);
    assert!(!app.ui_state.lock_unfocused);
    let first = app.sync_detached_modals().expect("tasks");
    assert!(app.ui_state.lock_unfocused);
    assert!(first.units() >= 2, "window open + unfocus");
    let second = app.sync_detached_modals();
    assert!(second.is_none(), "not repeated while the lock holds");
}

/// The lowest laid-out text of `modal`'s body in a window of its size.
fn content_bottom(modal: ModalId) -> (f32, f32) {
    let (mut app, _main) = test_app();
    open(&mut app, modal);
    let _ = app.sync_detached_modals();
    let id = window_of(&app, modal).expect("window");
    let size = app.modal_window_size(modal);
    let texts = lay_out_texts(app.view(id), size);
    let bottom = texts
        .iter()
        .map(|t| t.bounds.y + t.bounds.height)
        .fold(0.0_f32, f32::max);
    (size.height, bottom)
}

/// Fixed-size dialogs are sized to their content: nothing is cut off and
/// there is no large empty area (their windows cannot be resized, so a
/// wrong size would stay wrong). Dialogs whose body is a list the data
/// decides (Enable Version Control, the pickers) are left out.
#[test]
fn fixed_dialogs_fit_their_content_without_large_slack() {
    for modal in testable().filter(|m| crate::app::handlers::erc::modal_is_fixed(*m)) {
        if matches!(
            modal,
            ModalId::EnableVersionControl
                | ModalId::LibraryPicker
                | ModalId::LibraryPrimitivePicker
        ) {
            continue;
        }
        let (window, bottom) = content_bottom(modal);
        assert!(
            bottom <= window,
            "{modal:?}: content ends at {bottom}, window is {window}"
        );
        assert!(
            window - bottom <= 110.0,
            "{modal:?}: {} px of empty window below the content",
            window - bottom
        );
    }
}

fn quit_with_rows(rows: usize) -> (Signex, iced::window::Id) {
    let (mut app, _main) = test_app();
    app.ui_state.app_quit_confirm = Some(crate::app::AppQuitConfirmState {
        dirty_paths: (0..rows)
            .map(|i| format!("/p/sheet{i}.snxsch").into())
            .collect(),
        dirty_rows: Vec::new(),
    });
    let _ = app.sync_detached_modals();
    let id = window_of(&app, ModalId::AppQuitConfirm).expect("window");
    (app, id)
}

#[test]
fn a_list_dialog_is_sized_to_its_rows_and_caps_at_a_scrolling_list() {
    let (two, _) = quit_with_rows(2);
    let (thirty, _) = quit_with_rows(30);
    let h2 = two.modal_window_size(ModalId::AppQuitConfirm).height;
    let h30 = thirty.modal_window_size(ModalId::AppQuitConfirm).height;
    assert!(h2 < h30, "two rows ({h2}) are smaller than thirty ({h30})");
    let cap = crate::app::handlers::modal_sizing::LIST_MAX_HEIGHT;
    let one = quit_with_rows(1)
        .0
        .modal_window_size(ModalId::AppQuitConfirm)
        .height;
    assert!(
        h30 - one <= cap,
        "30 rows add at most the list cap, then it scrolls"
    );
    // Both fit their windows, without a big empty area.
    for rows in [2, 30] {
        let (app, id) = quit_with_rows(rows);
        let size = app.modal_window_size(ModalId::AppQuitConfirm);
        let texts = lay_out_texts(app.view(id), size);
        let visible_bottom = texts
            .iter()
            .filter(|t| !t.text.starts_with("sheet"))
            .map(|t| t.bounds.y + t.bounds.height)
            .fold(0.0_f32, f32::max);
        assert!(
            visible_bottom <= size.height,
            "{rows} rows: footer at {visible_bottom}, window {}",
            size.height
        );
        assert!(
            size.height - visible_bottom <= 60.0,
            "{rows} rows: {} px of slack",
            size.height - visible_bottom
        );
    }
}

#[test]
fn a_list_that_changes_while_the_dialog_is_open_resizes_its_window() {
    let (mut app, id) = quit_with_rows(2);
    // The window is already synced and recorded; a second pass is a no-op.
    let _ = app.sync_detached_modals();
    let before = app.ui_state.modal_window_sizes[&id];
    let confirm = app.ui_state.app_quit_confirm.as_mut().expect("open");
    confirm
        .dirty_paths
        .extend((10..20).map(|i| format!("/p/more{i}.snxsch").into()));
    let tasks = app.sync_detached_modals();
    let after = app.ui_state.modal_window_sizes[&id];
    assert!(
        after.1 > before.1,
        "the recorded height grew: {before:?} -> {after:?}"
    );
    assert!(
        tasks.is_some_and(|t| t.units() > 0),
        "a resize was requested"
    );
}

#[test]
fn dialog_windows_are_titled_without_an_application_suffix_and_drawn_flat() {
    let (app, id) = quit_with_rows(1);
    let title = app.title(id);
    let expected = if cfg!(target_os = "macos") {
        "Quit Signex"
    } else {
        "Exit Signex"
    };
    assert_eq!(title, expected);
    let (mut app, _) = quit_with_rows(1);
    for modal in [
        ModalId::RenameDialog,
        ModalId::About,
        ModalId::CreateChildSheet,
    ] {
        open(&mut app, modal);
    }
    let _ = app.sync_detached_modals();
    for (&id, kind) in &app.ui_state.windows {
        if matches!(kind, WindowKind::DetachedModal(_)) {
            assert!(!app.title(id).contains("— Signex"), "{}", app.title(id));
            let theme = app.theme(id).expect("theme");
            assert!(
                crate::styles::is_dialog_window(&theme),
                "dialog windows draw flat"
            );
        }
    }
    let main = app.ui_state.main_window_id.expect("main");
    assert!(!crate::styles::is_dialog_window(
        &app.theme(main).expect("theme")
    ));
}

/// Every dialog window is covered edge to edge by its body, and the
/// window's own background is the active theme's dialog surface, in a dark
/// theme and in Solarized Light and after switching between them.
#[test]
fn every_dialog_window_is_filled_by_its_body_on_the_theme_surface() {
    use signex_types::theme::ThemeId;
    for theme_id in [ThemeId::CatppuccinMocha, ThemeId::SolarizedLight] {
        for modal in testable() {
            let (mut app, _main) = test_app();
            app.ui_state.theme_id = theme_id;
            app.document_state.panel_ctx.tokens = app.tokens_for(theme_id);
            open(&mut app, modal);
            let _ = app.sync_detached_modals();
            let id = window_of(&app, modal).expect("window");
            let size = app.modal_window_size(modal);
            let root = crate::layout_probe::root_size(app.view(id), size);
            assert_eq!(
                (root.width, root.height),
                (size.width, size.height),
                "{modal:?} in {theme_id:?}: the body does not cover the window"
            );
            let paper = crate::styles::ti(app.document_state.panel_ctx.tokens.paper);
            let theme = app.theme(id).expect("theme");
            assert!(crate::styles::is_dialog_window(&theme));
            assert_eq!(
                theme.palette().background,
                paper,
                "{modal:?} in {theme_id:?}: the window background is not the surface token"
            );
            // Dialogs with a footer of buttons: the footer reaches the
            // window's right edge, so no fixed card width is left behind.
            if matches!(
                modal,
                ModalId::LibraryCloseConfirm
                    | ModalId::LibraryCreateOptions
                    | ModalId::LibraryDocumentOptions
                    | ModalId::LibraryRecovery
                    | ModalId::LibraryUpdates
                    | ModalId::LibraryPicker
                    | ModalId::LibraryPrimitivePicker
                    | ModalId::AppQuitConfirm
                    | ModalId::ProjectCloseConfirm
                    | ModalId::RemoveDialog
                    | ModalId::RenameDialog
                    | ModalId::CreateChildSheet
            ) {
                let right = lay_out_texts(app.view(id), size)
                    .iter()
                    .map(|t| t.bounds.x + t.bounds.width)
                    .fold(0.0_f32, f32::max);
                assert!(
                    size.width - right <= 48.0,
                    "{modal:?} in {theme_id:?}: content ends {} px short of the right edge",
                    size.width - right
                );
            }
        }
    }
}

#[test]
fn at_a_larger_interface_scale_fixed_dialogs_still_show_their_footer() {
    for modal in [
        ModalId::AppQuitConfirm,
        ModalId::CreateChildSheet,
        ModalId::RemoveDialog,
    ] {
        let (mut base, _main) = test_app();
        open(&mut base, modal);
        let one = base.modal_window_size(modal);

        let (mut app, _main) = test_app();
        app.ui_state.ui_scale = 1.5;
        open(&mut app, modal);
        let _ = app.sync_detached_modals();
        let id = window_of(&app, modal).expect("window");
        let size = app.modal_window_size(modal);
        assert_eq!(
            (size.width, size.height),
            ((one.width * 1.5).ceil(), (one.height * 1.5).ceil()),
            "{modal:?}: both dimensions scale"
        );
        // iced lays the window out at (window / scale): that is the room the
        // footer has, and it must all be inside it.
        let layout = iced::Size::new(size.width / 1.5, size.height / 1.5);
        let bottom = lay_out_texts(app.view(id), layout)
            .iter()
            .filter(|t| !t.text.starts_with("sheet"))
            .map(|t| t.bounds.y + t.bounds.height)
            .fold(0.0_f32, f32::max);
        assert!(
            bottom <= layout.height,
            "{modal:?}: footer at {bottom}, room {}",
            layout.height
        );
    }
}

#[test]
fn a_scale_change_resizes_every_open_dialog_once_and_closed_windows_are_forgotten() {
    let (mut app, _main) = test_app();
    open(&mut app, ModalId::About);
    open(&mut app, ModalId::RenameDialog);
    let _ = app.sync_detached_modals();
    let about = window_of(&app, ModalId::About).expect("about");
    let rename = window_of(&app, ModalId::RenameDialog).expect("rename");
    assert!(app.ui_state.modal_window_sizes.contains_key(&about));
    let before = app.ui_state.modal_window_sizes[&about];

    app.ui_state.ui_scale = 2.0;
    let tasks = app.sync_detached_modals().expect("resize tasks");
    assert!(tasks.units() >= 2, "both windows resize");
    assert_eq!(
        app.ui_state.modal_window_sizes[&about].1,
        (before.1 * 2.0).ceil()
    );
    assert!(app.sync_detached_modals().is_none(), "once per change");

    // Closing a window forgets its size.
    let close = app
        .window_close_message(rename, ModalId::RenameDialog)
        .expect("close");
    let _ = app.update(close);
    assert!(!app.ui_state.modal_window_sizes.contains_key(&rename));
    let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(about)));
    assert!(!app.ui_state.modal_window_sizes.contains_key(&about));
}
