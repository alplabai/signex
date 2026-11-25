//! Mockup row 64, decided 2026-11-24 (docs/DECISIONS.md): an undocked sheet
//! window has no zoom pill in v1.0, since it has no known canvas rectangle;
//! the main window keeps its pill.

use std::path::PathBuf;

use crate::app::{Message, Signex, TabInfo, TabKind, WindowMsg};
use crate::layout_probe::{LaidOutText, lay_out_texts};

const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);

fn shows(texts: &[LaidOutText], label: &str) -> bool {
    texts.iter().any(|t| t.text == label)
}

/// The main window shows `pin-main.snxsch`; an undocked window shows
/// `pin-window.snxsch`. Returns the app and both window ids.
fn app_with_an_undocked_sheet() -> (Signex, iced::window::Id, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    let shown = PathBuf::from("pin-main.snxsch");
    let other = PathBuf::from("pin-window.snxsch");
    for path in [&shown, &other] {
        let engine =
            signex_engine::Engine::new_with_path(Default::default(), Some(path.to_path_buf()))
                .expect("engine");
        app.document_state
            .engines
            .insert(path.to_path_buf(), engine);
        app.document_state.tabs.push(TabInfo {
            title: path.display().to_string(),
            path: path.to_path_buf(),
            cached_document: None,
            project_id: None,
            kind: TabKind::Schematic,
        });
    }
    let _ = app.update(Message::Window(WindowMsg::UndockTab(1)));
    let window = *app.ui_state.windows.keys().next().expect("undocked window");
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: other,
        id: window,
    }));
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(shown);
    (app, main, window)
}

#[test]
fn an_undocked_sheet_window_has_no_zoom_pill_and_the_main_window_has_one() {
    let (app, main, window) = app_with_an_undocked_sheet();
    let main_texts = lay_out_texts(app.view(main), WINDOW);
    assert!(shows(&main_texts, "Fit"), "the main canvas has the pill");
    let window_texts = lay_out_texts(app.view(window), WINDOW);
    assert!(
        window_texts.iter().any(|t| t.text.contains("pin-window")),
        "the undocked window draws its own sheet's tab: {window_texts:#?}"
    );
    assert!(
        !shows(&window_texts, "Fit"),
        "an undocked window has no known canvas rectangle, so no pill: {window_texts:#?}"
    );
}
