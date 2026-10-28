//! The library dialogs in their own windows: the per-browser ones stay bound
//! to the browser that opened them (docs/DETACHED_MODALS.md).

use std::path::{Path, PathBuf};

use crate::app::state::ModalId;
use crate::app::{InputMsg, Message, Signex, WindowMsg};
use crate::layout_probe::lay_out_texts;
use crate::library::state::{DeleteConfirmState, LibraryBrowserState};

fn two_browsers() -> (Signex, iced::window::Id, [PathBuf; 2]) {
    let (mut app, _task) = Signex::new();
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.main_window_opened = true;
    app.ui_state.first_run_tour_open = false;
    let paths = [
        PathBuf::from("/libs/a.snxlib"),
        PathBuf::from("/libs/b.snxlib"),
    ];
    for (path, pn) in paths.iter().zip(["PN-A", "PN-B"]) {
        let mut browser = LibraryBrowserState::new(path.clone());
        browser.delete_confirm = Some(DeleteConfirmState {
            table: "t".into(),
            row_id: signex_library::RowId::new(),
            internal_pn: pn.into(),
        });
        app.library.library_browsers.insert(path.clone(), browser);
    }
    (app, main, paths)
}

fn window_for(app: &Signex, path: &Path) -> iced::window::Id {
    app.browser_modal_window(ModalId::LibraryDeleteConfirm, path)
        .expect("a window bound to the browser")
}

fn delete_open(app: &Signex, path: &Path) -> bool {
    app.library.library_browsers[path].delete_confirm.is_some()
}

#[test]
fn each_browsers_dialog_gets_its_own_window_bound_to_that_browser() {
    let (mut app, _main, [a, b]) = two_browsers();
    let _ = app.sync_detached_modals();
    let (wa, wb) = (window_for(&app, &a), window_for(&app, &b));
    assert_ne!(wa, wb);
    let size = iced::Size::new(520.0, 260.0);
    let texts_a = lay_out_texts(app.view(wa), size);
    let texts_b = lay_out_texts(app.view(wb), size);
    let has = |texts: &[crate::layout_probe::LaidOutText], needle: &str| {
        texts.iter().any(|t| t.text.contains(needle))
    };
    assert!(
        has(&texts_a, "PN-A") && !has(&texts_a, "PN-B"),
        "window A shows browser A"
    );
    assert!(
        has(&texts_b, "PN-B") && !has(&texts_b, "PN-A"),
        "window B shows browser B"
    );
    // A second pass opens nothing new.
    let _ = app.sync_detached_modals();
    assert_eq!(app.ui_state.windows.len(), 2);
}

#[test]
fn answering_one_browsers_dialog_leaves_the_other_alone() {
    let (mut app, _main, [a, b]) = two_browsers();
    let _ = app.sync_detached_modals();
    let (wa, wb) = (window_for(&app, &a), window_for(&app, &b));
    // OS close of A's window is A's Cancel: nothing deleted, B untouched.
    let _ = app.update(Message::Window(WindowMsg::WindowCloseRequested(wa)));
    assert!(!delete_open(&app, &a), "A cancelled");
    assert!(delete_open(&app, &b), "B still asks");
    assert!(
        app.browser_modal_window(ModalId::LibraryDeleteConfirm, &a)
            .is_none()
    );
    assert_eq!(window_for(&app, &b), wb, "B keeps its window");
    // Esc in B's window cancels B, not A (already gone) and not "the first".
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: Some(wb) }));
    assert!(!delete_open(&app, &b));
}

#[test]
fn closing_a_browser_closes_its_dialogs_and_the_others_stay() {
    let (mut app, _main, [a, b]) = two_browsers();
    let _ = app.sync_detached_modals();
    let wb = window_for(&app, &b);
    app.library.library_browsers.remove(&a);
    let _ = app.sync_detached_modals();
    assert!(
        app.browser_modal_window(ModalId::LibraryDeleteConfirm, &a)
            .is_none()
    );
    assert_eq!(window_for(&app, &b), wb);
    assert!(delete_open(&app, &b), "nothing was answered for B");
}

#[test]
fn a_delete_confirm_locks_the_main_window() {
    let (mut app, main, _paths) = two_browsers();
    let _ = app.sync_detached_modals();
    assert!(app.locking_confirm_window().is_some());
    assert!(app.has_locking_confirm_modal());
    let key = iced::keyboard::Event::KeyPressed {
        key: iced::keyboard::Key::Character("w".into()),
        modified_key: iced::keyboard::Key::Character("w".into()),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::empty(),
        text: Some("w".into()),
        repeat: false,
    };
    assert!(
        app.route_key_for_test(main, &key).is_none(),
        "main takes no key"
    );
}

#[test]
fn a_tool_dialog_does_not_lock_and_a_recovery_prompt_is_a_confirm() {
    let (mut app, _main, _paths) = two_browsers();
    for browser in app.library.library_browsers.values_mut() {
        browser.delete_confirm = None;
    }
    app.ui_state.net_color_custom.show = true;
    let _ = app.sync_detached_modals();
    assert!(app.detached_window_of(ModalId::NetColorCustom).is_some());
    assert!(
        app.locking_confirm_window().is_none(),
        "a picker is a tool window"
    );
    // OS close of the picker cancels it.
    let id = app
        .detached_window_of(ModalId::NetColorCustom)
        .expect("window");
    let _ = app.update(Message::Window(WindowMsg::WindowCloseRequested(id)));
    assert!(!app.ui_state.net_color_custom.show);

    app.library.recovery = Some(crate::library::recovery::RecoveryDialog::LibraryMissing {
        path: "l.snxlib".into(),
    });
    let _ = app.sync_detached_modals();
    let id = app
        .detached_window_of(ModalId::LibraryRecovery)
        .expect("window");
    assert_eq!(app.locking_confirm_window(), Some(id));
    // Closing it is the Cancel choice, never a decision that deletes or rebinds.
    let message = app
        .window_close_message(id, ModalId::LibraryRecovery)
        .expect("a message");
    assert!(format!("{message:?}").contains("Cancel"), "{message:?}");
}

fn edit_row_state(path: &Path) -> crate::library::state::EditRowModalState {
    use signex_library::{
        ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
        ParamMap, PlmReserved, PrimitiveRef,
    };
    let row_id = signex_library::RowId::new();
    let draft = ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: row_id.0,
        internal_pn: InternalPn::new("TEST_PN"),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
        content_hash: [0u8; 32],
        server_revision: None,
    };
    let address = crate::library::state::EditorAddress::new(path.to_path_buf(), "t".into(), row_id);
    crate::library::state::EditRowModalState::new(address, draft)
}

#[test]
fn an_edit_form_whose_window_vanishes_keeps_its_text_and_comes_back_twice_at_most() {
    let (mut app, _main, [a, _b]) = two_browsers();
    for browser in app.library.library_browsers.values_mut() {
        browser.delete_confirm = None;
    }
    let mut state = edit_row_state(&a);
    state.tags_buf = "typed but not saved".into();
    app.library
        .library_browsers
        .get_mut(&a)
        .expect("a")
        .edit_modal = Some(state);
    for _ in 0..2 {
        let _ = app.sync_detached_modals();
        let id = app
            .browser_modal_window(ModalId::LibraryEditRow, &a)
            .expect("the edit window");
        let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(id)));
        let kept = app.library.library_browsers[&a]
            .edit_modal
            .as_ref()
            .expect("form kept");
        assert_eq!(kept.tags_buf, "typed but not saved");
    }
    let _ = app.sync_detached_modals();
    assert!(
        app.browser_modal_window(ModalId::LibraryEditRow, &a)
            .is_none(),
        "after two vanishes the form stays in the main window"
    );
    assert!(app.library.library_browsers[&a].edit_modal.is_some());
}

#[test]
fn the_per_browser_delete_confirm_fills_its_window_on_the_theme_surface() {
    use signex_types::theme::ThemeId;
    for theme_id in [ThemeId::CatppuccinMocha, ThemeId::SolarizedLight] {
        let (mut app, _main, [a, _b]) = two_browsers();
        app.ui_state.theme_id = theme_id;
        app.document_state.panel_ctx.tokens = app.tokens_for(theme_id);
        let _ = app.sync_detached_modals();
        let id = window_for(&app, &a);
        let size = crate::app::handlers::erc::modal_size(ModalId::LibraryDeleteConfirm);
        let root = crate::layout_probe::root_size(app.view(id), size);
        assert_eq!(
            (root.width, root.height),
            (size.width, size.height),
            "{theme_id:?}"
        );
        let right = lay_out_texts(app.view(id), size)
            .iter()
            .map(|t| t.bounds.x + t.bounds.width)
            .fold(0.0_f32, f32::max);
        assert!(
            size.width - right <= 48.0,
            "{theme_id:?}: {} px short",
            size.width - right
        );
        let paper = crate::styles::ti(app.document_state.panel_ctx.tokens.paper);
        assert_eq!(app.theme(id).expect("theme").palette().background, paper);
    }
}
