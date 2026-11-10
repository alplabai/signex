//! A menu or dialog over the schematic canvas takes the cursor: the canvas
//! under it gets no hover, so no card shows through the overlay.

use iced::{Point, Size};
use signex_types::schematic::{Point as SheetPoint, Symbol};

use super::*;
use crate::canvas::CanvasEvent;
use crate::canvas::overlay_camera_tests::graphite_app_showing;
use crate::layout_probe::{lay_out_texts, messages_for_moves};

const WINDOW: Size = Size::new(1400.0, 900.0);
const R6: uuid::Uuid = uuid::Uuid::from_u128(6);

fn app() -> (Signex, iced::window::Id) {
    let (mut app, main) = graphite_app_showing(|sheet| {
        sheet.symbols.push(Symbol {
            uuid: R6,
            lib_id: "Device:R".to_string(),
            reference: "R6".to_string(),
            value: "10k".to_string(),
            footprint: "Chip-0805".to_string(),
            position: SheetPoint::new(40.0, 40.0),
            ..Default::default()
        });
    });
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    (app, main)
}

fn saw_cursor_at(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|m| matches!(m, Message::CanvasEvent(CanvasEvent::CursorAt { .. })))
}

fn saw_cursor_left(messages: &[Message]) -> bool {
    messages
        .iter()
        .any(|m| matches!(m, Message::CanvasEvent(CanvasEvent::CursorLeft)))
}

/// A point on the canvas and the same point shifted into the menu card.
fn open_menu(app: &mut Signex) -> Point {
    let at = app.canvas_bounds().center();
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(at.x, at.y)));
    Point::new(at.x + 6.0, at.y + 6.0)
}

fn rest_on_symbol(app: &mut Signex) {
    let interaction = &mut app.interaction_state;
    interaction.hover_symbol_uuid = Some(R6);
    interaction.hover_started_at =
        Some(std::time::Instant::now() - std::time::Duration::from_secs(2));
    interaction.hover_screen_pos = Some((100.0, 100.0));
}

fn card_shown(app: &Signex, main: iced::window::Id) -> bool {
    lay_out_texts(app.view(main), WINDOW)
        .iter()
        .any(|t| t.text == "R6")
}

#[test]
fn the_canvas_hovers_when_nothing_is_open() {
    let (app, main) = app();
    let at = app.canvas_bounds().center();
    let messages = messages_for_moves(app.view(main), WINDOW, &[at]);
    assert!(saw_cursor_at(&messages), "{messages:?}");
}

#[test]
fn a_menu_stops_the_cursor_reaching_the_canvas() {
    let (mut app, main) = app();
    let over_menu = open_menu(&mut app);
    let messages = messages_for_moves(app.view(main), WINDOW, &[over_menu]);
    assert!(
        !saw_cursor_at(&messages),
        "hover went through: {messages:?}"
    );
}

#[test]
fn hover_resumes_when_the_menu_closes() {
    let (mut app, main) = app();
    let over_menu = open_menu(&mut app);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Close));
    let messages = messages_for_moves(app.view(main), WINDOW, &[over_menu]);
    assert!(saw_cursor_at(&messages), "{messages:?}");
}

#[test]
fn a_dialog_stops_the_cursor_reaching_the_canvas() {
    let (mut app, main) = app();
    let at = app.canvas_bounds().center();
    app.ui_state.about_open = true;
    let messages = messages_for_moves(app.view(main), WINDOW, &[at]);
    assert!(
        !saw_cursor_at(&messages),
        "hover went through: {messages:?}"
    );
}

#[test]
fn opening_a_menu_drops_the_hover_that_was_active() {
    let (mut app, main) = app();
    rest_on_symbol(&mut app);
    assert!(card_shown(&app, main), "the fixture shows no card");
    let _ = open_menu(&mut app);
    assert_eq!(app.interaction_state.hover_symbol_uuid, None);
    assert!(!card_shown(&app, main), "the card stayed under the menu");
}

#[test]
fn no_card_is_painted_over_an_open_menu() {
    let (mut app, main) = app();
    let _ = open_menu(&mut app);
    rest_on_symbol(&mut app);
    assert!(!card_shown(&app, main), "the card paints over the menu");
}

#[test]
fn leaving_the_canvas_clears_the_symbol_card() {
    let (mut app, main) = app();
    rest_on_symbol(&mut app);
    assert!(card_shown(&app, main), "the fixture shows no card");
    let canvas = app.canvas_bounds();
    let off = Point::new(canvas.x + canvas.width + 20.0, canvas.y + 10.0);
    let moves = [canvas.center(), off];
    let messages = messages_for_moves(app.view(main), WINDOW, &moves);
    assert!(saw_cursor_left(&messages), "{messages:?}");
    for message in messages {
        let _ = app.update(message);
    }
    assert_eq!(app.interaction_state.hover_symbol_uuid, None);
    assert!(!card_shown(&app, main), "the card stayed up");
}
