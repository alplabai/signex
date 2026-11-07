//! A placement command run with operands makes the document a click makes
//! (#2155): the same app-level commit, so undo, dirty and the Messages log
//! behave as for a click, and none of the click-only UI follow-up.

use signex_types::schematic::Point;

use super::*;
use crate::app::handlers::canvas::placement_tests::{app_with_tool, click, grid, sheet_shape};
use crate::keymap::AppCommandId;
use crate::panels::{PrePlacementData, PrePlacementKind};

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

fn pt(p: Point) -> String {
    format!("{},{}", p.x, p.y)
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let id = command(id);
    let args = CommandArgs::parse_operands(&id, operands).expect("operands");
    app.run_operands(&id, &args).0
}

fn arm(app: &mut Signex, id: &str) {
    let _ = app.dispatch_command_reporting(&command(id), CommandArgs::none(), None);
}

fn form(kind: PrePlacementKind, text: &str, rotation: f64) -> PrePlacementData {
    let mut pre = PrePlacementData::new("tool".into(), kind, text.into(), 0.0, 0.0);
    pre.rotation = rotation;
    pre.shape_width_mm = 0.4;
    pre.shape_fill = signex_types::schematic::FillType::Outline;
    pre
}

/// One placement id: its operands, and what a click run needs set first.
struct Case {
    id: &'static str,
    operands: Vec<String>,
    clicks: Vec<Point>,
    /// The form text and rotation the operands stand in for.
    form: Option<(&'static str, f64)>,
    /// The pending power port a click would place.
    power: Option<(&'static str, &'static str)>,
}

fn cases() -> Vec<Case> {
    let a = grid(8.0, 16.0);
    let b = grid(24.0, 24.0);
    let seg = || vec![format!("start={}", pt(a)), format!("end={}", pt(b))];
    let at = || vec![format!("at={}", pt(a))];
    let label = |id| Case {
        id,
        operands: vec![
            format!("at={}", pt(a)),
            "text=VCC".into(),
            "rotation=90".into(),
        ],
        clicks: vec![a],
        form: Some(("VCC", 90.0)),
        power: None,
    };
    let point = |id| Case {
        id,
        operands: at(),
        clicks: vec![a],
        form: None,
        power: None,
    };
    let segment = |id| Case {
        id,
        operands: seg(),
        clicks: vec![a, b],
        form: None,
        power: None,
    };
    let text = |id| Case {
        id,
        operands: vec![format!("at={}", pt(a)), "text=Hello".into()],
        clicks: vec![a],
        form: Some(("Hello", 0.0)),
        power: None,
    };
    vec![
        label("place_net_label"),
        label("place_local_net_label"),
        label("place_global_label"),
        label("place_hierarchical_label"),
        Case {
            id: "place_power_symbol",
            operands: vec![
                format!("at={}", pt(a)),
                "net_name=VCC".into(),
                "lib_id=power:VCC".into(),
            ],
            clicks: vec![a],
            form: None,
            power: Some(("VCC", "power:VCC")),
        },
        point("place_junction"),
        point("place_no_connect"),
        point("place_no_erc"),
        segment("place_wire"),
        segment("draw_wire"),
        segment("place_bus"),
        segment("draw_bus"),
        text("place_text"),
        text("place_text_string"),
        segment("draw_line"),
        segment("draw_graphic_line"),
        segment("draw_rectangle"),
    ]
}

/// The tool an id arms, read from an app it was run on without operands.
fn clicked(case: &Case) -> Signex {
    let mut app = app_with_tool(crate::app::Tool::Select);
    arm(&mut app, case.id);
    if let Some((text, rotation)) = case.form {
        let pre = form(PrePlacementKind::Other, text, rotation);
        app.document_state.panel_ctx.pre_placement = Some(pre);
    } else if matches!(
        case.id,
        "draw_line" | "draw_graphic_line" | "draw_rectangle"
    ) {
        app.document_state.panel_ctx.pre_placement = Some(form(PrePlacementKind::Other, "", 0.0));
    }
    if let Some((net, lib)) = case.power {
        app.interaction_state.pending_power = Some((net.into(), lib.into()));
    }
    for at in &case.clicks {
        click(&mut app, (at.x, at.y));
    }
    app
}

/// An app with the same form but not the operands' text: a run with operands
/// must take the operands over the form.
fn operand_app(case: &Case) -> Signex {
    let mut app = app_with_tool(crate::app::Tool::Select);
    if case.form.is_some() {
        let pre = form(PrePlacementKind::Other, "from the form", 0.0);
        app.document_state.panel_ctx.pre_placement = Some(pre);
    } else if matches!(
        case.id,
        "draw_line" | "draw_graphic_line" | "draw_rectangle"
    ) {
        app.document_state.panel_ctx.pre_placement = Some(form(PrePlacementKind::Other, "", 0.0));
    }
    app
}

fn operand_strs(case: &Case) -> Vec<&str> {
    case.operands.iter().map(String::as_str).collect()
}

#[test]
fn every_placement_command_with_operands_makes_the_clicked_document() {
    for case in cases() {
        let clicked = clicked(&case);
        let mut app = operand_app(&case);
        let empty = sheet_shape(&app);
        let outcome = run(&mut app, case.id, &operand_strs(&case));
        assert_eq!(outcome, CommandOutcome::Ran, "{}", case.id);
        assert_ne!(sheet_shape(&app), empty, "{} placed nothing", case.id);
        assert_eq!(sheet_shape(&app), sheet_shape(&clicked), "{}", case.id);
    }
}

#[test]
fn one_undo_removes_a_placement_made_with_operands() {
    for case in cases() {
        let mut app = operand_app(&case);
        let empty = sheet_shape(&app);
        run(&mut app, case.id, &operand_strs(&case));
        assert_ne!(sheet_shape(&app), empty, "{}", case.id);
        let _ = app.dispatch_command_reporting(&command("undo"), CommandArgs::none(), None);
        assert_eq!(sheet_shape(&app), empty, "{}", case.id);
    }
}

#[test]
fn a_placement_with_operands_leaves_the_tool_and_ui_state_alone() {
    for case in cases() {
        let mut app = operand_app(&case);
        let before = ui_state(&app);
        run(&mut app, case.id, &operand_strs(&case));
        assert_eq!(ui_state(&app), before, "{}", case.id);
        assert_eq!(app.interaction_state.current_tool, crate::app::Tool::Select);
    }
}

fn ui_state(app: &Signex) -> String {
    let i = &app.interaction_state;
    format!(
        "{:?} {} {:?} {} {:?} {:?} {:?}",
        i.current_tool,
        i.wire_drawing,
        i.wire_points,
        i.shape_anchor.is_some(),
        i.pending_port,
        i.pending_power,
        app.document_state
            .panel_ctx
            .pre_placement
            .as_ref()
            .map(|pp| (pp.label_text.clone(), pp.rotation)),
    )
}

#[test]
fn a_placement_id_without_operands_still_only_arms_its_tool() {
    for case in cases() {
        let mut app = operand_app(&case);
        let empty = sheet_shape(&app);
        arm(&mut app, case.id);
        assert_eq!(sheet_shape(&app), empty, "{}", case.id);
        assert_ne!(
            app.interaction_state.current_tool,
            crate::app::Tool::Select,
            "{}",
            case.id
        );
    }
}

#[test]
fn a_placement_with_operands_and_no_schematic_is_disabled() {
    let (mut app, _task) = Signex::new();
    for case in cases() {
        let outcome = run(&mut app, case.id, &operand_strs(&case));
        assert!(
            matches!(outcome, CommandOutcome::Disabled { .. }),
            "{}: {outcome:?}",
            case.id
        );
    }
}

#[test]
fn a_zero_length_segment_is_refused_and_changes_nothing() {
    let at = pt(grid(4.0, 4.0));
    let (start, end) = (format!("start={at}"), format!("end={at}"));
    for id in ["place_wire", "place_bus", "draw_line", "draw_rectangle"] {
        let mut app = app_with_tool(crate::app::Tool::Select);
        app.document_state.panel_ctx.pre_placement = Some(form(PrePlacementKind::Other, "", 0.0));
        let before = sheet_shape(&app);
        let outcome = run(&mut app, id, &[&start, &end]);
        assert!(
            matches!(outcome, CommandOutcome::Disabled { .. }),
            "{id}: {outcome:?}"
        );
        assert_eq!(sheet_shape(&app), before, "{id} changed the document");
    }
}

#[test]
fn an_off_grid_point_is_refused_and_changes_nothing() {
    let on = pt(grid(4.0, 4.0));
    let off = pt(Point::new(grid(4.0, 4.0).x + 0.3, grid(4.0, 4.0).y));
    let cases: [(&str, Vec<String>); 3] = [
        ("place_junction", vec![format!("at={off}")]),
        (
            "place_net_label",
            vec![format!("at={off}"), "text=VCC".into()],
        ),
        (
            "place_wire",
            vec![format!("start={on}"), format!("end={off}")],
        ),
    ];
    for (id, operands) in cases {
        let mut app = app_with_tool(crate::app::Tool::Select);
        app.document_state.panel_ctx.pre_placement = Some(form(PrePlacementKind::Other, "", 0.0));
        let before = sheet_shape(&app);
        let operands: Vec<&str> = operands.iter().map(String::as_str).collect();
        let outcome = run(&mut app, id, &operands);
        assert!(
            matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("grid")),
            "{id}: {outcome:?}"
        );
        assert_eq!(sheet_shape(&app), before, "{id} changed the document");
    }
}
