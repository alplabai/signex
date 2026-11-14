//! #2372: flipping the corner and stepping the corner mode while a wire is
//! drawn, through the catalog commands and `Signex::update`.

use iced::{Point as P, Rectangle, Size};
use signex_i18n::{ids, t};
use signex_types::corner::CornerMode;
use signex_types::schematic::Point;

use super::super::super::*;
use super::placement_tests::{app_with_tool, click, grid};
use crate::app::command::CommandArgs;
use crate::app::command::bridge::core_to_message;
use crate::keymap::AppCommandId;

/// Run the catalog command `id` the way a key bound to it does.
fn run(app: &mut Signex, id: &str) {
    let command = AppCommandId::new(id).expect("a catalog id");
    let message = core_to_message(&command, &CommandArgs::none()).expect("bridged");
    let _ = app.update(message);
}

fn flip(app: &mut Signex) {
    run(app, "switch_segment_posture");
}

fn cycle(app: &mut Signex) {
    run(app, "cycle_wire_bus_graphic_mode");
}

fn at(p: Point) -> (f64, f64) {
    (p.x, p.y)
}

/// The rubber-band legs the canvas draws towards `cursor`, in screen space.
fn rubber(app: &Signex, cursor: Point) -> Vec<(P, P)> {
    let window = iced::window::Id::unique();
    let canvas = crate::canvas::SchematicCanvas::new(
        app.interaction_state.active_canvas(),
        app.canvas_view_prefs(window),
    );
    let bounds = Rectangle::new(P::ORIGIN, Size::new(2000.0, 2000.0));
    canvas
        .wire_preview_strokes(bounds, P::ORIGIN, Some(cursor))
        .expect("a wire is in progress")
        .rubber
        .into_iter()
        .map(|(a, b, _)| (a, b))
        .collect()
}

fn is_horizontal((a, b): (P, P)) -> bool {
    a.y == b.y && a.x != b.x
}

fn is_vertical((a, b): (P, P)) -> bool {
    a.x == b.x && a.y != b.y
}

fn wire_ends(app: &Signex) -> Vec<(Point, Point)> {
    let engine = app.document_state.active_engine().expect("engine");
    engine
        .document()
        .wires
        .iter()
        .map(|w| (w.start, w.end))
        .collect()
}

fn has_wire(app: &Signex, a: Point, b: Point) -> bool {
    let near = |p: Point, q: Point| (p.x - q.x).abs() < 1e-6 && (p.y - q.y).abs() < 1e-6;
    wire_ends(app)
        .iter()
        .any(|&(s, e)| (near(s, a) && near(e, b)) || (near(s, b) && near(e, a)))
}

#[test]
fn the_flip_turns_the_preview_corner_and_adds_no_undo_step() {
    let mut app = app_with_tool(Tool::Wire);
    click(&mut app, at(grid(0.0, 0.0)));
    let cursor = grid(10.0, 6.0);

    let legs = rubber(&app, cursor);
    assert_eq!(legs.len(), 2);
    assert!(is_horizontal(legs[0]) && is_vertical(legs[1]), "{legs:?}");

    flip(&mut app);
    let legs = rubber(&app, cursor);
    assert_eq!(legs.len(), 2);
    assert!(is_vertical(legs[0]) && is_horizontal(legs[1]), "{legs:?}");

    let engine = app.document_state.active_engine().expect("engine");
    assert!(!engine.can_undo(), "a flip is not an edit");
    assert!(wire_ends(&app).is_empty());
    assert!(app.interaction_state.wire_drawing, "the run is still open");

    flip(&mut app);
    let legs = rubber(&app, cursor);
    assert!(is_horizontal(legs[0]), "a second flip turns it back");
}

#[test]
fn the_mode_steps_through_90_45_start_45_end_and_any_angle() {
    let mut app = app_with_tool(Tool::Wire);
    click(&mut app, at(grid(0.0, 0.0)));
    let mut seen = Vec::new();
    for _ in 0..4 {
        cycle(&mut app);
        seen.push(app.interaction_state.corner_rule.mode);
    }
    assert_eq!(
        seen,
        [
            CornerMode::Angle45Start,
            CornerMode::Angle45End,
            CornerMode::FreeAngle,
            CornerMode::Ortho90,
        ]
    );
    let engine = app.document_state.active_engine().expect("engine");
    assert!(!engine.can_undo(), "a mode change is not an edit");

    // 45 degrees start: the diagonal leads, the horizontal leg follows.
    cycle(&mut app);
    let legs = rubber(&app, grid(10.0, 6.0));
    assert_eq!(legs.len(), 2);
    assert!(!is_horizontal(legs[0]) && !is_vertical(legs[0]), "{legs:?}");
    assert!(is_horizontal(legs[1]), "{legs:?}");
    // Any angle: one straight leg to the cursor.
    cycle(&mut app);
    cycle(&mut app);
    assert_eq!(rubber(&app, grid(10.0, 6.0)).len(), 1);
}

#[test]
fn the_status_bar_names_the_corner_while_the_wire_tool_is_active() {
    let mut app = app_with_tool(Tool::Wire);
    let window = iced::window::Id::unique();
    let i18n = app.ui_state.i18n.clone();
    let note = |id| {
        Some(crate::status_bar::Activity::note(
            t!(i18n, id).into_string(),
        ))
    };

    assert_eq!(
        app.status_activity(window),
        note(ids::EDITORS_WIRE_CORNER_HORIZONTAL_FIRST)
    );
    flip(&mut app);
    assert_eq!(
        app.status_activity(window),
        note(ids::EDITORS_WIRE_CORNER_VERTICAL_FIRST)
    );
    cycle(&mut app);
    // 45 degrees start, flipped: the diagonal comes last.
    assert_eq!(
        app.status_activity(window),
        note(ids::EDITORS_WIRE_CORNER_DIAGONAL_LAST)
    );
    cycle(&mut app);
    assert_eq!(
        app.status_activity(window),
        note(ids::EDITORS_WIRE_CORNER_DIAGONAL_FIRST)
    );
    cycle(&mut app);
    assert_eq!(
        app.status_activity(window),
        note(ids::EDITORS_WIRE_CORNER_ANY_ANGLE)
    );

    app.interaction_state.current_tool = Tool::Select;
    assert_eq!(
        app.status_activity(window),
        None,
        "only while drawing wires"
    );
}

#[test]
fn committed_wires_follow_the_chosen_corner_and_the_flip_persists() {
    let mut app = app_with_tool(Tool::Wire);
    click(&mut app, at(grid(0.0, 0.0)));
    flip(&mut app);
    click(&mut app, at(grid(10.0, 6.0)));
    assert!(
        has_wire(&app, grid(0.0, 0.0), grid(0.0, 6.0))
            && has_wire(&app, grid(0.0, 6.0), grid(10.0, 6.0)),
        "vertical first: {:?}",
        wire_ends(&app)
    );

    // The next segment keeps the flip.
    click(&mut app, at(grid(14.0, 10.0)));
    assert!(
        has_wire(&app, grid(10.0, 6.0), grid(10.0, 10.0))
            && has_wire(&app, grid(10.0, 10.0), grid(14.0, 10.0)),
        "still vertical first: {:?}",
        wire_ends(&app)
    );
    assert!(app.interaction_state.corner_rule.is_flipped);
}

#[test]
fn a_45_degree_commit_matches_its_preview() {
    let mut app = app_with_tool(Tool::Bus);
    click(&mut app, at(grid(0.0, 0.0)));
    cycle(&mut app);
    cycle(&mut app);
    assert_eq!(
        app.interaction_state.corner_rule.mode,
        CornerMode::Angle45End
    );
    click(&mut app, at(grid(10.0, 4.0)));
    let buses: Vec<(Point, Point)> = app
        .document_state
        .active_engine()
        .expect("engine")
        .document()
        .buses
        .iter()
        .map(|b| (b.start, b.end))
        .collect();
    let near = |p: Point, q: Point| (p.x - q.x).abs() < 1e-6 && (p.y - q.y).abs() < 1e-6;
    let corner = grid(6.0, 0.0);
    assert_eq!(buses.len(), 2, "{buses:?}");
    assert!(near(buses[0].0, grid(0.0, 0.0)) && near(buses[0].1, corner));
    assert!(near(buses[1].0, corner) && near(buses[1].1, grid(10.0, 4.0)));
}
