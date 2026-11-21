//! Middle-button drag zoom on the symbol canvas (see
//! [`crate::canvas::middle_zoom`]). The camera lives in the editor, so each
//! move publishes a `ZoomFactor` about the press point. Never a click: no
//! menu, no selection, and an armed tool or its stash stays as it is.

use super::super::*;
use crate::canvas::middle_zoom::{MiddleZoom, pointer_y};
use iced::Rectangle;
use iced::mouse;
use iced::widget::canvas;

impl SymbolCanvas<'_> {
    /// Middle press: start the zoom about the press point.
    pub(in crate::library::editor::symbol::canvas) fn on_middle_press(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        let pos = cursor.position_in(bounds)?;
        if !state.panning {
            state.middle_zoom = pointer_y(cursor).map(|y| MiddleZoom::pressed(pos, y));
        }
        Some(canvas::Action::capture())
    }

    /// Middle release: end the zoom.
    pub(in crate::library::editor::symbol::canvas) fn on_middle_release(
        &self,
        state: &mut CanvasState,
    ) -> Option<canvas::Action<CanvasAction>> {
        state.middle_zoom.take()?;
        Some(canvas::Action::capture())
    }

    /// The zoom factor action for this pointer move, if it moved vertically.
    pub(in crate::library::editor::symbol::canvas) fn middle_zoom_moved(
        &self,
        state: &mut CanvasState,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        let drag = state.middle_zoom.as_mut()?;
        let factor = drag.dragged_to(pointer_y(cursor)?, self.zoom_step)?;
        drag.first_zoom();
        let anchor = drag.anchor();
        state.middle_zoom_follow = true;
        Some(
            canvas::Action::publish(CanvasAction::ZoomFactor {
                sx: anchor.x,
                sy: anchor.y,
                factor,
            })
            .and_capture(),
        )
    }

    /// The redraw after a zoom: the new camera has landed, so re-track the
    /// pointer and a tool's preview lands under it again.
    pub(in crate::library::editor::symbol::canvas) fn follow_middle_zoom(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        if !matches!(event, Event::Window(iced::window::Event::RedrawRequested(_)))
            || !std::mem::take(&mut state.middle_zoom_follow)
        {
            return None;
        }
        self.on_cursor_moved(state, bounds, cursor)
    }
}
