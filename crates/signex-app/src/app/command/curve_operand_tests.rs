//! The curve drawings with operands (#1267): `draw_ellipse`,
//! `draw_elliptical_arc` and `draw_round_rectangle` place the drawing the
//! clicks of the same tool place, as one undo step, and answer with its id
//! and kind.

use signex_types::schematic::{CurveShape, Point, SchDrawing, angle_deg};

use super::*;
use crate::app::handlers::canvas::placement_tests::{app_with_tool, click, doc, grid, sheet_shape};
use crate::keymap::AppCommandId;

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

fn pt(p: Point) -> String {
    format!("{},{}", p.x, p.y)
}

fn run(app: &mut Signex, id: &str, operands: &[String]) -> CommandOutcome {
    let id = command(id);
    let operands: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&id, &operands).expect("operands");
    app.run_operands_settled(&id, &args).0
}

/// An app on an empty sheet whose form holds the project defaults, so a
/// click places what an operand run places.
fn sheet() -> Signex {
    let mut app = app_with_tool(crate::app::Tool::Select);
    let pre = app.document_state.new_pre_placement(
        "tool".into(),
        crate::panels::PrePlacementKind::Other,
        String::new(),
        (0.0, 0.0),
    );
    app.document_view.panel_ctx.pre_placement = Some(pre);
    app
}

/// One curve id: its clicks, and the operands the same shape takes.
struct Case {
    id: &'static str,
    clicks: Vec<Point>,
    operands: Vec<String>,
    kind: &'static str,
}

fn cases() -> Vec<Case> {
    let c = grid(8.0, 16.0);
    let corner = grid(12.0, 18.0);
    let (rx, ry) = ((corner.x - c.x).abs(), (corner.y - c.y).abs());
    let (start, end) = (grid(10.0, 18.0), grid(4.0, 16.0));
    let arc_clicks = vec![c, grid(12.0, 0.0), grid(0.0, 18.0), start, end];
    let ry_arc = (arc_clicks[2].y - c.y).abs();
    let b = grid(24.0, 24.0);
    vec![
        Case {
            id: "draw_ellipse",
            clicks: vec![c, corner],
            operands: vec![
                format!("center={}", pt(c)),
                format!("radius_x={rx}"),
                format!("radius_y={ry}"),
            ],
            kind: "ellipse",
        },
        Case {
            id: "draw_elliptical_arc",
            operands: vec![
                format!("center={}", pt(c)),
                format!("radius_x={rx}"),
                format!("radius_y={ry_arc}"),
                format!("start_angle={}", angle_deg(c, start)),
                format!("end_angle={}", angle_deg(c, end)),
            ],
            clicks: arc_clicks,
            kind: "elliptical_arc",
        },
        Case {
            id: "draw_round_rectangle",
            clicks: vec![c, b],
            operands: vec![format!("start={}", pt(c)), format!("end={}", pt(b))],
            kind: "round_rect",
        },
    ]
}

fn clicked(case: &Case) -> Signex {
    let mut app = sheet();
    let _ = app.dispatch_command_reporting(&command(case.id), CommandArgs::none(), None);
    for at in &case.clicks {
        click(&mut app, (at.x, at.y));
    }
    app
}

#[test]
fn every_curve_command_with_operands_makes_the_clicked_drawing() {
    for case in cases() {
        let clicked = clicked(&case);
        let mut app = sheet();
        let empty = sheet_shape(&app);
        let outcome = run(&mut app, case.id, &case.operands);
        let CommandOutcome::Answered(answer) = outcome else {
            panic!("{}: {outcome:?}", case.id);
        };
        assert_eq!(answer["kind"], case.kind, "{}", case.id);
        let placed = doc(&app).drawings[0].uuid().to_string();
        assert_eq!(answer["id"], placed.as_str(), "{}", case.id);
        assert_ne!(sheet_shape(&app), empty, "{} placed nothing", case.id);
        assert_eq!(sheet_shape(&app), sheet_shape(&clicked), "{}", case.id);
    }
}

#[test]
fn one_undo_removes_a_curve_placed_with_operands() {
    for case in cases() {
        let mut app = sheet();
        let empty = sheet_shape(&app);
        run(&mut app, case.id, &case.operands);
        let _ = app.dispatch_command_reporting(&command("undo"), CommandArgs::none(), None);
        assert_eq!(sheet_shape(&app), empty, "{}", case.id);
    }
}

#[test]
fn a_curve_with_operands_and_no_schematic_is_disabled() {
    let (mut app, _task) = Signex::new();
    for case in cases() {
        let outcome = run(&mut app, case.id, &case.operands);
        assert!(
            matches!(outcome, CommandOutcome::Disabled { .. }),
            "{}: {outcome:?}",
            case.id
        );
    }
}

#[test]
fn an_arc_that_sweeps_nothing_is_refused_and_changes_nothing() {
    let mut app = sheet();
    let before = sheet_shape(&app);
    let operands = [
        format!("center={}", pt(grid(4.0, 4.0))),
        "radius_x=2.54".to_string(),
        "radius_y=1.27".to_string(),
        "start_angle=30".to_string(),
        "end_angle=-330".to_string(),
    ];
    let outcome = run(&mut app, "draw_elliptical_arc", &operands);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert_eq!(sheet_shape(&app), before);
}

#[test]
fn an_off_grid_centre_is_refused() {
    let mut app = sheet();
    let off = Point::new(grid(4.0, 4.0).x + 0.3, grid(4.0, 4.0).y);
    let operands = [
        format!("center={}", pt(off)),
        "radius_x=2.54".to_string(),
        "radius_y=1.27".to_string(),
    ];
    let outcome = run(&mut app, "draw_ellipse", &operands);
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert!(doc(&app).drawings.is_empty());
}

#[test]
fn a_corner_radius_operand_is_held_to_half_the_short_side() {
    let mut app = sheet();
    let (start, end) = (grid(0.0, 0.0), grid(8.0, 2.0));
    let operands = [
        format!("start={}", pt(start)),
        format!("end={}", pt(end)),
        "corner_radius=50".to_string(),
    ];
    run(&mut app, "draw_round_rectangle", &operands);
    let SchDrawing::Curve(c) = &doc(&app).drawings[0] else {
        panic!("a curve drawing");
    };
    let CurveShape::RoundRect { corner_radius, .. } = c.shape else {
        panic!("a round rectangle");
    };
    assert_eq!(corner_radius, (end.y - start.y) / 2.0);
}
