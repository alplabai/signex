use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::transmission_line_calculator::*;

/// Asserts that two floating-point values differ by no more than a scaled epsilon.
fn assert_close(actual: f64, expected: f64) {
    let tolerance = 1.0e-10 * expected.abs().max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

/// Asserts that two complex values are approximately equal.
fn assert_complex_close(actual: Complex, expected: Complex) {
    assert_close(actual.re, expected.re);
    assert_close(actual.im, expected.im);
}

/// Creates a complete one-port block suitable for writer tests.
fn one_port_block() -> SParameterBlock {
    let reference_impedance_ohm = 75.0;
    let s11 = Complex::new(0.3, -0.4);
    SParameterBlock {
        kind: SParameterKind::S1P,
        reference_impedance_ohm,
        port_reference_impedances_ohm: vec![reference_impedance_ohm],
        source_frequency_unit: ScalarUnit::MegaHertz,
        points: vec![SParameterPoint {
            frequency_hz: 2.0e6,
            s11,
            s21: None,
            s12: None,
            s22: None,
            z_s11: reflection_to_impedance(s11, reference_impedance_ohm),
        }],
        noise: Vec::new(),
        raw: String::new(),
    }
}

/// Creates a complete two-port block with per-port references and noise data.
fn two_port_block() -> SParameterBlock {
    let s11 = Complex::new(0.1, 0.2);
    SParameterBlock {
        kind: SParameterKind::S2P,
        reference_impedance_ohm: 50.0,
        port_reference_impedances_ohm: vec![50.0, 75.0],
        source_frequency_unit: ScalarUnit::MegaHertz,
        points: vec![SParameterPoint {
            frequency_hz: 100.0e6,
            s11,
            s21: Some(Complex::new(2.0, 0.5)),
            s12: Some(Complex::new(0.05, -0.02)),
            s22: Some(Complex::new(-0.1, 0.3)),
            z_s11: reflection_to_impedance(s11, 50.0),
        }],
        noise: vec![NoisePoint {
            frequency_hz: 90.0e6,
            fmin_db: 1.2,
            optimum_gamma: Complex::from_polar(0.25, 30.0),
            rn_ohm: 7.5,
            optimum_admittance: Complex::ZERO,
        }],
        raw: String::new(),
    }
}

/// Returns a unique path in the repository's temporary directory.
fn temporary_touchstone_path(extension: &str) -> PathBuf {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = workspace.join(".temp");
    fs::create_dir_all(&directory).unwrap();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    directory.join(format!(
        "touchstone-test-{}-{timestamp}.{extension}",
        std::process::id()
    ))
}

/// Verifies Version 1.x defaults, comments, and order-independent option tokens.
#[test]
fn parses_version1_defaults_comments_and_option_order() {
    let block =
        parse_touchstone("\u{feff}! leading comment\n# RI MHz R 75 S ! inline comment\n2 0.3 -0.4")
            .unwrap();

    assert_eq!(block.kind, SParameterKind::S1P);
    assert_eq!(block.source_frequency_unit, ScalarUnit::MegaHertz);
    assert_eq!(block.port_reference_impedances_ohm, vec![75.0]);
    assert_close(block.points[0].frequency_hz, 2.0e6);
    assert_complex_close(block.points[0].s11, Complex::new(0.3, -0.4));

    let defaults = parse_touchstone("#\n1 0.5 90").unwrap();
    assert_eq!(defaults.source_frequency_unit, ScalarUnit::GigaHertz);
    assert_close(defaults.reference_impedance_ohm, 50.0);
    assert_complex_close(defaults.points[0].s11, Complex::new(0.0, 0.5));
}

/// Verifies all three standard complex-number encodings.
#[test]
fn parses_real_imaginary_magnitude_angle_and_decibel_angle() {
    let cases = [
        ("RI", "0.3 0.4", Complex::new(0.3, 0.4)),
        ("MA", "0.5 53.13010235415598", Complex::new(0.3, 0.4)),
        ("DB", "-6.020599913279624 90", Complex::new(0.0, 0.5)),
    ];

    for (format, pair, expected) in cases {
        let raw = format!("# Hz S {format} R 50\n1 {pair}");
        let block = parse_touchstone(&raw).unwrap();
        assert_complex_close(block.points[0].s11, expected);
    }
}

/// Verifies standard two-port ordering and Version 1.x noise normalization.
#[test]
fn parses_version1_two_port_data_and_independent_noise_frequencies() {
    let block = parse_touchstone(
        "# GHz S RI R 50 75\n\
         1 0.1 0.2 2 0.5 0.05 -0.02 -0.1 0.3\n\
         ! Noise parameters\n\
         0.9 1.2 0.25 30 0.1",
    )
    .unwrap();

    assert_eq!(block.kind, SParameterKind::S2P);
    assert_eq!(block.port_reference_impedances_ohm, vec![50.0, 75.0]);
    assert_complex_close(block.points[0].s21.unwrap(), Complex::new(2.0, 0.5));
    assert_complex_close(block.points[0].s12.unwrap(), Complex::new(0.05, -0.02));
    assert_eq!(block.noise.len(), 1);
    assert_close(block.noise[0].frequency_hz, 0.9e9);
    assert_close(block.noise[0].rn_ohm, 5.0);
}

/// Verifies Version 2.x keywords, continuations, alternate ordering, and noise units.
#[test]
fn parses_version2_full_matrix_continuations_and_per_port_references() {
    let block = parse_touchstone(
        "[Version] 2.1\n\
         # MHz S RI R 50\n\
         [Number of Ports] 2\n\
         [Number of Frequencies] 1\n\
         [Number of Noise Frequencies] 1\n\
         [Reference] 50\n\
         75\n\
         [Two-Port Data Order] 12_21\n\
         [Matrix Format] Full\n\
         [Network Data]\n\
         100 0.1 0.2 0.05 -0.02 ! S11 and S12\n\
         2 0.5 -0.1 0.3 ! S21 and S22\n\
         [Noise Data]\n\
         90 1.2 0.25 30 7.5\n\
         [End]",
    )
    .unwrap();

    assert_eq!(block.port_reference_impedances_ohm, vec![50.0, 75.0]);
    assert_complex_close(block.points[0].s21.unwrap(), Complex::new(2.0, 0.5));
    assert_complex_close(block.points[0].s12.unwrap(), Complex::new(0.05, -0.02));
    assert_close(block.noise[0].frequency_hz, 90.0e6);
    assert_close(block.noise[0].rn_ohm, 7.5);
}

/// Verifies triangular two-port matrices are expanded symmetrically.
#[test]
fn parses_version2_triangular_matrix() {
    let block = parse_touchstone(
        "[Version] 2.0\n\
         # GHz S RI R 50\n\
         [Number of Ports] 2\n\
         [Two-Port Data Order] 21_12\n\
         [Number of Frequencies] 1\n\
         [Matrix Format] Lower\n\
         [Network Data]\n\
         1 0.1 0.2 0.3 0.4 0.5 0.6\n\
         [End]",
    )
    .unwrap();

    assert_complex_close(block.points[0].s11, Complex::new(0.1, 0.2));
    assert_complex_close(block.points[0].s21.unwrap(), Complex::new(0.3, 0.4));
    assert_complex_close(block.points[0].s12.unwrap(), Complex::new(0.3, 0.4));
    assert_complex_close(block.points[0].s22.unwrap(), Complex::new(0.5, 0.6));
}

/// Verifies the specification's two-port noise example and conventional default ordering.
#[test]
fn parses_touchstone_2_1_specification_noise_example() {
    let block = parse_touchstone(
        "! Example 20 from the Touchstone 2.1 specification\n\
         [Version] 2.1\n\
         #\n\
         [Number of Ports] 2\n\
         [Number of Frequencies] 2\n\
         [Number of Noise Frequencies] 2\n\
         [Reference] 50 25.0\n\
         [Network Data]\n\
         2 0.95 -26 3.57 157 0.04 76 0.66 -14\n\
         22 0.60 -144 1.30 40 0.14 40 0.56 -85\n\
         [Noise Data]\n\
         4 0.7 0.64 69 19\n\
         18 2.7 0.46 -33 20\n\
         [End]",
    )
    .unwrap();

    assert_eq!(block.kind, SParameterKind::S2P);
    assert_eq!(block.port_reference_impedances_ohm, vec![50.0, 25.0]);
    assert_close(block.points[0].frequency_hz, 2.0e9);
    assert_close(block.points[0].s21.unwrap().magnitude(), 3.57);
    assert_close(block.points[0].s12.unwrap().magnitude(), 0.04);
    assert_close(block.noise[0].frequency_hz, 4.0e9);
    assert_close(block.noise[0].rn_ohm, 19.0);
}

/// Verifies optional information blocks and case-insensitive keywords are ignored safely.
#[test]
fn parses_version2_information_block() {
    let block = parse_touchstone(
        "[version] 2.1\n\
         # khz s ma r 50\n\
         [number of ports] 1\n\
         [number of frequencies] 1\n\
         [begin information]\n\
         [Manufacturer] Signex\n\
         additional information text\n\
         [end information]\n\
         [network data]\n\
         2 0.5 90\n\
         [end]",
    )
    .unwrap();

    assert_close(block.points[0].frequency_hz, 2.0e3);
    assert_complex_close(block.points[0].s11, Complex::new(0.0, 0.5));
}

/// Verifies rounded duplicate frequency keys retain the last record.
#[test]
fn replaces_duplicate_canonical_frequency_records() {
    let block = parse_touchstone(
        "# Hz S RI R 50\n\
         1000.49 0.1 0\n\
         1000.40 0.2 0",
    )
    .unwrap();

    assert_eq!(block.points.len(), 1);
    assert_close(block.points[0].frequency_hz, 1000.0);
    assert_close(block.points[0].s11.re, 0.2);
}

/// Verifies malformed and unsupported documents return parsing errors.
#[test]
fn rejects_invalid_touchstone_documents() {
    let invalid_documents = [
        "",
        "[Version] 1.0\n# Hz S RI R 50\n1 0 0",
        "# Hz Z RI R 50\n1 50 0",
        "# THz S RI R 50\n1 0 0",
        "[Version] 2.1\n#\n[Number of Ports] 3\n[Number of Frequencies] 1\n[Network Data]\n1 0 0\n[End]",
        "[Version] 2.1\n#\n[Number of Ports] 1\n[Network Data]\n1 0 0\n[End]",
        "[Version] 2.1\n#\n[Number of Ports] 2\n[Number of Frequencies] 1\n[Two-Port Data Order] 21_12\n[Reference] 50\n[Network Data]\n1 0 0 0 0 0 0 0 0\n[End]",
        "[Version] 2.1\n#\n[Number of Ports] 1\n[Number of Frequencies] 2\n[Network Data]\n1 0 0\n[End]",
        "[Version] 2.1\n#\n[Number of Ports] 1\n[Number of Frequencies] 1\n[Network Data]\n1 0 0\n[Noise Data]\n[End]",
        "[Version] 2.1\n#\n[Number of Ports] 2\n[Two-Port Data Order] 21_12\n[Number of Frequencies] 1\n[Number of Noise Frequencies] 2\n[Network Data]\n1 0 0 0 0 0 0 0 0\n[Noise Data]\n1 1 0.1 0 5\n[End]",
        "[Version] 2.1\n#\n[Number of Ports] 1\n[Number of Frequencies] 1\n[Network Data]\n1 NaN 0\n[End]",
        "[Version] 2.1\n#\n[Number of Ports] 1\n[Number of Frequencies] 1\n[Network Data]\n1 -0.5 0\n[End]",
        "[Version] 2.1\n#\n[Number of Ports] 1\n[Number of Frequencies] 1\n[Network Data]\n1 0 0",
        "[Version] 2.1\n#\n[Number of Ports] 1\n[Number of Frequencies] 1\n[Network Data]\n1 0 0\n[End]\nextra",
    ];

    for raw in invalid_documents {
        let error = parse_touchstone(raw).unwrap_err();
        assert!(matches!(error, SolveError::TouchstoneParseFailed { .. }));
    }
}

/// Verifies one-port data round-trips through every supported output format.
#[test]
fn serializes_and_parses_one_port_data_in_every_format() {
    let block = one_port_block();
    for format in [
        TouchstoneFormat::RealImaginary,
        TouchstoneFormat::MagnitudeAngle,
        TouchstoneFormat::DecibelAngle,
    ] {
        let raw = serialize_touchstone(&block, format).unwrap();
        assert!(raw.starts_with("! Touchstone 2.1 file written by Signex\n[Version] 2.1"));
        assert!(raw.ends_with("[End]\n"));
        let parsed = parse_touchstone(&raw).unwrap();
        assert_eq!(parsed.kind, block.kind);
        assert_eq!(parsed.port_reference_impedances_ohm, vec![75.0]);
        assert_complex_close(parsed.points[0].s11, block.points[0].s11);
    }
}

/// Verifies two-port references and noise data survive serialization.
#[test]
fn serializes_and_parses_two_port_data_with_noise() {
    let block = two_port_block();
    let raw = serialize_touchstone(&block, TouchstoneFormat::MagnitudeAngle).unwrap();

    assert!(raw.contains("[Two-Port Data Order] 21_12"));
    assert!(raw.contains("[Reference] 50 75"));
    assert!(raw.contains("[Number of Noise Frequencies] 1"));
    let parsed = parse_touchstone(&raw).unwrap();
    assert_eq!(parsed.port_reference_impedances_ohm, vec![50.0, 75.0]);
    assert_complex_close(parsed.points[0].s21.unwrap(), block.points[0].s21.unwrap());
    assert_complex_close(parsed.points[0].s12.unwrap(), block.points[0].s12.unwrap());
    assert_close(parsed.noise[0].frequency_hz, 90.0e6);
    assert_close(parsed.noise[0].rn_ohm, 7.5);
}

/// Verifies serialization orders frequency records deterministically.
#[test]
fn serializer_orders_frequency_records() {
    let mut block = one_port_block();
    let mut earlier = block.points[0].clone();
    earlier.frequency_hz = 1.0e6;
    block.points.insert(0, block.points[0].clone());
    block.points[0].frequency_hz = 3.0e6;
    block.points.push(earlier);

    let raw = serialize_touchstone(&block, TouchstoneFormat::RealImaginary).unwrap();
    let parsed = parse_touchstone(&raw).unwrap();
    let frequencies = parsed
        .points
        .iter()
        .map(|point| point.frequency_hz)
        .collect::<Vec<_>>();
    assert_eq!(frequencies, vec![1.0e6, 2.0e6, 3.0e6]);
}

/// Verifies file read and write APIs preserve a complete Touchstone block.
#[test]
fn reads_and_writes_touchstone_files() {
    let path = temporary_touchstone_path("s2p");
    let block = two_port_block();

    write_touchstone(&path, &block, TouchstoneFormat::RealImaginary).unwrap();
    let parsed = read_touchstone(&path).unwrap();
    fs::remove_file(&path).unwrap();

    assert_eq!(parsed.kind, SParameterKind::S2P);
    assert_eq!(parsed.port_reference_impedances_ohm, vec![50.0, 75.0]);
    assert_complex_close(parsed.points[0].s11, block.points[0].s11);
    assert_close(parsed.noise[0].rn_ohm, block.noise[0].rn_ohm);
}

/// Verifies file read failures retain path and I/O context.
#[test]
fn reports_touchstone_file_read_errors() {
    let path = temporary_touchstone_path("missing.s1p");
    let error = read_touchstone(&path).unwrap_err();

    assert!(matches!(error, SolveError::TouchstoneReadFailed { .. }));
    assert!(error.to_string().contains(path.to_string_lossy().as_ref()));
}

/// Verifies file write failures retain path and I/O context.
#[test]
fn reports_touchstone_file_write_errors() {
    let directory = temporary_touchstone_path("directory");
    fs::create_dir_all(&directory).unwrap();

    let error = write_touchstone(
        &directory,
        &one_port_block(),
        TouchstoneFormat::RealImaginary,
    )
    .unwrap_err();
    fs::remove_dir(&directory).unwrap();

    assert!(matches!(error, SolveError::TouchstoneWriteFailed { .. }));
    assert!(
        error
            .to_string()
            .contains(directory.to_string_lossy().as_ref())
    );
}

/// Verifies the writer rejects empty, inconsistent, and non-finite blocks.
#[test]
fn serializer_rejects_invalid_blocks() {
    let mut empty = one_port_block();
    empty.points.clear();

    let mut incomplete_two_port = two_port_block();
    incomplete_two_port.points[0].s21 = None;

    let mut non_finite = one_port_block();
    non_finite.points[0].frequency_hz = f64::INFINITY;

    let mut one_port_noise = one_port_block();
    one_port_noise.noise = two_port_block().noise;

    for block in [empty, incomplete_two_port, non_finite, one_port_noise] {
        let error = serialize_touchstone(&block, TouchstoneFormat::RealImaginary).unwrap_err();
        assert!(matches!(error, SolveError::TouchstoneWriteFailed { .. }));
    }
}

/// Verifies writer rules that are specific to units, references, and DB encoding.
#[test]
fn serializer_rejects_unrepresentable_output_options() {
    let mut tera_hertz = one_port_block();
    tera_hertz.source_frequency_unit = ScalarUnit::TeraHertz;

    let mut mismatched_references = one_port_block();
    mismatched_references.port_reference_impedances_ohm = vec![50.0];

    let mut zero_decibels = one_port_block();
    zero_decibels.points[0].s11 = Complex::ZERO;

    assert!(serialize_touchstone(&tera_hertz, TouchstoneFormat::RealImaginary).is_err());
    assert!(serialize_touchstone(&mismatched_references, TouchstoneFormat::RealImaginary).is_err());
    assert!(serialize_touchstone(&zero_decibels, TouchstoneFormat::DecibelAngle).is_err());
}

/// Verifies duplicate output frequencies are rejected instead of emitting invalid data.
#[test]
fn serializer_rejects_duplicate_frequencies() {
    let mut block = one_port_block();
    block.points.push(block.points[0].clone());

    let error = serialize_touchstone(&block, TouchstoneFormat::RealImaginary).unwrap_err();
    assert!(matches!(error, SolveError::TouchstoneWriteFailed { .. }));
}
