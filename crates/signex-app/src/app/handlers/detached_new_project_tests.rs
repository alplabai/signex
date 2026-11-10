//! The New Project dialog in its own OS window (#2277, docs/DETACHED_MODALS.md).

use iced::keyboard;

use crate::app::state::{ModalId, Signex};
use crate::app::{InputMsg, Message, NewProjectMsg, ProjectMsg, WindowMsg};
use crate::layout_probe::{find, lay_out_texts};
use crate::menu_bar::MenuMessage;

fn app_with_dialog() -> (Signex, iced::window::Id) {
    let (mut app, _task) = Signex::new();
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.main_window_opened = true;
    app.ui_state.first_run_tour_open = false;
    let _ = app.update(Message::Menu(MenuMessage::NewProject));
    let _ = app.sync_detached_modals();
    (app, main)
}

fn press(named: keyboard::key::Named) -> keyboard::Event {
    keyboard::Event::KeyPressed {
        key: keyboard::Key::Named(named),
        modified_key: keyboard::Key::Named(named),
        physical_key: keyboard::key::Physical::Unidentified(
            keyboard::key::NativeCode::Unidentified,
        ),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::default(),
        text: None,
        repeat: false,
    }
}

fn is_submit(message: &Option<Message>) -> bool {
    matches!(
        message,
        Some(Message::Project(ProjectMsg::Setup(
            NewProjectMsg::SubmitRequested
        )))
    )
}

#[test]
fn file_new_project_opens_a_native_modal_window_and_not_the_overlay() {
    let (mut app, main) = app_with_dialog();
    let window = app
        .detached_window_of(ModalId::NewProject)
        .expect("New Project has its own window");
    assert_eq!(app.title(window), "New Project");
    let main_paints = |app: &Signex| {
        lay_out_texts(app.view(main), iced::Size::new(1400.0, 900.0))
            .iter()
            .any(|t| t.text == "Create project")
    };
    assert!(
        !main_paints(&app),
        "the in-window overlay must not paint while the window exists"
    );
    // Control: with no window yet the in-window card is what shows.
    let (mut bare, bare_main) = (Signex::new().0, iced::window::Id::unique());
    bare.ui_state.main_window_id = Some(bare_main);
    bare.ui_state.first_run_tour_open = false;
    let _ = bare.update(Message::Menu(MenuMessage::NewProject));
    assert!(
        lay_out_texts(bare.view(bare_main), iced::Size::new(1400.0, 900.0))
            .iter()
            .any(|t| t.text == "Create project"),
        "the fallback card paints before the window exists"
    );
    assert_eq!(
        app.locking_confirm_window(),
        Some(window),
        "the main window is locked while the dialog is open"
    );
    // A second File > New Project raises the same window, it opens no second.
    let _ = app.update(Message::Menu(MenuMessage::NewProject));
    let _ = app.sync_detached_modals();
    assert_eq!(app.ui_state.windows.len(), 1);
    // The main window takes no ordinary key while locked.
    assert!(
        app.route_key_for_test(main, &press(keyboard::key::Named::Enter))
            .is_none()
    );
}

#[test]
fn enter_in_the_dialog_window_creates_and_an_open_dropdown_keeps_enter() {
    let (mut app, _main) = app_with_dialog();
    let window = app.detached_window_of(ModalId::NewProject).expect("window");
    assert!(is_submit(&app.route_key_for_test(
        window,
        &press(keyboard::key::Named::Enter)
    )));
    let _ = app.update(Message::from(NewProjectMsg::DropdownToggled(true)));
    assert!(
        app.route_key_for_test(window, &press(keyboard::key::Named::Enter))
            .is_none(),
        "an open dropdown owns Enter"
    );
}

#[test]
fn esc_and_the_os_close_cancel_the_dialog_and_close_its_window() {
    for via_esc in [false, true] {
        let (mut app, _main) = app_with_dialog();
        let window = app.detached_window_of(ModalId::NewProject).expect("window");
        let _ = app.update(if via_esc {
            Message::Input(InputMsg::EscapePressed {
                window: Some(window),
            })
        } else {
            Message::Window(WindowMsg::WindowCloseRequested(window))
        });
        let _ = app.sync_detached_modals();
        assert!(app.ui_state.new_project.is_none(), "esc: {via_esc}");
        assert!(
            app.detached_window_of(ModalId::NewProject).is_none(),
            "esc: {via_esc}"
        );
    }
}

#[test]
fn closing_the_window_while_create_writes_keeps_the_draft_and_the_window_returns() {
    let (mut app, _main) = app_with_dialog();
    let window = app.detached_window_of(ModalId::NewProject).expect("window");
    app.ui_state.new_project.as_mut().expect("draft").creating = Some("/p/x.snxprj".into());
    let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(window)));
    assert!(app.ui_state.new_project.is_some(), "the draft is kept");
    let _ = app.sync_detached_modals();
    assert!(app.detached_window_of(ModalId::NewProject).is_some());
}

/// With Customize open and a tall stack-up, the header-less card still has
/// its Cancel / Create project footer inside the window at any size.
#[test]
fn the_footer_stays_inside_the_window_at_any_size() {
    let (mut app, _main) = app_with_dialog();
    let state = app.ui_state.new_project.as_mut().expect("draft");
    state.customize_open = true;
    state.step_layers(30);
    let window = app.detached_window_of(ModalId::NewProject).expect("window");
    for (w, h) in [
        (860.0, 700.0),
        (860.0, 420.0),
        (700.0, 300.0),
        (1400.0, 900.0),
    ] {
        let size = iced::Size::new(w, h);
        let texts = lay_out_texts(app.view(window), size);
        for label in ["Cancel", "Create project"] {
            let b = find(&texts, label).bounds;
            assert!(
                b.height > 0.0 && b.y >= 0.0 && b.y + b.height <= h && b.x + b.width <= w,
                "{label:?} outside {w}x{h}: {b:?}"
            );
        }
    }
}
