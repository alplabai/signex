//! Middle-button drag zoom on the footprint canvas (see
//! [`crate::canvas::middle_zoom`]). Never a click: no menu, no selection, and
//! an armed tool stays as it is.

use iced::event::Event;
use iced::widget::canvas;
use iced::{Rectangle, mouse};

use crate::canvas::middle_zoom::{MiddleZoom, pointer_y};
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};

use super::super::{FootprintCanvas, FootprintCanvasState, MIN_SCALE};

impl FootprintCanvas<'_> {
    /// Middle press: start the zoom about the press point.
    pub(in crate::library::editor::footprint::canvas) fn on_middle_pressed(
        &self,
        cstate: &mut FootprintCanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<LibraryMessage>> {
        let pos = cursor.position_in(bounds)?;
        if !cstate.panning {
            cstate.middle_zoom = pointer_y(cursor).map(|y| MiddleZoom::pressed(pos, y));
        }
        Some(canvas::Action::capture())
    }

    /// Ends a zoom in flight on the events that can stand for its release.
    /// Runs before a fit can consume the event. `Some` for the release
    /// itself; the pointer or focus leaving goes on to its handlers.
    pub(in crate::library::editor::footprint::canvas) fn end_middle_zoom_on(
        &self,
        cstate: &mut FootprintCanvasState,
        event: &Event,
    ) -> Option<canvas::Action<LibraryMessage>> {
        let drag = cstate.middle_zoom?;
        match event {
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                cstate.middle_zoom = None;
                Some(if drag.has_zoomed() {
                    self.publish_view_scale(cstate).and_capture()
                } else {
                    canvas::Action::capture()
                })
            }
            Event::Mouse(mouse::Event::CursorLeft)
            | Event::Window(iced::window::Event::Unfocused) => {
                cstate.middle_zoom = None;
                None
            }
            _ => None,
        }
    }

    /// Pointer moved during the zoom: zoom about the press point, then do
    /// the usual tracking, so the ghost and the readout follow the pointer.
    pub(in crate::library::editor::footprint::canvas) fn on_middle_zoom_moved(
        &self,
        cstate: &mut FootprintCanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> canvas::Action<LibraryMessage> {
        let zoomed = self.step_middle_zoom(cstate, cursor);
        let Some(cursor_pos) = cursor.position_in(bounds) else {
            return if zoomed {
                self.publish_view_scale(cstate).and_capture()
            } else {
                canvas::Action::capture()
            };
        };
        let tracked = self.track_pointer(cstate, cursor_pos);
        match tracked.and_then(|action| action.into_inner().0) {
            Some(message) => canvas::Action::publish(message).and_capture(),
            None if zoomed => self.publish_view_changed(cstate, cursor_pos),
            None => canvas::Action::capture(),
        }
    }

    /// Zoom about the press point for this pointer move. Returns whether
    /// the camera was asked to change.
    fn step_middle_zoom(
        &self,
        cstate: &mut FootprintCanvasState,
        cursor: mouse::Cursor,
    ) -> bool {
        let Some(drag) = cstate.middle_zoom.as_mut() else {
            return false;
        };
        let anchor = drag.anchor();
        let Some(factor) = pointer_y(cursor).and_then(|y| drag.dragged_to(y, self.zoom_step))
        else {
            return false;
        };
        drag.first_zoom();
        cstate.zoom_about(anchor, factor, MIN_SCALE);
        self.cache.clear();
        true
    }

    /// Publish the camera's scale for the zoom readout.
    fn publish_view_scale(&self, cstate: &FootprintCanvasState) -> canvas::Action<LibraryMessage> {
        canvas::Action::publish(LibraryMessage::EditorEvent {
            library_path: self.address.library_path.clone(),
            table: self.address.table.clone(),
            row_id: self.address.row_id,
            msg: EditorMsg::Footprint(FootprintEditorMsg::ViewScaleChanged {
                px_per_mm: cstate.scale,
            }),
        })
    }
}
