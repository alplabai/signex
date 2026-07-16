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
                    SymbolTool::PlaceRectangle if state.rect_from.is_some() => {
                        state.rect_from = None;
                        state.rect_cursor = None;
                        true
                    }
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
                    // Esc drops the in-flight Polygon stash — no
                    // commit, regardless of vertex count.
                    SymbolTool::PlacePolygon if !state.polygon_vertices.is_empty() => {
                        state.polygon_vertices.clear();
                        state.polygon_cursor = None;
                        state.polygon_last_click_time = None;
                        state.polygon_last_click_pos = None;
                        true
                    }
                    _ => false,
                };
                if cancelled {
                    return Some(canvas::Action::capture());
                }
                None
            }
            // Enter commits the in-flight Polygon stash once it holds
            // >= 3 vertices; below that it's a no-op (keep collecting).
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
                if self.tool == SymbolTool::PlacePolygon && state.polygon_vertices.len() >= 3 =>
            {
                Some(super::tools::commit_polygon(state))
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
