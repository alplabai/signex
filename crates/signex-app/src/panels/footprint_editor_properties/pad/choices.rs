//! The choices the pad form offers: the copper shapes and the hole shapes
//! the pad model stores.

use signex_i18n::{MessageId, ids};

/// Pick-list proxy for `signex_library::PadShape`: Round / Rectangular /
/// Rounded Rectangle / Chamfered Rectangle / Trapezoid. Custom shapes are
/// drawn in Sketch mode.
/// Schema-mapping notes:
///   - Octagonal and Donut are deliberately not offered: neither has
///     a variant on `signex_library::PadShape`, the sketch pad
///     schema, or the bake path, so choosing one could only be
///     stored as Round. A picker entry that silently changes on save
///     is a data-loss trap; add them back only together with a
///     native schema variant and bake geometry.
///   - Chamfered Rectangle uses the existing `Chamfered` variant
///     with sensible defaults (25% chamfer, all corners).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PadShapeChoice {
    Round,
    Rectangular,
    RoundedRectangle,
    ChamferedRectangle,
    /// A trapezoid (#2242); the pad form shows its Delta X / Delta Y.
    Trapezoid,
}

impl PadShapeChoice {
    pub(super) const ALL: &'static [PadShapeChoice] = &[
        PadShapeChoice::Round,
        PadShapeChoice::Rectangular,
        PadShapeChoice::RoundedRectangle,
        PadShapeChoice::ChamferedRectangle,
        PadShapeChoice::Trapezoid,
    ];

    pub(super) fn from_lib(s: &signex_library::PadShape) -> Self {
        match s {
            signex_library::PadShape::Round => PadShapeChoice::Round,
            signex_library::PadShape::Rect => PadShapeChoice::Rectangular,
            signex_library::PadShape::RoundRect { .. } => PadShapeChoice::RoundedRectangle,
            signex_library::PadShape::Chamfered { .. } => PadShapeChoice::ChamferedRectangle,
            signex_library::PadShape::Trapezoid { .. } => PadShapeChoice::Trapezoid,
            // Oval / Custom have no picker entry; the picker shows
            // Round for them and only rewrites the pad when the user
            // actually picks a shape. Custom Shape is intentionally
            // absent — use sketch mode for freeform geometry.
            _ => PadShapeChoice::Round,
        }
    }
    pub(super) fn to_lib(self) -> signex_library::PadShape {
        use signex_library::primitive::footprint::ChamferedCorners;
        match self {
            PadShapeChoice::Round => signex_library::PadShape::Round,
            PadShapeChoice::Rectangular => signex_library::PadShape::Rect,
            PadShapeChoice::RoundedRectangle => {
                signex_library::PadShape::RoundRect { radius_ratio: 0.25 }
            }
            PadShapeChoice::ChamferedRectangle => signex_library::PadShape::Chamfered {
                chamfer_ratio: 0.25,
                corners: ChamferedCorners::all(),
            },
            // The pad form seeds a visible delta from the pad size.
            PadShapeChoice::Trapezoid => signex_library::PadShape::Trapezoid { delta: [0.0, 0.0] },
        }
    }
}

impl std::fmt::Display for PadShapeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PadShapeChoice::Round => "Round",
            PadShapeChoice::Rectangular => "Rectangular",
            PadShapeChoice::RoundedRectangle => "Rounded Rectangle",
            PadShapeChoice::ChamferedRectangle => "Chamfered Rectangle",
            PadShapeChoice::Trapezoid => "Trapezoid",
        })
    }
}

/// The hole shapes the pad model stores: a round drill, or a drill with a
/// slot length. A rectangular hole has no field in the model, so it is not
/// offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HoleShapeChoice {
    Round,
    Slot,
}

impl HoleShapeChoice {
    pub(super) const ALL: [HoleShapeChoice; 2] = [HoleShapeChoice::Round, HoleShapeChoice::Slot];

    /// The shape of a hole with `slot_length`.
    pub(super) fn of(slot_length_mm: Option<f64>) -> Self {
        if slot_length_mm.is_some() {
            HoleShapeChoice::Slot
        } else {
            HoleShapeChoice::Round
        }
    }

    pub(super) fn label(self) -> MessageId {
        match self {
            HoleShapeChoice::Round => ids::EDITORS_FP_PAD_HOLE_ROUND,
            HoleShapeChoice::Slot => ids::EDITORS_FP_PAD_HOLE_SLOT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HoleShapeChoice, PadShapeChoice};

    /// #964: whatever the shape picker offers must come back as the
    /// same choice once stored, or the pad changes under the user.
    #[test]
    fn every_offered_shape_round_trips_through_the_library_shape() {
        for choice in PadShapeChoice::ALL {
            assert_eq!(
                PadShapeChoice::from_lib(&choice.to_lib()),
                *choice,
                "{choice} is stored as a different shape"
            );
        }
    }

    /// Only the hole shapes the model stores are offered, and a stored slot
    /// length reads back as a slot.
    #[test]
    fn the_hole_shape_follows_the_stored_slot_length() {
        assert_eq!(HoleShapeChoice::ALL.len(), 2);
        assert_eq!(HoleShapeChoice::of(None), HoleShapeChoice::Round);
        assert_eq!(HoleShapeChoice::of(Some(1.8)), HoleShapeChoice::Slot);
    }
}
