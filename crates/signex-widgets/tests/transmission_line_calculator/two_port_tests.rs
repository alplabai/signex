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

/// Asserts that two S-parameter matrices are numerically close.
fn assert_s_parameters_close(left: SParameterMatrix, right: SParameterMatrix) {
    assert_complex_close(left.s11, right.s11);
    assert_complex_close(left.s12, right.s12);
    assert_complex_close(left.s21, right.s21);
    assert_complex_close(left.s22, right.s22);
}

/// Verifies S-to-ABCD round trips for unequal complex power-wave references.
#[test]
fn s_parameters_round_trip_with_complex_port_references() {
    let original = SParameterMatrix::new(
        Complex::new(0.1, 0.05),
        Complex::new(0.02, -0.01),
        Complex::new(2.0, 0.3),
        Complex::new(-0.2, 0.1),
    );
    let port_1_reference = Complex::new(50.0, 5.0);
    let port_2_reference = Complex::new(75.0, -10.0);

    let restored = original
        .to_abcd(port_1_reference, port_2_reference)
        .unwrap()
        .to_s_parameters(port_1_reference, port_2_reference)
        .unwrap();

    assert_s_parameters_close(restored, original);
}

/// Verifies exact bilateral transducer gain including reverse isolation feedback.
#[test]
fn s_parameter_matrix_computes_bilateral_transducer_gain() {
    let matrix = SParameterMatrix::new(
        Complex::new(0.2, 0.0),
        Complex::new(0.1, 0.0),
        Complex::new(2.0, 0.0),
        Complex::new(0.3, 0.0),
    );

    let gain = matrix
        .transducer_gain(Complex::new(0.1, 0.0), Complex::new(0.2, 0.0))
        .unwrap();

    assert_close(gain, 4.51895822797498);
}

/// Verifies that singular conversions and invalid references report errors.
#[test]
fn two_port_solver_reports_singular_and_invalid_inputs() {
    let reference = Complex::new(50.0, 0.0);
    let no_forward_transmission =
        SParameterMatrix::new(Complex::ZERO, Complex::ZERO, Complex::ZERO, Complex::ZERO);

    assert_eq!(
        no_forward_transmission.to_abcd(reference, reference),
        Err(TwoPortError::SingularSParameterMatrix)
    );
    assert_eq!(
        AbcdMatrix::new(Complex::ONE, Complex::ZERO, Complex::ZERO, Complex::ONE,)
            .to_s_parameters(Complex::ZERO, reference),
        Err(TwoPortError::InvalidReferenceImpedance)
    );
}

/// Verifies that a passive Smith-chart chain produces true solved S21.
#[test]
fn passive_smith_chart_chain_produces_two_port_s_parameters() {
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: None,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 25.0,
            esl_h: 0.0,
            tolerance_percent: None,
        },
    ];

    let points = solve_two_port_s_parameters(&circuit, &[1.0e9], false, 50.0).unwrap();

    assert_eq!(points.len(), 1);
    assert_complex_close(points[0].s_parameters.s11, Complex::new(0.2, 0.0));
    assert_complex_close(points[0].s_parameters.s21, Complex::new(0.8, 0.0));
}

/// Verifies that the rust-rf-backed solver preserves a matched quarter-wave line.
#[test]
fn passive_two_port_solver_preserves_quarter_wave_line() {
    let frequency_hz = 1.0e9;
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: None,
        },
        SmithChartElement::TransmissionLine {
            length_m: SPEED_OF_LIGHT_M_PER_S / (4.0 * frequency_hz),
            characteristic_impedance_ohm: 50.0,
            effective_dielectric: 1.0,
            tolerance_percent: None,
        },
    ];

    let point = solve_two_port_s_parameters(&circuit, &[frequency_hz], false, 50.0).unwrap()[0];

    assert_complex_close(point.s_parameters.s11, Complex::ZERO);
    assert_complex_close(point.s_parameters.s21, Complex::new(0.0, -1.0));
    assert_complex_close(point.s_parameters.s12, Complex::new(0.0, -1.0));
    assert_complex_close(point.s_parameters.s22, Complex::ZERO);
}

/// Verifies that one rust-rf network carries the complete frequency-dependent sweep.
#[test]
fn passive_two_port_solver_preserves_frequency_dependent_parasitics() {
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: None,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 25.0,
            esl_h: 10.0e-9,
            tolerance_percent: None,
        },
    ];
    let frequencies_hz = [1.0e6, 1.0e9];

    let points = solve_two_port_s_parameters(&circuit, &frequencies_hz, false, 50.0).unwrap();

    assert_eq!(points.len(), frequencies_hz.len());
    assert_eq!(points[0].frequency_hz, frequencies_hz[0]);
    assert_eq!(points[1].frequency_hz, frequencies_hz[1]);
    assert!(points[1].s_parameters.s21.im.abs() > points[0].s_parameters.s21.im.abs());
}

/// Verifies rust-rf line, stub, and transformer networks match the impedance walk.
#[test]
fn distributed_two_port_chain_matches_smith_chart_impedance_walk() {
    let frequency_hz = 1.0e9;
    let wavelength_m = SPEED_OF_LIGHT_M_PER_S / frequency_hz;
    let load = Complex::new(30.0, 10.0);
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: load,
            tolerance_percent: None,
        },
        SmithChartElement::TransmissionLine {
            length_m: wavelength_m / 8.0,
            characteristic_impedance_ohm: 60.0,
            effective_dielectric: 1.0,
            tolerance_percent: None,
        },
        SmithChartElement::OpenStub {
            length_m: wavelength_m / 10.0,
            characteristic_impedance_ohm: 75.0,
            effective_dielectric: 1.0,
            tolerance_percent: None,
        },
        SmithChartElement::ShortedStub {
            length_m: wavelength_m / 12.0,
            characteristic_impedance_ohm: 45.0,
            effective_dielectric: 1.0,
            tolerance_percent: None,
        },
        SmithChartElement::Transformer {
            model: TransformerModel::Ideal,
            l1_h: 0.0,
            l2_h: 0.0,
            coupling_or_turns_ratio: 1.5,
        },
    ];
    let settings = SmithChartSettings {
        frequency_hz,
        span_hz: 0.0,
        ..SmithChartSettings::default()
    };

    let analysis = analyze_smith_chart(&circuit, settings).unwrap();
    let s_parameters = solve_two_port_s_parameters(&circuit, &[frequency_hz], false, 50.0).unwrap()
        [0]
    .s_parameters;
    let abcd = s_parameters
        .to_abcd(Complex::new(50.0, 0.0), Complex::new(50.0, 0.0))
        .unwrap();

    assert_complex_close(
        abcd.input_impedance(load).unwrap(),
        analysis.nominal.impedance,
    );
}

/// Verifies that ABCD element order matches the existing load-to-source impedance walk.
#[test]
fn passive_two_port_input_impedance_matches_smith_chart_solver() {
    let load = Complex::new(30.0, 10.0);
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: load,
            tolerance_percent: None,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 10.0,
            esl_h: 0.0,
            tolerance_percent: None,
        },
        SmithChartElement::ShuntResistor {
            resistance_ohm: 100.0,
            esl_h: 0.0,
            tolerance_percent: None,
        },
    ];
    let settings = SmithChartSettings {
        frequency_hz: 1.0e9,
        span_hz: 0.0,
        ..SmithChartSettings::default()
    };

    let analysis = analyze_smith_chart(&circuit, settings).unwrap();
    let s_parameters = analysis.two_port_s_parameters[0].s_parameters;
    let abcd = s_parameters
        .to_abcd(Complex::new(50.0, 0.0), Complex::new(50.0, 0.0))
        .unwrap();

    assert_complex_close(
        abcd.input_impedance(load).unwrap(),
        analysis.nominal.impedance,
    );
}

/// Verifies that unsupported coupled transformers retain the caller's fallback path.
#[test]
fn passive_two_port_solver_skips_unsupported_coupled_transformer() {
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: None,
        },
        SmithChartElement::Transformer {
            model: TransformerModel::CoupledInductor,
            l1_h: 1.0e-9,
            l2_h: 2.0e-9,
            coupling_or_turns_ratio: 0.9,
        },
    ];

    let points = solve_two_port_s_parameters(&circuit, &[1.0e9], false, 50.0).unwrap();

    assert!(points.is_empty());
}
