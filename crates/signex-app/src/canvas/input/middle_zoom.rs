//! Middle-button drag zoom on the schematic canvas (see
//! [`crate::canvas::middle_zoom`]). Never a click: no menu, no selection, and
//! an armed tool or a wire in progress stays as it is.

use super::super::middle_zoom::{MiddleZoom, pointer_y};
use super::super::*;

impl SchematicCanvas<'_> {
    /// Middle press: start the zoom about the press point.
    pub(in crate::canvas) fn update_middle_pressed(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if !state.panning
            && let (Some(pos), Some(y)) = (cursor.position_in(bounds), pointer_y(cursor))
        {
            state.middle_zoom = Some(MiddleZoom::pressed(pos, y));
        }
        Some(canvas::Action::capture())
    }

    /// Middle release: end the zoom, and report where the pointer now is on
    /// the sheet, since the zoom moved the sheet under it.
    pub(in crate::canvas) fn update_middle_released(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let ended = state.middle_zoom.take();
        if ended.is_some_and(|drag| drag.has_zoomed())
            && let Some(pos) = cursor.position_in(bounds)
        {
            let world = self.camera().screen_to_world(pos, bounds);
            let zoom_pct = self.camera().zoom_percent();
            return Some(
                canvas::Action::publish(Message::CanvasEvent(CanvasEvent::CursorAt {
                    x: world.x,
                    y: world.y,
                    zoom_pct,
                }))
                .and_capture(),
            );
        }
        Some(canvas::Action::capture())
    }

    /// Ends a zoom in flight on the events that can stand for its release.
    /// Runs before a pending fit can consume the event. `Some` for the
    /// release itself; the pointer or focus leaving goes on to its handlers.
    pub(in crate::canvas) fn end_middle_zoom_on(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        state.middle_zoom?;
        match event {
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                self.update_middle_released(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::CursorLeft)
            | Event::Window(iced::window::Event::Unfocused) => {
                state.middle_zoom = None;
                None
            }
            _ => None,
        }
    }

    /// Pointer moved during the zoom: zoom about the press point, then do
    /// the usual tracking, so a floating move and the hover follow the
    /// pointer at the new scale.
    pub(in crate::canvas) fn update_middle_zoom_moved(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let drag = state.middle_zoom.as_mut()?;
        let anchor = drag.anchor();
        let zoomed = pointer_y(cursor)
            .and_then(|y| drag.dragged_to(y, self.prefs.zoom_step))
            .is_some_and(|factor| self.camera_mut().zoom_by_factor(anchor, factor));
        let first = zoomed && drag.first_zoom();
        let outside = cursor.position_in(bounds).is_none();

        let tracked = self
            .update_cursor_moved(state, bounds, cursor)
            .and_then(|action| action.into_inner().0);
        // The first zoom counts as a view gesture. Off the canvas no hover
        // reports, so `ViewMoved` carries the new camera to the view.
        let message = match tracked {
            Some(Message::CanvasEvent(CanvasEvent::CursorLeft)) => tracked,
            _ if first || (zoomed && outside) => Some(Message::CanvasEvent(CanvasEvent::ViewMoved)),
            other => other,
        };
        Some(match message {
            Some(message) => canvas::Action::publish(message).and_capture(),
            None => canvas::Action::capture(),
        })
    }
}
