//! #2836: the Linear and Aligned dimension tools driven once by their three
//! canvas clicks (the GUI path) and once by their operands (the agent's and
//! the CLI's path). Both leave the same sheet, each placement is one undo
//! step, and the operands stand for the tool's Properties form without ever
//! reading it.

use serde_json::json;
use signex_types::schematic::{
    DimensionArrows, DimensionDirection, DimensionUnits, Point, SchDimension, SchDrawing,
};

use super::*;
use crate::app::Tool;
use crate::app::handlers::canvas::placement_tests::{app_with_tool, click, doc, grid, sheet_shape};
use crate::keymap::AppCommandId;

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let id = command(id);
    let args = CommandArgs::parse_operands(&id, operands).expect("operands");
    app.run_operands_settled(&id, &args).0
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refused_with(outcome: &CommandOutcome, needle: &str) -> bool {
    matches!(outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains(needle))
}

fn undo(app: &mut Signex) {
    let _ = app.dispatch_command_reporting(&command("undo"), CommandArgs::none(), None);
}

fn pt(name: &str, p: Point) -> String {
    format!("{name}={},{}", p.x, p.y)
}

fn only_dimension(app: &Signex) -> &SchDimension {
    let drawings = &doc(app).drawings;
    assert_eq!(drawings.len(), 1, "{drawings:?}");
    match &drawings[0] {
        SchDrawing::Dimension(d) => d,
        other => panic!("not a dimension: {other:?}"),
    }
}

/// Each tool, its command and its three clicks: the two measured points,
/// then a point on the dimension line.
fn cases() -> Vec<(Tool, &'static str, [Point; 3])> {
    vec![
        (
            Tool::LinearDimension,
            "place_linear_dimension",
            [grid(0.0, 0.0), grid(10.0, 2.0), grid(4.0, -4.0)],
        ),
        (
            Tool::AlignedDimension,
            "place_aligned_dimension",
            [grid(0.0, 0.0), grid(3.0, 4.0), grid(-2.0, 4.0)],
        ),
    ]
}

fn operands_for(points: &[Point; 3]) -> Vec<String> {
    vec![
        pt("start", points[0]),
        pt("end", points[1]),
        pt("at", points[2]),
    ]
}

/// The operands of a horizontal 12.70 mm span, plus `extra`.
fn span_with(extra: &[&str]) -> Vec<String> {
    let mut operands = operands_for(&[grid(0.0, 0.0), grid(10.0, 0.0), grid(4.0, -4.0)]);
    operands.extend(extra.iter().map(|s| (*s).to_string()));
    operands
}

fn run_owned(app: &mut Signex, id: &str, operands: &[String]) -> CommandOutcome {
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    run(app, id, &refs)
}

#[test]
fn each_dimension_command_with_operands_makes_the_clicked_document() {
    for (tool, id, points) in cases() {
        // The palette's, key's and menu's route: arm, then click.
        let mut clicked = app_with_tool(Tool::Select);
        let _ = clicked.dispatch_command_reporting(&command(id), CommandArgs::none(), None);
        assert_eq!(clicked.interaction_state.current_tool, tool, "{id}");
        for p in &points {
            click(&mut clicked, (p.x, p.y));
        }
        let mut app = app_with_tool(Tool::Select);
        let answer = answered(run_owned(&mut app, id, &operands_for(&points)));
        assert_eq!(sheet_shape(&app), sheet_shape(&clicked), "{id}");
        let placed = only_dimension(&app);
        assert_eq!(answer["id"], json!(placed.uuid.to_string()), "{id}");
        assert_eq!(answer["kind"], json!("dimension"), "{id}");
        assert_eq!(answer["label"], json!(placed.label()), "{id}");
        // Nothing click-only: no tool armed, no points held.
        assert_eq!(app.interaction_state.current_tool, Tool::Select, "{id}");
        assert!(
            app.interaction_state.annotation_tool.points.is_empty(),
            "{id}"
        );
        undo(&mut app);
        assert!(doc(&app).drawings.is_empty(), "{id}: one undo step");
    }
}

#[test]
fn a_linear_dimension_takes_its_axis_from_the_points_and_an_aligned_one_measures_straight() {
    let mut linear = app_with_tool(Tool::Select);
    let points = [grid(0.0, 0.0), grid(3.0, 4.0), grid(8.0, 2.0)];
    let answer = answered(run_owned(
        &mut linear,
        "place_linear_dimension",
        &operands_for(&points),
    ));
    let d = only_dimension(&linear);
    assert_eq!(
        d.direction,
        DimensionDirection::Vertical,
        "further apart in Y"
    );
    assert_eq!(answer["label"], json!("5.08 mm"));
    assert!(
        (d.line()[0].x - points[2].x).abs() < 1e-9,
        "the line runs through `at`"
    );

    let mut aligned = app_with_tool(Tool::Select);
    let answer = answered(run_owned(
        &mut aligned,
        "place_aligned_dimension",
        &operands_for(&points),
    ));
    assert_eq!(
        only_dimension(&aligned).direction,
        DimensionDirection::Aligned
    );
    assert_eq!(answer["label"], json!("6.35 mm"));
}

#[test]
fn the_operands_set_what_the_tool_form_sets() {
    let mut app = app_with_tool(Tool::Select);
    let operands = span_with(&[
        "direction=Horizontal",
        "units=mil",
        "precision=0",
        "arrows=open",
    ]);
    let answer = answered(run_owned(&mut app, "place_aligned_dimension", &operands));
    let d = only_dimension(&app);
    assert_eq!(
        d.direction,
        DimensionDirection::Horizontal,
        "a word in any case"
    );
    assert_eq!(d.units, DimensionUnits::Mil);
    assert_eq!(d.precision, 0);
    assert_eq!(d.arrows, DimensionArrows::Open);
    assert_eq!(d.override_text, None);
    assert_eq!(answer["label"], json!("500 mil"));
    assert_eq!(answer["dimension"]["units"], json!("mil"));

    let mut typed = app_with_tool(Tool::Select);
    let operands = span_with(&["text=pitch 2.54"]);
    let answer = answered(run_owned(&mut typed, "place_linear_dimension", &operands));
    assert_eq!(
        only_dimension(&typed).override_text.as_deref(),
        Some("pitch 2.54")
    );
    assert_eq!(answer["label"], json!("pitch 2.54"));
}

/// What the operands leave out is the form's default, never what the person
/// typed in the armed tool's form (#2326).
#[test]
fn the_persons_tool_form_is_never_read() {
    let mut app = app_with_tool(Tool::Select);
    let form = &mut app.interaction_state.annotation_tool.dimension;
    form.units = DimensionUnits::In;
    form.precision = 3;
    form.arrows = DimensionArrows::FilledOutside;
    form.direction = Some(DimensionDirection::Vertical);
    form.override_text = Some("typed".to_string());
    form.flipped = true;
    let answer = answered(run_owned(
        &mut app,
        "place_linear_dimension",
        &span_with(&[]),
    ));
    let d = only_dimension(&app);
    assert_eq!(d.direction, DimensionDirection::Horizontal);
    assert_eq!(d.units, DimensionUnits::Mm);
    assert_eq!(d.precision, 2);
    assert_eq!(d.arrows, DimensionArrows::FilledInside);
    assert_eq!(answer["label"], json!("12.70 mm"));
    assert!(
        (d.line()[0].y - grid(4.0, -4.0).y).abs() < 1e-9,
        "`at` sets the side, not Space"
    );
    // The person's form is left as it was.
    let form = &app.interaction_state.annotation_tool.dimension;
    assert_eq!(form.units, DimensionUnits::In);
    assert_eq!(form.override_text.as_deref(), Some("typed"));
}

#[test]
fn bad_operands_are_refused_with_the_reason_and_place_nothing() {
    let off_grid = vec![
        "start=0.3,0".to_string(),
        pt("end", grid(10.0, 0.0)),
        pt("at", grid(4.0, -4.0)),
    ];
    let same = operands_for(&[grid(2.0, 2.0), grid(2.0, 2.0), grid(4.0, -4.0)]);
    let mut across = operands_for(&[grid(0.0, 0.0), grid(0.0, 6.0), grid(4.0, -4.0)]);
    across.push("direction=horizontal".to_string());
    let cases: Vec<(Vec<String>, &str)> = vec![
        (off_grid, "is not where a click lands"),
        (same, "measure nothing"),
        (across, "measure nothing along a horizontal dimension"),
        (span_with(&["precision=4"]), "from 0 to 3"),
        (span_with(&["precision=two"]), "`precision`"),
        (
            span_with(&["units=furlong"]),
            "`units`: `furlong` is not one of mm, mil, in",
        ),
        (
            span_with(&["arrows=big"]),
            "filled_inside, filled_outside, open",
        ),
        (
            span_with(&["direction=diagonal"]),
            "horizontal, vertical, aligned",
        ),
    ];
    for (operands, needle) in cases {
        let mut app = app_with_tool(Tool::Select);
        let outcome = run_owned(&mut app, "place_linear_dimension", &operands);
        assert!(refused_with(&outcome, needle), "{needle}: {outcome:?}");
        assert!(doc(&app).drawings.is_empty(), "{needle}");
    }
}
