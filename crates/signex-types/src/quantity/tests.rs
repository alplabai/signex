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
            "ohm", "F", "H", "V", "A", "W", "Hz", "%", "ppm/C", "V/us", "AWG"
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
    for u in [
        Unit::Percent,
        Unit::PpmPerKelvin,
        Unit::VoltPerMicrosecond,
        Unit::Awg,
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
        ("fF", Femto, Unit::Farad),
        ("k ohm", Kilo, Unit::Ohm),
        ("kF", Kilo, Unit::Farad),
    ];
    for (s, p, u) in cases {
        assert_eq!(split_unit_string(s), Some((p, u)), "{s:?}");
    }
    for s in [
        "", "k", "k%", "kppm/C", "kAWG", "xF", "kk", "mm", "foo", "µs",
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
