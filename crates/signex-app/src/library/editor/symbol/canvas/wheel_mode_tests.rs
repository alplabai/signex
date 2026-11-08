//! "Mouse wheel" preference in the Symbol Editor canvas.

use super::*;
use crate::canvas::WheelMode;
use iced::Point;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

fn action_for(
    mode: WheelMode,
    zoom_modifier: bool,
    shift_held: bool,
    delta: mouse::ScrollDelta,
) -> Option<CanvasAction> {
    let symbol = Symbol::empty("T");
    let camera = crate::canvas::Camera::default();
    let canvas = SymbolCanvas::new(
        &symbol,
        None,
        SymbolTool::Select,
        &[],
        state::PartView::normal(1),
        false,
        &camera,
        1.27,
        true,
        crate::render_config::GridStyle::default(),
        true,
        Color::WHITE,
        Color::from_rgb(0.2, 0.4, 0.9),
        Color::BLACK,
        Color::BLACK,
        Color::BLACK,
    )
    .with_wheel_mode(mode);
    let state = CanvasState {
        zoom_modifier,
        shift_held,
        ..CanvasState::default()
    };
    let cursor = mouse::Cursor::Available(Point::new(400.0, 300.0));
    canvas
        .on_wheel_scrolled(&state, &delta, BOUNDS, cursor)
        .and_then(|action| action.into_inner().0)
}

const NOTCH: mouse::ScrollDelta = mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 };

#[test]
fn zoom_mode_zooms_on_a_wheel_notch() {
    assert!(matches!(
        action_for(WheelMode::Zoom, false, false, NOTCH),
        Some(CanvasAction::Zoom { .. })
    ));
}

#[test]
fn scroll_mode_pans_vertically_and_shift_pans_horizontally() {
    assert!(matches!(
        action_for(WheelMode::Scroll, false, false, NOTCH),
        Some(CanvasAction::Pan { dx, dy }) if dx == 0.0 && dy != 0.0
    ));
    assert!(matches!(
        action_for(WheelMode::Scroll, false, true, NOTCH),
        Some(CanvasAction::Pan { dx, dy }) if dx != 0.0 && dy == 0.0
    ));
}

#[test]
fn scroll_mode_ctrl_wheel_zooms() {
    assert!(matches!(
        action_for(WheelMode::Scroll, true, false, NOTCH),
        Some(CanvasAction::Zoom { .. })
    ));
}
