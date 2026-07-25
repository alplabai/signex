use iced::Color;
use signex_widgets::passive_calculator::{
    BandColor, ComponentKind, ESeries, PreferredComponent, RatedPower, ResistorColorCode, RkmCode,
    TemperatureCoefficient, Tolerance,
};

fn e24_component(significand: u16, decade: i8) -> PreferredComponent {
    PreferredComponent {
        number: ESeries::E24
            .preferred_numbers()
            .find(|number| number.significand == significand)
            .unwrap(),
        decade,
    }
}

#[test]
fn screen_colors_match_the_documented_ral_approximations() {
    let expected = [
        (BandColor::Pink, Color::from_rgb8(0xD8, 0xA0, 0xA6)),
        (BandColor::Black, Color::from_rgb8(0x0A, 0x0A, 0x0D)),
        (BandColor::Brown, Color::from_rgb8(0x7A, 0x3B, 0x2E)),
        (BandColor::Red, Color::from_rgb8(0xAF, 0x2B, 0x1E)),
        (BandColor::Orange, Color::from_rgb8(0xF6, 0x78, 0x28)),
        (BandColor::Yellow, Color::from_rgb8(0xF3, 0xDA, 0x0B)),
        (BandColor::Green, Color::from_rgb8(0x57, 0xA6, 0x39)),
        (BandColor::Blue, Color::from_rgb8(0x28, 0x74, 0xB2)),
        (BandColor::Violet, Color::from_rgb8(0x83, 0x63, 0x9D)),
        (BandColor::Grey, Color::from_rgb8(0x7E, 0x8B, 0x92)),
        (BandColor::White, Color::from_rgb8(0xE9, 0xE5, 0xCE)),
    ];
    for (band, expected_color) in expected {
        assert_eq!(band.color(), expected_color, "{}", band.name());
    }
}

#[test]
fn ral_associations_match_the_requested_wikipedia_table() {
    let expected = [
        (BandColor::Pink, Some(3015)),
        (BandColor::Silver, None),
        (BandColor::Gold, None),
        (BandColor::Black, Some(9005)),
        (BandColor::Brown, Some(8003)),
        (BandColor::Red, Some(3000)),
        (BandColor::Orange, Some(2003)),
        (BandColor::Yellow, Some(1021)),
        (BandColor::Green, Some(6018)),
        (BandColor::Blue, Some(5015)),
        (BandColor::Violet, Some(4005)),
        (BandColor::Grey, Some(7000)),
        (BandColor::White, Some(1013)),
    ];
    for (color, ral) in expected {
        assert_eq!(color.ral(), ral, "{}", color.name());
    }
}

#[test]
fn four_band_22_kilohm_code_is_red_red_orange_gold() {
    let code = ResistorColorCode::for_component(e24_component(22, 4), Tolerance::Percent5).unwrap();
    assert_eq!(
        code.bands,
        [
            BandColor::Red,
            BandColor::Red,
            BandColor::Orange,
            BandColor::Gold
        ]
    );
    assert_eq!(
        code.accessible_label(),
        "Red – Red – Orange – Gold (IEC 60062, Ω)"
    );
}

#[test]
fn five_band_precision_code_uses_three_significant_digits() {
    let component = PreferredComponent {
        number: ESeries::E96
            .preferred_numbers()
            .find(|number| number.significand == 226)
            .unwrap(),
        decade: 3,
    };
    let code = ResistorColorCode::for_component(component, Tolerance::Percent1).unwrap();
    assert_eq!(
        code.bands,
        [
            BandColor::Red,
            BandColor::Red,
            BandColor::Blue,
            BandColor::Brown,
            BandColor::Brown,
        ]
    );
}

#[test]
fn trailing_zero_can_be_encoded_as_a_digit_or_as_part_of_the_multiplier() {
    let representations = ResistorColorCode::representations_for_kind(
        ComponentKind::Resistor,
        e24_component(10, 4),
        Tolerance::Percent5,
    );
    assert_eq!(representations.len(), 2);
    assert_eq!(
        representations[0].bands,
        [
            BandColor::Brown,
            BandColor::Black,
            BandColor::Orange,
            BandColor::Gold,
        ]
    );
    assert_eq!(
        representations[1].bands,
        [
            BandColor::Brown,
            BandColor::Black,
            BandColor::Black,
            BandColor::Red,
            BandColor::Gold,
        ]
    );
}

#[test]
fn precision_value_with_a_trailing_zero_has_two_and_three_digit_representations() {
    let component = PreferredComponent {
        number: ESeries::E96
            .preferred_numbers()
            .find(|number| number.significand == 200)
            .unwrap(),
        decade: 3,
    };
    let representations = ResistorColorCode::representations_for_kind(
        ComponentKind::Resistor,
        component,
        Tolerance::Percent1,
    );
    assert_eq!(representations.len(), 2);
    assert_eq!(
        representations
            .iter()
            .map(|representation| representation.bands.clone())
            .collect::<Vec<_>>(),
        [
            vec![
                BandColor::Red,
                BandColor::Black,
                BandColor::Black,
                BandColor::Brown,
                BandColor::Brown,
            ],
            vec![
                BandColor::Red,
                BandColor::Black,
                BandColor::Red,
                BandColor::Brown,
            ],
        ]
    );
}

#[test]
fn alternative_representations_are_available_for_every_component_kind() {
    for (kind, decade) in [
        (ComponentKind::Resistor, 2),
        (ComponentKind::Capacitor, -10),
        (ComponentKind::Inductor, -4),
    ] {
        let representations = ResistorColorCode::representations_for_kind(
            kind,
            e24_component(22, decade),
            Tolerance::Percent5,
        );
        assert_eq!(representations.len(), 2, "{kind}");
        assert_ne!(representations[0].bands, representations[1].bands, "{kind}");
    }
}

#[test]
fn resistor_code_appends_tolerance_and_temperature_coefficient_bands() {
    let representations = ResistorColorCode::representations_for_kind_with_temperature_coefficient(
        ComponentKind::Resistor,
        e24_component(22, 2),
        Tolerance::Percent1,
        Some(TemperatureCoefficient::Ppm5),
    );
    assert_eq!(
        representations
            .iter()
            .map(|representation| representation.bands.clone())
            .collect::<Vec<_>>(),
        [
            vec![
                BandColor::Red,
                BandColor::Red,
                BandColor::Brown,
                BandColor::Brown,
                BandColor::Violet,
            ],
            vec![
                BandColor::Red,
                BandColor::Red,
                BandColor::Black,
                BandColor::Black,
                BandColor::Brown,
                BandColor::Violet,
            ],
        ]
    );
}

#[test]
fn rkm_code_formats_the_complete_resistor_example() {
    let code = RkmCode::for_component(
        ComponentKind::Resistor,
        e24_component(47, 3),
        Tolerance::Percent1,
    )
    .with_temperature_coefficient(TemperatureCoefficient::Ppm5)
    .with_rated_power(RatedPower::Watts0_63);
    assert_eq!(code.to_string(), "4K7 FM 0W63");
}

#[test]
fn rkm_value_letters_follow_the_component_convention() {
    for (kind, component, expected) in [
        (ComponentKind::Resistor, e24_component(47, -1), "R47 J"),
        (ComponentKind::Resistor, e24_component(47, 2), "470R J"),
        (ComponentKind::Capacitor, e24_component(47, -6), "4u7 J"),
        (ComponentKind::Inductor, e24_component(47, -6), "4R7 J"),
    ] {
        assert_eq!(
            RkmCode::for_component(kind, component, Tolerance::Percent5).to_string(),
            expected
        );
    }
}

#[test]
fn every_supported_tolerance_has_an_rkm_letter() {
    for (tolerance, expected) in [
        (Tolerance::Percent20, 'M'),
        (Tolerance::Percent10, 'K'),
        (Tolerance::Percent5, 'J'),
        (Tolerance::Percent2, 'G'),
        (Tolerance::Percent1, 'F'),
        (Tolerance::Percent0_5, 'D'),
        (Tolerance::Percent0_25, 'C'),
        (Tolerance::Percent0_1, 'B'),
        (Tolerance::Percent0_05, 'W'),
        (Tolerance::Percent0_02, 'P'),
        (Tolerance::Percent0_01, 'L'),
    ] {
        let code = RkmCode::for_component(ComponentKind::Resistor, e24_component(47, 3), tolerance);
        assert!(code.to_string().ends_with(expected));
    }
}

#[test]
fn current_precision_tolerance_colors_are_supported() {
    assert_eq!(
        BandColor::for_tolerance(Tolerance::Percent0_05),
        Some(BandColor::Orange)
    );
    assert_eq!(
        BandColor::for_tolerance(Tolerance::Percent0_02),
        Some(BandColor::Yellow)
    );
    assert_eq!(
        BandColor::for_tolerance(Tolerance::Percent0_01),
        Some(BandColor::Grey)
    );
}

#[test]
fn multiplier_outside_iec_band_range_has_no_color_code() {
    assert!(ResistorColorCode::for_component(e24_component(10, -4), Tolerance::Percent5).is_none());
}

#[test]
fn capacitor_code_encodes_the_value_in_picofarads() {
    let code = ResistorColorCode::for_kind(
        ComponentKind::Capacitor,
        e24_component(47, -8),
        Tolerance::Percent5,
    )
    .unwrap();
    assert_eq!(
        code.bands,
        [
            BandColor::Yellow,
            BandColor::Violet,
            BandColor::Orange,
            BandColor::Gold,
        ]
    );
    assert!(code.accessible_label().contains("capacitor code, pF"));
}

#[test]
fn inductor_code_uses_the_common_microhenry_convention() {
    let code = ResistorColorCode::for_kind(
        ComponentKind::Inductor,
        e24_component(33, -2),
        Tolerance::Percent5,
    )
    .unwrap();
    assert_eq!(
        code.bands,
        [
            BandColor::Orange,
            BandColor::Orange,
            BandColor::Orange,
            BandColor::Gold,
        ]
    );
    assert!(
        code.accessible_label()
            .contains("manufacturer convention, µH")
    );
}

#[test]
fn unsupported_capacitor_tolerance_is_called_out_without_a_false_band() {
    let code = ResistorColorCode::for_kind(
        ComponentKind::Capacitor,
        e24_component(47, -8),
        Tolerance::Percent0_25,
    )
    .unwrap();
    assert_eq!(code.bands.len(), 3);
    assert!(
        code.accessible_label()
            .contains("selected tolerance has no capacitor color band")
    );
}
