use iced::Color;
use signex_widgets::passive_calculator::{
    BandColor, ComponentKind, ESeries, PreferredComponent, ResistorColorCode, Tolerance,
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
