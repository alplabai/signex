//! The corner rule for a run drawn from an anchor to the cursor: how it
//! bends into two legs. Schematic wires and buses use it now; interactive
//! track routing on a board is meant to use the same rule (#2372). Also the
//! drag wiring mode: how wires that follow a dragged pin bend (#1669).

use crate::schematic::Point;

/// Legs and offsets shorter than this (mm) count as zero.
const LEG_TOLERANCE_MM: f64 = 0.01;

/// How a run bends between its anchor and the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CornerMode {
    /// Two axis-aligned legs meeting at a right angle.
    #[default]
    Ortho90,
    /// A 45° diagonal leg first, then an axis-aligned leg.
    Angle45Start,
    /// An axis-aligned leg first, then a 45° diagonal leg.
    Angle45End,
    /// One straight leg at any slope.
    FreeAngle,
}

impl CornerMode {
    /// The mode after this one: 90°, 45° start, 45° end, any angle, 90°.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Ortho90 => Self::Angle45Start,
            Self::Angle45Start => Self::Angle45End,
            Self::Angle45End => Self::FreeAngle,
            Self::FreeAngle => Self::Ortho90,
        }
    }

    /// Every mode, in [`Self::next`] order.
    pub const ALL: [Self; 4] = [
        Self::Ortho90,
        Self::Angle45Start,
        Self::Angle45End,
        Self::FreeAngle,
    ];

    /// The mode's name for an agent or the CLI (`route_options`, #2719).
    pub const fn token(self) -> &'static str {
        match self {
            Self::Ortho90 => "90",
            Self::Angle45Start => "45_start",
            Self::Angle45End => "45_end",
            Self::FreeAngle => "any_angle",
        }
    }

    /// The mode named `token` ([`Self::token`]), any case.
    pub fn from_token(token: &str) -> Option<Self> {
        let token = token.trim();
        Self::ALL
            .into_iter()
            .find(|m| m.token().eq_ignore_ascii_case(token))
    }
}

/// The leg order a rule draws, once its flip is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bend {
    HorizontalFirst,
    VerticalFirst,
    DiagonalFirst,
    DiagonalLast,
    /// One leg, no corner.
    Straight,
}

/// A corner mode and whether its leg order is flipped. The flip swaps the
/// two legs: horizontal-first becomes vertical-first, a diagonal start
/// becomes a diagonal end. It has no effect on [`CornerMode::FreeAngle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CornerRule {
    pub mode: CornerMode,
    pub is_flipped: bool,
}

impl CornerRule {
    /// This rule with the leg order swapped.
    #[must_use]
    pub fn flipped(self) -> Self {
        Self {
            is_flipped: !self.is_flipped,
            ..self
        }
    }

    /// This rule in the next mode, keeping the flip.
    #[must_use]
    pub fn with_next_mode(self) -> Self {
        Self {
            mode: self.mode.next(),
            ..self
        }
    }

    /// The leg order this rule draws.
    #[must_use]
    pub fn bend(self) -> Bend {
        match (self.mode, self.is_flipped) {
            (CornerMode::FreeAngle, _) => Bend::Straight,
            (CornerMode::Ortho90, false) => Bend::HorizontalFirst,
            (CornerMode::Ortho90, true) => Bend::VerticalFirst,
            (CornerMode::Angle45Start, false) | (CornerMode::Angle45End, true) => {
                Bend::DiagonalFirst
            }
            (CornerMode::Angle45Start, true) | (CornerMode::Angle45End, false) => {
                Bend::DiagonalLast
            }
        }
    }

    /// The legs from `start` to `end`: one when the run is straight already
    /// (on an axis, or on an exact diagonal in a 45° mode), else two meeting
    /// at the corner. None when the two points are the same.
    #[must_use]
    pub fn legs(self, start: Point, end: Point) -> Vec<(Point, Point)> {
        let (dx, dy) = (end.x - start.x, end.y - start.y);
        let (adx, ady) = (dx.abs(), dy.abs());
        if adx < LEG_TOLERANCE_MM && ady < LEG_TOLERANCE_MM {
            return Vec::new();
        }
        let on_axis = adx < LEG_TOLERANCE_MM || ady < LEG_TOLERANCE_MM;
        let corner = match self.bend() {
            Bend::Straight => None,
            _ if on_axis => None,
            Bend::HorizontalFirst => Some(Point::new(end.x, start.y)),
            Bend::VerticalFirst => Some(Point::new(start.x, end.y)),
            Bend::DiagonalFirst | Bend::DiagonalLast if (adx - ady).abs() < LEG_TOLERANCE_MM => {
                None
            }
            bend @ (Bend::DiagonalFirst | Bend::DiagonalLast) => {
                let d = adx.min(ady);
                let (diag_x, diag_y) = (d.copysign(dx), d.copysign(dy));
                Some(if bend == Bend::DiagonalFirst {
                    Point::new(start.x + diag_x, start.y + diag_y)
                } else {
                    Point::new(end.x - diag_x, end.y - diag_y)
                })
            }
        };
        match corner {
            Some(corner) => vec![(start, corner), (corner, end)],
            None => vec![(start, end)],
        }
    }
}

/// How a drag bends the wires that follow a moved pin. Only an inserted
/// corner is affected; a wire that can stay straight stays straight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragWiringMode {
    /// The rubber band picks the corner: it keeps each wire's direction at
    /// its fixed end, and slides a plain elbow when it can.
    #[default]
    Auto,
    /// An inserted corner leaves the fixed end horizontally.
    HorizontalFirst,
    /// An inserted corner leaves the fixed end vertically.
    VerticalFirst,
    /// No corner: the wire end goes straight to the pin, at any slope.
    Straight,
    /// A 45° leg leaves the fixed end, then an axis-aligned leg (#2516).
    Angle45Start,
    /// An axis-aligned leg leaves the fixed end, then a 45° leg (#2516).
    Angle45End,
}

impl DragWiringMode {
    /// The mode after this one: auto, horizontal first, vertical first,
    /// straight, 45° start, 45° end, auto.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::HorizontalFirst,
            Self::HorizontalFirst => Self::VerticalFirst,
            Self::VerticalFirst => Self::Straight,
            Self::Straight => Self::Angle45Start,
            Self::Angle45Start => Self::Angle45End,
            Self::Angle45End => Self::Auto,
        }
    }

    /// The drawing rule this mode bends by, from the fixed end to the pin;
    /// `None` for Auto, which is the rubber band's own rule.
    #[must_use]
    pub fn rule(self) -> Option<CornerRule> {
        let mode = match self {
            Self::Auto => return None,
            Self::HorizontalFirst | Self::VerticalFirst => CornerMode::Ortho90,
            Self::Straight => CornerMode::FreeAngle,
            Self::Angle45Start => CornerMode::Angle45Start,
            Self::Angle45End => CornerMode::Angle45End,
        };
        Some(CornerRule {
            mode,
            is_flipped: self == Self::VerticalFirst,
        })
    }

    /// The rules a drag tries, in order: the mode's own, then its flip (the
    /// other L, or the 45° leg at the other end) when that one collides.
    /// Straight has one; Auto has none.
    #[must_use]
    pub fn rules(self) -> Vec<CornerRule> {
        let Some(rule) = self.rule() else {
            return Vec::new();
        };
        if rule.bend() == Bend::Straight {
            vec![rule]
        } else {
            vec![rule, rule.flipped()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_wiring_modes_cycle_back_to_auto() {
        let mut mode = DragWiringMode::default();
        assert_eq!(mode, DragWiringMode::Auto);
        let mut seen = Vec::new();
        for _ in 0..6 {
            mode = mode.next();
            seen.push(mode);
        }
        assert_eq!(
            seen,
            [
                DragWiringMode::HorizontalFirst,
                DragWiringMode::VerticalFirst,
                DragWiringMode::Straight,
                DragWiringMode::Angle45Start,
                DragWiringMode::Angle45End,
                DragWiringMode::Auto
            ]
        );
    }

    /// #2719: every corner mode has its own token, read back in any case,
    /// and `ALL` runs in `next` order.
    #[test]
    fn corner_mode_tokens_round_trip_in_cycle_order() {
        for (i, mode) in CornerMode::ALL.into_iter().enumerate() {
            assert_eq!(CornerMode::from_token(mode.token()), Some(mode));
            let upper = mode.token().to_ascii_uppercase();
            assert_eq!(CornerMode::from_token(&upper), Some(mode));
            assert_eq!(mode.next(), CornerMode::ALL[(i + 1) % 4]);
        }
        assert_eq!(CornerMode::from_token("arc"), None);
    }

    /// #2516: a drag mode bends by the drawing rule, and falls back to its
    /// flip; Straight has no fallback and Auto no rule.
    #[test]
    fn drag_wiring_modes_bend_by_the_drawing_rule_then_its_flip() {
        let ortho = rule(CornerMode::Ortho90, false);
        let start = rule(CornerMode::Angle45Start, false);
        let end = rule(CornerMode::Angle45End, false);
        assert!(DragWiringMode::Auto.rules().is_empty());
        assert_eq!(
            DragWiringMode::HorizontalFirst.rules(),
            [ortho, ortho.flipped()]
        );
        assert_eq!(
            DragWiringMode::VerticalFirst.rules(),
            [ortho.flipped(), ortho]
        );
        assert_eq!(
            DragWiringMode::Straight.rules(),
            [rule(CornerMode::FreeAngle, false)]
        );
        assert_eq!(
            DragWiringMode::Angle45Start.rules(),
            [start, start.flipped()]
        );
        assert_eq!(DragWiringMode::Angle45End.rules(), [end, end.flipped()]);
        // From the fixed end (0, 0) to the pin (10, -4).
        let pin = Point::new(10.0, -4.0);
        let first_corner = |mode: DragWiringMode| corner(mode.rule().expect("a rule"), pin);
        let cases = [
            (DragWiringMode::Angle45Start, Point::new(4.0, -4.0)),
            (DragWiringMode::Angle45End, Point::new(6.0, 0.0)),
            (DragWiringMode::HorizontalFirst, Point::new(10.0, 0.0)),
            (DragWiringMode::VerticalFirst, Point::new(0.0, -4.0)),
        ];
        for (mode, expected) in cases {
            assert_eq!(first_corner(mode), expected, "{mode:?}");
        }
    }

    fn rule(mode: CornerMode, is_flipped: bool) -> CornerRule {
        CornerRule { mode, is_flipped }
    }

    fn corner(rule: CornerRule, end: Point) -> Point {
        let legs = rule.legs(Point::new(0.0, 0.0), end);
        assert_eq!(legs.len(), 2, "{rule:?} to {end:?}: {legs:?}");
        assert_eq!(legs[0].1, legs[1].0);
        assert_eq!(legs[1].1, end);
        legs[0].1
    }

    #[test]
    fn the_flip_swaps_horizontal_first_for_vertical_first() {
        let end = Point::new(10.0, 4.0);
        assert_eq!(
            corner(rule(CornerMode::Ortho90, false), end),
            Point::new(10.0, 0.0)
        );
        assert_eq!(
            corner(rule(CornerMode::Ortho90, true), end),
            Point::new(0.0, 4.0)
        );
    }

    #[test]
    fn a_45_degree_mode_puts_the_diagonal_at_the_start_or_the_end() {
        let end = Point::new(10.0, -4.0);
        assert_eq!(
            corner(rule(CornerMode::Angle45Start, false), end),
            Point::new(4.0, -4.0)
        );
        assert_eq!(
            corner(rule(CornerMode::Angle45End, false), end),
            Point::new(6.0, 0.0)
        );
        // The flip swaps the legs, so a flipped start is an end.
        assert_eq!(
            rule(CornerMode::Angle45Start, true).bend(),
            Bend::DiagonalLast
        );
        assert_eq!(
            rule(CornerMode::Angle45End, true).bend(),
            Bend::DiagonalFirst
        );
        // A steep run: the axis leg is vertical.
        assert_eq!(
            corner(rule(CornerMode::Angle45Start, false), Point::new(-3.0, 8.0)),
            Point::new(-3.0, 3.0)
        );
    }

    #[test]
    fn straight_runs_have_one_leg_and_a_zero_run_none() {
        let origin = Point::new(0.0, 0.0);
        for mode in [
            CornerMode::Ortho90,
            CornerMode::Angle45Start,
            CornerMode::Angle45End,
            CornerMode::FreeAngle,
        ] {
            for is_flipped in [false, true] {
                let r = rule(mode, is_flipped);
                assert!(r.legs(origin, origin).is_empty());
                let flat = Point::new(7.0, 0.0);
                assert_eq!(r.legs(origin, flat), vec![(origin, flat)]);
                let tall = Point::new(0.0, -7.0);
                assert_eq!(r.legs(origin, tall), vec![(origin, tall)]);
            }
        }
        let diagonal = Point::new(5.0, 5.0);
        assert_eq!(
            rule(CornerMode::Angle45End, false).legs(origin, diagonal),
            vec![(origin, diagonal)]
        );
        let slope = Point::new(5.0, 2.0);
        assert_eq!(
            rule(CornerMode::FreeAngle, true).legs(origin, slope),
            vec![(origin, slope)]
        );
    }

    #[test]
    fn modes_cycle_in_order_and_keep_the_flip() {
        let mut r = rule(CornerMode::Ortho90, true);
        let mut seen = Vec::new();
        for _ in 0..4 {
            r = r.with_next_mode();
            assert!(r.is_flipped);
            seen.push(r.mode);
        }
        assert_eq!(
            seen,
            [
                CornerMode::Angle45Start,
                CornerMode::Angle45End,
                CornerMode::FreeAngle,
                CornerMode::Ortho90
            ]
        );
        assert!(!r.flipped().is_flipped);
    }
}
