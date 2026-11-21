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
/// Most time between the clicks of a double-click (ms), and most travel
/// between them (px), as on the schematic canvas.
const DOUBLE_CLICK_MS: u128 = 300;
const DOUBLE_CLICK_MAX_PX: f32 = 5.0;

#[derive(Debug, Default)]
pub(super) struct Gesture {
    modifiers: Modifiers,
    press: Option<iced::Point>,
    /// The press has passed the drag threshold and owns the canvas marquee,
    /// or (with `carrying`) drags the selected footprints.
    dragging: bool,
    /// The press landed on a footprint: a drag moves it (#2240, #2358).
    carrying: bool,
    /// A tool or a carry waited for clicks at the press: its release is a
    /// click wherever it lands (#2396).
    clicks_only: bool,
    /// When and where the last plain click was: a second one soon after,
    /// nearby, is a double-click.
    last_click: Option<(std::time::Instant, iced::Point)>,
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
                // Ctrl pressed or released mid-drag switches whether the
                // tracks follow at once, not at the next pointer move.
                if !(self.carrying && self.dragging && canvas.carry.get()) {
                    return None;
                }
                let (press, pos) = (self.press?, cursor.position_in(bounds)?);
                let msg = self.drag_message(canvas, press, pos, bounds);
                Some(canvas::Action::publish(Message::PcbCanvas(msg)))
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
                self.clicks_only = canvas.is_clicks_only();
                let at = canvas.screen_to_world(pos, bounds);
                // Ctrl alone still drags, leaving the tracks; Shift (or
                // Ctrl+Shift) draws a box from a footprint too.
                self.carrying = !self.clicks_only
                    && (self.modifiers.is_empty() || leaves_tracks(self.modifiers))
                    && canvas.grabs(at, hover_reach_mm(canvas.scale()));
                Some(canvas::Action::publish(message).and_capture())
            }
            // The focus left: a modifier released in another window never
            // reaches this one, so a stale Ctrl or Shift would turn every
            // later click into a toggle (#2396).
            Event::Window(iced::window::Event::Unfocused) => {
                self.modifiers = Modifiers::default();
                self.pointer_gone(canvas)
            }
            // The pointer left mid-gesture: its release may never arrive.
            Event::Mouse(mouse::Event::CursorLeft) => self.pointer_gone(canvas),
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
                let msg = self.drag_message(canvas, press, pos, bounds);
                Some(canvas::Action::publish(Message::PcbCanvas(msg)).and_capture())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let press = self.press?;
                if self.clicks_only {
                    // No box: the canvas keeps publishing the pointer.
                    return None;
                }
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
                let clicks_only = self.clicks_only;
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
                let msg = if carried {
                    PcbCanvasMsg::FootprintDropped {
                        from: canvas.screen_to_world(press, bounds),
                        to: canvas.screen_to_world(pos, bounds),
                    }
                } else if !clicks_only && past_threshold(press, pos) {
                    // Seen from below (#2394) the box is judged mirrored.
                    let (from, to, deg) = canvas.camera_ref().box_frame(
                        canvas.screen_to_world(press, bounds),
                        canvas.screen_to_world(pos, bounds),
                    );
                    PcbCanvasMsg::BoxSelected {
                        from,
                        to,
                        mode: box_mode(self.modifiers),
                        view_rotation_deg: f64::from(deg),
                    }
                } else if self.modifiers.is_empty() && self.second_click(press) {
                    PcbCanvasMsg::DoubleClicked {
                        at: canvas.screen_to_world(press, bounds),
                        tolerance_mm: hover_reach_mm(canvas.scale()),
                    }
                } else {
                    PcbCanvasMsg::Clicked {
                        at: canvas.screen_to_world(press, bounds),
                        tolerance_mm: hover_reach_mm(canvas.scale()),
                        mode: click_mode(self.modifiers),
                    }
                };
                Some(canvas::Action::publish(Message::PcbCanvas(msg)).and_capture())
            }
            _ => None,
        }
    }

    /// The pointer or the focus left mid-gesture: drop the press, and a
    /// footprint drag it carries.
    fn pointer_gone(&mut self, canvas: &PcbCanvas) -> Option<canvas::Action<Message>> {
        self.press?;
        let carried = self.carrying && self.dragging;
        self.drop_press(canvas);
        if carried {
            let msg = Message::PcbCanvas(PcbCanvasMsg::FootprintDragCancelled);
            return Some(canvas::Action::publish(msg));
        }
        Some(canvas::Action::request_redraw())
    }

    /// A plain click at `pos` (canvas pixels): whether it is the second of
    /// a double-click, as the schematic canvas reads one (300 ms, 5 px).
    fn second_click(&mut self, pos: iced::Point) -> bool {
        let now = std::time::Instant::now();
        let double = self.last_click.is_some_and(|(when, at)| {
            let dist = (pos.x - at.x).hypot(pos.y - at.y);
            now.duration_since(when).as_millis() < DOUBLE_CLICK_MS && dist < DOUBLE_CLICK_MAX_PX
        });
        self.last_click = if double { None } else { Some((now, pos)) };
        double
    }

    /// The drag pressed at `press` (canvas pixels) is at `pos`.
    fn drag_message(
        &self,
        canvas: &PcbCanvas,
        press: iced::Point,
        pos: iced::Point,
        bounds: Rectangle,
    ) -> PcbCanvasMsg {
        PcbCanvasMsg::FootprintDragged {
            from: canvas.screen_to_world(press, bounds),
            to: canvas.screen_to_world(pos, bounds),
            tolerance_mm: hover_reach_mm(canvas.scale()),
            leaves_tracks: leaves_tracks(self.modifiers),
        }
    }

    fn drop_press(&mut self, canvas: &PcbCanvas) {
        self.press = None;
        self.dragging = false;
        self.carrying = false;
        self.clicks_only = false;
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
        (camera.zoom_percent(), camera.scale())
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

/// A click: Shift, Ctrl or Cmd toggles the item under it and keeps the
/// rest; a plain click replaces (#2396).
fn click_mode(mods: Modifiers) -> ClickMode {
    if mods.shift() || mods.control() || mods.command() {
        ClickMode::Toggle
    } else {
        ClickMode::Replace
    }
}

/// A footprint drag with Ctrl (Cmd on macOS) and no other modifier leaves
/// the tracks on its pads in place (Caner, 2026-11-21).
fn leaves_tracks(mods: Modifiers) -> bool {
    (mods.control() || mods.command()) && !mods.shift() && !mods.alt()
}

/// A box: Shift adds, Ctrl or Cmd toggles, a plain box replaces.
fn box_mode(mods: Modifiers) -> ClickMode {
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
    /// Canvas pixels to board mm under the live camera, its turn included.
    pub fn screen_to_world(&self, p: iced::Point, _bounds: Rectangle) -> Point {
        let w = self.camera.borrow().screen_to_world(p);
        Point::new(f64::from(w.x), f64::from(w.y))
    }

    pub(super) fn scale(&self) -> f32 {
        self.camera.borrow().scale()
    }
}

#[cfg(test)]
#[path = "gesture_tests.rs"]
mod tests;
