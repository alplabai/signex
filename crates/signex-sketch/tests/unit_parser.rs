//! Integration tests for the strict-unit parser
//! (`crates/signex-sketch/src/unit.rs`).
//!
//! Covers Task 4.1 of `docs/internal/SKETCH_MODE_v0.13_PLAN.md`.

use signex_sketch::unit::{Quantity, Unit, UnitError, UnitFamily, parse_quantity};

const EPS: f64 = 1e-10;

#[test]
fn parse_mm() {
    let q = parse_quantity("0.5mm").unwrap();
    assert_eq!(q, Quantity::length(0.5));
    assert_eq!(q.unit, Unit::Mm);
}

#[test]
fn parse_mil_to_mm() {
    let q = parse_quantity("100mil").unwrap();
    assert_eq!(q.unit, Unit::Mil);
    assert!((q.as_mm().unwrap() - 2.54).abs() < EPS);
}

#[test]
fn parse_in_to_mm() {
    let q = parse_quantity("1in").unwrap();
    assert_eq!(q.unit, Unit::In);
    assert!((q.as_mm().unwrap() - 25.4).abs() < EPS);
}

#[test]
fn parse_um_to_mm() {
    let q = parse_quantity("1000um").unwrap();
    assert_eq!(q.unit, Unit::Um);
    assert!((q.as_mm().unwrap() - 1.0).abs() < EPS);
}

#[test]
fn parse_deg_to_rad() {
    let q = parse_quantity("90deg").unwrap();
    assert_eq!(q.unit, Unit::Deg);
    assert!((q.as_rad().unwrap() - std::f64::consts::FRAC_PI_2).abs() < EPS);
}

#[test]
fn parse_rad_identity() {
    let q = parse_quantity("1rad").unwrap();
    assert_eq!(q.unit, Unit::Rad);
    assert!((q.as_rad().unwrap() - 1.0).abs() < EPS);
}

#[test]
fn parse_dimensionless() {
    let q = parse_quantity("16").unwrap();
    assert_eq!(q, Quantity::count(16.0));
    assert_eq!(q.unit, Unit::Dimensionless);
}

#[test]
fn parse_mismatch_unit_fails() {
    let err = parse_quantity("0.5xyz").unwrap_err();
    assert!(matches!(err, UnitError::Parse(_)));
}

#[test]
fn as_mm_rejects_angle() {
    let q = parse_quantity("90deg").unwrap();
    let err = q.as_mm().unwrap_err();
    match err {
        UnitError::WrongFamily { expected, got } => {
            assert_eq!(expected, UnitFamily::Length);
            assert_eq!(got, UnitFamily::Angle);
        }
        other => panic!("expected WrongFamily, got {other:?}"),
    }
}

#[test]
fn as_rad_rejects_length() {
    let q = parse_quantity("1mm").unwrap();
    let err = q.as_rad().unwrap_err();
    match err {
        UnitError::WrongFamily { expected, got } => {
            assert_eq!(expected, UnitFamily::Angle);
            assert_eq!(got, UnitFamily::Length);
        }
        other => panic!("expected WrongFamily, got {other:?}"),
    }
}

#[test]
fn longest_suffix_match() {
    // "100mil" must match "mil", NOT split as "100m" + "il"
    // (and not "10" + "0mi" + "l", etc.).
    let q = parse_quantity("100mil").unwrap();
    assert_eq!(q.unit, Unit::Mil);
    assert_eq!(q.value, 100.0);
}

#[test]
fn whitespace_tolerated() {
    // Leading/trailing whitespace and whitespace between the number
    // and the suffix must both parse.
    let a = parse_quantity(" 0.5 mm ").unwrap();
    assert_eq!(a, Quantity::length(0.5));

    let b = parse_quantity("100 mil").unwrap();
    assert_eq!(b.unit, Unit::Mil);
    assert!((b.as_mm().unwrap() - 2.54).abs() < EPS);

    let c = parse_quantity(" 16 ").unwrap();
    assert_eq!(c, Quantity::count(16.0));
}

#[test]
fn quantity_serde_round_trip() {
    let q = Quantity::length(2.54);
    let s = serde_json::to_string(&q).unwrap();
    let r: Quantity = serde_json::from_str(&s).unwrap();
    assert_eq!(q, r);

    let q2 = parse_quantity("90deg").unwrap();
    let s2 = serde_json::to_string(&q2).unwrap();
    let r2: Quantity = serde_json::from_str(&s2).unwrap();
    assert_eq!(q2, r2);
}

#[test]
fn family_partition() {
    assert_eq!(Unit::Mm.family(), UnitFamily::Length);
    assert_eq!(Unit::Mil.family(), UnitFamily::Length);
    assert_eq!(Unit::In.family(), UnitFamily::Length);
    assert_eq!(Unit::Um.family(), UnitFamily::Length);
    assert_eq!(Unit::Deg.family(), UnitFamily::Angle);
    assert_eq!(Unit::Rad.family(), UnitFamily::Angle);
    assert_eq!(Unit::Dimensionless.family(), UnitFamily::Count);
}

#[test]
fn as_count_only_dimensionless() {
    let q = parse_quantity("16").unwrap();
    assert_eq!(q.as_count().unwrap(), 16.0);

    let q2 = parse_quantity("0.5mm").unwrap();
    let err = q2.as_count().unwrap_err();
    match err {
        UnitError::WrongFamily { expected, got } => {
            assert_eq!(expected, UnitFamily::Count);
            assert_eq!(got, UnitFamily::Length);
        }
        other => panic!("expected WrongFamily, got {other:?}"),
    }
}

#[test]
fn negative_and_decimal() {
    let q = parse_quantity("-1.5mm").unwrap();
    assert_eq!(q.unit, Unit::Mm);
    assert_eq!(q.value, -1.5);

    let q2 = parse_quantity("2.5rad").unwrap();
    assert!((q2.as_rad().unwrap() - 2.5).abs() < EPS);
}

#[test]
fn empty_or_just_suffix_fails() {
    assert!(parse_quantity("").is_err());
    assert!(parse_quantity("  ").is_err());
    // A bare "mm" with no number is malformed.
    assert!(parse_quantity("mm").is_err());
}
