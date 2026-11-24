//! Mockup row 157: a selected Bezier's handles. A press takes the nearest
//! one, the drop publishes the curve's new points, Shift mirrors the control
//! points and a right press or Esc cancels.

use super::bezier_grip::{HandleShape, dragged_points, handle_marks, resting_beziers};
use super::move_pick::tests::{BOUNDS, screen};
use super::tests::test_prefs;
use super::*;
use crate::app::sch_tables::{AnnotationEdit, apply_annotation_edit};
use iced::widget::canvas::Program as _;
use signex_types::schematic::{
    Point as SheetPoint, SchBezier, SchDrawing, SchematicSheet, SelectedItem, SelectedKind,
};

const CURVE: uuid::Uuid = uuid::Uuid::from_u128(21);

fn bezier(locked: bool) -> SchBezier {
    SchBezier {
        uuid: CURVE,
        start: SheetPoint::new(10.0, 10.0),
        c1: SheetPoint::new(10.0, 30.0),
        c2: SheetPoint::new(40.0, 30.0),
        end: SheetPoint::new(40.0, 10.0),
        width: 0.0,
        stroke_color: None,
        line_style: Default::default(),
        ends: Default::default(),
        locked,
    }
}

fn slot_with(locked: bool, selected: bool) -> CanvasSlot {
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        drawings: vec![SchDrawing::Bezier(bezier(locked))],
        ..Default::default()
    };
    let mut slot = CanvasSlot::new();
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
    ));
    if selected {
        slot.selected = vec![SelectedItem::new(CURVE, SelectedKind::Drawing)];
    }
    slot
}

fn send(
    slot: &CanvasSlot,
    state: &mut CanvasState,
    event: Event,
    at: iced::Point,
) -> Option<Message> {
    let all: std::collections::HashSet<_> = crate::active_bar::SelectionFilter::ALL
        .iter()
        .copied()
        .collect();
    let overrides = std::collections::HashMap::new();
    let view = SchematicCanvas::new(slot, test_prefs(&overrides, &all));
    let action = view.update(state, &event, BOUNDS, mouse::Cursor::Available(at))?;
    action.into_inner().0
}

fn left(pressed: bool) -> Event {
    let button = mouse::Button::Left;
    Event::Mouse(if pressed {
        mouse::Event::ButtonPressed(button)
    } else {
        mouse::Event::ButtonReleased(button)
    })
}

fn moved(to: iced::Point) -> Event {
    Event::Mouse(mouse::Event::CursorMoved { position: to })
}

#[test]
fn a_press_takes_the_handle_under_it() {
    let slot = slot_with(false, true);
    for (point, (x, y)) in [(10.0, 10.0), (10.0, 30.0), (40.0, 30.0), (40.0, 10.0)]
        .into_iter()
        .enumerate()
    {
        let mut state = CanvasState::default();
        send(&slot, &mut state, left(true), screen(&slot, x, y));
        assert_eq!(state.bezier_drag.map(|d| d.point), Some(point));
    }
}

#[test]
fn an_unselected_or_locked_curve_has_no_handles() {
    for slot in [slot_with(false, false), slot_with(true, true)] {
        let mut state = CanvasState::default();
        send(&slot, &mut state, left(true), screen(&slot, 10.0, 30.0));
        assert!(state.bezier_drag.is_none());
    }
}

#[test]
fn dragging_a_control_handle_drops_the_new_points_on_the_grid_step() {
    let slot = slot_with(false, true);
    let mut state = CanvasState::default();
    send(&slot, &mut state, left(true), screen(&slot, 10.0, 30.0));
    // Ten 1.27 mm grid steps to the right.
    let to = screen(&slot, 22.7, 30.0);
    send(&slot, &mut state, moved(to), to);
    let dropped = send(&slot, &mut state, left(false), to);
    let Some(Message::CanvasEvent(CanvasEvent::BezierPointsDragged { drawing, points })) = &dropped
    else {
        panic!("the release did not drop the handle: {dropped:?}");
    };
    assert_eq!(*drawing, CURVE);
    assert!((points[1].x - 22.7).abs() < 1e-3, "{points:?}");
    assert_eq!(points[2], SheetPoint::new(40.0, 30.0), "the other stays");
    assert!(state.bezier_drag.is_none(), "the drop ends the drag");
}

#[test]
fn a_press_and_release_on_a_handle_is_no_edit() {
    let slot = slot_with(false, true);
    let mut state = CanvasState::default();
    let at = screen(&slot, 10.0, 30.0);
    send(&slot, &mut state, left(true), at);
    assert!(send(&slot, &mut state, left(false), at).is_none());
}

#[test]
fn a_right_press_cancels_the_drag() {
    let slot = slot_with(false, true);
    let mut state = CanvasState::default();
    send(&slot, &mut state, left(true), screen(&slot, 10.0, 30.0));
    let to = screen(&slot, 22.7, 30.0);
    send(&slot, &mut state, moved(to), to);
    let right = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));
    send(&slot, &mut state, right, to);
    assert!(state.bezier_drag.is_none());
    assert!(send(&slot, &mut state, left(false), to).is_none());
}

#[test]
fn shift_mirrors_the_other_control_point_through_the_chord_middle() {
    let b = bezier(false);
    let points = dragged_points(b.points(), 1, (2.0, 4.0), true);
    assert_eq!(points[1], SheetPoint::new(12.0, 34.0));
    assert_eq!(points[2], SheetPoint::new(38.0, -14.0));
    assert_eq!((points[0], points[3]), (b.start, b.end));
    let end_only = dragged_points(b.points(), 3, (2.0, 0.0), true);
    assert_eq!(end_only[2], b.c2, "an end moves alone");
}

#[test]
fn the_dropped_points_are_one_annotation_edit() {
    let drawing = SchDrawing::Bezier(bezier(false));
    let points = dragged_points(bezier(false).points(), 2, (1.27, 0.0), false);
    let next = apply_annotation_edit(&drawing, &AnnotationEdit::BezierPoints(points));
    let Some(SchDrawing::Bezier(next)) = next else {
        panic!("the edit did not apply");
    };
    assert_eq!(next.points(), points);
}

#[test]
fn the_ends_take_square_handles_and_the_control_points_round_ones() {
    let screen = [0.0_f32, 1.0, 2.0, 3.0].map(|x| iced::Point::new(x, 0.0));
    let marks = handle_marks(screen, None);
    let shapes = marks.map(|m| m.shape);
    assert_eq!(
        shapes,
        [
            HandleShape::Square,
            HandleShape::Round,
            HandleShape::Round,
            HandleShape::Square,
        ]
    );
    assert_eq!(marks.map(|m| m.at), screen, "each handle sits on its point");
    assert!(
        marks.iter().all(|m| !m.filled),
        "resting handles are hollow"
    );
}

#[test]
fn only_the_held_handle_is_filled() {
    let screen = [iced::Point::ORIGIN; 4];
    let filled = handle_marks(screen, Some(1)).map(|m| m.filled);
    assert_eq!(filled, [false, true, false, false]);
}

#[test]
fn the_handles_take_the_theme_accent_not_the_canvas_selection_green() {
    let slot = slot_with(false, true);
    let all: std::collections::HashSet<_> = crate::active_bar::SelectionFilter::ALL
        .iter()
        .copied()
        .collect();
    let overrides = std::collections::HashMap::new();
    let view = SchematicCanvas::new(&slot, test_prefs(&overrides, &all));
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    assert_eq!(
        view.handle_ink(),
        crate::render_config::to_iced(&tokens.accent)
    );
}

#[test]
fn a_held_curve_shows_no_resting_handles_under_its_drag() {
    let slot = slot_with(false, true);
    let sheet = slot.active_snapshot().expect("the sheet is loaded");
    let resting = |dragged| {
        resting_beziers(sheet, &slot.selected, dragged)
            .map(|b| b.uuid)
            .collect::<Vec<_>>()
    };
    assert_eq!(resting(None), vec![CURVE]);
    assert!(resting(Some(CURVE)).is_empty());
    let other = uuid::Uuid::from_u128(99);
    assert_eq!(resting(Some(other)), vec![CURVE]);
}

#[test]
fn a_press_on_a_handle_moves_its_curve_off_the_resting_handles() {
    let slot = slot_with(false, true);
    let mut state = CanvasState::default();
    send(&slot, &mut state, left(true), screen(&slot, 40.0, 30.0));
    let dragged = state.bezier_drag.map(|d| d.drawing);
    assert_eq!(dragged, Some(CURVE));
    let sheet = slot.active_snapshot().expect("the sheet is loaded");
    assert_eq!(resting_beziers(sheet, &slot.selected, dragged).count(), 0);
}
