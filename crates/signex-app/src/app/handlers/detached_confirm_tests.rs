//! The detached confirms on the quit and data-loss path keep every guarantee
//! of the in-window ones (docs/DETACHED_MODALS.md). The #1543 close-guard
//! tests in `tests/regression/project.rs` run unchanged beside these.

use crate::app::state::ModalId;
use crate::app::{InputMsg, Message, ProjectCloseChoice, ProjectMsg, Signex, WindowMsg};

fn quit_app() -> (Signex, iced::window::Id) {
    let (mut app, _task) = Signex::new();
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.main_window_opened = true;
    app.ui_state.first_run_tour_open = false;
    app.document_state
        .mark_document_dirty(std::path::Path::new("/p/board.snxsch"));
    (app, main)
}

/// Raise the quit confirm the way every quit route does, and give it its window.
fn raise_quit_confirm(app: &mut Signex, main: iced::window::Id) -> iced::window::Id {
    let _ = app.update(Message::Window(WindowMsg::WindowCloseRequested(main)));
    assert!(
        app.ui_state.app_quit_confirm.is_some(),
        "the confirm opened"
    );
    let _ = app.sync_detached_modals();
    app.detached_window_of(ModalId::AppQuitConfirm)
        .expect("its own window")
}

#[test]
fn every_quit_route_opens_the_confirm_and_never_exits_behind_it() {
    for route in [
        Message::Window(WindowMsg::CloseMainWindow),
        Message::Window(WindowMsg::WindowCloseRequested(iced::window::Id::unique())),
    ] {
        let (mut app, main) = quit_app();
        let route = match route {
            Message::Window(WindowMsg::WindowCloseRequested(_)) => {
                Message::Window(WindowMsg::WindowCloseRequested(main))
            }
            other => other,
        };
        // The routing itself (no window bookkeeping around it).
        let task = app.dispatch_update(route);
        assert!(app.ui_state.app_quit_confirm.is_some());
        assert_eq!(task.units(), 0, "no close or exit behind the modal");
        assert!(
            app.document_state
                .is_document_dirty(std::path::Path::new("/p/board.snxsch"))
        );
    }
}

#[test]
fn closing_the_confirm_window_is_cancel_never_discard() {
    let (mut app, main) = quit_app();
    let confirm = raise_quit_confirm(&mut app, main);
    let _ = app.update(Message::Window(WindowMsg::WindowCloseRequested(confirm)));
    assert!(app.ui_state.app_quit_confirm.is_none());
    assert!(
        app.document_state
            .is_document_dirty(std::path::Path::new("/p/board.snxsch")),
        "nothing was discarded"
    );
    assert!(
        app.detached_window_of(ModalId::AppQuitConfirm).is_none(),
        "its window is gone"
    );
    // The app is still open: another quit request asks again.
    let _ = app.update(Message::Window(WindowMsg::WindowCloseRequested(main)));
    assert!(app.ui_state.app_quit_confirm.is_some());
}

#[test]
fn a_second_quit_request_focuses_the_confirm_instead_of_opening_another() {
    let (mut app, main) = quit_app();
    let confirm = raise_quit_confirm(&mut app, main);
    let windows_before = app.ui_state.windows.len();
    for request in [
        Message::Window(WindowMsg::WindowCloseRequested(main)),
        Message::Window(WindowMsg::CloseMainWindow),
    ] {
        let task = app.update(request);
        assert!(task.units() > 0, "a focus request for the confirm");
        assert!(
            app.ui_state.app_quit_confirm.is_some(),
            "still the same confirm"
        );
    }
    assert_eq!(
        app.ui_state.windows.len(),
        windows_before,
        "no second window"
    );
    assert_eq!(
        app.detached_window_of(ModalId::AppQuitConfirm),
        Some(confirm)
    );
}

#[test]
fn cancel_leaves_everything_open_and_discard_all_exits() {
    let (mut app, main) = quit_app();
    raise_quit_confirm(&mut app, main);
    let task = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::Cancel,
    )));
    assert!(app.ui_state.app_quit_confirm.is_none());
    assert!(
        app.document_state
            .is_document_dirty(std::path::Path::new("/p/board.snxsch"))
    );
    // The confirm window closes; nothing else runs.
    assert!(task.units() <= 1);

    let (mut app, main) = quit_app();
    raise_quit_confirm(&mut app, main);
    let task = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    assert!(app.ui_state.app_quit_confirm.is_none());
    assert!(task.units() > 0, "Discard All closes the main window");
}

#[test]
fn esc_in_the_locked_main_window_cancels_the_quit_confirm() {
    let (mut app, main) = quit_app();
    raise_quit_confirm(&mut app, main);
    let _ = app.update(Message::Input(InputMsg::EscapePressed {
        window: Some(main),
    }));
    assert!(app.ui_state.app_quit_confirm.is_none());
    assert!(
        app.document_state
            .is_document_dirty(std::path::Path::new("/p/board.snxsch"))
    );
}

#[test]
fn the_main_window_closing_still_ends_the_app_through_secondary_window_closed() {
    // Ordering: the main window's own close event is what exits, whatever
    // dialog windows exist.
    let (mut app, main) = quit_app();
    raise_quit_confirm(&mut app, main);
    let task = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(main)));
    assert!(task.units() > 0, "iced::exit() is scheduled");
}

#[test]
fn the_other_confirms_close_to_their_safe_choice() {
    for (modal, needle) in [
        (ModalId::ProjectCloseConfirm, "Cancel"),
        (ModalId::AppQuitConfirm, "Cancel"),
        (ModalId::AutosaveRecovery, "Later"),
        (ModalId::NetlistIncompletePrompt, "NetlistCancelIncomplete"),
        (ModalId::ErrorNotice, "DismissErrorNotice"),
    ] {
        let message = Signex::detached_modal_close_message(modal).expect("a close message");
        assert!(
            format!("{message:?}").contains(needle),
            "{modal:?}: {message:?}"
        );
    }
}

#[test]
fn a_notice_raised_over_a_detached_dialog_gets_its_own_window() {
    let (mut app, _main) = quit_app();
    app.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
        target_path: "p.snxsch".into(),
        tree_path: vec![0],
        project_id: None,
        buffer: "p".into(),
        error: None,
        is_project_rename: false,
    });
    let _ = app.sync_detached_modals();
    app.document_state.error_notice = Some(crate::app::state::ErrorNotice::create("it failed"));
    let _ = app.sync_detached_modals();
    assert!(app.detached_window_of(ModalId::RenameDialog).is_some());
    assert!(
        app.detached_window_of(ModalId::ErrorNotice).is_some(),
        "reachable"
    );
    // Dismissing it leaves the other dialog alone.
    let _ = app.update(Message::Overlay(crate::app::OverlayMsg::DismissErrorNotice));
    assert!(app.detached_window_of(ModalId::ErrorNotice).is_none());
    assert!(app.detached_window_of(ModalId::RenameDialog).is_some());
}

#[test]
fn discard_all_discards_only_what_the_confirm_listed() {
    let (mut app, main) = quit_app();
    raise_quit_confirm(&mut app, main);
    // An edit lands after the confirm opened, through a route nobody locked.
    let late = std::path::PathBuf::from("/p/late.snxsch");
    app.document_state.mark_document_dirty(&late);
    let task = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    // Nothing was discarded and the app did not exit: the confirm is back,
    // listing the new item.
    let confirm = app.ui_state.app_quit_confirm.as_ref().expect("re-prompted");
    assert!(
        confirm.dirty_paths.contains(&late),
        "the new item is listed"
    );
    assert!(
        app.ui_state.quit_confirm_refreshed,
        "the dialog says the list changed"
    );
    let _ = app.sync_detached_modals();
    let window = app
        .detached_window_of(ModalId::AppQuitConfirm)
        .expect("window");
    let texts = crate::layout_probe::lay_out_texts(app.view(window), iced::Size::new(540.0, 380.0));
    assert!(
        texts
            .iter()
            .any(|t| t.text.contains("The unsaved list changed"))
    );
    assert!(app.document_state.is_document_dirty(&late));
    assert!(
        app.document_state
            .is_document_dirty(std::path::Path::new("/p/board.snxsch")),
        "the first item is untouched"
    );
    // Only the window bookkeeping ran: no close, no exit, no recovery sweep.
    assert!(task.units() <= 1, "units: {}", task.units());

    // Answering again, now with the full list, discards.
    let task = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    assert!(app.ui_state.app_quit_confirm.is_none());
    assert!(task.units() > 1, "now it exits");
}

#[test]
fn a_lock_closes_the_menus_and_the_palette_behind_it() {
    let (mut app, _main) = quit_app();
    app.ui_state.command_palette.open = true;
    app.ui_state.panel_list_open = true;
    app.interaction_state.context_menu = Some(crate::app::ContextMenuState {
        x: 10.0,
        y: 10.0,
        world: None,
    });
    let _ = app.sync_detached_modals();
    assert!(app.interaction_state.context_menu.is_some(), "no lock yet");
    app.ui_state.app_quit_confirm = Some(crate::app::AppQuitConfirmState {
        dirty_paths: vec!["/p/board.snxsch".into()],
        dirty_rows: Vec::new(),
    });
    let _ = app.sync_detached_modals();
    assert!(app.locking_confirm_window().is_some());
    assert!(app.interaction_state.context_menu.is_none());
    assert!(!app.ui_state.command_palette.open);
    assert!(!app.ui_state.panel_list_open);
}

#[test]
fn the_locking_window_is_the_topmost_dialog_every_time() {
    let (mut app, main) = quit_app();
    app.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
        target_path: "p.snxsch".into(),
        tree_path: vec![0],
        project_id: None,
        buffer: "p".into(),
        error: None,
        is_project_rename: false,
    });
    raise_quit_confirm(&mut app, main);
    let quit = app
        .detached_window_of(ModalId::AppQuitConfirm)
        .expect("quit window");
    for _ in 0..20 {
        assert_eq!(
            app.locking_confirm_window(),
            Some(quit),
            "Esc acts on the quit confirm"
        );
    }
}

#[test]
fn a_recovery_prompt_whose_window_vanishes_twice_stays_in_the_window() {
    let (mut app, _main) = quit_app();
    app.document_state
        .autosave
        .offer(crate::autosave::RecoveryOffer {
            path: "p.snxsch".into(),
            age_secs: 90,
            older_than_file: false,
        });
    for expected_windows in [1usize, 1] {
        let _ = app.sync_detached_modals();
        let id = app
            .detached_window_of(ModalId::AutosaveRecovery)
            .expect("window");
        assert_eq!(app.ui_state.windows.len(), expected_windows);
        let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(id)));
    }
    let _ = app.sync_detached_modals();
    assert!(
        app.detached_window_of(ModalId::AutosaveRecovery).is_none(),
        "no third window: the prompt paints in the main window"
    );
    assert!(
        app.document_state.autosave.current_offer().is_some(),
        "never decided for the user"
    );
}

#[test]
fn discard_all_never_deletes_a_copy_the_user_kept_with_later() {
    // An earlier run's copy kept with "Later" is deferred, never written by
    // this run, so it is not among the sidecars Discard All removes. Only a
    // copy the user chose to Restore is adopted (it then describes the open
    // document) and a copy this run wrote is tracked.
    let (mut app, _main) = quit_app();
    let kept = std::path::PathBuf::from("/p/kept-later.snxsch");
    let restored = std::path::PathBuf::from("/p/restored.snxsch");
    app.document_state.autosave.defer(&kept);
    app.document_state.autosave.adopt_on_disk(&restored);
    let removed = app.document_state.autosave.take_all_written();
    assert!(
        !removed.contains(&kept),
        "a copy kept with Later survives Discard All"
    );
    assert!(
        removed.contains(&restored),
        "only the adopted (restored) copy is removed"
    );
}
