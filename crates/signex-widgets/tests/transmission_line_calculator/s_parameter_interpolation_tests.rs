use super::*;

/// Asserts that two scalar values are numerically close.
fn assert_close(left: f64, right: f64) {
    assert!((left - right).abs() < 1.0e-9, "{left} != {right}");
}

/// Asserts that two complex values are numerically close.
fn assert_complex_close(left: Complex, right: Complex) {
    assert_close(left.re, right.re);
    assert_close(left.im, right.im);
}

/// Creates a two-port block with two Cartesian samples.
fn two_sample_block() -> SParameterBlock {
    parse_touchstone(
        "# GHz S RI R 50
1.0 0.0 0.0 1.0 0.0 0.1 0.0 0.2 0.0
2.0 1.0 1.0 3.0 -1.0 0.3 0.2 0.4 -0.2",
    )
    .unwrap()
}

/// Verifies Cartesian-linear interpolation of every two-port S-parameter.
#[test]
fn s_parameter_block_interpolates_all_two_port_values() {
    let block = two_sample_block();

    let point = block.interpolate(1.5e9).unwrap();

    assert_close(point.frequency_hz, 1.5e9);
    assert_complex_close(point.s11, Complex::new(0.5, 0.5));
    assert_complex_close(point.s21.unwrap(), Complex::new(2.0, -0.5));
    assert_complex_close(point.s12.unwrap(), Complex::new(0.2, 0.1));
    assert_complex_close(point.s22.unwrap(), Complex::new(0.3, -0.1));
    assert_complex_close(
        point.z_s11,
        reflection_to_impedance(point.s11, block.reference_impedance_ohm()),
    );
}

/// Verifies that interpolation clamps requests outside the measured frequency range.
#[test]
fn s_parameter_block_clamps_outside_frequency_range() {
    let block = two_sample_block();

    let below = block.interpolate(0.5e9).unwrap();
    let above = block.interpolate(2.5e9).unwrap();

    assert_close(below.frequency_hz, 1.0e9);
    assert_complex_close(below.s11, Complex::ZERO);
    assert_close(above.frequency_hz, 2.0e9);
    assert_complex_close(above.s11, Complex::new(1.0, 1.0));
}

/// Verifies that interpolation does not depend on input sample order.
#[test]
fn s_parameter_block_interpolates_unsorted_samples() {
    let original = two_sample_block();
    let mut points = original.points();
    points.reverse();
    let block = SParameterBlock::from_samples(
        SParameterKind::S2P,
        original.port_reference_impedances_ohm(),
        original.source_frequency_unit,
        points,
        original.noise(),
        original.raw.clone(),
    )
    .unwrap();

    let point = block.interpolate(1.25e9).unwrap();

    assert_close(point.frequency_hz, 1.25e9);
    assert_complex_close(point.s11, Complex::new(0.25, 0.25));
    assert_complex_close(point.s21.unwrap(), Complex::new(1.5, -0.25));
}

/// Verifies that one-port interpolation keeps unavailable matrix entries absent.
#[test]
fn one_port_interpolation_keeps_two_port_values_absent() {
    let block = parse_touchstone(
        "# GHz S RI R 50
1.0 0.0 0.0
2.0 0.4 -0.2",
    )
    .unwrap();

    let point = block.interpolate(1.5e9).unwrap();

    assert_complex_close(point.s11, Complex::new(0.2, -0.1));
    assert_eq!(point.s21, None);
    assert_eq!(point.s12, None);
    assert_eq!(point.s22, None);
    assert_eq!(point.s_parameter_matrix(), None);
}

/// Verifies that invalid or empty interpolation requests return no sample.
#[test]
fn s_parameter_interpolation_rejects_invalid_requests() {
    let block = two_sample_block();
    assert_eq!(block.interpolate(f64::NAN), None);

    let empty = SParameterBlock::from_samples(
        SParameterKind::S2P,
        vec![50.0, 50.0],
        ScalarUnit::GigaHertz,
        Vec::new(),
        Vec::new(),
        String::new(),
    )
    .unwrap();
    assert_eq!(empty.interpolate(1.0e9), None);
}

/// Verifies that Smith-chart gain analysis uses interpolated bilateral S-parameters.
#[test]
fn smith_chart_gain_uses_exact_bilateral_transducer_gain() {
    let block = parse_touchstone(
        "# GHz S RI R 50
1.0 0.2 0.0 2.0 0.0 0.1 0.0 0.3 0.0
2.0 0.2 0.0 2.0 0.0 0.1 0.0 0.3 0.0",
    )
    .unwrap();
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0 * 1.1 / 0.9, 0.0),
            tolerance_percent: None,
        },
        SmithChartElement::SParameter(block),
        SmithChartElement::LoadTermination {
            impedance: Complex::new(75.0, 0.0),
            tolerance_percent: None,
        },
    ];

    let analysis = analyze_smith_chart(
        &circuit,
        SmithChartSettings {
            frequency_hz: 1.5e9,
            span_hz: 0.0,
            ..SmithChartSettings::default()
        },
    )
    .unwrap();

    assert_close(analysis.s_parameter_gain.len() as f64, 1.0);
    assert_close(analysis.s_parameter_gain[0].frequency_hz, 1.5e9);
    assert_close(
        analysis.s_parameter_gain[0].transducer_gain_linear,
        4.51895822797498,
    );
}
