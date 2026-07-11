//! Camera input — scroll-wheel zoom (cursor-anchored). The
//! `WheelScrolled` branch of `Program::update`, extracted verbatim;
//! same delta normalisation, epsilon guard, and `Zoom` publish site.

use super::super::*;
use iced::Rectangle;
use iced::mouse;
use iced::widget::canvas;

impl SymbolCanvas<'_> {
    /// Handle a scroll-wheel tick: publish a cursor-anchored `Zoom`.
    pub(in crate::library::editor::symbol::canvas) fn on_wheel_scrolled(
        &self,
        delta: &mouse::ScrollDelta,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        let pos = cursor.position_in(bounds)?;
        let dy = match delta {
            mouse::ScrollDelta::Lines { y, .. } => *y,
            mouse::ScrollDelta::Pixels { y, .. } => *y / 30.0,
        };
        if dy.abs() < f32::EPSILON {
            return None;
        }
        Some(
            canvas::Action::publish(CanvasAction::Zoom {
                sx: pos.x,
                sy: pos.y,
                delta: dy,
            })
            .and_capture(),
        )
    }
}
