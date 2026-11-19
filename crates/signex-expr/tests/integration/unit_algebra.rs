//! `*`, `/` and `^` derive the result's unit from dimension vectors.

use signex_expr::{Dimension, ExprError, Quantity, Unit};

use crate::value;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}

#[test]
fn ohms_law_gives_ohms_and_watts() {
    let r = value("3.3V / 10mA").unwrap();
    assert_eq!(r.unit, Unit::Derived(Dimension::RESISTANCE));
    assert!(close(r.value, 330.0));

    let p = value("3.3V * 10mA").unwrap();
    assert_eq!(p.unit, Unit::Derived(Dimension::POWER));
    assert!(close(p.value, 0.033));
}

#[test]
fn rc_corner_frequency_is_in_hertz() {
    let f = value("1 / (2 * pi * 1kΩ * 100nF)").unwrap();
    assert_eq!(f.unit, Unit::Derived(Dimension::FREQUENCY));
    assert!(close(
        f.value,
        1.0 / (2.0 * std::f64::consts::PI * 1e3 * 100e-9)
    ));
}

#[test]
fn a_dimension_without_a_name_is_a_compound() {
    let area = value("2mm * 3mm").unwrap();
    assert_eq!(area.unit, Unit::Derived(Dimension::new(2, 0, 0, 0, 0)));
    assert!(close(area.value, 6.0));
    assert_eq!(area.to_string(), "6mm²");

    let slew = value("5V / 1us").unwrap();
    assert_eq!(slew.unit, Unit::Derived(Dimension::new(0, 0, -1, 1, 0)));
    assert_eq!(slew.to_string(), "5000000V/s");
}

#[test]
fn equal_dimensions_cancel_to_a_plain_number() {
    assert_eq!(value("6mm / 2mm").unwrap(), Quantity::count(3.0));
    let ratio = value("5V / 2V").unwrap();
    assert_eq!(ratio.unit, Unit::Dimensionless);
    assert!(close(ratio.value, 2.5));
    // An area over a length is a length again, in mm.
    assert_eq!(value("6mm * 2mm / 3mm").unwrap(), Quantity::length(4.0));
}

#[test]
fn scaling_keeps_the_written_length_unit() {
    let q = value("2 * 100mil").unwrap();
    assert_eq!(q.unit, Unit::Mil);
    assert!(close(q.value, 200.0));
}

#[test]
fn pow_needs_a_whole_exponent_on_a_unit() {
    let sq = value("3mm ^ 2").unwrap();
    assert_eq!(sq.unit, Unit::Derived(Dimension::new(2, 0, 0, 0, 0)));
    assert!(close(sq.value, 9.0));
    assert_eq!(value("3mm ^ 1").unwrap(), Quantity::length(3.0));
    assert!(matches!(value("3mm ^ 0.5"), Err(ExprError::Domain(_))));
    assert!(matches!(
        value("2 ^ 1mm"),
        Err(ExprError::UnitMismatch { .. })
    ));
    // A plain base still takes any exponent.
    assert!(close(value("4 ^ 0.5").unwrap().value, 2.0));
}

#[test]
fn exponents_are_capped() {
    assert!(matches!(value("1mm ^ 9"), Err(ExprError::Domain(_))));
    let src = ["1mm"; 9].join(" * ");
    assert!(matches!(value(&src), Err(ExprError::Domain(_))));
    let eight = ["1mm"; 8].join(" * ");
    assert!(value(&eight).is_ok());
}

#[test]
fn derived_values_add_and_compare_by_dimension() {
    let sum = value("1kΩ + 500Ω").unwrap();
    assert_eq!(sum.unit, Unit::Derived(Dimension::RESISTANCE));
    assert!(close(sum.value, 1500.0));
    assert_eq!(value("1kΩ > 500Ω").unwrap(), Quantity::count(1.0));
    assert_eq!(value("1mA == 1000uA").unwrap(), Quantity::count(1.0));
}
