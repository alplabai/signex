use super::{
    ChannelFormatError, ChannelOccurrence, ChannelRepeat, MAX_CHANNELS, RepeatStack, channel_alpha,
    check_format, entry_port_name, renamed_entry, repeated_entry_name, sheet_entry_port,
    sheet_entry_renamed,
};
use crate::project::ChannelNaming;

fn naming(designator_format: &str, net_format: &str) -> ChannelNaming {
    ChannelNaming {
        designator_format: designator_format.to_string(),
        net_format: net_format.to_string(),
    }
}

fn channel(index: u32) -> ChannelOccurrence {
    ChannelOccurrence {
        name: format!("CH{index}"),
        index,
        number: index,
        of: 4,
    }
}

#[test]
fn parses_the_canonical_form() {
    let r = ChannelRepeat::parse("Repeat(CH, 1, 4)").unwrap();
    assert_eq!(r.channel, "CH");
    assert_eq!((r.from, r.to), (1, 4));
    assert_eq!(r.count(), 4);
    assert_eq!(r.to_designator(), "Repeat(CH, 1, 4)");
}

#[test]
fn parse_tolerates_whitespace_and_formats_canonically() {
    let r = ChannelRepeat::parse("  Repeat ( CH ,1,  6 ) ").unwrap();
    assert_eq!(r.to_designator(), "Repeat(CH, 1, 6)");
    assert_eq!(ChannelRepeat::parse(&r.to_designator()), Some(r));
}

#[test]
fn parse_refuses_what_is_not_a_repeat() {
    for text in [
        "",
        "buck_channel",
        "repeat(CH, 1, 4)",
        "REPEAT(CH, 1, 4)",
        "Repeat(CH, 1)",
        "Repeat(CH, 1, 4, 5)",
        "Repeat(CH, 4, 1)",
        "Repeat(CH, -1, 4)",
        "Repeat(CH, a, 4)",
        "Repeat(1CH, 1, 4)",
        "Repeat(C H, 1, 4)",
        "Repeat(, 1, 4)",
        "Repeat(CH, 1, 4) x",
        "Repeat CH, 1, 4",
    ] {
        assert_eq!(ChannelRepeat::parse(text), None, "{text:?}");
    }
}

#[test]
fn a_single_channel_and_the_channel_cap_parse() {
    assert_eq!(ChannelRepeat::parse("Repeat(A, 3, 3)").unwrap().count(), 1);
    let last = MAX_CHANNELS;
    let at_cap = format!("Repeat(CH, 1, {last})");
    assert_eq!(ChannelRepeat::parse(&at_cap).unwrap().count(), MAX_CHANNELS);
    let past_cap = format!("Repeat(CH, 1, {})", last + 1);
    assert_eq!(ChannelRepeat::parse(&past_cap), None);
    assert_eq!(ChannelRepeat::parse("Repeat(CH, 0, 4294967295)"), None);
}

#[test]
fn occurrences_number_and_index_every_channel() {
    let r = ChannelRepeat::parse("Repeat(CH, 3, 5)").unwrap();
    let occ = r.occurrences();
    let names: Vec<&str> = occ.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["CH3", "CH4", "CH5"]);
    assert_eq!(
        occ[1],
        ChannelOccurrence {
            name: "CH4".into(),
            index: 2,
            number: 4,
            of: 3,
        }
    );
}

#[test]
fn repeated_entry_names() {
    assert_eq!(repeated_entry_name("Repeat(VOUT_3V3)"), Some("VOUT_3V3"));
    assert_eq!(
        repeated_entry_name(" Repeat( VOUT_3V3 ) "),
        Some("VOUT_3V3")
    );
    assert_eq!(repeated_entry_name("VOUT_3V3"), None);
    assert_eq!(repeated_entry_name("repeat(VOUT_3V3)"), None);
    assert_eq!(repeated_entry_name("Repeat()"), None);
    assert_eq!(repeated_entry_name("Repeat(A B)"), None);
    assert_eq!(repeated_entry_name("Repeat(CH, 1, 4)"), None);
}

/// #2659: a `Repeat(<net>)` entry meets the port `<net>`.
#[test]
fn an_entry_meets_its_port() {
    assert_eq!(entry_port_name(" Repeat( VOUT_3V3 ) "), Some("VOUT_3V3"));
    assert_eq!(entry_port_name(" EN "), Some("EN"));
    assert_eq!(entry_port_name("Repeat(A B)"), Some("Repeat(A B)"));
    assert_eq!(entry_port_name("  "), None);
}

/// #2659: a renamed port keeps a repeat entry a repeat.
#[test]
fn a_renamed_repeat_entry_stays_a_repeat() {
    assert_eq!(
        renamed_entry("Repeat(VOUT)", "VOUT_3V3"),
        "Repeat(VOUT_3V3)"
    );
    assert_eq!(renamed_entry("VIN", "VIN_12V"), "VIN_12V");
}

/// #2659: only a repeated sheet symbol's `Repeat(<net>)` entry meets `<net>`;
/// on a plain symbol the written name is the port, and a rename takes the
/// port's plain name.
#[test]
fn a_sheet_entry_meets_its_port_by_the_symbols_repeat() {
    let repeated = "Repeat(CH, 1, 4)";
    assert_eq!(sheet_entry_port(repeated, "Repeat(VOUT)"), Some("VOUT"));
    assert_eq!(
        sheet_entry_port("Buck", " Repeat(VOUT) "),
        Some("Repeat(VOUT)")
    );
    assert_eq!(sheet_entry_port("Buck", " EN "), Some("EN"));
    assert_eq!(sheet_entry_port("Buck", "  "), None);
    assert_eq!(
        sheet_entry_renamed(repeated, "Repeat(VOUT)", "VOUT_5V"),
        "Repeat(VOUT_5V)"
    );
    assert_eq!(sheet_entry_renamed("Buck", "Repeat(VOUT)", "VOUT"), "VOUT");
}

#[test]
fn default_naming_matches_the_mockup() {
    let n = ChannelNaming::default();
    assert_eq!(n.designator_format, "${Channel}_${Designator}");
    assert_eq!(n.net_format, "${Net}_${Channel}");
    assert_eq!(n.designator("R5", &channel(1)), "CH1_R5");
    assert_eq!(n.net("VOUT_3V3", &channel(1)), "VOUT_3V3_CH1");
}

#[test]
fn every_designator_token_expands() {
    let ch = channel(1);
    let cases = [
        ("${Designator}_${Channel}", "R5_CH1"),
        ("${Channel}_${Designator}", "CH1_R5"),
        ("${Designator}${ChannelAlpha}", "R5A"),
        ("${Designator}_${Index}", "R5_1"),
    ];
    for (format, want) in cases {
        assert_eq!(naming(format, "").designator("R5", &ch), want, "{format}");
    }
}

#[test]
fn index_and_alpha_follow_the_position_not_the_number() {
    let ch = ChannelOccurrence {
        name: "CH7".into(),
        index: 2,
        number: 7,
        of: 3,
    };
    let n = naming("${Designator}${ChannelAlpha}_${Index}", "${Net}_${Channel}");
    assert_eq!(n.designator("U1", &ch), "U1B_2");
    assert_eq!(n.net("SW", &ch), "SW_CH7");
}

#[test]
fn unknown_and_unclosed_tokens_stay_literal() {
    let n = naming("${Board}_${Designator}", "${Net}_${Designator}_${Channel");
    assert_eq!(n.designator("R5", &channel(2)), "${Board}_R5");
    assert_eq!(n.net("FB", &channel(2)), "FB_${Designator}_${Channel");
}

#[test]
fn channel_alpha_runs_past_z() {
    assert_eq!(channel_alpha(1), "A");
    assert_eq!(channel_alpha(26), "Z");
    assert_eq!(channel_alpha(27), "AA");
    assert_eq!(channel_alpha(28), "AB");
    assert_eq!(channel_alpha(52), "AZ");
    assert_eq!(channel_alpha(53), "BA");
    assert_eq!(channel_alpha(0), "");
}

#[test]
fn custom_formats_are_checked() {
    assert_eq!(check_format("${Channel}_${Designator}"), Ok(()));
    assert_eq!(check_format("${Net}-${ChannelAlpha}"), Ok(()));
    assert_eq!(check_format(""), Err(ChannelFormatError::Empty));
    assert_eq!(
        check_format("${Channel} ${Designator}"),
        Err(ChannelFormatError::Whitespace)
    );
    assert_eq!(
        check_format("${Channel}_${Designator"),
        Err(ChannelFormatError::Unclosed)
    );
    assert_eq!(
        check_format("${Chanel}_${Designator}"),
        Err(ChannelFormatError::UnknownToken("Chanel".into()))
    );
    assert_eq!(
        check_format("X_${Designator}"),
        Err(ChannelFormatError::NoChannelToken)
    );
    assert_eq!(
        check_format("${Index}"),
        Err(ChannelFormatError::NoBaseToken)
    );
}

#[test]
fn a_repeated_symbol_stacks_behind_itself_with_a_count_pill() {
    let stack = RepeatStack::new((10.0, 20.0), (40.0, 30.0), 4);
    assert_eq!(stack.layers[0].rect, [12.54, 22.54, 52.54, 52.54]);
    assert_eq!(stack.layers[1].rect, [11.27, 21.27, 51.27, 51.27]);
    assert!(
        stack.layers[0].alpha < stack.layers[1].alpha,
        "back is fainter"
    );
    let [x0, y0, x1, y1] = stack.badge;
    assert!(
        x0 > 10.0 && x1 < 50.0 && y0 > 20.0 && y1 < 50.0,
        "inside the symbol"
    );
    assert!(x1 > 45.0 && y0 < 22.0, "at its top-right corner");
    assert_eq!(stack.badge_text, "×4");
}

#[test]
fn only_a_repeat_designator_has_a_stack() {
    let mut sheet = crate::schematic::ChildSheet {
        uuid: uuid::Uuid::nil(),
        name: "buck".into(),
        filename: "buck.snxsch".into(),
        position: crate::schematic::Point::new(0.0, 0.0),
        size: (20.0, 20.0),
        stroke_width: 0.0,
        fill: Default::default(),
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    };
    assert_eq!(RepeatStack::of(&sheet), None);
    sheet.name = "Repeat(CH, 2, 3)".into();
    assert_eq!(
        RepeatStack::of(&sheet).map(|s| s.badge_text),
        Some("×2".into())
    );
}
