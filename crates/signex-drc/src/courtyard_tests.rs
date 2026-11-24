//! The courtyard gap is measured on the courtyard outline; the line width
//! the board draws it at is display only (maintainer decision 2026-11-24).

use signex_types::pcb::{FpGraphic, PcbBoard, Point};
use signex_types::violation::Severity;

use super::violations;
use crate::tests::{board, footprint};
use crate::{DrcRules, Limit, NetScope, UserRule};

/// The rule's minimum gap: 0.25 mm.
const LIMIT: i64 = 250_000;

/// A 2 mm square courtyard drawn at `width`, its corner at the origin.
fn courtyard(width: f64) -> FpGraphic {
    FpGraphic {
        graphic_type: "poly".into(),
        layer: "TopCourtyard".into(),
        width,
        start: None,
        end: None,
        center: None,
        mid: None,
        radius: 0.0,
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(0.0, 2.0),
        ],
        text: String::new(),
        font_size: 0.0,
        position: None,
        rotation: 0.0,
        fill: String::new(),
    }
}

/// Two parts whose courtyard outlines are `gap_mm` apart, drawn at `width`.
fn two_parts(gap_mm: f64, width: f64) -> PcbBoard {
    let mut left = footprint(1, Vec::new());
    left.graphics.push(courtyard(width));
    let mut right = footprint(2, Vec::new());
    right.position = Point::new(2.0 + gap_mm, 0.0);
    right.graphics.push(courtyard(width));
    board(vec![left, right], Vec::new(), Vec::new())
}

fn rules(b: &PcbBoard) -> DrcRules {
    let gap = UserRule {
        name: "courtyard_gap".into(),
        severity: Severity::Error,
        message: None,
        nets: NetScope::Board,
        layers: None,
        pair: None,
        limits: vec![Limit::MinCourtyardGap(LIMIT)],
    };
    DrcRules::for_board(b, &[])
        .unwrap()
        .with_user_rules(b, &[], [gap])
}

/// The measured gaps of every courtyard finding on `b`.
fn gaps(b: &PcbBoard) -> Vec<i64> {
    violations(b, &rules(b))
        .iter()
        .map(|v| v.measured_nm)
        .collect()
}

#[test]
fn the_courtyard_gap_is_the_same_at_stroke_0_and_0_05() {
    let thin = gaps(&two_parts(0.2, 0.0));
    let stroked = gaps(&two_parts(0.2, 0.05));
    assert_eq!(thin, [200_000], "the outlines are 0.2 mm apart");
    assert_eq!(stroked, thin, "the stroke does not narrow the gap");
}

/// 0.27 mm passes 0.25 mm; half of a 0.05 mm stroke on each side would
/// bring it to 0.22 mm and flag it.
#[test]
fn a_stroked_courtyard_raises_no_new_gap_finding() {
    assert!(gaps(&two_parts(0.27, 0.0)).is_empty());
    assert!(gaps(&two_parts(0.27, 0.05)).is_empty());
}
