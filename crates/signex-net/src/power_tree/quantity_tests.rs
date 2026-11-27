//! #1313: the numbers a power tree reads from text.

use super::*;

#[test]
fn a_rail_parameter_names_its_rail_and_role() {
    assert_eq!(
        rail_parameter("Rail Supply +3V3"),
        Some(("+3V3", RailRole::Supply))
    );
    assert_eq!(
        rail_parameter("Rail Load  VIN_12V "),
        Some(("VIN_12V", RailRole::Load))
    );
    assert_eq!(rail_parameter("Rail Load  "), None);
    assert_eq!(rail_parameter("Rail Loads +3V3"), None);
    assert_eq!(rail_parameter("Efficiency"), None);
}

#[test]
fn a_current_reads_in_a_ma_and_ua_and_refuses_the_rest() {
    assert_eq!(parse_current_a("0.25"), Some(0.25));
    assert_eq!(parse_current_a("3A"), Some(3.0));
    assert_eq!(parse_current_a("250 mA"), Some(0.25));
    assert_eq!(parse_current_a("80uA"), Some(80e-6));
    assert_eq!(parse_current_a("80 µA"), Some(80e-6));
    for text in ["", "lots", "-1", "NaN", "inf", "1 kA"] {
        assert_eq!(parse_current_a(text), None, "{text}");
    }
}

#[test]
fn an_efficiency_is_a_percentage_or_a_fraction_up_to_one() {
    assert_eq!(parse_efficiency("88 %"), Some(0.88));
    assert_eq!(parse_efficiency("100%"), Some(1.0));
    assert_eq!(parse_efficiency("0.9"), Some(0.9));
    for text in ["88", "0", "0 %", "120 %", "-0.5", "high"] {
        assert_eq!(parse_efficiency(text), None, "{text}");
    }
}

#[test]
fn a_net_name_spells_its_voltage_in_its_last_voltage_word() {
    let cases = [
        ("+3V3", Some(3.3)),
        ("3V3", Some(3.3)),
        ("VOUT_3V3", Some(3.3)),
        ("VIN_12V", Some(12.0)),
        ("+5V", Some(5.0)),
        ("5V0", Some(5.0)),
        ("1V8", Some(1.8)),
        ("+3.3V", Some(3.3)),
        ("-12V", Some(-12.0)),
        ("12V_IN", Some(12.0)),
        ("VBUS", None),
        ("VCC", None),
        ("V12", None),
        ("3V3A", None),
        ("0V", None),
    ];
    for (name, volts) in cases {
        assert_eq!(voltage_from_net_name(name), volts, "{name}");
    }
}

#[test]
fn grounds_are_told_by_their_words() {
    for name in [
        "GND", "AGND", "PGND", "GND_ISO", "GNDA", "VSS", "EARTH", "0V",
    ] {
        assert!(is_ground_net(name), "{name}");
    }
    for name in ["+3V3", "VBUS", "VSSA_3V3X", "GROUNDED_SENSE"] {
        assert!(!is_ground_net(name), "{name}");
    }
}
