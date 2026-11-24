//! Edge pan (#2872): the pointer in a thin band at a canvas edge pans the
//! view while a tool is armed or a drag or move is in progress. This module
//! holds the band geometry and the link a canvas shares with the app; the
//! timer and the pan itself live in `crate::app::edge_pan`.

use std::cell::Cell;

use iced::event::Event;
use iced::{Point, Size, mouse};

/// The pointer over one canvas, in canvas pixels, and that canvas's size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgePointer {
    pub at: Point,
    pub size: Size,
}

/// What a canvas and the app share about edge panning. The canvas writes
/// where the pointer is on every event; the app reads it on each tick and
/// marks the view moved, so the canvas replays the pointer at its next
/// redraw and what follows the pointer (a ghost, a dragged object) catches
/// up. A canvas whose camera lives in the widget takes the pan it is owed.
#[derive(Debug, Default)]
pub struct EdgePanLink {
    pointer: Cell<Option<EdgePointer>>,
    view_moved: Cell<bool>,
    owed_pan: Cell<(f32, f32)>,
    dragging: Cell<bool>,
}

/// A copy is a fresh link: what one canvas saw is not another's.
impl Clone for EdgePanLink {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl EdgePanLink {
    /// A canvas event saw the pointer at `at` (canvas pixels), or not over
    /// the canvas (`None`: off it, or a panel or overlay covers it).
    pub fn observe(&self, at: Option<Point>, size: Size) {
        self.pointer.set(at.map(|at| EdgePointer { at, size }));
    }

    /// Where the pointer was at the canvas's last event.
    pub fn pointer(&self) -> Option<EdgePointer> {
        self.pointer.get()
    }

    /// The app moved the view under the pointer.
    pub fn mark_view_moved(&self) {
        self.view_moved.set(true);
    }

    /// The app moved a widget-held camera by `(dx, dy)` canvas pixels; the
    /// canvas applies it at its next event.
    pub fn owe_pan(&self, dx: f32, dy: f32) {
        let (x, y) = self.owed_pan.get();
        self.owed_pan.set((x + dx, y + dy));
        self.view_moved.set(true);
    }

    /// The pan owed and not yet taken.
    pub fn owed_pan(&self) -> (f32, f32) {
        self.owed_pan.get()
    }

    /// Take the owed pan; `None` when nothing is owed.
    pub fn take_owed_pan(&self) -> Option<(f32, f32)> {
        let owed = self.owed_pan.replace((0.0, 0.0));
        (owed != (0.0, 0.0)).then_some(owed)
    }

    /// A widget-held drag is in flight (the footprint canvas writes this).
    pub fn set_dragging(&self, dragging: bool) {
        self.dragging.set(dragging);
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging.get()
    }

    /// The event a canvas handles in place of `event`: once the view moved
    /// under a still pointer, its next redraw replays the pointer as a move.
    pub fn replayed(&self, event: &Event, cursor: mouse::Cursor) -> Option<Event> {
        if !matches!(
            event,
            Event::Window(iced::window::Event::RedrawRequested(_))
        ) || !self.view_moved.get()
        {
            return None;
        }
        self.view_moved.set(false);
        let position = cursor.position()?;
        Some(Event::Mouse(mouse::Event::CursorMoved { position }))
    }
}

/// The width of the band at each edge, in logical pixels: two lines of body
/// text, so it follows the type scale rather than a fixed size.
pub fn band_px() -> f32 {
    crate::styles::TypeScale::DEFAULT.body * 2.0
}

/// The pan one tick owes for `pointer`, in canvas pixels toward the edge it
/// is near (positive x: the right edge): `max_step` at the edge, falling to
/// nothing at the band's inner side. `None` outside every band, or on a
/// canvas too small to have one.
pub fn edge_step(pointer: EdgePointer, band: f32, max_step: f32) -> Option<(f32, f32)> {
    let EdgePointer { at, size } = pointer;
    let usable = band.is_finite() && max_step.is_finite() && band > 0.0 && max_step > 0.0;
    if !usable || size.width <= band * 2.0 || size.height <= band * 2.0 {
        return None;
    }
    let axis = |pos: f32, len: f32| -> f32 {
        if !(0.0..=len).contains(&pos) {
            return 0.0;
        }
        let depth = ((band - pos.min(len - pos)) / band).clamp(0.0, 1.0);
        if pos < len / 2.0 {
            -depth * max_step
        } else {
            depth * max_step
        }
    };
    let step = (axis(at.x, size.width), axis(at.y, size.height));
    (step != (0.0, 0.0)).then_some(step)
}

impl super::CanvasSlot {
    /// Move the sheet on screen by `(dx, dy)` canvas pixels, as a
    /// right-button drag does.
    pub fn pan_view(&self, dx: f32, dy: f32) {
        self.camera_mut().pan(dx, dy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Size = Size {
        width: 800.0,
        height: 600.0,
    };

    fn at(x: f32, y: f32) -> EdgePointer {
        EdgePointer {
            at: Point::new(x, y),
            size: SIZE,
        }
    }

    #[test]
    fn the_band_steps_toward_its_edge_faster_nearer_it() {
        let (band, max) = (24.0, 12.0);
        let (near, _) = edge_step(at(799.0, 300.0), band, max).expect("in the right band");
        let (far, _) = edge_step(at(780.0, 300.0), band, max).expect("in the right band");
        assert!(near > far && far > 0.0, "{near} vs {far}");
        let (left, _) = edge_step(at(1.0, 300.0), band, max).expect("in the left band");
        assert!(left < 0.0);
        let (_, down) = edge_step(at(400.0, 599.0), band, max).expect("in the bottom band");
        assert!(down > 0.0);
    }

    #[test]
    fn the_middle_and_a_tiny_canvas_have_no_step() {
        assert_eq!(edge_step(at(400.0, 300.0), 24.0, 12.0), None);
        let tiny = EdgePointer {
            at: Point::new(1.0, 1.0),
            size: Size::new(40.0, 40.0),
        };
        assert_eq!(edge_step(tiny, 24.0, 12.0), None);
    }

    #[test]
    fn a_moved_view_replays_the_pointer_once_at_the_next_redraw() {
        let link = EdgePanLink::default();
        let cursor = mouse::Cursor::Available(Point::new(5.0, 6.0));
        let redraw = Event::Window(iced::window::Event::RedrawRequested(
            iced::time::Instant::now(),
        ));
        assert!(link.replayed(&redraw, cursor).is_none());
        link.mark_view_moved();
        let moved = Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(5.0, 6.0),
        });
        assert!(
            link.replayed(&moved, cursor).is_none(),
            "only a redraw replays"
        );
        assert!(matches!(
            link.replayed(&redraw, cursor),
            Some(Event::Mouse(mouse::Event::CursorMoved { position })) if position == Point::new(5.0, 6.0)
        ));
        assert!(link.replayed(&redraw, cursor).is_none());
    }

    #[test]
    fn an_owed_pan_adds_up_and_is_taken_once() {
        let link = EdgePanLink::default();
        assert_eq!(link.take_owed_pan(), None);
        link.owe_pan(-3.0, 0.0);
        link.owe_pan(-2.0, 1.0);
        assert_eq!(link.take_owed_pan(), Some((-5.0, 1.0)));
        assert_eq!(link.take_owed_pan(), None);
    }
}
