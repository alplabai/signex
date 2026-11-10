//! Camera input — scroll-wheel zoom (cursor-anchored). The
//! `WheelScrolled` branch of `Program::update`, extracted verbatim;
//! same delta normalisation, epsilon guard, and `Zoom` publish site.

use super::super::*;
use iced::Rectangle;
use iced::mouse;
use iced::widget::canvas;

impl SymbolCanvas<'_> {
    /// Handle a scroll-wheel tick: publish a cursor-anchored `Zoom`, or a
    /// `Pan` (see [`crate::canvas::wheel_intent`] for the preferences).
    pub(in crate::library::editor::symbol::canvas) fn on_wheel_scrolled(
        &self,
        state: &CanvasState,
        delta: &mouse::ScrollDelta,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        let pos = cursor.position_in(bounds)?;
        let intent = crate::canvas::wheel_intent(
            delta,
            30.0,
            self.trackpad_pan,
            self.wheel_mode,
            crate::canvas::WheelKeys {
                zoom: state.zoom_modifier,
                shift: state.shift_held,
            },
        );
        match intent {
            crate::canvas::ScrollIntent::Zoom { notches } => {
                // NaN passes the `<` test, so reject it explicitly.
                if notches.is_nan() || notches.abs() < f32::EPSILON {
                    return None;
                }
                Some(
                    canvas::Action::publish(CanvasAction::Zoom {
                        sx: pos.x,
                        sy: pos.y,
                        delta: notches,
                    })
                    .and_capture(),
                )
            }
            crate::canvas::ScrollIntent::Pan { dx, dy } => {
                if dx == 0.0 && dy == 0.0 {
                    return None;
                }
                Some(canvas::Action::publish(CanvasAction::Pan { dx, dy }).and_capture())
            }
        }
    }

    /// Handle a trackpad pinch: publish a cursor-anchored `ZoomFactor`.
    pub(in crate::library::editor::symbol::canvas) fn on_pinch_gesture(
        &self,
        delta: f32,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        let pos = cursor.position_in(bounds)?;
        let factor = crate::canvas::Camera::pinch_factor(delta);
        if factor == 1.0 {
            return None;
        }
        Some(
            canvas::Action::publish(CanvasAction::ZoomFactor {
                sx: pos.x,
                sy: pos.y,
                factor,
            })
            .and_capture(),
        )
    }
}
