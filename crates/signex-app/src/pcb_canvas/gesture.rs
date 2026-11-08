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
use crate::app::Message;

/// A press that moves farther than this (px) is a box, not a click.
const DRAG_THRESHOLD_PX: f32 = 4.0;
/// How far from the pointer an item still counts as under it (px).
const HIT_REACH_PX: f32 = 4.0;

#[derive(Debug, Default)]
pub(super) struct Gesture {
    modifiers: Modifiers,
    press: Option<iced::Point>,
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
                let pos = cursor.position_in(bounds)?;
                self.press = Some(pos);
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let press = self.press?;
                let pos = cursor.position_in(bounds)?;
                if !past_threshold(press, pos) {
                    return None;
                }
                canvas.marquee.set(Some((
                    canvas.screen_to_world(press, bounds),
                    canvas.screen_to_world(pos, bounds),
                )));
                Some(canvas::Action::request_redraw().and_capture())
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let press = self.press.take()?;
                canvas.marquee.set(None);
                let pos = cursor
                    .position()
                    .map(|p| iced::Point::new(p.x - bounds.x, p.y - bounds.y))
                    .unwrap_or(press);
                let mode = click_mode(self.modifiers);
                let msg = if past_threshold(press, pos) {
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
