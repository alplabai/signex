//! Component height and 3D body clearance (#2939): each rule passes one
//! board and breaks another, a side condition narrows it, and a body that
//! starts above the board leaves room for a short part under it.

use signex_types::part::FootprintSourceRef;
use signex_types::pcb::{Footprint, PadType, PcbBoard, Point};
use signex_types::violation::Severity;
use uuid::Uuid;

use super::{BodyModel, BodyModels, HeightSource, describe_on, part_body, violations};
use crate::tests::{board, footprint, pad};
use crate::{DrcRules, Limit, NetScope, UserRule, ViolationKind};

const MM: i64 = 1_000_000;

/// A part `n` with one 1 mm square pad at its origin, at `x` mm, standing
/// `height` mm tall (stored).
fn part(n: u128, x: f64, height: f64) -> Footprint {
    let mut f = footprint(n, vec![pad(n, (0.0, 0.0), 0, PadType::Smd, "TopCopper")]);
    f.position = Point::new(x, 0.0);
    f.component.height_mm = height;
    f
}

fn rule(name: &str, layers: Option<u32>, limit: Limit) -> UserRule {
    UserRule {
        name: name.into(),
        severity: Severity::Error,
        message: None,
        nets: NetScope::Board,
        layers,
        pair: None,
        limits: vec![limit],
    }
}

fn rules(b: &PcbBoard, written: Vec<UserRule>) -> DrcRules {
    DrcRules::for_board(b, &[])
        .unwrap()
        .with_user_rules(b, &[], written)
}

/// The findings of `kind` with their measured values.
fn found(b: &PcbBoard, written: Vec<UserRule>, models: &BodyModels) -> Vec<(ViolationKind, i64)> {
    violations(b, &rules(b, written), models)
        .violations
        .iter()
        .map(|v| (v.kind, v.measured_nm))
        .collect()
}

fn source(n: u128) -> FootprintSourceRef {
    FootprintSourceRef {
        library: Uuid::from_u128(1),
        footprint: Uuid::from_u128(n),
        version: "1".into(),
    }
}

#[test]
fn a_part_taller_than_the_limit_is_found_and_one_under_it_passes() {
    let tall = board(vec![part(1, 0.0, 7.0)], Vec::new(), Vec::new());
    let short = board(vec![part(1, 0.0, 5.0)], Vec::new(), Vec::new());
    let limit = || vec![rule("height", None, Limit::MaxComponentHeight(6 * MM))];
    let none = BodyModels::new();
    assert_eq!(
        found(&tall, limit(), &none),
        [(ViolationKind::ComponentHeight, 7 * MM)]
    );
    assert!(found(&short, limit(), &none).is_empty());
    let v = &violations(&tall, &rules(&tall, limit()), &none).violations[0];
    assert_eq!(
        describe_on(&tall, v).as_deref(),
        Some("U1 is 7 mm tall, needs at most 6 mm")
    );
}

#[test]
fn a_side_condition_checks_that_side_only() {
    let mut bottom = part(2, 10.0, 7.0);
    bottom.layer = "BottomCopper".into();
    let b = board(vec![part(1, 0.0, 7.0), bottom], Vec::new(), Vec::new());
    let bottom_only = vec![rule(
        "bottom_height",
        Some(1 << 31),
        Limit::MaxComponentHeight(6 * MM),
    )];
    let v = violations(&b, &rules(&b, bottom_only), &BodyModels::new()).violations;
    assert_eq!(v.len(), 1);
    assert_eq!(
        v[0].items,
        [pcb_geom::ItemRef::Footprint(b.footprints[1].uuid)]
    );
}

#[test]
fn a_part_with_no_height_is_not_checked_and_no_rule_reads_no_body() {
    let b = board(vec![part(1, 0.0, 0.0)], Vec::new(), Vec::new());
    let limit = vec![rule("height", None, Limit::MaxComponentHeight(0))];
    assert!(found(&b, limit, &BodyModels::new()).is_empty());
    let mut broken = part(2, 0.0, 3.0);
    broken.position = Point::new(f64::NAN, 0.0);
    let b = board(vec![broken], Vec::new(), Vec::new());
    let result = violations(&b, &rules(&b, Vec::new()), &BodyModels::new());
    assert!(result.violations.is_empty() && result.rejected.is_empty());
}

#[test]
fn a_library_model_wins_over_the_stored_height() {
    let mut f = part(1, 0.0, 2.0);
    f.library_ref = Some(source(1));
    let points = vec![[-1.0, -1.0, -0.4], [1.0, 1.0, 8.5]];
    let models = BodyModels::from([(source(1), BodyModel::Points(points))]);
    let body = part_body(&f, &models).unwrap().unwrap();
    assert_eq!(body.source, HeightSource::Model);
    assert_eq!((body.bottom_nm, body.top_nm), (0, 8_500_000));
    let extruded = BodyModels::from([(
        source(1),
        BodyModel::Extrusion {
            outline: None,
            offset_z_mm: 0.5,
            height_mm: 3.0,
        },
    )]);
    let body = part_body(&f, &extruded).unwrap().unwrap();
    assert_eq!(body.source, HeightSource::Body);
    assert_eq!((body.bottom_nm, body.top_nm), (500_000, 3_500_000));
    let stored = part_body(&f, &BodyModels::new()).unwrap().unwrap();
    assert_eq!(
        (stored.source, stored.top_nm),
        (HeightSource::Stored, 2 * MM)
    );
}

/// Two parts whose 1 mm pads are `gap` mm apart, both 1 mm tall.
fn side_by_side(gap: f64) -> PcbBoard {
    board(
        vec![part(1, 0.0, 1.0), part(2, 1.0 + gap, 1.0)],
        Vec::new(),
        Vec::new(),
    )
}

#[test]
fn two_bodies_closer_than_the_limit_are_found() {
    let limit = || vec![rule("bodies", None, Limit::MinBodyClearance(500_000))];
    let none = BodyModels::new();
    assert_eq!(
        found(&side_by_side(0.3), limit(), &none),
        [(ViolationKind::BodyClearance, 300_000)]
    );
    assert!(found(&side_by_side(0.6), limit(), &none).is_empty());
}

#[test]
fn a_short_part_under_a_raised_body_is_measured_in_3d() {
    // Part 1's body starts 2 mm above the board and covers part 2, which
    // is 1 mm tall: they are 1 mm apart, straight up.
    let mut over = part(1, 0.0, 0.0);
    over.library_ref = Some(source(1));
    let models = BodyModels::from([(
        source(1),
        BodyModel::Extrusion {
            outline: Some(vec![[-2.0, -2.0], [2.0, -2.0], [2.0, 2.0], [-2.0, 2.0]]),
            offset_z_mm: 2.0,
            height_mm: 1.0,
        },
    )]);
    let b = board(vec![over, part(2, 0.0, 1.0)], Vec::new(), Vec::new());
    let wide = vec![rule("bodies", None, Limit::MinBodyClearance(1_500_000))];
    assert_eq!(
        found(&b, wide, &models),
        [(ViolationKind::BodyClearance, MM)]
    );
    let narrow = vec![rule("bodies", None, Limit::MinBodyClearance(500_000))];
    assert!(found(&b, narrow, &models).is_empty());
    let touching = vec![rule("bodies", None, Limit::MinBodyClearance(0))];
    assert!(
        found(&b, touching, &models).is_empty(),
        "1 mm apart is no touch"
    );
}

#[test]
fn parts_on_opposite_sides_are_never_a_pair() {
    let mut under = part(2, 0.0, 1.0);
    under.layer = "BottomCopper".into();
    let b = board(vec![part(1, 0.0, 1.0), under], Vec::new(), Vec::new());
    let limit = vec![rule("bodies", None, Limit::MinBodyClearance(500_000))];
    assert!(found(&b, limit, &BodyModels::new()).is_empty());
}
