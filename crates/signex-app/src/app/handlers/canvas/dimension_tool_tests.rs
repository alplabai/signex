//! The armed dimension tool's Properties, Tab and Space (mockup row 157),
//! driven through `Signex::update`.

use signex_types::schematic::{DimensionDirection, DimensionUnits, SchDimension, SchDrawing};

use super::placement_tests::{app_with_tool, click, doc};
use crate::app::sch_tables::AnnotationEdit;
use crate::app::{EditMsg, InputMsg, Message, Signex, Tool, ToolMessage};
use crate::panels::{AnnotationField, AnnotationPanelMsg, PanelMsg};

fn armed(tool: Tool) -> Signex {
    let mut app = app_with_tool(tool);
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(tool)));
    app
}

fn panel(app: &mut Signex, msg: AnnotationPanelMsg) {
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::Annotation(msg),
    )));
}

fn space(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::RotateSelectedCounterclockwise));
}

fn tab(app: &mut Signex) {
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
}

fn dimensions(app: &Signex) -> Vec<&SchDimension> {
    doc(app)
        .drawings
        .iter()
        .filter_map(|d| match d {
            SchDrawing::Dimension(d) => Some(d),
            _ => None,
        })
        .collect()
}

fn place(app: &mut Signex) {
    click(app, (0.0, 0.0));
    click(app, (12.7, 0.0));
    click(app, (5.08, 5.08));
}

#[test]
fn properties_shows_the_armed_tool_before_the_first_click() {
    let app = armed(Tool::LinearDimension);
    let state = app.annotation_panel_state();
    let form = state.dimension_tool.expect("the tool's form");
    assert!(!form.is_aligned);
    assert!(!form.has_from && !form.has_to);
    assert!(
        app.document_state
            .panel_ctx
            .annotation
            .dimension_tool
            .is_some()
    );
}

#[test]
fn the_form_follows_the_clicks_and_leaves_with_the_tool() {
    let mut app = armed(Tool::AlignedDimension);
    click(&mut app, (2.54, 2.54));
    let form = app.dimension_tool_form().expect("armed");
    assert!(form.is_aligned && form.has_from && !form.has_to);
    assert_eq!(form.dimension.from.x, 2.54);

    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
    assert!(
        app.document_state
            .panel_ctx
            .annotation
            .dimension_tool
            .is_none()
    );
    assert!(app.annotation_panel_state().dimension_tool.is_none());
}

#[test]
fn a_form_edit_sets_the_next_dimension_and_leaves_the_sheet_alone() {
    let mut app = armed(Tool::LinearDimension);
    panel(
        &mut app,
        AnnotationPanelMsg::Picked(AnnotationEdit::Units(DimensionUnits::Mil)),
    );
    assert!(doc(&app).drawings.is_empty(), "no edit reaches the sheet");
    place(&mut app);
    assert_eq!(dimensions(&app)[0].units, DimensionUnits::Mil);
}

#[test]
fn a_picked_direction_wins_over_the_drag() {
    let mut app = armed(Tool::LinearDimension);
    panel(
        &mut app,
        AnnotationPanelMsg::Picked(AnnotationEdit::Direction(DimensionDirection::Vertical)),
    );
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 2.54));
    click(&mut app, (5.08, 5.08));
    assert_eq!(dimensions(&app)[0].direction, DimensionDirection::Vertical);
}

#[test]
fn space_puts_the_line_on_the_other_side_for_one_dimension() {
    let mut plain = armed(Tool::LinearDimension);
    place(&mut plain);
    let plain_offset = dimensions(&plain)[0].offset;

    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 0.0));
    space(&mut app);
    let draft = app
        .interaction_state
        .active_canvas()
        .annotation_draft
        .clone()
        .expect("draft");
    assert!(draft.flipped, "the preview flips with the key");
    click(&mut app, (5.08, 5.08));
    assert!((dimensions(&app)[0].offset + plain_offset).abs() < 1e-9);

    place(&mut app);
    assert!(
        (dimensions(&app)[1].offset - plain_offset).abs() < 1e-9,
        "the next dimension starts unflipped"
    );
}

#[test]
fn space_before_the_first_point_flips_nothing() {
    let mut app = armed(Tool::LinearDimension);
    space(&mut app);
    assert!(!app.interaction_state.annotation_tool.dimension.flipped);
}

#[test]
fn space_while_paused_keeps_the_side() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    tab(&mut app);
    space(&mut app);
    assert!(!app.interaction_state.annotation_tool.dimension.flipped);
}

#[test]
fn tab_edits_the_value_of_the_next_dimension_only() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 0.0));
    tab(&mut app);
    assert!(app.interaction_state.active_canvas().placement_paused);
    assert!(
        app.annotation_panel_state().dimension_tool.is_some(),
        "the paused form stays the dimension's"
    );
    panel(
        &mut app,
        AnnotationPanelMsg::Picked(AnnotationEdit::OverrideEnabled(true)),
    );
    panel(
        &mut app,
        AnnotationPanelMsg::FieldTyped(AnnotationField::OverrideText, "J1 pitch".into()),
    );
    tab(&mut app);
    assert!(!app.interaction_state.active_canvas().placement_paused);
    click(&mut app, (5.08, 5.08));
    assert_eq!(
        dimensions(&app)[0].override_text.as_deref(),
        Some("J1 pitch")
    );

    place(&mut app);
    assert_eq!(dimensions(&app)[1].override_text, None);
}

#[test]
fn a_blank_override_places_the_measured_value() {
    let mut app = armed(Tool::LinearDimension);
    panel(
        &mut app,
        AnnotationPanelMsg::Picked(AnnotationEdit::OverrideEnabled(true)),
    );
    assert_eq!(
        app.interaction_state
            .annotation_tool
            .dimension
            .override_text
            .as_deref(),
        Some(""),
        "no sample length is taken as the value"
    );
    place(&mut app);
    assert_eq!(dimensions(&app)[0].override_text, None);
}

#[test]
fn esc_while_paused_drops_the_form_edits_and_keeps_the_points() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    tab(&mut app);
    panel(
        &mut app,
        AnnotationPanelMsg::Picked(AnnotationEdit::Units(DimensionUnits::In)),
    );
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));

    let tool = &app.interaction_state.annotation_tool;
    assert_eq!(tool.dimension.units, DimensionUnits::Mm);
    assert_eq!(tool.points.len(), 1);
    assert_eq!(app.interaction_state.current_tool, Tool::LinearDimension);
    assert!(!app.interaction_state.active_canvas().placement_paused);
}

#[test]
fn the_hint_names_tab_and_space_from_the_second_point() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    let hint = app
        .interaction_state
        .active_canvas()
        .annotation_draft
        .as_ref()
        .expect("draft")
        .hint
        .clone();
    assert!(hint.contains("Tab edits value") && hint.contains("Space flips side"));
}
