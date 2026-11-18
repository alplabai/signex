//! A trapezoid pad's delta (#2242): the Delta X / Delta Y rows of the pad
//! form, for the selected pad and for the next pad placed. A value that
//! would make no copper with the pad's size (the board's rule,
//! `signex_types::pcb_pad_shape`) is not written, like a size of 0.

use signex_library::PadShape;
use signex_types::pcb::Point;
use signex_types::pcb_pad_shape::trapezoid_corners;

use super::super::*;

/// Which delta a row edits: `0` is Delta X, `1` Delta Y.
pub(crate) type DeltaAxis = usize;

/// `shape` with its delta on `axis` set to `value`, when it is a trapezoid
/// and the result fits a pad of `size_mm`; the other axis is cleared, as a
/// trapezoid narrows along one axis only.
fn with_delta(
    shape: &PadShape,
    size_mm: (f64, f64),
    axis: DeltaAxis,
    value: f64,
) -> Option<PadShape> {
    let PadShape::Trapezoid { .. } = shape else {
        return None;
    };
    let mut delta = [0.0; 2];
    delta[axis.min(1)] = value;
    trapezoid_corners(
        Point::new(size_mm.0, size_mm.1),
        Point::new(delta[0], delta[1]),
    )
    .ok()?;
    Some(PadShape::Trapezoid { delta })
}

/// Whether `shape` still makes copper on a pad of `size_mm`: a size edit
/// that would leave a trapezoid's delta too wide is not written, as the
/// board's pad edit refuses it.
pub(crate) fn size_fits_shape(shape: &PadShape, size_mm: (f64, f64)) -> bool {
    let PadShape::Trapezoid { delta } = shape else {
        return true;
    };
    trapezoid_corners(
        Point::new(size_mm.0, size_mm.1),
        Point::new(delta[0], delta[1]),
    )
    .is_ok()
}

/// The shape picked for a pad that was `previous`: a trapezoid picked again
/// keeps its delta, and a pad turning into one gets its top edge half its
/// width longer, so the change shows.
pub(crate) fn seeded_trapezoid(shape: PadShape, previous: &PadShape, size_x_mm: f64) -> PadShape {
    match (&shape, previous) {
        (PadShape::Trapezoid { .. }, PadShape::Trapezoid { delta }) => {
            PadShape::Trapezoid { delta: *delta }
        }
        (PadShape::Trapezoid { delta }, _) if *delta == [0.0, 0.0] => PadShape::Trapezoid {
            delta: [size_x_mm.abs() * 0.5, 0.0],
        },
        _ => shape,
    }
}

impl Signex {
    pub(crate) fn fp_editor_set_selected_pad_trapezoid_delta(
        &mut self,
        idx: usize,
        axis: DeltaAxis,
        value: String,
    ) -> bool {
        let Some(parsed) = crate::numeric_field::parse_field_f64(&value) else {
            return true;
        };
        let field = if axis == 0 {
            "pad_trapezoid_delta_x"
        } else {
            "pad_trapezoid_delta_y"
        };
        self.with_selected_pad_field(idx, Some(field), |pad| {
            if let Some(shape) = with_delta(&pad.shape, pad.size_mm, axis, parsed) {
                pad.shape = shape;
            }
        });
        true
    }

    pub(crate) fn fp_editor_set_next_pad_trapezoid_delta(
        &mut self,
        axis: DeltaAxis,
        value: String,
    ) -> bool {
        let Some(parsed) = crate::numeric_field::parse_field_f64(&value) else {
            return true;
        };
        if let Some(editor) = self.active_footprint_editor_mut() {
            let defaults = &mut editor.state.next_pad_defaults;
            let size = (defaults.size_x_mm, defaults.size_y_mm);
            if let Some(shape) = with_delta(&defaults.shape, size, axis, parsed) {
                defaults.shape = shape;
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delta_that_fits_is_set_and_clears_the_other_axis() {
        let shape = PadShape::Trapezoid { delta: [0.0, 0.3] };
        assert_eq!(
            with_delta(&shape, (2.0, 1.0), 0, 1.0),
            Some(PadShape::Trapezoid { delta: [1.0, 0.0] })
        );
    }

    #[test]
    fn a_delta_that_makes_no_copper_or_a_shape_without_one_is_not_set() {
        let shape = PadShape::Trapezoid { delta: [0.0, 0.0] };
        assert_eq!(with_delta(&shape, (2.0, 1.0), 1, 2.0), None);
        assert_eq!(with_delta(&PadShape::Rect, (2.0, 1.0), 0, 0.5), None);
    }

    #[test]
    fn a_size_that_leaves_the_delta_too_wide_does_not_fit() {
        // Size X 1.0 seeds delta 0.5; Size X 0.2 would make no copper.
        let shape = PadShape::Trapezoid { delta: [0.5, 0.0] };
        assert!(size_fits_shape(&shape, (1.0, 1.0)));
        assert!(!size_fits_shape(&shape, (0.2, 1.0)));
        assert!(size_fits_shape(&PadShape::Rect, (0.2, 1.0)));
    }

    #[test]
    fn turning_into_a_trapezoid_seeds_a_visible_delta() {
        let picked = PadShape::Trapezoid { delta: [0.0, 0.0] };
        assert_eq!(
            seeded_trapezoid(picked.clone(), &PadShape::Rect, 1.6),
            PadShape::Trapezoid { delta: [0.8, 0.0] }
        );
        // Picking Trapezoid again keeps the delta the pad has.
        let kept = PadShape::Trapezoid { delta: [0.0, 0.2] };
        assert_eq!(seeded_trapezoid(picked, &kept, 1.6), kept);
    }
}
