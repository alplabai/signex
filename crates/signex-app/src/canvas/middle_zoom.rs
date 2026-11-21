//! Middle-button drag zoom, shared by the schematic, board, footprint and
//! symbol canvases: up zooms in, down zooms out, about the press point.
//! Each canvas keeps a [`MiddleZoom`] in its widget state and applies the
//! factors through its own zoom path, so its own scale limits hold.

use iced::{Point, mouse};

/// Vertical drag, in logical pixels, that zooms as far as one wheel notch.
/// The notch itself is the Preferences "Zoom speed".
pub const MIDDLE_DRAG_PIXELS_PER_NOTCH: f32 = 20.0;

/// One held middle-button drag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MiddleZoom {
    /// The press point in canvas pixels: the zoom's fixed point.
    anchor: Point,
    /// Window y of the last move. Window space, so the drag keeps working
    /// when the pointer crosses the canvas edge.
    last_y: f32,
    /// The drag has changed the camera.
    zoomed: bool,
}

impl MiddleZoom {
    /// A drag pressed at `anchor` (canvas pixels), `window_y` in the window.
    pub fn pressed(anchor: Point, window_y: f32) -> Self {
        Self {
            anchor,
            last_y: window_y,
            zoomed: false,
        }
    }

    /// The press point in canvas pixels.
    pub fn anchor(&self) -> Point {
        self.anchor
    }

    /// Whether the drag has changed the camera.
    pub fn has_zoomed(&self) -> bool {
        self.zoomed
    }

    /// Record that the camera changed; true the first time.
    pub fn first_zoom(&mut self) -> bool {
        !std::mem::replace(&mut self.zoomed, true)
    }

    /// The pointer is at `window_y`: the scale factor for the distance moved
    /// since the last move at per-notch factor `step`, or `None` when the
    /// move has no vertical part.
    pub fn dragged_to(&mut self, window_y: f32, step: f32) -> Option<f32> {
        let moved_up = self.last_y - window_y;
        if moved_up == 0.0 || !moved_up.is_finite() {
            return None;
        }
        self.last_y = window_y;
        let factor = step.powf(moved_up / MIDDLE_DRAG_PIXELS_PER_NOTCH);
        (factor.is_finite() && factor > 0.0 && factor != 1.0).then_some(factor)
    }
}

/// The pointer's window y, also while it is over a layer above the canvas.
pub fn pointer_y(cursor: mouse::Cursor) -> Option<f32> {
    cursor.land().position().map(|position| position.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: f32 = 1.1;

    #[test]
    fn up_is_above_one_down_is_below_one_and_sideways_is_nothing() {
        let mut drag = MiddleZoom::pressed(Point::new(10.0, 10.0), 100.0);
        assert!(drag.dragged_to(80.0, STEP).is_some_and(|f| f > 1.0));
        assert!(drag.dragged_to(100.0, STEP).is_some_and(|f| f < 1.0));
        assert_eq!(drag.dragged_to(100.0, STEP), None);
    }

    #[test]
    fn one_notch_of_drag_is_one_zoom_step() {
        let mut drag = MiddleZoom::pressed(Point::ORIGIN, 100.0);
        let factor = drag
            .dragged_to(100.0 - MIDDLE_DRAG_PIXELS_PER_NOTCH, STEP)
            .expect("a vertical move");
        assert!((factor - STEP).abs() < 1e-5, "{factor}");
    }

    #[test]
    fn a_drag_in_two_moves_zooms_as_far_as_in_one() {
        let mut one = MiddleZoom::pressed(Point::ORIGIN, 100.0);
        let mut two = MiddleZoom::pressed(Point::ORIGIN, 100.0);
        let whole = one.dragged_to(40.0, STEP).expect("moved");
        let first = two.dragged_to(75.0, STEP).expect("moved");
        let second = two.dragged_to(40.0, STEP).expect("moved");
        assert!((first * second - whole).abs() < 1e-4);
    }

    #[test]
    fn a_non_finite_position_is_ignored_and_keeps_the_last_one() {
        let mut drag = MiddleZoom::pressed(Point::ORIGIN, 100.0);
        assert_eq!(drag.dragged_to(f32::NAN, STEP), None);
        assert_eq!(drag.dragged_to(f32::INFINITY, STEP), None);
        assert!(drag.dragged_to(80.0, STEP).is_some_and(|f| f > 1.0));
    }

    #[test]
    fn only_the_first_zoom_reports_first() {
        let mut drag = MiddleZoom::pressed(Point::ORIGIN, 0.0);
        assert!(!drag.has_zoomed());
        assert!(drag.first_zoom());
        assert!(!drag.first_zoom());
        assert!(drag.has_zoomed());
    }

    #[test]
    fn a_levitating_pointer_still_reports_its_height() {
        let at = Point::new(3.0, 42.0);
        assert_eq!(pointer_y(mouse::Cursor::Levitating(at)), Some(42.0));
        assert_eq!(pointer_y(mouse::Cursor::Unavailable), None);
    }
}
