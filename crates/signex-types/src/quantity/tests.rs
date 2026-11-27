use super::*;

fn q(base: f64, unit: Unit) -> Quantity {
    Quantity::new(base, unit).unwrap()
}

fn parts(m: &str, p: SiPrefix, u: Unit) -> Quantity {
    quantity_from_parts(m, p, u).unwrap()
}

fn render(base: f64, unit: Unit, style: ValueStyle) -> String {
    format_quantity(&q(base, unit), style, false)
}

#[test]
fn mantissa_accepts_plain_numbers() {
    let cases = [
        ("4.7", 4.7),
        ("  100 ", 100.0),
        ("-40", -40.0),
        ("+3.3", 3.3),
        ("1e3", 1000.0),
        (".5", 0.5),
        ("0", 0.0),
    ];
    for (input, want) in cases {
        assert_eq!(parse_mantissa(input), Ok(want), "{input:?}");
    }
}

#[test]
fn mantissa_rejects_everything_else() {
    use MantissaError::*;
    assert_eq!(parse_mantissa(""), Err(Empty));
    assert_eq!(parse_mantissa("   "), Err(Empty));
    for s in ["10k", "4k7", "4.7uF", "10 kΩ", "2R2", "100n", "abc", "k"] {
        assert!(matches!(parse_mantissa(s), Err(HasLetters(_))), "{s:?}");
    }
    assert!(matches!(parse_mantissa("4,7"), Err(Comma(_))));
    assert!(matches!(parse_mantissa("1,5"), Err(Comma(_))));
    assert!(matches!(parse_mantissa("--1"), Err(NotANumber(_))));
    assert!(matches!(parse_mantissa("5%"), Err(NotANumber(_))));
    for s in ["nan", "NaN", "inf", "-inf", "1e999"] {
        assert!(matches!(parse_mantissa(s), Err(NonFinite(_))), "{s:?}");
    }
    assert!(
        parse_mantissa("10k")
            .unwrap_err()
            .to_string()
            .contains("pick")
    );
}

/// 4.7 + k and 4700 + none are the same double; so are 4.7 + n and
/// 4700 + p. Float multiplication does not give that.
#[test]
fn join_is_bit_equal_where_multiplication_is_not() {
    let a = parts("4.7", SiPrefix::Kilo, Unit::Ohm);
    let b = parts("4700", SiPrefix::One, Unit::Ohm);
    assert_eq!(a.to_bits(), b.to_bits());
    assert_eq!(a, b);
    let c = parts("4.7", SiPrefix::Nano, Unit::Farad);
    let d = parts("4700", SiPrefix::Pico, Unit::Farad);
    assert_eq!(c.to_bits(), d.to_bits());
    assert_eq!(c.base(), 4.7e-9);
    // The naive product is off by an ulp for some of these.
    let naive_misses = (1..=999)
        .map(f64::from)
        .map(|m| m / 10.0)
        .filter(|m| m * 1e-9 != format!("{m}e-9").parse::<f64>().unwrap())
        .count();
    assert!(naive_misses > 0, "pick a mantissa where multiply differs");
    for m in (1..=999).map(|m| f64::from(m) / 10.0) {
        let joined = Quantity::from_mantissa(m, SiPrefix::Nano, Unit::Farad).unwrap();
        assert_eq!(joined.base(), format!("{m}e-9").parse::<f64>().unwrap());
    }
    // Number-field exponent and prefix combine.
    assert_eq!(parts("1e3", SiPrefix::Kilo, Unit::Ohm).base(), 1e6);
    assert_eq!(parts("-2.5", SiPrefix::Milli, Unit::Volt).base(), -2.5e-3);
}

#[test]
fn parts_errors() {
    assert!(matches!(
        quantity_from_parts("1e308", SiPrefix::Giga, Unit::Ohm),
        Err(MantissaError::NonFinite(_))
    ));
    assert!(matches!(
        quantity_from_parts("5", SiPrefix::Kilo, Unit::Percent),
        Err(MantissaError::PrefixNotAllowed(_))
    ));
    assert!(matches!(
        quantity_from_parts("4k7", SiPrefix::One, Unit::Ohm),
        Err(MantissaError::HasLetters(_))
    ));
}

#[test]
fn units_tokens_glyphs_and_prefix_sets() {
    for u in Unit::STANDARD {
        assert_eq!(Unit::from_alias(u.token()), Some(u.clone()), "{u:?}");
    }
    let tokens: Vec<_> = Unit::STANDARD
        .iter()
        .map(|u| u.token().to_string())
        .collect();
    assert_eq!(
        tokens,
        [
            "ohm", "F", "H", "V", "A", "W", "Hz", "%", "ppm/C", "V/us", "AWG", "bps", "degC"
        ]
    );
    assert_eq!(Unit::Ohm.glyph(), "\u{03A9}");
    assert_eq!(Unit::PpmPerKelvin.glyph(), "ppm/°C");
    assert_eq!(Unit::VoltPerMicrosecond.glyph(), "V/\u{00B5}s");
    use SiPrefix::*;
    assert_eq!(SiPrefix::all_for(&Unit::Farad), &[Pico, Nano, Micro, One]);
    assert_eq!(
        SiPrefix::all_for(&Unit::Ohm),
        &[Milli, One, Kilo, Mega, Giga]
    );
    assert_eq!(SiPrefix::all_for(&Unit::Henry), &[Nano, Micro, Milli, One]);
    assert_eq!(SiPrefix::all_for(&Unit::Volt), &[Micro, Milli, One, Kilo]);
    assert_eq!(
        SiPrefix::all_for(&Unit::BitsPerSecond),
        &[One, Kilo, Mega, Giga]
    );
    assert_eq!(Unit::DegreeCelsius.glyph(), "°C");
    for u in [
        Unit::Percent,
        Unit::PpmPerKelvin,
        Unit::VoltPerMicrosecond,
        Unit::Awg,
        Unit::DegreeCelsius,
        Unit::Other("x".into()),
    ] {
        assert_eq!(SiPrefix::all_for(&u), &[One], "{u:?}");
        assert!(!u.takes_prefix());
    }
    assert_eq!(SiPrefix::ALL.len(), 10);
    assert_eq!(SiPrefix::Femto.exponent(), -15);
    assert_eq!(SiPrefix::Tera.exponent(), 12);
}

#[test]
fn split_unit_string_real_units() {
    use SiPrefix::*;
    let cases = [
        ("kΩ", Kilo, Unit::Ohm),
        ("kΩ", Kilo, Unit::Ohm),
        ("Ω", One, Unit::Ohm),
        ("\u{2126}", One, Unit::Ohm),
        ("ohm", One, Unit::Ohm),
        ("kOhms", Kilo, Unit::Ohm),
        ("Mohm", Mega, Unit::Ohm),
        ("mohm", Milli, Unit::Ohm),
        ("µF", Micro, Unit::Farad),
        ("\u{03BC}F", Micro, Unit::Farad),
        ("uF", Micro, Unit::Farad),
        ("nH", Nano, Unit::Henry),
        ("V", One, Unit::Volt),
        ("%", One, Unit::Percent),
        ("kHz", Kilo, Unit::Hertz),
        ("Hz", One, Unit::Hertz),
        ("ppm/C", One, Unit::PpmPerKelvin),
        ("ppm/°C", One, Unit::PpmPerKelvin),
        ("V/us", One, Unit::VoltPerMicrosecond),
        ("V/µs", One, Unit::VoltPerMicrosecond),
        ("AWG", One, Unit::Awg),
        ("k ohm", Kilo, Unit::Ohm),
        ("mF", Milli, Unit::Farad),
        ("mhz", Mega, Unit::Hertz),
        ("mhz ", Mega, Unit::Hertz),
        ("MHz", Mega, Unit::Hertz),
        ("kHz", Kilo, Unit::Hertz),
        ("bps", One, Unit::BitsPerSecond),
        ("kbps", Kilo, Unit::BitsPerSecond),
        ("Kbps", Kilo, Unit::BitsPerSecond),
        ("Mbps", Mega, Unit::BitsPerSecond),
        ("Gbps", Giga, Unit::BitsPerSecond),
        ("kbit/s", Kilo, Unit::BitsPerSecond),
        ("°C", One, Unit::DegreeCelsius),
        ("\u{2103}", One, Unit::DegreeCelsius),
        ("degC", One, Unit::DegreeCelsius),
        ("degc", One, Unit::DegreeCelsius),
    ];
    for (s, p, u) in cases {
        assert_eq!(split_unit_string(s), Some((p, u)), "{s:?}");
    }
    for s in [
        "", "k", "k%", "kppm/C", "kAWG", "xF", "kk", "mm", "foo", "µs", "Bps", "kBps", "mbps",
        "k°C", "C", "kdegC",
    ] {
        assert_eq!(split_unit_string(s), None, "{s:?}");
    }
}

#[test]
fn legacy_prefixed_unit_reads_as_base() {
    let legacy = quantity_from_stored(4.7, "kΩ").unwrap();
    let canonical = quantity_from_stored(4700.0, "ohm").unwrap();
    assert_eq!(legacy.base(), 4700.0);
    assert_eq!(legacy, canonical);
    assert_eq!(quantity_from_stored(100.0, "nF").unwrap().base(), 100e-9);
    assert_eq!(quantity_from_stored(5.0, "%").unwrap().base(), 5.0);
    let other = quantity_from_stored(5.0, "furlong").unwrap();
    assert_eq!(
        (other.base(), other.unit()),
        (5.0, &Unit::Other("furlong".into()))
    );
    assert_eq!(quantity_from_stored(f64::NAN, "ohm"), None);
}

/// A prefix outside the unit's set is kept as written, not reinterpreted.
#[test]
fn stored_prefix_outside_the_units_set_stays_as_written() {
    for s in ["kF", "mHz"] {
        let q = quantity_from_stored(2.0, s).unwrap();
        assert_eq!(q.base(), 2.0, "{s}");
        assert_eq!(q.unit(), &Unit::Other(s.to_string()), "{s}");
    }
    let mhz = quantity_from_stored(2.0, "mhz").unwrap();
    assert_eq!((mhz.base(), mhz.unit()), (2.0e6, &Unit::Hertz));
    let mf = quantity_from_stored(4.7, "mF").unwrap();
    assert_eq!((mf.base(), mf.unit()), (4.7e-3, &Unit::Farad));
}

#[test]
fn significant_digit_cap_rounds_the_display() {
    let sig = |base: f64, n: usize| {
        format_quantity_sig(&q(base, Unit::Ohm), ValueStyle::Engineering, false, n)
    };
    let parallel = 10_000.0 * 4_700.0 / 14_700.0;
    assert_eq!(sig(parallel, 4), "3.197 kΩ");
    assert!(
        q(parallel, Unit::Ohm).to_string().len() > 12,
        "uncapped keeps digits"
    );
    assert_eq!(sig(4700.0, 4), "4.7 kΩ");
    assert_eq!(sig(999_960.0, 4), "1 MΩ");
    assert_eq!(sig(123_456.0, 2), "120 kΩ");
    assert_eq!(sig(0.1 + 0.2, 1), "300 mΩ");
}

#[test]
fn unit_strings_match_across_prefixes() {
    assert!(unit_string_matches("kΩ", "ohm"));
    assert!(unit_string_matches("ohm", "ohm"));
    assert!(unit_string_matches("µF", "F"));
    assert!(!unit_string_matches("kF", "ohm"));
    assert!(!unit_string_matches("F", "ohm"));
    assert!(unit_string_matches("weird", "weird"));
    assert!(!unit_string_matches("weird", "ohm"));
}

#[test]
fn negative_zero_is_normalised() {
    let z = q(-0.0, Unit::Ohm);
    assert_eq!(z.to_bits(), 0.0f64.to_bits());
    assert_eq!(z, q(0.0, Unit::Ohm));
    assert_eq!(format_eng(-0.0, &Unit::Ohm), "0 \u{03A9}");
    assert!(Quantity::new(f64::NAN, Unit::Ohm).is_none());
    assert!(Quantity::new(f64::INFINITY, Unit::Ohm).is_none());
    let mut set = std::collections::HashSet::new();
    set.insert(q(4700.0, Unit::Ohm));
    assert!(set.contains(&parts("4.7", SiPrefix::Kilo, Unit::Ohm)));
    assert!(!set.contains(&q(4700.0, Unit::Farad)));
}

/// Design table 1.3, all three profiles.
#[test]
fn golden_format_table() {
    use Unit::*;
    use ValueStyle::*;
    // (base, unit, engineering, rkm, ascii)
    let rows: Vec<(f64, Unit, &str, &str, &str)> = vec![
        (0.0, Ohm, "0 Ω", "0R", "0 ohm"),
        (4.7, Ohm, "4.7 Ω", "4R7", "4.7 ohm"),
        (470.0, Ohm, "470 Ω", "470R", "470 ohm"),
        (4700.0, Ohm, "4.7 kΩ", "4k7", "4.7 kohm"),
        (4990.0, Ohm, "4.99 kΩ", "4k99", "4.99 kohm"),
        (1.5e6, Ohm, "1.5 MΩ", "1M5", "1.5 Mohm"),
        (2.2e-3, Ohm, "2.2 mΩ", "2m2", "2.2 mohm"),
        (4.7e-12, Farad, "4.7 pF", "4p7", "4.7 pF"),
        (100e-9, Farad, "100 nF", "100n", "100 nF"),
        (0.47e-6, Farad, "470 nF", "470n", "470 nF"),
        (4.7e-3, Farad, "4700 µF", "4700u", "4700 uF"),
        (10e-6, Henry, "10 µH", "10u", "10 uH"),
        (3.3, Volt, "3.3 V", "3.3 V", "3.3 V"),
        (1500.0, Volt, "1.5 kV", "1.5 kV", "1.5 kV"),
        (
            -200.0,
            PpmPerKelvin,
            "−200 ppm/°C",
            "−200 ppm/°C",
            "-200 ppm/C",
        ),
        (10.0, VoltPerMicrosecond, "10 V/µs", "10 V/µs", "10 V/us"),
        (18.0, Awg, "18 AWG", "18 AWG", "18 AWG"),
        (20e3, BitsPerSecond, "20 kbps", "20 kbps", "20 kbps"),
        (480e6, BitsPerSecond, "480 Mbps", "480 Mbps", "480 Mbps"),
        (1e9, BitsPerSecond, "1 Gbps", "1 Gbps", "1 Gbps"),
        (500.0, BitsPerSecond, "500 bps", "500 bps", "500 bps"),
        (-40.0, DegreeCelsius, "−40 °C", "−40 °C", "-40 degC"),
        (125.0, DegreeCelsius, "125 °C", "125 °C", "125 degC"),
    ];
    for (base, unit, eng, rkm, ascii) in rows {
        assert_eq!(render(base, unit.clone(), Engineering), eng, "{base:e} eng");
        assert_eq!(render(base, unit.clone(), Rkm), rkm, "{base:e} rkm");
        assert_eq!(render(base, unit, Ascii), ascii, "{base:e} ascii");
    }
}

#[test]
fn tolerance_plus_minus_profiles() {
    let t = q(1.0, Unit::Percent);
    assert_eq!(format_quantity(&t, ValueStyle::Engineering, true), "±1 %");
    assert_eq!(format_quantity(&t, ValueStyle::Rkm, true), "±1%");
    assert_eq!(format_quantity(&t, ValueStyle::Ascii, true), "+/-1 %");
    assert_eq!(format_quantity(&t, ValueStyle::Engineering, false), "1 %");
}

#[test]
fn no_float_noise_and_no_truncation() {
    // 4.7 nF must never render as 4.700000000000001 nF.
    let nf = Quantity::from_mantissa(4.7, SiPrefix::Nano, Unit::Farad).unwrap();
    assert_eq!(nf.to_string(), "4.7 nF");
    assert_eq!(
        parts("4.7", SiPrefix::Nano, Unit::Farad).to_string(),
        "4.7 nF"
    );
    // No 4-digit truncation.
    assert_eq!(format_eng(4.99001e3, &Unit::Ohm), "4.99001 kΩ");
    assert_eq!(format_eng(123456.789, &Unit::Ohm), "123.456789 kΩ");
    assert_eq!(format_eng(1.0e-3 + 1.0e-9, &Unit::Henry), "1.000001 mH");
    // More than 15 digits are cut at 15.
    assert_eq!(format_eng(0.1 + 0.2, &Unit::Volt), "300 mV");
}

#[test]
fn prefix_choice_stays_in_allowed_set_with_exponent_fallback() {
    assert_eq!(format_eng(4.7e-3, &Unit::Farad), "4700 µF");
    assert_eq!(format_eng(0.5, &Unit::Farad), "500000 µF");
    assert_eq!(format_eng(2.0, &Unit::Farad), "2 F");
    assert_eq!(format_eng(1.2e-16, &Unit::Farad), "1.2e-16 F");
    assert_eq!(format_eng(1e-3, &Unit::Henry), "1 mH");
    assert_eq!(format_eng(5e15, &Unit::Ohm), "5e15 Ω");
    assert_eq!(format_eng(999.9e6, &Unit::Ohm), "999.9 MΩ");
    assert_eq!(format_eng(1e-5, &Unit::Percent), "1e-5 %");
    assert_eq!(format_eng(0.5, &Unit::Percent), "0.5 %");
    assert_eq!(format_eng(0.5e-12, &Unit::Farad), "0.5 pF");
    let (m, p) = q(4700.0, Unit::Ohm).eng_parts();
    assert_eq!((m.as_str(), p), ("4.7", SiPrefix::Kilo));
    let (m, p) = q(4.7e-3, Unit::Farad).eng_parts();
    assert_eq!((m.as_str(), p), ("4700", SiPrefix::Micro));
    let (m, p) = q(1.2e-16, Unit::Farad).eng_parts();
    assert_eq!((m.as_str(), p), ("1.2e-16", SiPrefix::One));
    // Every mantissa a picker shows is a valid number-field entry.
    assert!(parse_mantissa(&m).is_ok());
}

#[test]
fn other_unit_displays_as_written() {
    let o = q(5.0, Unit::Other("widgets".into()));
    assert_eq!(o.to_string(), "5 widgets");
    assert_eq!(q(5.0, Unit::Other(String::new())).to_string(), "5");
}

/// Fixed-seed sweep: any value up to 15 significant digits survives
/// format -> split + number-field parse bit-for-bit, in all profiles that
/// are parseable (Engineering and Ascii).
#[test]
fn round_trip_property_is_bit_equal() {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move |n: u64| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) % n
    };
    for _ in 0..30_000 {
        let unit = Unit::STANDARD[next(Unit::STANDARD.len() as u64) as usize].clone();
        let sig = 1 + next(15) as usize;
        let digits: String = (0..sig)
            .map(|i| char::from(b'0' + if i == 0 { 1 + next(9) } else { next(10) } as u8))
            .collect();
        let exp = next(31) as i32 - 18;
        let sign = if next(4) == 0 { "-" } else { "" };
        let text = format!("{sign}{}.{}e{exp}", &digits[..1], &digits[1..]);
        let base: f64 = text.parse().unwrap();
        let original = q(base, unit.clone());
        for style in [ValueStyle::Engineering, ValueStyle::Ascii] {
            let out = format_quantity(&original, style, false).replace('\u{2212}', "-");
            let (num, suffix) = out.split_once(' ').unwrap();
            let (prefix, u) = split_unit_string(suffix).unwrap();
            assert_eq!(u, unit, "{out}");
            let back = quantity_from_parts(num, prefix, u).unwrap();
            assert_eq!(back.to_bits(), original.to_bits(), "{text} -> {out}");
        }
    }
}

#[test]
fn round_trip_zero() {
    for unit in Unit::STANDARD {
        let out = format_eng(0.0, &unit);
        let (num, suffix) = out.split_once(' ').unwrap();
        assert_eq!(parse_mantissa(num), Ok(0.0));
        assert_eq!(split_unit_string(suffix).unwrap().1, unit);
    }
}

#[test]
fn an_exponent_that_does_not_fit_is_refused_not_dropped() {
    // "1e-99999999999" parses as 0.0 alone; joined with kilo it must not
    // turn into "1e3" (1000) by dropping the unreadable exponent.
    assert!(quantity_from_parts("1e-99999999999", SiPrefix::Kilo, Unit::Ohm).is_err());
    assert!(quantity_from_parts("1e2147483647", SiPrefix::Kilo, Unit::Ohm).is_err());
}

/// mockup row 52: data rate is a real unit with k, M and G.
#[test]
fn a_data_rate_is_entered_in_a_prefix_and_stored_as_bits_per_second() {
    let rate = parts("20", SiPrefix::Kilo, Unit::BitsPerSecond);
    assert_eq!(rate.base(), 20_000.0);
    assert_eq!(rate.to_string(), "20 kbps");
    assert_eq!(quantity_from_stored(20.0, "kbps").unwrap(), rate);
    assert_eq!(quantity_from_stored(20_000.0, "bps").unwrap(), rate);
    // Bytes per second is not this unit; it is kept as written.
    assert_eq!(
        quantity_from_stored(20.0, "kBps").unwrap().unit(),
        &Unit::Other("kBps".into())
    );
}

/// mockup row 52: °C is a unit, takes no prefix, may be
/// negative.
#[test]
fn celsius_takes_no_prefix_and_reads_its_spellings() {
    let t = parts("-40", SiPrefix::One, Unit::DegreeCelsius);
    assert_eq!(t.to_string(), "−40 °C");
    for spelling in ["°C", "\u{2103}", "degC"] {
        assert_eq!(
            quantity_from_stored(-40.0, spelling).unwrap(),
            t,
            "{spelling}"
        );
    }
    assert!(matches!(
        quantity_from_parts("1", SiPrefix::Kilo, Unit::DegreeCelsius),
        Err(MantissaError::PrefixNotAllowed(_))
    ));
}

/// The new units sort after every older family, so existing sorted
/// columns keep their order.
#[test]
fn new_units_rank_after_the_older_families() {
    let mut keys = [
        q(25.0, Unit::DegreeCelsius),
        q(1e6, Unit::BitsPerSecond),
        q(18.0, Unit::Awg),
        q(1.0, Unit::Ohm),
    ];
    keys.sort_by(compare_for_sort);
    let units: Vec<&Unit> = keys.iter().map(Quantity::unit).collect();
    assert_eq!(
        units,
        [
            &Unit::Ohm,
            &Unit::Awg,
            &Unit::BitsPerSecond,
            &Unit::DegreeCelsius
        ]
    );
    assert_eq!(
        sort_key_from_value_text("20kbps"),
        Some(q(20e3, Unit::BitsPerSecond))
    );
    assert_eq!(
        sort_key_from_value_text("125 °C"),
        Some(q(125.0, Unit::DegreeCelsius))
    );
}

#[test]
fn the_displayed_number_drops_prefix_and_unit() {
    assert_eq!(q(100e-9, Unit::Farad).displayed_number(), 100.0);
    assert_eq!(q(4700.0, Unit::Ohm).displayed_number(), 4.7);
    assert_eq!(q(-40.0, Unit::DegreeCelsius).displayed_number(), -40.0);
    assert_eq!(q(1.2e-16, Unit::Farad).displayed_number(), 1.2e-16);
    assert_eq!(q(100e-9, Unit::Farad).number_in(SiPrefix::Micro), 0.1);
    assert_eq!(q(3.3, Unit::Volt).number_in(SiPrefix::Milli), 3300.0);
    assert_eq!(q(0.1 + 0.2, Unit::Volt).number_in(SiPrefix::One), 0.3);
}

fn range(a: f64, b: f64, unit: Unit) -> QuantityRange {
    QuantityRange::new(q(a, unit.clone()), q(b, unit)).unwrap()
}

#[test]
fn a_range_orders_its_ends_and_refuses_mixed_units() {
    let r = range(3.6, 3.0, Unit::Volt);
    assert_eq!((r.low().base(), r.high().base()), (3.0, 3.6));
    assert_eq!(r.end(RangeEnd::Max).base(), 3.6);
    assert!(QuantityRange::new(q(1.0, Unit::Volt), q(1.0, Unit::Ampere)).is_none());
    // A one-point range is valid.
    assert!(range(5.0, 5.0, Unit::Volt).covers(&q(5.0, Unit::Volt)));
}

#[test]
fn covers_and_within_include_the_ends() {
    let supply = range(3.0, 3.6, Unit::Volt);
    assert!(supply.covers(&q(3.3, Unit::Volt)));
    assert!(supply.covers(&q(3.0, Unit::Volt)));
    assert!(supply.covers(&q(3.6, Unit::Volt)));
    assert!(!supply.covers(&q(5.0, Unit::Volt)));
    assert!(!supply.covers(&q(3.3, Unit::Ampere)));
    let temp = range(-40.0, 85.0, Unit::DegreeCelsius);
    let c = |b| q(b, Unit::DegreeCelsius);
    assert!(temp.is_within(&c(-40.0), &c(125.0)));
    assert!(!temp.is_within(&c(-20.0), &c(125.0)));
    assert!(!temp.is_within(&c(-40.0), &c(70.0)));
}

#[test]
fn display_ends_share_a_prefix_or_carry_their_own() {
    use ValueStyle::*;
    let show = |r: &QuantityRange| r.display_ends(Engineering);
    assert_eq!(
        show(&range(3.0, 3.6, Unit::Volt)),
        ("3".into(), "3.6 V".into())
    );
    assert_eq!(
        show(&range(-40.0, 125.0, Unit::DegreeCelsius)),
        ("−40".into(), "125 °C".into())
    );
    assert_eq!(
        show(&range(0.5, 5.5, Unit::Volt)),
        ("500 mV".into(), "5.5 V".into())
    );
    assert_eq!(
        range(-40.0, 125.0, Unit::DegreeCelsius).display_ends(Ascii),
        ("-40".into(), "125 degC".into())
    );
    assert_eq!(
        range(2e3, 1e6, Unit::BitsPerSecond).display_ends(Engineering),
        ("2 kbps".into(), "1 Mbps".into())
    );
}

#[test]
fn ranges_sort_by_the_chosen_end_then_the_other() {
    use std::cmp::Ordering::*;
    let a = range(-40.0, 85.0, Unit::DegreeCelsius);
    let b = range(-40.0, 125.0, Unit::DegreeCelsius);
    let c = range(0.0, 85.0, Unit::DegreeCelsius);
    assert_eq!(compare_ranges(&a, &b, RangeEnd::Max), Less);
    assert_eq!(compare_ranges(&c, &b, RangeEnd::Max), Less);
    assert_eq!(
        compare_ranges(&a, &c, RangeEnd::Max),
        Less,
        "tie on max, then min"
    );
    assert_eq!(compare_ranges(&c, &b, RangeEnd::Min), Greater);
    assert_eq!(compare_ranges(&a, &a.clone(), RangeEnd::Min), Equal);
}

#[test]
fn range_end_serialises_as_min_or_max() {
    assert_eq!(serde_json::to_string(&RangeEnd::Max).unwrap(), "\"max\"");
    let back: RangeEnd = serde_json::from_str("\"min\"").unwrap();
    assert_eq!(back, RangeEnd::Min);
}
