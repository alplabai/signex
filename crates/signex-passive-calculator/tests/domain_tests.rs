use signex_passive_calculator::{ComponentKind, ESeries, PreferredComponent, SiPrefix, Tolerance};

#[test]
fn every_e_series_has_the_standard_length_and_is_sorted() {
    let expected = [3, 6, 12, 24, 48, 96, 192];
    for (series, expected_length) in ESeries::ALL.into_iter().zip(expected) {
        assert_eq!(series.values().len(), expected_length, "{series}");
        assert!(
            series.values().windows(2).all(|pair| pair[0] < pair[1]),
            "{series}"
        );
    }
}

#[test]
fn e192_contains_the_standard_920_exception() {
    assert!(ESeries::E192.values().contains(&920));
    assert!(!ESeries::E192.values().contains(&919));
}

#[test]
fn preferred_components_keep_decimal_identity_separate_from_binary_value() {
    let component = PreferredComponent {
        number: ESeries::E24
            .preferred_numbers()
            .find(|number| number.significand == 47)
            .unwrap(),
        decade: 3,
    };
    assert_eq!(component.number.significand, 47);
    assert_eq!(component.number.decimal_places, 1);
    assert!((component.value() - 4_700.0).abs() < 1e-12);
    assert_eq!(component.multiplier_exponent(), 2);
}

#[test]
fn resistor_prefixes_are_the_color_multiplier_intersection() {
    assert_eq!(
        SiPrefix::for_kind(ComponentKind::Resistor),
        [
            SiPrefix::Milli,
            SiPrefix::None,
            SiPrefix::Kilo,
            SiPrefix::Mega,
            SiPrefix::Giga
        ]
    );
}

#[test]
fn reactive_modes_include_practical_small_prefixes() {
    for kind in [ComponentKind::Capacitor, ComponentKind::Inductor] {
        let prefixes = SiPrefix::for_kind(kind);
        assert!(prefixes.contains(&SiPrefix::Pico));
        assert!(prefixes.contains(&SiPrefix::Nano));
        assert!(prefixes.contains(&SiPrefix::Micro));
    }
}

#[test]
fn tolerance_fractions_match_their_labels() {
    assert_eq!(Tolerance::ALL.len(), 11);
    assert_eq!(Tolerance::Percent5.fraction(), 0.05);
    assert_eq!(Tolerance::Percent0_01.percent_label(), "±0.01%");
}
