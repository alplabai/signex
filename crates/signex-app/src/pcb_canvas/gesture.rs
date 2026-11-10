//! Left-button gestures on the PCB canvas (#2191): click, and a dragged
//! box. Pan and zoom stay in the canvas; this only reports what the
//! pointer did.

use iced::event::Event;
use iced::keyboard::Modifiers;
use iced::widget::canvas;
use iced::{Rectangle, mouse};
use pcb_geom::ClickMode;
use signex_types::pcb::Point;

use super::{PcbCanvas, PcbCanvasMsg};
use crate::app::{ContextMenuMsg, Message};

/// A press that moves farther than this (px) is a box, not a click.
const DRAG_THRESHOLD_PX: f32 = 4.0;
/// How far from the pointer an item still counts as under it (px).
const HIT_REACH_PX: f32 = 4.0;

#[derive(Debug, Default)]
pub(super) struct Gesture {
    modifiers: Modifiers,
    press: Option<iced::Point>,
    /// The press has passed the drag threshold and owns the canvas marquee,
    /// or (with `carrying`) drags the selected footprints.
    dragging: bool,
    /// The press landed on a selected footprint: a drag moves it (#2240).
    carrying: bool,
}

impl Gesture {
    /// Handle the events that belong to selection; `None` leaves the event
    /// to the canvas.
    pub(super) fn handle(
        &mut self,
        canvas: &PcbCanvas,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                self.modifiers = *mods;
                None
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // A release lost outside the window leaves a stale press;
                // a drag it still carries ends with nothing moved.
                let discarded = self.carrying && self.dragging && canvas.carry.get();
                self.drop_press(canvas);
                let message = if discarded {
                    Message::PcbCanvas(PcbCanvasMsg::FootprintDragCancelled)
                } else {
                    // The canvas captures the press, so the app-wide
                    // listener never sees it: close an open context menu
                    // here, whether or not the press ends in a release.
                    Message::ContextMenu(ContextMenuMsg::Close)
                };
                let Some(pos) = cursor.position_in(bounds) else {
                    return discarded.then(|| canvas::Action::publish(message));
                };
                self.press = Some(pos);
                let at = canvas.screen_to_world(pos, bounds);
                self.carrying =
                    self.modifiers.is_empty() && canvas.grabs(at, hover_reach_mm(canvas.scale()));
                Some(canvas::Action::publish(message).and_capture())
            }
            // The pointer or the focus left mid-gesture: its release may
            // never arrive here.
            Event::Mouse(mouse::Event::CursorLeft)
            | Event::Window(iced::window::Event::Unfocused) => {
                self.press?;
                let carried = self.carrying && self.dragging;
                self.drop_press(canvas);
                if carried {
                    let msg = Message::PcbCanvas(PcbCanvasMsg::FootprintDragCancelled);
                    return Some(canvas::Action::publish(msg));
                }
                Some(canvas::Action::request_redraw())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if self.carrying => {
                let press = self.press?;
                if self.dragging && !canvas.carry.get() {
                    // Esc dropped the drag: the gesture is over.
                    self.drop_press(canvas);
                    return None;
                }
                let pos = cursor.position_in(bounds)?;
                if !self.dragging && !past_threshold(press, pos) {
                    return None;
                }
                self.dragging = true;
                canvas.carry.set(true);
                let msg = PcbCanvasMsg::FootprintDragged {
                    from: canvas.screen_to_world(press, bounds),
                    to: canvas.screen_to_world(pos, bounds),
                };
                Some(canvas::Action::publish(Message::PcbCanvas(msg)).and_capture())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let press = self.press?;
                if self.dragging && canvas.marquee.get().is_none() {
                    // Esc cleared the box: the gesture is over.
                    self.drop_press(canvas);
                    return None;
                }
                let pos = cursor.position_in(bounds)?;
                if !self.dragging && !past_threshold(press, pos) {
                    return None;
                }
                self.dragging = true;
                canvas.marquee.set(Some((
                    canvas.screen_to_world(press, bounds),
                    canvas.screen_to_world(pos, bounds),
                )));
                // The readout keeps following the pointer while the box grows.
                let moved = pointer_moved(canvas, pos, bounds);
                Some(canvas::Action::publish(Message::PcbCanvas(moved)).and_capture())
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let press = self.press?;
                let carried = self.carrying && self.dragging;
                let cancelled = if carried {
                    !canvas.carry.get()
                } else {
                    self.dragging && canvas.marquee.get().is_none()
                };
                self.drop_press(canvas);
                if cancelled {
                    return Some(canvas::Action::capture());
                }
                let pos = cursor
                    .position()
                    .map(|p| iced::Point::new(p.x - bounds.x, p.y - bounds.y))
                    .unwrap_or(press);
                let mode = click_mode(self.modifiers);
                let msg = if carried {
                    PcbCanvasMsg::FootprintDropped {
                        from: canvas.screen_to_world(press, bounds),
                        to: canvas.screen_to_world(pos, bounds),
                    }
                } else if past_threshold(press, pos) {
                    PcbCanvasMsg::BoxSelected {
                        from: canvas.screen_to_world(press, bounds),
                        to: canvas.screen_to_world(pos, bounds),
                        mode,
                    }
                } else {
                    PcbCanvasMsg::Clicked {
                        at: canvas.screen_to_world(press, bounds),
                        tolerance_mm: hover_reach_mm(canvas.scale()),
                        mode,
                    }
                };
                Some(canvas::Action::publish(Message::PcbCanvas(msg)).and_capture())
            }
            _ => None,
        }
    }

    fn drop_press(&mut self, canvas: &PcbCanvas) {
        self.press = None;
        self.dragging = false;
        self.carrying = false;
        canvas.marquee.set(None);
        canvas.carry.set(false);
    }
}

/// The pointer at `pos` (canvas pixels), as the app wants to hear it.
pub(super) fn pointer_moved(
    canvas: &PcbCanvas,
    pos: iced::Point,
    bounds: Rectangle,
) -> PcbCanvasMsg {
    let (zoom_pct, scale) = {
        let camera = canvas.camera.borrow();
        (camera.zoom_percent(), camera.scale)
    };
    PcbCanvasMsg::PointerMoved {
        at: canvas.screen_to_world(pos, bounds),
        zoom_pct,
        tolerance_mm: hover_reach_mm(scale),
    }
}

/// The hover reach, in mm, at the canvas's zoom.
pub(super) fn hover_reach_mm(scale: f32) -> f64 {
    f64::from(HIT_REACH_PX / scale)
}

/// Shift adds, Ctrl or Cmd toggles, nothing replaces.
fn click_mode(mods: Modifiers) -> ClickMode {
    if mods.shift() {
        ClickMode::Add
    } else if mods.control() || mods.command() {
        ClickMode::Toggle
    } else {
        ClickMode::Replace
    }
}

fn past_threshold(a: iced::Point, b: iced::Point) -> bool {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    dx * dx + dy * dy > DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX
}

impl PcbCanvas {
    pub(super) fn screen_to_world(&self, p: iced::Point, bounds: Rectangle) -> Point {
        let w = self.camera.borrow().screen_to_world(p, bounds);
        Point::new(f64::from(w.x), f64::from(w.y))
    }

    pub(super) fn scale(&self) -> f32 {
        self.camera.borrow().scale
    }
}

#[cfg(test)]
#[path = "gesture_tests.rs"]
mod tests;
