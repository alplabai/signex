//! Middle-button drag zoom on the board canvas (see
//! [`crate::canvas::middle_zoom`]). Never a click: no menu, no selection, and
//! an armed tool or a route in progress stays as it is.

use iced::event::Event;
use iced::widget::canvas;
use iced::{Rectangle, mouse};

use super::{PcbCanvas, PcbCanvasState, gesture};
use crate::app::Message;
use crate::canvas::CanvasEvent;
use crate::canvas::middle_zoom::{MiddleZoom, pointer_y};

impl PcbCanvas {
    /// Handle the events of the middle-button zoom. Runs before a pending
    /// fit can consume them. `None` leaves the event to the canvas.
    pub(super) fn middle_zoom_event(
        &self,
        state: &mut PcbCanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                // A press outside the board belongs to the widget under it.
                let pos = cursor.position_in(bounds)?;
                if !state.panning {
                    state.middle_zoom = pointer_y(cursor).map(|y| MiddleZoom::pressed(pos, y));
                }
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
            | Event::Mouse(mouse::Event::CursorLeft)
            | Event::Window(iced::window::Event::Unfocused) => {
                state.middle_zoom = None;
                None
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                let ended = state.middle_zoom.take()?;
                // The zoom moved the board under the pointer: report it.
                Some(match cursor.position_in(bounds) {
                    Some(pos) if ended.has_zoomed() => {
                        canvas::Action::publish(self.pointer_report(pos, bounds)).and_capture()
                    }
                    _ => canvas::Action::capture(),
                })
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                self.middle_zoom_moved(state, bounds, cursor)
            }
            _ => None,
        }
    }

    /// Pointer moved during the zoom: zoom about the press point. The
    /// pointer report keeps an armed tool's ghost under the pointer.
    fn middle_zoom_moved(
        &self,
        state: &mut PcbCanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let drag = state.middle_zoom.as_mut()?;
        let anchor = drag.anchor();
        let zoomed = pointer_y(cursor)
            .and_then(|y| drag.dragged_to(y, self.zoom_step))
            .is_some_and(|factor| self.camera.borrow_mut().zoom_by_factor(anchor, factor));
        if zoomed {
            self.bg_cache.clear();
        }
        let first = zoomed && drag.first_zoom();
        let inside = cursor.position_in(bounds);
        // The first zoom counts as a view gesture. Off the canvas no pointer
        // report goes out, so `ViewMoved` carries the new camera to the view.
        if first || (zoomed && inside.is_none()) {
            return Some(
                canvas::Action::publish(Message::CanvasEvent(CanvasEvent::ViewMoved)).and_capture(),
            );
        }
        Some(match inside {
            Some(pos) => canvas::Action::publish(self.pointer_report(pos, bounds)).and_capture(),
            None => canvas::Action::capture(),
        })
    }

    /// What the canvas tells the app about the pointer at `pos`.
    pub(crate) fn pointer_report(&self, pos: iced::Point, bounds: Rectangle) -> Message {
        if self.selectable {
            return Message::PcbCanvas(gesture::pointer_moved(self, pos, bounds));
        }
        let (world, zoom_pct) = {
            let camera = self.camera.borrow();
            (camera.screen_to_world(pos), camera.zoom_percent())
        };
        Message::CanvasEvent(CanvasEvent::CursorAt {
            x: world.x,
            y: world.y,
            zoom_pct,
        })
    }
}
