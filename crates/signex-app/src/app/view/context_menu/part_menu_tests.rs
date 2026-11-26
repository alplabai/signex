//! #3063: the Properties part ⋯ menu answers Esc, closes with every other
//! menu, and opens in the window that shows the Properties panel.

use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::layout_probe::{find, lay_out_texts};
use crate::panels::{PanelKind, PanelMsg, PartPanelMsg};

const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);
const PART: uuid::Uuid = uuid::Uuid::from_u128(42);

fn part(msg: PartPanelMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::Part(msg)))
}

/// The app with the selected part's ⋯ menu opened by a press at `at`.
fn app_with_part_menu(at: (f32, f32)) -> (Signex, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = app.ui_state.main_window_id.expect("the main window");
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    app.interaction_state.last_mouse_pos = at;
    let _ = app.update(part(PartPanelMsg::MenuToggled(PART)));
    // The panel shows the part as selected.
    app.document_state.panel_ctx.selected_uuid = Some(PART);
    assert_eq!(app.document_state.panel_ctx.part_panel.menu_for, Some(PART));
    (app, main)
}

#[test]
fn esc_closes_the_part_menu() {
    let (mut app, _main) = app_with_part_menu((100.0, 100.0));
    let esc = app.escape_overlay_message().expect("the menu claims Esc");
    assert!(
        matches!(
            esc,
            Message::Dock(DockMessage::Panel(PanelMsg::Part(
                PartPanelMsg::MenuDismissed
            )))
        ),
        "{esc:?}"
    );
    let _ = app.update(esc);
    assert_eq!(app.document_state.panel_ctx.part_panel.menu_for, None);
}

#[test]
fn closing_every_context_menu_closes_the_part_menu() {
    let (mut app, main) = app_with_part_menu((100.0, 100.0));
    assert!(!app.part_menu_overlay(main).is_empty());
    app.close_all_context_menus();
    assert_eq!(app.document_state.panel_ctx.part_panel.menu_for, None);
    assert!(app.part_menu_overlay(main).is_empty());
}

/// A Properties panel floated into a window of its own opens the menu in
/// that window, at the press, and not in the main window.
#[test]
fn a_detached_properties_window_opens_the_menu_in_itself() {
    let (mut app, main) = app_with_part_menu((120.0, 160.0));
    let panel = iced::window::Id::unique();
    app.ui_state.windows.insert(
        panel,
        crate::app::state::WindowKind::DetachedPanel(PanelKind::Properties),
    );
    assert!(
        app.part_menu_overlay(main).is_empty(),
        "not in the main window"
    );
    let texts = lay_out_texts(app.view(panel), WINDOW);
    let row = find(&texts, "Open Datasheet").bounds;
    assert!(
        (row.y - 160.0).abs() < 40.0,
        "level with the press: {row:?}"
    );
    let main_texts = lay_out_texts(app.view(main), WINDOW);
    assert!(
        !main_texts.iter().any(|t| t.text == "Open Datasheet"),
        "the main window paints no part menu"
    );
    // Its Esc and its scrim close the menu the detached panel shows.
    let esc = app.escape_overlay_message().expect("the menu claims Esc");
    let _ = app.update(esc);
    assert_eq!(app.document_state.panel_ctx.part_panel.menu_for, None);
}
