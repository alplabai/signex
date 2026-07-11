//! Keyboard handling — the `KeyPressed` branch of `Program::update`,
//! extracted verbatim. Escape cancels an in-progress multi-click draw;
//! Delete/Backspace, Home, Ctrl+A, Space (rotate), and undo/redo keep
//! identical key matching and publish sites.

use super::super::*;
use iced::widget::canvas;

impl SymbolCanvas<'_> {
    /// Handle a key press over the canvas.
    pub(in crate::library::editor::symbol::canvas) fn on_key_pressed(
        &self,
        state: &mut CanvasState,
        key: &iced::keyboard::Key,
        modifiers: &iced::keyboard::Modifiers,
    ) -> Option<canvas::Action<CanvasAction>> {
        match key {
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                let cancelled = match self.tool {
                    SymbolTool::PlaceLine if state.line_from.is_some() => {
                        state.line_from = None;
                        state.line_cursor = None;
                        true
                    }
                    SymbolTool::PlaceCircle if state.circle_center.is_some() => {
                        state.circle_center = None;
                        state.circle_cursor = None;
                        true
                    }
                    SymbolTool::PlaceArc
                        if state.arc_center.is_some() || state.arc_radius_start.is_some() =>
                    {
                        state.arc_center = None;
                        state.arc_radius_start = None;
                        state.arc_cursor = None;
                        state.arc_end_deg_unwrapped = None;
                        true
                    }
                    _ => false,
                };
                if cancelled {
                    return Some(canvas::Action::capture());
                }
                None
            }
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete)
            | iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                Some(canvas::Action::publish(CanvasAction::DeleteSelected))
            }
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Home) => {
                Some(canvas::Action::publish(CanvasAction::Fit))
            }
            iced::keyboard::Key::Character(c) if c.as_str() == "a" && modifiers.command() => Some(
                canvas::Action::publish(CanvasAction::Select(SymbolSelection::All)).and_capture(),
            ),
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Space) => {
                let pivot_mode = if modifiers.alt() {
                    RotatePivotMode::GeometryCenter
                } else {
                    RotatePivotMode::WorldOrigin
                };
                Some(
                    canvas::Action::publish(CanvasAction::RotateSelected {
                        clockwise: !modifiers.shift(),
                        pivot_mode,
                    })
                    .and_capture(),
                )
            }
            iced::keyboard::Key::Character(c) if c == " " => {
                let pivot_mode = if modifiers.alt() {
                    RotatePivotMode::GeometryCenter
                } else {
                    RotatePivotMode::WorldOrigin
                };
                Some(
                    canvas::Action::publish(CanvasAction::RotateSelected {
                        clockwise: !modifiers.shift(),
                        pivot_mode,
                    })
                    .and_capture(),
                )
            }
            // Undo: Ctrl+Z
            iced::keyboard::Key::Character(c)
                if c.as_str() == "z" && modifiers.command() && !modifiers.shift() =>
            {
                Some(canvas::Action::publish(CanvasAction::Undo).and_capture())
            }
            // Redo: Ctrl+Y  or  Ctrl+Shift+Z
            iced::keyboard::Key::Character(c)
                if (c.as_str() == "y" && modifiers.command())
                    || (c.as_str() == "z" && modifiers.command() && modifiers.shift()) =>
            {
                Some(canvas::Action::publish(CanvasAction::Redo).and_capture())
            }
            _ => None,
        }
    }
}
