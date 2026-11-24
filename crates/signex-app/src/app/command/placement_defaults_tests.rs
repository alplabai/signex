//! The defaults/form split (#2326 slice 6.P3): a person's click places what
//! their pre-placement form holds, as before; an agent or CLI run places what
//! its operands name and the project defaults for the rest, whatever the
//! person's form holds (docs/DECISIONS.md, 2026-11-24).

use signex_types::schematic::{
    FillType, HAlign, LabelType, SCHEMATIC_PT_TO_MM, SchDrawing, SchematicSheet, VAlign,
    WireWidthPreset,
};
use signex_types::theme::Color;

use super::*;
use crate::app::Tool;
use crate::app::handlers::canvas::placement_tests::{app_with_tool, click, doc, grid};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::keymap::AppCommandId;
use crate::panels::{PrePlacementData, PrePlacementKind};

const RED: Color = Color {
    r: 200,
    g: 10,
    b: 10,
    a: 255,
};

/// A form far from every default: 270 degrees, 8 pt, right and top
/// justified, a 0.4 mm outlined shape, a Medium red wire.
fn persons_form(kind: PrePlacementKind) -> PrePlacementData {
    let mut pre = PrePlacementData::new("tool".into(), kind, "FROM_FORM".into(), 0.0, 0.0);
    pre.rotation = 270.0;
    pre.font_size_pt = 8;
    pre.justify_h = HAlign::Right;
    pre.justify_v = VAlign::Top;
    pre.shape_width_mm = 0.4;
    pre.shape_fill = FillType::Outline;
    pre.wire_width = WireWidthPreset::Medium;
    pre.wire_color = Some(RED);
    pre
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands");
    app.run_operands_settled(&id, &args).0
}

fn at(i: f64, j: f64) -> String {
    let p = grid(i, j);
    format!("{},{}", p.x, p.y)
}

/// (a) The person's own placement is unchanged: the click passes the form's
/// values, field for field as on trunk.
#[test]
fn a_click_places_what_the_persons_form_holds() {
    let mut app = app_with_tool(Tool::Label);
    app.document_state.panel_ctx.pre_placement = Some(persons_form(PrePlacementKind::Other));
    let p = grid(8.0, 16.0);
    click(&mut app, (p.x, p.y));
    let label = &doc(&app).labels[0];
    assert_eq!(label.text, "FROM_FORM");
    assert_eq!(label.label_type, LabelType::Net);
    assert_eq!(label.rotation, 270.0);
    assert_eq!(label.font_size, 8.0 * SCHEMATIC_PT_TO_MM);
    assert_eq!(
        (label.justify, label.justify_v),
        (HAlign::Right, VAlign::Top)
    );

    let mut app = app_with_tool(Tool::Rectangle);
    app.document_state.panel_ctx.pre_placement = Some(persons_form(PrePlacementKind::Other));
    for p in [grid(8.0, 16.0), grid(24.0, 24.0)] {
        click(&mut app, (p.x, p.y));
    }
    match &doc(&app).drawings[0] {
        SchDrawing::Rect { width, fill, .. } => {
            assert_eq!((*width, *fill), (0.4, FillType::Outline));
        }
        other => panic!("not a rectangle: {other:?}"),
    }

    let mut app = app_with_tool(Tool::Wire);
    app.document_state.panel_ctx.pre_placement = Some(persons_form(PrePlacementKind::Wire));
    for p in [grid(0.0, 10.0), grid(10.0, 10.0)] {
        click(&mut app, (p.x, p.y));
    }
    let wire = &doc(&app).wires[0];
    assert_eq!((wire.stroke_width, wire.color), (0.5, Some(RED)));
}

/// A project sheet whose defaults differ from the built-in table and from
/// the person's form, with that form open. Keep the `TempDir` alive for the
/// test; dropping it removes the project directory.
fn project_app(form: PrePlacementKind) -> (Signex, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a temp project dir");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["root.snxsch"]);
    let sheet = dir.path().join("root.snxsch");
    open_with(&mut app.document_state, &sheet, SchematicSheet::default());
    app.document_state.active_path = Some(sheet.clone());
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    let defaults = &mut app.document_state.projects[0].data.schematic_defaults;
    defaults.text_size_pt = Some(14.0);
    defaults.shape_stroke_mm = Some(0.3);
    defaults.shape_fill = Some(FillType::Background);
    defaults.font_family = Some("Roboto".into());
    app.document_state.panel_ctx.pre_placement = Some(persons_form(form));
    let document = format!("document={}", sheet.display());
    (app, document, dir)
}

fn sheet(app: &Signex) -> &SchematicSheet {
    let path = app.document_state.active_path.as_ref().expect("a sheet");
    app.document_state.engines[path].document()
}

/// (b) An agent or CLI run without form operands takes the project
/// defaults, not the person's form (the approved behaviour change).
#[test]
fn an_operand_run_takes_the_project_defaults_over_the_persons_form() {
    let (mut app, document, _dir) = project_app(PrePlacementKind::Other);
    let defaults = app.document_state.active_schematic_defaults();
    let label_at = format!("at={}", at(8.0, 16.0));
    let outcome = run(
        &mut app,
        "place_net_label",
        &[&document, &label_at, "text=VCC"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    let label = &sheet(&app).labels[0];
    assert_eq!(label.rotation, 0.0);
    assert_eq!(label.font_size, defaults.placed_text_mm());
    assert_eq!(label.font_size, 14.0 * SCHEMATIC_PT_TO_MM);
    assert_eq!(label.font_family.as_deref(), Some("Roboto"));
    assert_eq!(
        (label.justify, label.justify_v),
        (HAlign::Left, VAlign::Bottom)
    );

    let (start, end) = (
        format!("start={}", at(8.0, 16.0)),
        format!("end={}", at(24.0, 24.0)),
    );
    let outcome = run(&mut app, "draw_rectangle", &[&document, &start, &end]);
    assert_eq!(outcome, CommandOutcome::Ran);
    let rect = sheet(&app).drawings.iter().find_map(|d| match d {
        SchDrawing::Rect { width, fill, .. } => Some((*width, *fill)),
        _ => None,
    });
    assert_eq!(rect, Some((0.3, FillType::Background)));
    assert_eq!(
        rect,
        Some((defaults.shape_stroke_mm(0.0), defaults.shape_fill))
    );

    let (mut app, document, _dir) = project_app(PrePlacementKind::Wire);
    let (start, end) = (
        format!("start={}", at(0.0, 10.0)),
        format!("end={}", at(10.0, 10.0)),
    );
    let outcome = run(
        &mut app,
        "place_wire",
        &[&document, &start, &end, "allow_open=true"],
    );
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    let wire = &sheet(&app).wires[0];
    assert_eq!((wire.stroke_width, wire.color), (0.0, None));
}

/// (c) An agent run that names a form value as an operand places it.
#[test]
fn an_operand_run_places_the_values_its_operands_name() {
    let (mut app, document, _dir) = project_app(PrePlacementKind::Other);
    let label_at = format!("at={}", at(8.0, 16.0));
    let outcome = run(
        &mut app,
        "place_global_label",
        &[&document, &label_at, "text=CLK", "rotation=180"],
    );
    assert_eq!(outcome, CommandOutcome::Ran);
    let label = &sheet(&app).labels[0];
    assert_eq!(label.text, "CLK");
    assert_eq!(label.rotation, 180.0);
    assert_eq!(label.label_type, LabelType::Global);
}
