use super::*;

/// Asserts that close.
fn assert_close(left: f64, right: f64) {
    assert!((left - right).abs() < 1.0e-9, "{left} != {right}");
}

/// Verifies that matched load has zero reflection.
#[test]
fn matched_load_has_zero_reflection() {
    let result = solve(Complex::new(50.0, 0.0), &[], SolveSettings::default()).unwrap();

    assert_close(result.reflection_coefficient.re, 0.0);
    assert_close(result.reflection_coefficient.im, 0.0);
    assert_close(result.vswr, 1.0);
}

/// Verifies that public solver rejects non finite settings and elements.
#[test]
fn public_solver_rejects_non_finite_settings_and_elements() {
    let settings = SolveSettings {
        frequency_hz: f64::NAN,
        ..SolveSettings::default()
    };
    assert_eq!(
        solve(Complex::new(50.0, 0.0), &[], settings),
        Err(SolveError::NonPositiveFrequency)
    );

    let element = CircuitElement {
        name: "Invalid resistor".to_string(),
        kind: ElementKind::SeriesResistor,
        value: f64::INFINITY,
        enabled: true,
    };
    assert_eq!(
        solve(
            Complex::new(50.0, 0.0),
            &[element],
            SolveSettings::default()
        ),
        Err(SolveError::NonPositiveElementValue {
            kind: ElementKind::SeriesResistor,
        })
    );
}

/// Verifies that reflection round trips to impedance.
#[test]
fn reflection_round_trips_to_impedance() {
    let impedance = Complex::new(73.0, 31.0);
    let gamma = impedance_to_reflection(impedance, 50.0);
    let restored = reflection_to_impedance(gamma, 50.0);

    assert_close(restored.re, impedance.re);
    assert_close(restored.im, impedance.im);
}

/// Verifies that ideal open and short loads map to the Smith-chart boundary.
#[test]
fn open_and_short_loads_have_boundary_reflections() {
    let open = impedance_to_reflection(Complex::new(f64::INFINITY, 0.0), 50.0);
    let short = impedance_to_reflection(Complex::ZERO, 50.0);

    assert_close(open.re, 1.0);
    assert_close(open.im, 0.0);
    assert_close(short.re, -1.0);
    assert_close(short.im, 0.0);
}

/// Verifies that series inductor adds positive reactance.
#[test]
fn series_inductor_adds_positive_reactance() {
    let element = CircuitElement::new("L1", ElementKind::SeriesInductor, 10.0e-9);
    let result = solve(
        Complex::new(50.0, 0.0),
        &[element],
        SolveSettings {
            frequency_hz: 1.0e9,
            ..SolveSettings::default()
        },
    )
    .unwrap();

    assert_close(result.impedance.re, 50.0);
    assert_close(result.impedance.im, TAU * 1.0e9 * 10.0e-9);
}

/// Verifies that shunt resistor parallel combines.
#[test]
fn shunt_resistor_parallel_combines() {
    let element = CircuitElement::new("Rsh", ElementKind::ShuntResistor, 100.0);
    let result = solve(
        Complex::new(100.0, 0.0),
        &[element],
        SolveSettings::default(),
    )
    .unwrap();

    assert_close(result.impedance.re, 50.0);
    assert_close(result.impedance.im, 0.0);
}

/// Verifies that half wave transmission line preserves load.
#[test]
fn half_wave_transmission_line_preserves_load() {
    let settings = SolveSettings {
        frequency_hz: 1.0e9,
        reference_impedance_ohm: 50.0,
        velocity_factor: 1.0,
    };
    let half_wave_m = SPEED_OF_LIGHT_M_PER_S / (2.0 * settings.frequency_hz);
    let line = CircuitElement::new("TL", ElementKind::TransmissionLine, half_wave_m);
    let result = solve(Complex::new(30.0, 10.0), &[line], settings).unwrap();

    assert_close(result.impedance.re, 30.0);
    assert_close(result.impedance.im, 10.0);
}

/// Verifies that a quarter-wave line performs the expected impedance inversion.
#[test]
fn quarter_wave_transmission_line_inverts_load() {
    let settings = SolveSettings {
        frequency_hz: 1.0e9,
        reference_impedance_ohm: 50.0,
        velocity_factor: 1.0,
    };
    let quarter_wave_m = SPEED_OF_LIGHT_M_PER_S / (4.0 * settings.frequency_hz);
    let line = CircuitElement::new("TL", ElementKind::TransmissionLine, quarter_wave_m);
    let result = solve(Complex::new(100.0, 0.0), &[line], settings).unwrap();

    assert_close(result.impedance.re, 25.0);
    assert_close(result.impedance.im, 0.0);
}

/// Verifies that a half-wave open stub retains the singular-network error.
#[test]
fn half_wave_open_stub_is_singular() {
    let settings = SolveSettings {
        frequency_hz: 1.0e9,
        reference_impedance_ohm: 50.0,
        velocity_factor: 1.0,
    };
    let half_wave_m = SPEED_OF_LIGHT_M_PER_S / (2.0 * settings.frequency_hz);
    let stub = CircuitElement::new("Stub", ElementKind::OpenStub, half_wave_m);

    assert_eq!(
        solve(Complex::new(50.0, 0.0), &[stub], settings),
        Err(SolveError::SingularNetwork {
            kind: ElementKind::OpenStub,
        })
    );
}

/// Verifies that smith chart analysis emits s2p stability circles.
#[test]
fn smith_chart_analysis_emits_s2p_stability_circles() {
    let block = parse_touchstone(
        "# GHz S MA R 50
1.0 0.5 0 2.0 90 0.1 0 0.4 -45
1.4 0.6 10 1.8 80 0.08 -5 0.35 -35",
    )
    .unwrap();
    let circuit = vec![SmithChartElement::SParameter(block)];
    let result = analyze_smith_chart(
        &circuit,
        SmithChartSettings {
            frequency_hz: 1.0e9,
            reference_impedance_ohm: 50.0,
            span_hz: 0.5e9,
            ..SmithChartSettings::default()
        },
    )
    .unwrap();

    assert_eq!(result.stability_circles.len(), 2);
    assert!(
        result
            .stability_circles
            .iter()
            .all(|circle| circle.source_radius.is_finite() && circle.load_radius.is_finite())
    );
}

/// Verifies that smith chart analysis emits s2p gain and noise circles.
#[test]
fn smith_chart_analysis_emits_s2p_gain_and_noise_circles() {
    let block = parse_touchstone(
        "# GHz S MA R 50
1.0 0.5 0 2.0 90 0.1 0 0.4 -45
! Noise parameters
1.0 1.0 0.25 35 0.1",
    )
    .unwrap();
    let circuit = vec![SmithChartElement::SParameter(block)];
    let settings = SmithChartSettings {
        frequency_hz: 1.0e9,
        reference_impedance_ohm: 50.0,
        span_hz: 0.0,
        ..SmithChartSettings::default()
    };

    let gain = solve_s_parameter_gain_circles(&circuit, &settings, &[1.0], &[0.5]);
    let noise = solve_noise_figure_circles(&circuit, &settings, &[1.5]);

    assert_eq!(gain.len(), 2);
    assert!(gain.iter().all(|circle| circle.radius.is_finite()));
    assert_close(gain[0].center.re, 0.478776675162869);
    assert_close(gain[0].center.im, 0.0);
    assert_close(gain[0].radius, 0.179681431352376);
    assert_eq!(noise.len(), 1);
    assert_close(noise[0].center.re, 0.130828054247831);
    assert_close(noise[0].center.im, 0.0916067897936398);
    assert_close(noise[0].radius, 0.588840926050854);

    assert!(solve_s_parameter_gain_circles(&circuit, &settings, &[2.0], &[]).is_empty());
    assert!(solve_noise_figure_circles(&circuit, &settings, &[0.5, f64::NAN]).is_empty());
}

/// Verifies that smith chart analysis interpolates the active S-parameter point for zero span.
#[test]
fn smith_chart_analysis_interpolates_active_s_parameter_point_for_zero_span() {
    let block = parse_touchstone(
        "# GHz S MA R 50
0.8 0.4 0 1.8 90 0.05 0 0.3 -45
1.0 0.5 0 2.0 90 0.1 0 0.4 -45
1.2 0.6 0 2.2 90 0.15 0 0.5 -45",
    )
    .unwrap();
    let circuit = vec![SmithChartElement::SParameter(block)];
    let result = analyze_smith_chart(
        &circuit,
        SmithChartSettings {
            frequency_hz: 0.9e9,
            reference_impedance_ohm: 50.0,
            span_hz: 0.0,
            ..SmithChartSettings::default()
        },
    )
    .unwrap();

    assert_close(result.active_frequency_hz, 0.9e9);
    assert_eq!(result.frequency_results.len(), 1);
    assert_close(result.frequency_results[0].frequency_hz, 0.9e9);
    assert_close(result.frequency_results[0].reflection_coefficient.re, 0.45);
    assert_eq!(result.s_parameter_gain.len(), 1);
    assert_close(result.s_parameter_gain[0].frequency_hz, 0.9e9);
    assert_eq!(result.stability_circles.len(), 1);
    assert_close(result.stability_circles[0].frequency_hz, 0.9e9);
}

/// Verifies that smith chart analysis emits s2p gain variant matrix.
#[test]
fn smith_chart_analysis_emits_s2p_gain_variant_matrix() {
    let block = parse_touchstone(
        "# GHz S MA R 50
1.0 0.5 0 2.0 90 0.1 0 0.4 -45",
    )
    .unwrap();
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: None,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 10.0,
            esl_h: 0.0,
            tolerance_percent: Some(10.0),
        },
        SmithChartElement::SParameter(block),
        SmithChartElement::LoadTermination {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: Some(5.0),
        },
    ];
    let result = analyze_smith_chart(
        &circuit,
        SmithChartSettings {
            frequency_hz: 1.0e9,
            reference_impedance_ohm: 50.0,
            span_hz: 0.0,
            ..SmithChartSettings::default()
        },
    )
    .unwrap();

    assert_eq!(result.s_parameter_gain.len(), 1);
    assert_eq!(result.s_parameter_gain_variants.len(), 9);
    assert!(
        result
            .s_parameter_gain_variants
            .iter()
            .all(|trace| trace.len() == 1)
    );
}

/// Verifies that smith chart analysis emits s1p reflection variants.
#[test]
fn smith_chart_analysis_emits_s1p_reflection_variants() {
    let block = parse_touchstone(
        "# GHz S MA R 50
1.0 0.5 0",
    )
    .unwrap();
    let circuit = vec![
        SmithChartElement::SParameter(block),
        SmithChartElement::SeriesResistor {
            resistance_ohm: 10.0,
            esl_h: 0.0,
            tolerance_percent: Some(10.0),
        },
        SmithChartElement::LoadTermination {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: None,
        },
    ];
    let result = analyze_smith_chart(
        &circuit,
        SmithChartSettings {
            frequency_hz: 1.0e9,
            reference_impedance_ohm: 50.0,
            span_hz: 0.0,
            ..SmithChartSettings::default()
        },
    )
    .unwrap();

    assert_eq!(result.s1p_reflection_variants.len(), 3);
    assert!(
        result
            .s1p_reflection_variants
            .iter()
            .all(|trace| trace.len() == 1)
    );
    let nominal = result.s1p_reflection_variants.last().unwrap()[0].reflection_coefficient;
    assert_close(nominal.re, 110.0 / 210.0);
    assert_close(nominal.im, 0.0);
}

/// Verifies that smith chart analysis uses noise parameter frequencies for noise outputs.
#[test]
fn smith_chart_analysis_uses_noise_parameter_frequencies_for_noise_outputs() {
    let block = parse_touchstone(
        "# GHz S MA R 50
1.0 0.5 0 2.0 90 0.1 0 0.4 -45
1.4 0.55 0 2.2 90 0.1 0 0.4 -45
1.8 0.58 0 2.3 90 0.1 0 0.4 -45
2.0 0.6 0 2.5 90 0.1 0 0.4 -45
! Noise parameters
1.4 1.0 0.25 35 0.1
1.8 1.2 0.30 45 0.1",
    )
    .unwrap();
    assert_eq!(block.noise().len(), 2);
    let active_frequency =
        select_active_frequency(&[SmithChartElement::SParameter(block.clone())], 1.0e9);
    assert_close(active_frequency, 1.0e9);
    assert_eq!(
        noise_frequency_samples(
            &block,
            &SmithChartSettings {
                frequency_hz: 1.0e9,
                reference_impedance_ohm: 50.0,
                span_hz: 0.5e9,
                ..SmithChartSettings::default()
            },
            active_frequency
        )
        .len(),
        1
    );
    let circuit = vec![SmithChartElement::SParameter(block)];
    assert_eq!(
        solve_noise_figure(
            &circuit,
            &SmithChartSettings {
                frequency_hz: 1.0e9,
                reference_impedance_ohm: 50.0,
                span_hz: 0.5e9,
                ..SmithChartSettings::default()
            },
            active_frequency
        )
        .unwrap()
        .len(),
        1
    );
    let result = analyze_smith_chart(
        &circuit,
        SmithChartSettings {
            frequency_hz: 1.0e9,
            reference_impedance_ohm: 50.0,
            span_hz: 0.5e9,
            ..SmithChartSettings::default()
        },
    )
    .unwrap();

    assert_eq!(result.noise_figure.len(), 1);
    assert_close(result.noise_figure[0].frequency_hz, 1.4e9);

    let circles = solve_noise_figure_circles(
        &circuit,
        &SmithChartSettings {
            frequency_hz: 0.9e9,
            reference_impedance_ohm: 50.0,
            ..SmithChartSettings::default()
        },
        &[1.5],
    );
    assert_eq!(circles.len(), 1);
    assert_close(circles[0].frequency_hz, 1.4e9);
}

/// Verifies that smith chart analysis interpolates custom impedance.
#[test]
fn smith_chart_analysis_interpolates_custom_impedance() {
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: None,
        },
        SmithChartElement::Custom {
            interpolation: CustomInterpolation::Linear,
            points: vec![
                CustomPoint {
                    frequency_hz: 1.0e9,
                    impedance: Complex::new(0.0, 0.0),
                },
                CustomPoint {
                    frequency_hz: 2.0e9,
                    impedance: Complex::new(20.0, 10.0),
                },
            ],
        },
    ];
    let result = analyze_smith_chart(
        &circuit,
        SmithChartSettings {
            frequency_hz: 1.5e9,
            reference_impedance_ohm: 50.0,
            ..SmithChartSettings::default()
        },
    )
    .unwrap();

    assert_close(result.nominal.impedance.re, 60.0);
    assert_close(result.nominal.impedance.im, 5.0);
}

/// Verifies that smith chart analysis expands tolerance variants.
#[test]
fn smith_chart_analysis_expands_tolerance_variants() {
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: Some(10.0),
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 10.0,
            esl_h: 0.0,
            tolerance_percent: Some(5.0),
        },
    ];
    let result = analyze_smith_chart(&circuit, SmithChartSettings::default()).unwrap();

    assert_eq!(result.tolerance_results.len(), 4);
    let mut impedance_re = result
        .tolerance_results
        .iter()
        .map(|result| format_number(result.impedance.re))
        .collect::<Vec<_>>();
    impedance_re.sort();
    assert_eq!(impedance_re, ["54.5", "55.5", "64.5", "65.5"]);
}

/// Verifies that smith chart analysis expands tolerance frequency variants.
#[test]
fn smith_chart_analysis_expands_tolerance_frequency_variants() {
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: Some(10.0),
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 10.0,
            esl_h: 0.0,
            tolerance_percent: Some(5.0),
        },
    ];
    let result = analyze_smith_chart(
        &circuit,
        SmithChartSettings {
            frequency_hz: 1.0e9,
            span_hz: 1.0e6,
            resolution: 1,
            ..SmithChartSettings::default()
        },
    )
    .unwrap();

    assert_eq!(result.frequency_result_variants.len(), 5);
    assert!(
        result
            .frequency_result_variants
            .iter()
            .all(|trace| trace.len() == 3)
    );
    let mut variant_impedance_re = result
        .frequency_result_variants
        .iter()
        .take(4)
        .map(|trace| format_number(trace[1].impedance.re))
        .collect::<Vec<_>>();
    variant_impedance_re.sort();
    assert_eq!(variant_impedance_re, ["54.5", "55.5", "64.5", "65.5"]);
    assert_close(
        result.frequency_result_variants.last().unwrap()[1]
            .impedance
            .re,
        60.0,
    );
    assert_eq!(
        result.frequency_results,
        *result.frequency_result_variants.last().unwrap()
    );
}

/// Verifies that runtime adjustments match cm slider scaling.
#[test]
fn runtime_adjustments_match_cm_slider_scaling() {
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 10.0),
            tolerance_percent: None,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 10.0,
            esl_h: 0.0,
            tolerance_percent: None,
        },
    ];
    let adjustments = vec![
        RuntimeAdjustment {
            real_slider_percent: Some(10.0),
            imaginary_slider_percent: Some(-50.0),
            ..RuntimeAdjustment::default()
        },
        RuntimeAdjustment {
            value_slider_percent: Some(50.0),
            ..RuntimeAdjustment::default()
        },
    ];

    let adjusted = apply_runtime_adjustments(&circuit, &adjustments);
    match adjusted[0] {
        SmithChartElement::BlackBox { impedance, .. } => {
            assert_close(impedance.re, 55.0);
            assert_close(impedance.im, 5.0);
        }
        _ => panic!("expected adjusted black box"),
    }
    match adjusted[1] {
        SmithChartElement::SeriesResistor { resistance_ohm, .. } => {
            assert_close(resistance_ohm, 15.0);
        }
        _ => panic!("expected adjusted series resistor"),
    }

    let result = analyze_smith_chart_with_runtime_adjustments(
        &circuit,
        SmithChartSettings::default(),
        &adjustments,
    )
    .unwrap();
    assert_close(result.nominal.impedance.re, 70.0);
    assert_close(result.nominal.impedance.im, 5.0);
}

/// Verifies that runtime adjustments apply before tolerance variants.
#[test]
fn runtime_adjustments_apply_before_tolerance_variants() {
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: None,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 10.0,
            esl_h: 0.0,
            tolerance_percent: Some(10.0),
        },
    ];
    let adjustments = vec![
        RuntimeAdjustment::default(),
        RuntimeAdjustment {
            value_slider_percent: Some(100.0),
            ..RuntimeAdjustment::default()
        },
    ];

    let result = analyze_smith_chart_with_runtime_adjustments(
        &circuit,
        SmithChartSettings::default(),
        &adjustments,
    )
    .unwrap();
    let mut variant_re = result
        .tolerance_results
        .iter()
        .map(|result| format_number(result.impedance.re))
        .collect::<Vec<_>>();
    variant_re.sort();
    assert_eq!(variant_re, ["68", "72"]);
}

/// Verifies that ideal transformer scales impedance by ratio squared.
#[test]
fn ideal_transformer_scales_impedance_by_ratio_squared() {
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(25.0, 5.0),
            tolerance_percent: None,
        },
        SmithChartElement::Transformer {
            model: TransformerModel::Ideal,
            l1_h: 0.0,
            l2_h: 0.0,
            coupling_or_turns_ratio: 2.0,
        },
    ];
    let result = analyze_smith_chart(&circuit, SmithChartSettings::default()).unwrap();

    assert_close(result.nominal.impedance.re, 100.0);
    assert_close(result.nominal.impedance.im, 20.0);
}

/// Verifies that smith chart analysis uses settings reference impedance.
#[test]
fn smith_chart_analysis_uses_settings_reference_impedance() {
    let circuit = vec![SmithChartElement::LoadTermination {
        impedance: Complex::new(75.0, 0.0),
        tolerance_percent: None,
    }];
    let result = analyze_smith_chart(
        &circuit,
        SmithChartSettings {
            reference_impedance_ohm: 75.0,
            ..SmithChartSettings::default()
        },
    )
    .unwrap();

    assert_close(result.nominal.normalized_impedance.re, 1.0);
    assert_close(result.nominal.reflection_coefficient.magnitude(), 0.0);
}

/// Verifies that smith chart analysis emits element impedance arcs for nominal and tolerance variants.
#[test]
fn smith_chart_analysis_emits_element_impedance_arcs_for_nominal_and_tolerance_variants() {
    let circuit = vec![
        SmithChartElement::BlackBox {
            impedance: Complex::new(25.0, 5.0),
            tolerance_percent: None,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 10.0,
            esl_h: 0.0,
            tolerance_percent: Some(10.0),
        },
        SmithChartElement::ShuntCapacitor {
            capacitance_f: 2.0e-12,
            esr_ohm: 0.0,
            esl_h: 0.0,
            tolerance_percent: None,
        },
        SmithChartElement::TransmissionLine {
            length_m: 0.01,
            characteristic_impedance_ohm: 50.0,
            effective_dielectric: 1.0,
            tolerance_percent: None,
        },
    ];

    let result = analyze_smith_chart(
        &circuit,
        SmithChartSettings {
            frequency_hz: 1.0e9,
            reference_impedance_ohm: 50.0,
            ..SmithChartSettings::default()
        },
    )
    .unwrap();

    assert_eq!(result.tolerance_results.len(), 2);
    assert_eq!(result.impedance_arcs.len(), 9);
    let nominal_arcs = result
        .impedance_arcs
        .iter()
        .filter(|arc| arc.variant_index == 0)
        .collect::<Vec<_>>();
    assert_eq!(nominal_arcs.len(), 3);
    assert!(nominal_arcs.iter().all(|arc| {
        arc.points.len() == WEBSITE_IMPEDANCE_ARC_SEGMENTS + 1
            && arc
                .points
                .iter()
                .all(|point| point.re.is_finite() && point.im.is_finite())
    }));
    assert_eq!(nominal_arcs[0].element_index, 1);
    assert_eq!(nominal_arcs[0].element_name, "Series Resistor");
    assert_close(nominal_arcs[0].points[0].re, 25.0);
    assert_close(nominal_arcs[0].points.last().unwrap().re, 35.0);
    assert_close(
        nominal_arcs.last().unwrap().points.last().unwrap().re,
        result.nominal.impedance.re,
    );
    assert_close(
        nominal_arcs.last().unwrap().points.last().unwrap().im,
        result.nominal.impedance.im,
    );
}

/// Verifies that compact circuit tokens preserve repeated ordered rows.
#[test]
fn compact_circuit_tokens_preserve_repeated_ordered_rows() {
    let circuit = vec![
        SmithChartElement::LoadTermination {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: None,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 5.0,
            esl_h: 0.0,
            tolerance_percent: None,
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 7.0,
            esl_h: 0.0,
            tolerance_percent: Some(1.0),
        },
        SmithChartElement::ShuntCapacitor {
            capacitance_f: 2.0e-12,
            esr_ohm: 0.0,
            esl_h: 0.0,
            tolerance_percent: None,
        },
        SmithChartElement::TransmissionLine {
            length_m: 0.01,
            characteristic_impedance_ohm: 75.0,
            effective_dielectric: 2.2,
            tolerance_percent: None,
        },
        SmithChartElement::ShuntResistor {
            resistance_ohm: 100.0,
            esl_h: 0.0,
            tolerance_percent: None,
        },
    ];

    let tokens = serialize_circuit_tokens(&circuit);
    assert!(tokens.contains("transmissionLine"));
    assert!(tokens.contains("__shortedRes"));
    assert_eq!(split_circuit_tokens(&tokens).len(), circuit.len());
    let legacy_blank_tokens = "loadTerm_50_0___transmissionLine_10_mm__75_2.2__shortedRes_100_ohm_";
    assert_eq!(split_circuit_tokens(legacy_blank_tokens).len(), 3);
}

/// Verifies that smith chart SVG export contains chart and result marker.
#[test]
fn smith_chart_svg_export_contains_chart_and_result_marker() {
    let result = solve(
        Complex::new(75.0, 25.0),
        &[],
        SolveSettings {
            frequency_hz: 1.0e9,
            reference_impedance_ohm: 50.0,
            velocity_factor: 1.0,
        },
    )
    .unwrap();
    let svg = render_smith_chart_svg(
        Some(&result),
        SmithChartSvgOptions {
            width: 640.0,
            height: 480.0,
            reference_impedance_ohm: 50.0,
            show_grid: true,
            show_admittance: true,
            show_vswr: true,
            show_q: true,
            resistance_labels: vec![0.25, 1.0, 3.0],
            reactance_labels: vec![-3.0, 1.0, 3.0],
            z_markers: vec![Complex::new(25.0, 10.0)],
            vswr_circles: vec![2.0],
            q_circles: vec![3.0],
            stability_circles: vec![StabilityCircle {
                frequency_hz: 1.0e9,
                source_center: Complex::new(0.1, 0.2),
                source_radius: 0.3,
                load_center: Complex::new(-0.2, -0.1),
                load_radius: 0.25,
            }],
            gain_circles: vec![GainCircle {
                frequency_hz: 1.0e9,
                port: GainCirclePort::Input,
                target_gain_db: 1.0,
                center: Complex::new(0.2, -0.1),
                radius: 0.2,
            }],
            noise_figure_circles: vec![NoiseFigureCircle {
                frequency_hz: 1.0e9,
                target_noise_figure_db: 1.5,
                center: Complex::new(-0.1, 0.15),
                radius: 0.18,
            }],
            impedance_arc_traces: vec![SmithChartSvgTrace {
                label: "Series Resistor".to_string(),
                color: "#ee5c52".to_string(),
                points: vec![Complex::new(-0.1, 0.05), Complex::new(0.0, 0.1)],
            }],
            s_parameter_traces: vec![SmithChartSvgTrace {
                label: "S11".to_string(),
                color: "#0072b2".to_string(),
                points: vec![Complex::new(0.1, 0.1), Complex::new(0.2, 0.15)],
            }],
        },
    );

    assert!(svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg""#));
    assert!(svg.contains(r#"aria-label="Smith chart""#));
    assert!(svg.contains(r#"clipPath id="smith-chart-disk""#));
    assert!(svg.contains(r#"id="smith-chart-grid""#));
    assert!(svg.contains(r#"id="admittance-smith-chart-grid""#));
    assert!(svg.contains(r#"data-grid-kind="resistance""#));
    assert!(svg.contains(r#"data-grid-kind="reactance""#));
    assert!(svg.contains(r#"data-grid-hierarchy="major""#));
    assert!(svg.contains(r#"data-grid-hierarchy="minor""#));
    assert!(svg.contains(r#"<polyline fill="none""#));
    assert!(svg.contains(r#"<text "#));
    assert!(svg.contains(">VSWR 2.0<"));
    assert!(svg.contains(">Q 3.0<"));
    assert!(!svg.contains(">VSWR 1.5<"));
    assert!(!svg.contains(">Q 0.5<"));
    assert!(svg.contains(">25.0+10.0j<"));
    assert!(svg.contains(r##"fill="#f4da76""##));
    assert!(svg.contains(">S11<"));
    assert!(svg.contains(r##"stroke="#0072b2""##));
    assert!(svg.contains(">Series Resistor<"));
    assert!(svg.contains(">1.0 dB in<"));
    assert!(svg.contains(">1.5 dB NF<"));
    assert!(svg.contains(">+1.0j<"));
    assert!(svg.contains(">3.0<"));
    assert!(svg.contains(">+3.0j<"));
    assert!(svg.contains(r##"stroke="#ff8a5c""##));
    assert!(svg.contains(r##"stroke="#7aa7ff""##));
    assert!(svg.contains(r##"fill="#ee5c52""##));
    assert!(svg.ends_with("</svg>"));
}

/// Verifies that length units convert wavelength and degrees.
#[test]
fn length_units_convert_wavelength_and_degrees() {
    let wavelength = length_to_meters(1.0, ScalarUnit::Wavelength, 1.0e9, 1.0).unwrap();
    let degrees = length_to_meters(180.0, ScalarUnit::Degree, 1.0e9, 1.0).unwrap();

    assert_close(wavelength, SPEED_OF_LIGHT_M_PER_S / 1.0e9);
    assert_close(degrees, SPEED_OF_LIGHT_M_PER_S / (2.0 * 1.0e9));
}
