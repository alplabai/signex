use super::*;

fn class(name: &str, nets: &[&str], patterns: &[&str]) -> NetClass {
    NetClass {
        name: name.into(),
        nets: nets.iter().map(|s| s.to_string()).collect(),
        patterns: patterns.iter().map(|s| s.to_string()).collect(),
        ..NetClass::default()
    }
}

#[test]
fn glob_star_matches_any_run_including_empty() {
    assert!(glob_matches("VCC*", "VCC"));
    assert!(glob_matches("VCC*", "VCC_3V3"));
    assert!(glob_matches("*_N", "DATA_N"));
    assert!(glob_matches("A*B*C", "AxxBxxC"));
    assert!(glob_matches("*", ""));
    assert!(glob_matches("*", "anything"));
}

#[test]
fn glob_must_match_the_whole_name() {
    assert!(!glob_matches("VCC*", "AVCC"));
    assert!(!glob_matches("VCC", "VCC_3V3"));
    assert!(!glob_matches("*_N", "DATA_NX"));
    assert!(!glob_matches("", "x"));
    assert!(glob_matches("", ""));
}

#[test]
fn glob_question_mark_is_exactly_one_char() {
    assert!(glob_matches("D?", "D1"));
    assert!(!glob_matches("D?", "D"));
    assert!(!glob_matches("D?", "D12"));
}

#[test]
fn glob_is_case_sensitive_and_literal_elsewhere() {
    assert!(!glob_matches("vcc*", "VCC"));
    assert!(glob_matches("a.b", "a.b"));
    assert!(!glob_matches("a.b", "axb"));
}

#[test]
fn glob_backtracking_stays_linear_on_hostile_input() {
    let text = "a".repeat(5000);
    assert!(!glob_matches("*a*a*a*a*a*a*b", &text));
}

#[test]
fn membership_is_exact_or_pattern() {
    let c = class("Power", &["GND"], &["VCC*"]);
    assert!(c.contains_net("GND"));
    assert!(c.contains_net("VCC_3V3"));
    assert!(!c.contains_net("gnd"));
    assert!(!c.contains_net("SDA"));
}

#[test]
fn first_matching_class_in_table_order_wins() {
    let table = vec![
        class("Specific", &["VCC_3V3"], &[]),
        class("Broad", &[], &["VCC*"]),
    ];
    assert_eq!(
        first_matching_class(&table, "VCC_3V3").map(|c| c.name.as_str()),
        Some("Specific")
    );
    assert_eq!(
        first_matching_class(&table, "VCC_5V").map(|c| c.name.as_str()),
        Some("Broad")
    );
    let reversed: Vec<_> = table.into_iter().rev().collect();
    assert_eq!(
        first_matching_class(&reversed, "VCC_3V3").map(|c| c.name.as_str()),
        Some("Broad"),
        "table order, not specificity, decides"
    );
    assert!(first_matching_class(&reversed, "SDA").is_none());
}

#[test]
fn hex_colour_parses_and_formats() {
    assert_eq!(
        parse_hex_color("#FF8000"),
        Some(Color::new(255, 128, 0, 255))
    );
    assert_eq!(
        parse_hex_color("ff800080"),
        Some(Color::new(255, 128, 0, 128))
    );
    assert_eq!(format_hex_color(Color::new(255, 128, 0, 255)), "#FF8000");
    assert_eq!(format_hex_color(Color::new(255, 128, 0, 128)), "#FF800080");
}

#[test]
fn hex_colour_rejects_malformed_input_without_panicking() {
    for bad in [
        "",
        "#",
        "#FFF",
        "#GG0000",
        "#12345",
        "#1234567",
        "\u{e9}\u{20ac}\u{20ac}\u{20ac}",
    ] {
        assert_eq!(parse_hex_color(bad), None, "{bad:?}");
    }
}

#[test]
fn class_round_trips_through_json() {
    let c = NetClass {
        name: "HighCurrent".into(),
        nets: vec!["VBAT".into()],
        patterns: vec!["MOTOR_*".into()],
        style: NetClassStyle {
            color: Some(Color::new(255, 0, 0, 255)),
            wire_width_mm: Some(0.4),
            line: NetLineStyle::Dashed,
        },
        trace_width: 1.5,
        ..NetClass::default()
    };
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains("\"#FF0000\""), "{json}");
    assert_eq!(serde_json::from_str::<NetClass>(&json).unwrap(), c);
}

#[test]
fn empty_optional_fields_are_not_written() {
    let json = serde_json::to_string(&NetClass::named("Bare")).unwrap();
    assert_eq!(json, r#"{"name":"Bare"}"#);
}

#[test]
fn invalid_colour_is_an_error_not_a_silent_drop() {
    let json = r#"{"name":"X","style":{"color":"nope"}}"#;
    assert!(serde_json::from_str::<NetClass>(json).is_err());
}
