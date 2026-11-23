//! The armed dimension tool's Properties, Tab and Space (mockup row 157),
//! driven through `Signex::update`.

use iced::advanced::shell::{Bus, Waker};
use iced::advanced::widget::operation::focusable;
use iced::keyboard::{self, Key};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use signex_types::schematic::{DimensionDirection, DimensionUnits, SchDimension, SchDrawing};

use super::placement_tests::{app_with_tool, click, doc};
use crate::app::sch_tables::AnnotationEdit;
use crate::app::{EditMsg, InputMsg, Message, Signex, Tool, ToolMessage};
use crate::panels::{AnnotationField, AnnotationPanelMsg, DIMENSION_OVERRIDE_INPUT, PanelMsg};

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
fn a_direction_picked_under_linear_does_not_reach_aligned() {
    let mut app = armed(Tool::LinearDimension);
    panel(
        &mut app,
        AnnotationPanelMsg::Picked(AnnotationEdit::Direction(DimensionDirection::Horizontal)),
    );
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(
        Tool::AlignedDimension,
    )));
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 12.7));
    click(&mut app, (0.0, 12.7));
    assert_eq!(dimensions(&app)[0].direction, DimensionDirection::Aligned);
}

#[test]
fn a_direction_picked_under_aligned_does_not_reach_linear() {
    let mut app = armed(Tool::AlignedDimension);
    panel(
        &mut app,
        AnnotationPanelMsg::Picked(AnnotationEdit::Direction(DimensionDirection::Aligned)),
    );
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(
        Tool::LinearDimension,
    )));
    place(&mut app);
    assert_eq!(
        dimensions(&app)[0].direction,
        DimensionDirection::Horizontal
    );
}

/// The points differ in height: the flipped line lands past the other
/// point, not between the two.
#[test]
fn space_clears_both_points_when_they_differ_in_height() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 10.16));
    click(&mut app, (12.7, 0.0));
    space(&mut app);
    click(&mut app, (5.08, 15.24));
    let d = dimensions(&app)[0];
    assert_eq!(d.direction, DimensionDirection::Horizontal);
    let line = d.from.y + d.offset;
    let (low, high) = (d.from.y.min(d.to.y), d.from.y.max(d.to.y));
    assert!(
        line < low || line > high,
        "the line at {line} sits between the points"
    );
    assert!((line - (d.from.y + d.to.y - 15.24)).abs() < 1e-9);
}

#[test]
fn text_typed_for_one_dimension_leaves_with_it() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 0.0));
    tab(&mut app);
    let field = AnnotationField::DimensionPoint(false, crate::app::sch_tables::Axis::X);
    panel(
        &mut app,
        AnnotationPanelMsg::FieldTyped(field, "2.54".into()),
    );
    assert!(app.annotation_panel_state().draft.is_some());
    tab(&mut app);
    click(&mut app, (5.08, 5.08));
    assert_eq!(app.annotation_panel_state().draft, None);
}

#[test]
fn text_typed_in_the_form_leaves_with_the_tool() {
    let mut app = armed(Tool::LinearDimension);
    panel(
        &mut app,
        AnnotationPanelMsg::FieldTyped(AnnotationField::OverrideText, "J1".into()),
    );
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(
        Tool::LinearDimension,
    )));
    assert_eq!(app.annotation_panel_state().draft, None);
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

/// The live Properties panel: focus the Override box as Tab's task does,
/// press `key` with `text`, and hand what the box published to `update`.
/// Returns whether a widget captured the key.
fn press_in_override(app: &mut Signex, key: Key, text: Option<&str>) -> bool {
    // The bare test sheet has no render snapshot to set this from.
    app.document_state.panel_ctx.has_schematic = true;
    let size = iced::Size::new(300.0, 1000.0);
    let mut renderer = crate::layout_probe::headless_renderer();
    let mut published = Bus::new();
    let captured = {
        let view = crate::panels::view_properties(&app.document_state.panel_ctx);
        let mut ui = UserInterface::build(view, size, Cache::default(), &mut renderer);
        let mut focus = focusable::focus::<()>(iced::widget::Id::new(DIMENSION_OVERRIDE_INPUT));
        ui.operate(&renderer, &mut focus);
        let event = iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyJ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: text.map(Into::into),
            repeat: false,
        });
        let (_, statuses) = ui.update(
            &iced::window::Headless,
            &Waker::noop(),
            std::slice::from_ref(&event),
            iced::mouse::Cursor::Unavailable,
            &mut renderer,
            &mut published,
        );
        statuses[0] == iced::event::Status::Captured
    };
    for message in published {
        let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(message)));
    }
    captured
}

fn letter_j() -> Key {
    Key::Character("j".into())
}

fn override_text(app: &Signex) -> Option<&str> {
    app.interaction_state
        .annotation_tool
        .dimension
        .override_text
        .as_deref()
}

#[test]
fn tab_puts_the_keys_in_the_override_box_and_typing_turns_it_on() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 0.0));
    assert!(
        !press_in_override(&mut app, letter_j(), Some("j")),
        "before Tab the switched-off box takes no keys"
    );
    assert_eq!(override_text(&app), None);

    tab(&mut app);
    assert_eq!(override_text(&app), None, "Tab alone leaves Override off");
    assert!(press_in_override(&mut app, letter_j(), Some("j")));
    assert_eq!(override_text(&app), Some("j"));
}

#[test]
fn the_focused_box_leaves_tab_to_the_tool() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    tab(&mut app);
    let tab_key = Key::Named(keyboard::key::Named::Tab);
    assert!(!press_in_override(&mut app, tab_key, Some("\t")));
    assert_eq!(override_text(&app), None);
}

#[test]
fn tab_with_nothing_typed_places_the_measured_value() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 0.0));
    tab(&mut app);
    tab(&mut app);
    assert!(!app.interaction_state.active_canvas().placement_paused);
    click(&mut app, (5.08, 5.08));
    assert_eq!(dimensions(&app)[0].override_text, None);
}

#[test]
fn emptying_the_box_turns_override_off_again() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 0.0));
    tab(&mut app);
    for typed in ["J", ""] {
        panel(
            &mut app,
            AnnotationPanelMsg::FieldTyped(AnnotationField::OverrideText, typed.into()),
        );
    }
    assert_eq!(override_text(&app), None);
    assert_eq!(
        app.document_state.panel_ctx.annotation.draft, None,
        "no empty draft hides the measured value after the second Tab"
    );
}

#[test]
fn tab_unfolds_the_text_section_so_typing_reaches_the_override_box() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 0.0));
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::ToggleSection(crate::panels::DIMENSION_TEXT_SECTION.into()),
    )));
    assert!(
        app.document_state
            .panel_ctx
            .collapsed_sections
            .contains(crate::panels::DIMENSION_TEXT_SECTION)
    );

    tab(&mut app);
    assert!(press_in_override(&mut app, letter_j(), Some("j")));
    assert_eq!(override_text(&app), Some("j"));
}

#[test]
fn the_form_is_paused_only_between_the_two_tabs() {
    let mut app = armed(Tool::LinearDimension);
    let shown = |app: &Signex| {
        let form = app
            .document_state
            .panel_ctx
            .annotation
            .dimension_tool
            .as_ref();
        form.expect("the tool's form").is_paused
    };
    assert!(!shown(&app));
    tab(&mut app);
    assert!(shown(&app));
    tab(&mut app);
    assert!(!shown(&app));
}

#[test]
fn the_direction_carries_to_the_next_dimension_and_the_override_does_not() {
    let mut app = armed(Tool::LinearDimension);
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 12.7));
    tab(&mut app);
    panel(
        &mut app,
        AnnotationPanelMsg::Picked(AnnotationEdit::Direction(DimensionDirection::Vertical)),
    );
    panel(
        &mut app,
        AnnotationPanelMsg::FieldTyped(AnnotationField::OverrideText, "J1".into()),
    );
    tab(&mut app);
    click(&mut app, (5.08, 5.08));
    // Wider than tall: the drag alone would make this one horizontal.
    click(&mut app, (0.0, 0.0));
    click(&mut app, (12.7, 2.54));
    click(&mut app, (5.08, 5.08));

    let placed = dimensions(&app);
    assert_eq!(placed[0].override_text.as_deref(), Some("J1"));
    assert_eq!(placed[1].override_text, None);
    assert_eq!(placed[1].direction, DimensionDirection::Vertical);
}
