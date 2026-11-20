//! min, max, abs, sqrt, round, floor, ceil and the constant pi.

use signex_expr::{Dimension, ExprError, Quantity, Unit, parse};

use crate::value;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

#[test]
fn pi_is_a_constant() {
    assert_eq!(value("pi").unwrap(), Quantity::count(std::f64::consts::PI));
}

#[test]
fn min_and_max_need_one_dimension_and_keep_the_winners_unit() {
    let q = value("min(1mm, 100mil, 0.5mm)").unwrap();
    assert_eq!(q, Quantity::length(0.5));
    let q = value("max(1mm, 100mil)").unwrap();
    assert_eq!(q.unit, Unit::Mil);
    assert!(close(q.value, 100.0));
    assert!(matches!(
        value("min(1mm, 1V)"),
        Err(ExprError::UnitMismatch { .. })
    ));
}

#[test]
fn abs_keeps_the_unit() {
    assert_eq!(value("abs(-2mm)").unwrap(), Quantity::length(2.0));
}

#[test]
fn sqrt_halves_even_exponents() {
    assert_eq!(value("sqrt(16)").unwrap(), Quantity::count(4.0));
    assert_eq!(value("sqrt(4mm * 9mm)").unwrap(), Quantity::length(6.0));
    assert!(matches!(value("sqrt(1mm)"), Err(ExprError::Domain(_))));
    assert!(matches!(value("sqrt(0 - 1)"), Err(ExprError::Domain(_))));
}

#[test]
fn round_with_a_step_is_unit_independent() {
    let r = value("round(4.6kΩ, 1kΩ)").unwrap();
    assert_eq!(r.unit, Unit::Derived(Dimension::RESISTANCE));
    assert!(close(r.value, 5000.0));
    let r = value("floor(4.6kΩ, 1kΩ)").unwrap();
    assert!(close(r.value, 4000.0));
    let r = value("ceil(4.1kΩ, 1kΩ)").unwrap();
    assert!(close(r.value, 5000.0));
    // The same length in two units rounds to the same multiple.
    let a = value("round(2.6mm, 1mm)").unwrap();
    let b = value("round(102.36220472mil, 1mm)").unwrap();
    assert!(close(a.as_mm().unwrap(), b.as_mm().unwrap()));
}

#[test]
fn one_argument_rounding_is_for_plain_numbers() {
    assert_eq!(value("round(2.5)").unwrap(), Quantity::count(3.0));
    assert_eq!(value("floor(2.7)").unwrap(), Quantity::count(2.0));
    assert_eq!(value("ceil(2.1)").unwrap(), Quantity::count(3.0));
    assert!(matches!(
        value("round(2.5mm)"),
        Err(ExprError::WrongFamily { .. })
    ));
    assert!(matches!(
        value("round(2.5mm, 1V)"),
        Err(ExprError::UnitMismatch { .. })
    ));
    assert!(matches!(value("round(2.5, 0)"), Err(ExprError::Domain(_))));
}

#[test]
fn wrong_argument_counts_and_unknown_functions_do_not_parse() {
    assert!(parse("min(1)").is_err());
    assert!(parse("abs(1, 2)").is_err());
    assert!(parse("round()").is_err());
    assert!(parse("frobnicate(1)").is_err());
    // Without a call, a function name is an ordinary reference.
    assert!(parse("min").is_ok());
}
