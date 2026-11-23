//! The cursor every canvas shows when no drag or handle claims it (Caner,
//! 2026-11-21): the OS arrow while the selection tool is idle, the
//! crosshair while a placement, drawing or routing tool, or a measure, is
//! active. See docs/RENDERING_RULES.md, "Canvas cursor".

use iced::mouse;

/// The cursor over a canvas with `tool_active` or not.
pub fn tool_cursor(tool_active: bool) -> mouse::Interaction {
    if tool_active {
        mouse::Interaction::Crosshair
    } else {
        mouse::Interaction::Idle
    }
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
