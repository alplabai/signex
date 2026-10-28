use super::*;

const MU: &str = "µ";

fn close(a: f64, b: f64) -> bool {
    a == b || ((a - b) / b).abs() < 1e-12
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
    assert!(matches!(parse_mantissa("--1"), Err(NotANumber(_))));
    assert!(matches!(parse_mantissa("5%"), Err(NotANumber(_))));
    assert!(matches!(parse_mantissa("1.2.3"), Err(NotANumber(_))));
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

#[test]
fn pickers_list_prefixes_and_units() {
    assert_eq!(SiPrefix::all_for(Unit::Ohm).len(), 8);
    assert_eq!(SiPrefix::all_for(Unit::Percent), &[SiPrefix::One]);
    assert_eq!(SiPrefix::all_for(Unit::Celsius), &[SiPrefix::One]);
    assert_eq!(SiPrefix::all_for(Unit::Ppm), &[SiPrefix::One]);
    assert_eq!(Unit::ALL.len(), 11);
}

#[test]
fn quantity_to_base_and_back() {
    let q = Quantity {
        mantissa: 4.7,
        prefix: SiPrefix::Kilo,
        unit: Unit::Ohm,
    };
    assert_eq!(q.to_base(), 4700.0);
    assert_eq!(q.unit_string(), "kΩ");
    assert_eq!(q.to_string(), "4.7 kΩ");
    let r = Quantity::from_base_eng(100e-9, Unit::Farad);
    assert_eq!((r.prefix, r.unit), (SiPrefix::Nano, Unit::Farad));
    assert!(close(r.mantissa, 100.0));
    let z = Quantity::from_base_eng(0.0, Unit::Ohm);
    assert_eq!((z.mantissa, z.prefix), (0.0, SiPrefix::One));
    let p = Quantity::from_base_eng(5.0, Unit::Percent);
    assert_eq!(p.prefix, SiPrefix::One);
    // Display rounding carries to the next prefix; the mantissa stays exact.
    assert_eq!(
        Quantity::from_base_eng(999.96, Unit::Ohm).prefix,
        SiPrefix::Kilo
    );
}

#[test]
fn from_base_eng_keeps_mantissa_in_range() {
    for unit in Unit::ALL.into_iter().filter(|u| u.takes_prefix()) {
        for x in [
            1e-12, 3.3e-9, 4.7e-6, 0.001, 1.0, 999.0, 4700.0, 2.2e6, 9.9e9,
        ] {
            let q = Quantity::from_base_eng(x, unit);
            assert!((1.0..1000.0).contains(&q.mantissa), "{x:e}: {q:?}");
            assert!(close(q.to_base(), x), "{x:e}");
        }
    }
}

#[test]
fn split_unit_string_real_units() {
    use SiPrefix::*;
    use Unit::*;
    let cases = [
        ("kΩ", Kilo, Ohm),
        ("Ω", One, Ohm),
        ("µF", Micro, Farad),
        ("uF", Micro, Farad),
        ("nH", Nano, Henry),
        ("V", One, Volt),
        ("%", One, Percent),
        ("MΩ", Mega, Ohm),
        ("mΩ", Milli, Ohm),
        ("pF", Pico, Farad),
        ("kHz", Kilo, Hertz),
        ("Hz", One, Hertz),
        ("ppm", One, Ppm),
        ("°C", One, Celsius),
        ("ms", Milli, Second),
        ("s", One, Second),
        ("mW", Milli, Watt),
        ("k ohm", Kilo, Ohm),
    ];
    for (s, p, u) in cases {
        assert_eq!(split_unit_string(s), Some((p, u)), "{s:?}");
    }
    for s in ["", "k", "k%", "m°C", "kppm", "xF", "kk", "mm", "foo"] {
        assert_eq!(split_unit_string(s), None, "{s:?}");
    }
}

#[test]
fn base_value_maps_measurement() {
    assert_eq!(base_value(4.7, "kΩ"), Some((4700.0, Unit::Ohm)));
    assert_eq!(base_value(100.0, "nF"), Some((100e-9, Unit::Farad)));
    assert_eq!(
        base_value(1.0, &format!("{MU}F")),
        Some((1e-6, Unit::Farad))
    );
    assert_eq!(base_value(5.0, "%"), Some((5.0, Unit::Percent)));
    assert_eq!(base_value(5.0, "furlong"), None);
    assert_eq!(base_value(f64::MAX, "G" /* prefix only */), None);
    assert_eq!(base_value(f64::MAX, "GΩ"), None);
}

#[test]
fn format_table() {
    use Unit::*;
    let cases = [
        (4700.0, Ohm, "4.7 kΩ"),
        (100e-9, Farad, "100 nF"),
        (1e-6, Farad, "1 µF"),
        (0.0, Ohm, "0 Ω"),
        (-0.0, Ohm, "0 Ω"),
        (2.2, Ohm, "2.2 Ω"),
        (0.47, Ohm, "470 mΩ"),
        (1.5e6, Ohm, "1.5 MΩ"),
        (999.96, Ohm, "1 kΩ"),
        (1000.0, Ohm, "1 kΩ"),
        (999.0, Ohm, "999 Ω"),
        (12345.0, Ohm, "12.35 kΩ"),
        (3.3, Volt, "3.3 V"),
        (2.2e-12, Farad, "2.2 pF"),
        (1e-15, Farad, "0.001 pF"),
        (1e12, Ohm, "1000 GΩ"),
        (5.0, Percent, "5 %"),
        (20.0, Ppm, "20 ppm"),
        (-40.0, Celsius, "-40 °C"),
        (-4700.0, Ohm, "-4.7 kΩ"),
        (1e-3, Henry, "1 mH"),
        (1e-6, Henry, "1 µH"),
    ];
    for (v, u, want) in cases {
        assert_eq!(format_eng(v, u), want, "{v:e}");
    }
}

#[test]
fn format_parts_splits_mantissa_and_prefix() {
    assert_eq!(
        format_parts(4700.0, Unit::Ohm),
        ("4.7".to_string(), SiPrefix::Kilo)
    );
    assert_eq!(
        format_parts(0.0, Unit::Farad),
        ("0".to_string(), SiPrefix::One)
    );
    assert_eq!(
        format_parts(12.5, Unit::Percent),
        ("12.5".to_string(), SiPrefix::One)
    );
}

#[test]
fn every_prefix_boundary_formats_in_range() {
    for unit in Unit::ALL.into_iter().filter(|u| u.takes_prefix()) {
        for exp in -12..=9 {
            let x = 10f64.powi(exp.max(0)) / 10f64.powi((-exp).max(0));
            let (m, p) = format_parts(x, unit);
            let m: f64 = m.parse().unwrap();
            assert!((1.0..1000.0).contains(&m), "{x:e} {unit}: {m}");
            assert!(close(p.scale(m), x), "{x:e} {unit}");
        }
    }
}

/// Fixed-seed sweep: values with at most 4 significant digits (all the
/// formatter keeps) survive format -> split_unit_string + parse_mantissa
/// to 1e-12.
#[test]
fn round_trip_property() {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move |n: u64| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) % n
    };
    for _ in 0..20_000 {
        let unit = Unit::ALL[next(Unit::ALL.len() as u64) as usize];
        let choices = SiPrefix::all_for(unit);
        let prefix = choices[next(choices.len() as u64) as usize];
        let digits = 1000 + next(9000) as i32;
        let mantissa = f64::from(digits) / 10f64.powi(next(3) as i32 + 1);
        let sign = if next(4) == 0 { -1.0 } else { 1.0 };
        let x = sign * prefix.scale(mantissa);
        let text = format_eng(x, unit);
        let (num, suffix) = text.split_once(' ').unwrap();
        let (p, u) = split_unit_string(suffix).unwrap();
        assert_eq!(u, unit, "{text}");
        let back = p.scale(parse_mantissa(num).unwrap());
        assert!(close(back, x), "{text}: {back:e} != {x:e}");
        let (stored, _) = base_value(parse_mantissa(num).unwrap(), suffix).unwrap();
        assert_eq!(stored, back);
    }
}

#[test]
fn round_trip_zero_and_extremes() {
    for unit in Unit::ALL {
        let text = format_eng(0.0, unit);
        let (num, suffix) = text.split_once(' ').unwrap();
        assert_eq!(parse_mantissa(num), Ok(0.0));
        assert_eq!(split_unit_string(suffix).unwrap().1, unit);
    }
    for x in [1e-12, 9.999e-12, 999.9e9, 1.0, 999.9] {
        let q = Quantity::from_base_eng(x, Unit::Ohm);
        assert!(close(q.to_base(), x), "{x:e}");
    }
}
