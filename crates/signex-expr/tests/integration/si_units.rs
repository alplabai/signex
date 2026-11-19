//! Electrical units with SI prefixes, RKM notation and the bare `m` rule.

use signex_expr::{
    BareM, Dimension, ExprError, LiteralContext, Quantity, Unit, bare_m_meaning, parse_quantity,
    parse_quantity_with,
};

use crate::value;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1e-30)
}

#[test]
fn prefixes_scale_a_plain_number() {
    let q = parse_quantity("4.7k").unwrap();
    assert_eq!(q.unit, Unit::Dimensionless);
    assert!(close(q.value, 4700.0));
    assert!(close(parse_quantity("100n").unwrap().value, 100e-9));
    assert!(close(parse_quantity("2.2M").unwrap().value, 2.2e6));
    assert!(close(parse_quantity("1G").unwrap().value, 1e9));
    assert!(close(parse_quantity("3p").unwrap().value, 3e-12));
}

#[test]
fn named_units_take_prefixes() {
    let c = parse_quantity("100nF").unwrap();
    assert_eq!(c.unit, Unit::Derived(Dimension::CAPACITANCE));
    assert!(close(c.value, 100e-9));

    let r = parse_quantity("4.7kΩ").unwrap();
    assert_eq!(r.unit, Unit::Derived(Dimension::RESISTANCE));
    assert!(close(r.value, 4700.0));
    assert_eq!(parse_quantity("4.7kohm").unwrap(), r);
    assert_eq!(parse_quantity("4.7k\u{2126}").unwrap(), r);

    for (src, dim, base) in [
        ("10mV", Dimension::VOLTAGE, 10e-3),
        ("2.5A", Dimension::CURRENT, 2.5),
        ("1us", Dimension::TIME, 1e-6),
        ("1µs", Dimension::TIME, 1e-6),
        ("16MHz", Dimension::FREQUENCY, 16e6),
        ("10uH", Dimension::INDUCTANCE, 10e-6),
        ("33mW", Dimension::POWER, 33e-3),
        ("4.7mF", Dimension::CAPACITANCE, 4.7e-3),
    ] {
        let q = parse_quantity(src).unwrap();
        assert_eq!(q.unit, Unit::Derived(dim), "{src}");
        assert!(close(q.value, base), "{src}: {}", q.value);
    }
}

#[test]
fn rkm_is_an_alias_for_the_decimal_form() {
    assert_eq!(
        parse_quantity("4k7").unwrap(),
        parse_quantity("4.7k").unwrap()
    );
    assert_eq!(
        parse_quantity("2n2F").unwrap(),
        parse_quantity("2.2nF").unwrap()
    );
    assert_eq!(value("4k7").unwrap(), value("4.7k").unwrap());
    let doubled = value("4k7Ω * 2").unwrap();
    assert_eq!(doubled.unit, Unit::Derived(Dimension::RESISTANCE));
    assert!(close(doubled.value, 9400.0));
}

#[test]
fn bare_m_rule() {
    assert_eq!(bare_m_meaning(LiteralContext::Length), BareM::Metre);
    assert_eq!(bare_m_meaning(LiteralContext::General), BareM::Milli);

    let general = parse_quantity("10m").unwrap();
    assert_eq!(general.unit, Unit::Dimensionless);
    assert!(close(general.value, 0.01));

    let length = parse_quantity_with("10m", LiteralContext::Length).unwrap();
    assert_eq!(length, Quantity::length(10_000.0));

    // m before another unit symbol is always milli; mm stays the millimetre.
    let mv = parse_quantity_with("10mV", LiteralContext::Length).unwrap();
    assert_eq!(mv.unit, Unit::Derived(Dimension::VOLTAGE));
    assert!(close(mv.value, 0.01));
    assert_eq!(
        parse_quantity_with("10mm", LiteralContext::Length).unwrap(),
        Quantity::length(10.0)
    );
}

#[test]
fn bare_m_in_a_parsed_expression_follows_the_context() {
    let length = signex_expr::parse_with("1m + 1mm", LiteralContext::Length).unwrap();
    let q = signex_expr::eval(&length, &signex_expr::NoBindings).unwrap();
    assert_eq!(q, Quantity::length(1001.0));
    assert!(close(value("1m * 1000").unwrap().value, 1.0));
}

#[test]
fn unknown_units_and_mismatches_are_errors() {
    assert!(parse_quantity("1xF").is_err());
    assert!(parse_quantity("1nm").is_err(), "no nanometre in the table");
    assert!(matches!(
        value("1V + 1A"),
        Err(ExprError::UnitMismatch { .. })
    ));
    assert!(matches!(
        value("1kΩ + 1mm"),
        Err(ExprError::UnitMismatch { .. })
    ));
    assert!(matches!(
        value("1V < 2"),
        Err(ExprError::UnitMismatch { .. })
    ));
}

#[test]
fn values_show_in_their_named_unit() {
    assert_eq!(value("3.3V / 10mA").unwrap().to_string(), "330Ω");
    assert_eq!(value("3.3V * 10mA").unwrap().to_string(), "33mW");
    assert_eq!(value("100nF").unwrap().to_string(), "100nF");
    assert_eq!(value("2.54mm").unwrap().to_string(), "2.54mm");
    assert_eq!(value("4.7k").unwrap().to_string(), "4700");
}

#[test]
fn geometry_context_refuses_prefix_only_literals() {
    assert_eq!(bare_m_meaning(LiteralContext::Geometry), BareM::Refused);
    for literal in ["10m", "1u", "4k7", "2n", "2n2", "5k"] {
        assert!(
            parse_quantity_with(literal, LiteralContext::Geometry).is_err(),
            "{literal} must be refused in a sketch parameter"
        );
    }
    assert_eq!(
        parse_quantity_with("10mm", LiteralContext::Geometry).unwrap(),
        Quantity::length(10.0)
    );
    assert_eq!(
        parse_quantity_with("16", LiteralContext::Geometry).unwrap(),
        Quantity::count(16.0)
    );
    assert!(signex_expr::parse_with("1u + 1mm", LiteralContext::Geometry).is_err());
}
