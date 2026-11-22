//! Properties fields of the Elliptical Arc and Round Rectangle (#2838).

use crate::library::editor::symbol::rect_end_for_extent;
use crate::panels::GraphicFieldId as F;
use signex_library::SymbolGraphicKind as K;

/// Write `field` on an elliptical arc or round rectangle. `false` when
/// `kind` is neither, so the caller handles it. A field the kind does not
/// have is ignored.
pub(super) fn apply(kind: &mut K, field: F, value: f64) -> bool {
    match kind {
        K::EllipticalArc {
            center,
            radius_x,
            radius_y,
            start_deg,
            end_deg,
        } => {
            match field {
                F::CenterX => center[0] = value,
                F::CenterY => center[1] = value,
                F::RadiusX => *radius_x = value,
                F::RadiusY => *radius_y = value,
                // Kept in `[0, 360)`, as for an arc.
                F::StartDeg => *start_deg = value.rem_euclid(360.0),
                F::EndDeg => *end_deg = value.rem_euclid(360.0),
                _ => {}
            }
            true
        }
        K::RoundRectangle {
            from,
            to,
            corner_radius,
        } => {
            match field {
                F::FromX => from[0] = value,
                F::FromY => from[1] = value,
                F::ToX => to[0] = value,
                F::ToY => to[1] = value,
                F::Width => to[0] = rect_end_for_extent(from[0], to[0], value),
                F::Height => to[1] = rect_end_for_extent(from[1], to[1], value),
                F::CornerRadius => {
                    *corner_radius = value.min(K::max_corner_radius(*from, *to));
                }
                _ => {}
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_corner_radius_field_is_clamped_to_half_the_short_side() {
        let mut kind = K::round_rectangle([0.0, 0.0], [10.16, 5.08]);
        assert!(apply(&mut kind, F::CornerRadius, 9.0));
        let K::RoundRectangle { corner_radius, .. } = kind else {
            panic!("expected a RoundRectangle");
        };
        assert_eq!(corner_radius, 2.54);
    }

    #[test]
    fn elliptical_arc_fields_write_radii_and_reduce_angles() {
        let mut kind = K::EllipticalArc {
            center: [0.0, 0.0],
            radius_x: 5.08,
            radius_y: 2.54,
            start_deg: 0.0,
            end_deg: 90.0,
        };
        assert!(apply(&mut kind, F::RadiusY, 3.81));
        assert!(apply(&mut kind, F::EndDeg, -60.0));
        let K::EllipticalArc {
            radius_y, end_deg, ..
        } = kind
        else {
            panic!("expected an EllipticalArc");
        };
        assert_eq!((radius_y, end_deg), (3.81, 300.0));
    }

    #[test]
    fn another_kind_is_left_to_the_caller() {
        let mut kind = K::Line {
            from: [0.0, 0.0],
            to: [1.0, 1.0],
        };
        assert!(!apply(&mut kind, F::FromX, 5.0));
    }
}
