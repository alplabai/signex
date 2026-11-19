//! #2532 — the Tab form's Position X / Y (a wire's Start and End) are
//! editable: a typed point is where OK places the next object, Y read from
//! the sheet's page origin; untyped fields follow the cursor; the typed
//! point is not carried over to the object after it.

use signex_app::app::{EditMsg, Message, Signex, Tool};
use signex_app::canvas::CanvasEvent;
use signex_app::dock::DockMessage;
use signex_app::panels::{PanelMsg, PositionField, PrePlacementPositionMsg};
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::Point;

use crate::library_place_symbol::{app_with_empty_sheet, doc};

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn tab_in(app: &mut Signex, tool: Tool) {
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(tool)));
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
}

fn type_field(app: &mut Signex, field: PositionField, text: &str) {
    panel(
        app,
        PanelMsg::PrePlacementPosition(PrePlacementPositionMsg::FieldTyped(field, text.to_owned())),
    );
}

/// The stored Y a Y typed as `shown` stands for on the active sheet.
fn stored_y(app: &Signex, shown: f64) -> f64 {
    app.document_state.panel_ctx.shown_y(shown)
}

fn typed(app: &Signex, field: PositionField) -> Option<f64> {
    app.document_state
        .panel_ctx
        .pre_placement
        .as_ref()
        .and_then(|pp| pp.position.value(field))
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

#[test]
fn a_typed_position_and_ok_place_a_net_label_there() {
    let mut app = app_with_empty_sheet();
    tab_in(&mut app, Tool::Label);
    type_field(&mut app, PositionField::X, "101.6");
    type_field(&mut app, PositionField::Y, "76.2");
    panel(&mut app, PanelMsg::ConfirmPrePlacement);

    let labels = &doc(&app).labels;
    assert_eq!(labels.len(), 1, "OK places the label, no click needed");
    let want = Point::new(101.6, stored_y(&app, 76.2));
    assert!(
        near(labels[0].position, want),
        "{:?} != {want:?}",
        labels[0].position
    );
    assert!(!app.interaction_state.active_canvas().placement_paused);
}

#[test]
fn a_typed_position_and_ok_place_a_junction_there() {
    let mut app = app_with_empty_sheet();
    tab_in(&mut app, Tool::Junction);
    type_field(&mut app, PositionField::X, "114.3");
    type_field(&mut app, PositionField::Y, "76.2");
    panel(&mut app, PanelMsg::ConfirmPrePlacement);

    let junctions = &doc(&app).junctions;
    assert_eq!(junctions.len(), 1);
    let want = Point::new(114.3, stored_y(&app, 76.2));
    assert!(
        near(junctions[0].position, want),
        "{:?} != {want:?}",
        junctions[0].position
    );
}

#[test]
fn typed_wire_start_and_end_and_ok_place_one_wire_in_one_undo_step() {
    let mut app = app_with_empty_sheet();
    tab_in(&mut app, Tool::Wire);
    type_field(&mut app, PositionField::X, "88.9");
    type_field(&mut app, PositionField::Y, "63.5");
    type_field(&mut app, PositionField::EndX, "114.3");
    type_field(&mut app, PositionField::EndY, "63.5");
    panel(&mut app, PanelMsg::ConfirmPrePlacement);

    let wires = doc(&app).wires.clone();
    assert_eq!(wires.len(), 1, "one straight wire from Start to End");
    let y = stored_y(&app, 63.5);
    assert!(
        near(wires[0].start, Point::new(88.9, y)),
        "{:?}",
        wires[0].start
    );
    assert!(
        near(wires[0].end, Point::new(114.3, y)),
        "{:?}",
        wires[0].end
    );

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert!(
        doc(&app).wires.is_empty(),
        "one undo step takes the wire away"
    );
}

#[test]
fn a_cursor_move_updates_the_untyped_fields_only() {
    let mut app = app_with_empty_sheet();
    tab_in(&mut app, Tool::Label);
    type_field(&mut app, PositionField::X, "50");
    let _ = app.update(Message::CanvasEvent(CanvasEvent::CursorAt {
        x: 25.4,
        y: 38.1,
        zoom_pct: 100.0,
    }));
    let pp = app
        .document_state
        .panel_ctx
        .pre_placement
        .as_ref()
        .expect("form open");
    assert!((pp.cursor_x_mm - 25.4).abs() < 1e-3, "{}", pp.cursor_x_mm);
    assert!((pp.cursor_y_mm - 38.1).abs() < 1e-3, "{}", pp.cursor_y_mm);
    assert_eq!(
        pp.position.value(PositionField::X),
        Some(50.0),
        "typed X stays"
    );
    assert_eq!(
        pp.position.value(PositionField::Y),
        None,
        "Y follows the cursor"
    );
}

#[test]
fn the_typed_position_is_cleared_after_placement() {
    let mut app = app_with_empty_sheet();
    tab_in(&mut app, Tool::Label);
    type_field(&mut app, PositionField::X, "101.6");
    type_field(&mut app, PositionField::Y, "76.2");
    panel(&mut app, PanelMsg::ConfirmPrePlacement);
    assert_eq!(typed(&app, PositionField::X), None);
    assert_eq!(typed(&app, PositionField::Y), None);

    // OK again with nothing typed places nothing more.
    panel(&mut app, PanelMsg::ConfirmPrePlacement);
    assert_eq!(doc(&app).labels.len(), 1);
}

#[test]
fn ok_with_nothing_typed_only_resumes_placing() {
    let mut app = app_with_empty_sheet();
    tab_in(&mut app, Tool::Junction);
    assert!(app.interaction_state.active_canvas().placement_paused);
    panel(&mut app, PanelMsg::ConfirmPrePlacement);
    assert!(!app.interaction_state.active_canvas().placement_paused);
    assert!(doc(&app).junctions.is_empty());
}
