//! A new wire that starts on an existing wire (its end or its body) takes
//! that wire's colour and width; one that only ends on it does not
//! (docs/DECISIONS.md, 2026-11-21).

use signex_app::app::{EditMsg, InputMsg, Message, Signex, Tool};
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::{Point, Wire};
use signex_types::theme::Color;

use crate::library_place_symbol::{app_with_empty_sheet, click, doc};

const RED: Color = Color::new(255, 0, 0, 255);

/// Draw one wire from `a` to `b` with the Wire tool and end the route.
fn draw(app: &mut Signex, a: (f64, f64), b: (f64, f64)) {
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
    click(app, a);
    click(app, b);
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
}

/// An app with one horizontal wire, styled `width` and `color` through
/// engine commands. The wire as placed.
fn app_with_wire(width: f64, color: Option<Color>) -> (Signex, Wire) {
    let mut app = app_with_empty_sheet();
    draw(&mut app, (10.0, 20.0), (30.0, 20.0));
    let id = doc(&app).wires[0].uuid;
    let engine = app
        .document_state
        .engines
        .values_mut()
        .next()
        .expect("engine");
    engine
        .execute(signex_engine::Command::SetWireWidth {
            wire_id: id,
            width_mm: width,
        })
        .expect("width");
    engine
        .execute(signex_engine::Command::SetWireColor { wire_id: id, color })
        .expect("colour");
    let wire = doc(&app).wires[0].clone();
    (app, wire)
}

fn new_wire(app: &Signex, old: &Wire) -> Wire {
    let wires = &doc(app).wires;
    assert_eq!(wires.len(), 2, "one wire was added: {wires:?}");
    wires
        .iter()
        .find(|w| w.uuid != old.uuid)
        .expect("new wire")
        .clone()
}

fn xy(p: Point) -> (f64, f64) {
    (p.x, p.y)
}

#[test]
fn a_wire_started_on_a_red_wires_end_takes_its_colour_and_width() {
    let (mut app, old) = app_with_wire(0.4, Some(RED));
    draw(&mut app, xy(old.end), (old.end.x, old.end.y + 10.0));
    let new = new_wire(&app, &old);
    assert_eq!(new.color, Some(RED));
    assert_eq!(new.stroke_width, 0.4);
}

#[test]
fn a_wire_started_on_a_red_wires_body_takes_its_colour_and_width() {
    let (mut app, old) = app_with_wire(0.4, Some(RED));
    let mid = ((old.start.x + old.end.x) / 2.0, old.start.y);
    draw(&mut app, mid, (mid.0, mid.1 + 10.0));
    let wires = &doc(&app).wires;
    let new = wires
        .iter()
        .find(|w| w.start.x == mid.0 && w.end.x == mid.0)
        .expect("vertical wire");
    assert_eq!(new.color, Some(RED));
    assert_eq!(new.stroke_width, 0.4);
}

#[test]
fn a_wire_started_in_empty_space_keeps_the_default_even_ending_on_a_red_wire() {
    let (mut app, old) = app_with_wire(0.4, Some(RED));
    draw(&mut app, (old.end.x, old.end.y + 10.0), xy(old.end));
    let new = new_wire(&app, &old);
    assert_eq!(new.color, None);
    assert_eq!(new.stroke_width, 0.0);
}

#[test]
fn a_wire_started_on_a_default_wire_keeps_the_default() {
    let (mut app, old) = app_with_wire(0.0, None);
    draw(&mut app, xy(old.end), (old.end.x, old.end.y + 10.0));
    let new = new_wire(&app, &old);
    assert_eq!(new.color, None);
    assert_eq!(new.stroke_width, 0.0);
}

#[test]
fn undo_removes_the_inheriting_wire_and_leaves_the_red_wire() {
    let (mut app, old) = app_with_wire(0.4, Some(RED));
    draw(&mut app, xy(old.end), (old.end.x, old.end.y + 10.0));
    let _ = new_wire(&app, &old);
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).wires, vec![old]);
}
