//! #1057 — snapping onto the 1.27 mm grid must land on an exact nanometre
//! multiple. The grid pitch is stored as `f32`; widening it with
//! `f64::from` gives `1.2699999809…` and puts a point 100 steps out 2 nm
//! off the grid (126999998 nm instead of 127000000 nm).

use super::click_click_move::{app_with_wires, selection_of, wire_start};
use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::Message;
use signex_app::canvas::CanvasEvent;
use signex_types::schematic::{Point, Wire};

const STEPS_100_NM: i64 = 127_000_000;

fn nm(mm: f64) -> i64 {
    (mm * 1e6).round() as i64
}

/// Zero-length so the wire's anchor is its start point.
fn wire_at(x: f64) -> (Wire, uuid::Uuid) {
    let uuid = uuid::Uuid::now_v7();
    let wire = Wire {
        unknown_columns: Default::default(),
        uuid,
        start: Point::new(x, 0.0),
        end: Point::new(x, 0.0),
        stroke_width: 0.0,
        locked: false,
    };
    (wire, uuid)
}

#[test]
fn align_to_grid_lands_on_an_exact_nm_multiple() {
    let (wire, id) = wire_at(126.9);
    let (mut app, path) = app_with_wires(vec![wire]);
    app.ui_state.grid_size_mm = 1.27;
    app.interaction_state.active_canvas_mut().selected = selection_of(id);

    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::AlignToGrid,
    )));

    assert_eq!(nm(wire_start(&app, &path, id).x), STEPS_100_NM);
}

#[test]
fn move_selected_lands_on_an_exact_nm_multiple() {
    let (wire, id) = wire_at(0.0);
    let (mut app, path) = app_with_wires(vec![wire]);
    app.ui_state.grid_size_mm = 1.27;
    app.ui_state.snap_enabled = true;
    app.interaction_state.active_canvas_mut().selected = selection_of(id);

    let _ = app.update(Message::CanvasEvent(CanvasEvent::MoveSelected {
        dx: 127.1,
        dy: 0.0,
        connections: Default::default(),
    }));

    assert_eq!(nm(wire_start(&app, &path, id).x), STEPS_100_NM);
}
