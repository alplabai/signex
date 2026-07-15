use super::*;

fn assert_close(left: f64, right: f64) {
    assert!((left - right).abs() < 1.0e-9, "{left} != {right}");
}

#[test]
fn matched_load_has_zero_reflection() {
    let result = solve(Complex::new(50.0, 0.0), &[], SolveSettings::default()).unwrap();

    assert_close(result.reflection_coefficient.re, 0.0);
    assert_close(result.reflection_coefficient.im, 0.0);
    assert_close(result.vswr, 1.0);
}

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

#[test]
fn reflection_round_trips_to_impedance() {
    let impedance = Complex::new(73.0, 31.0);
    let gamma = impedance_to_reflection(impedance, 50.0);
    let restored = reflection_to_impedance(gamma, 50.0);

    assert_close(restored.re, impedance.re);
    assert_close(restored.im, impedance.im);
}

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

#[test]
fn touchstone_parser_accepts_s2p_ma_and_noise() {
    let raw = "# GHz S MA R 50
1.0 0.5 0 2.0 90 0.1 0 0.4 -45
! Noise parameters
1.0 1.2 0.3 45 0.1";

    let block = parse_touchstone(raw).unwrap();

    assert_eq!(block.kind, SParameterKind::S2P);
    assert_eq!(block.points.len(), 1);
    assert_close(block.points[0].frequency_hz, 1.0e9);
    assert_close(block.points[0].s21.unwrap().magnitude(), 2.0);
    assert_eq!(block.noise.len(), 1);
    assert_close(block.noise[0].rn_ohm, 5.0);
}

#[test]
fn touchstone_parser_rounds_frequency_keys_like_website() {
    let block = parse_touchstone(
        "# Hz S RI R 50
1000.49 0.1 0.0
1000.51 0.2 0.0
! Noise parameters
1000.49 1.0 0.1 0 0.05
1000.51 1.5 0.2 0 0.10",
    )
    .unwrap();

    assert_eq!(block.points.len(), 2);
    assert_close(block.points[0].frequency_hz, 1000.0);
    assert_close(block.points[1].frequency_hz, 1001.0);
    assert_eq!(block.noise.len(), 2);
    assert_close(block.noise[0].frequency_hz, 1000.0);
    assert_close(block.noise[1].frequency_hz, 1001.0);
}

#[test]
fn touchstone_parser_replaces_duplicate_frequency_rows_like_website() {
    let block = parse_touchstone(
        "# Hz S RI R 50
1000.49 0.1 0.0
1000.40 0.2 0.0
! Noise parameters
1000.49 1.0 0.1 0 0.05
1000.40 1.5 0.2 0 0.10",
    )
    .unwrap();

    assert_eq!(block.points.len(), 1);
    assert_close(block.points[0].frequency_hz, 1000.0);
    assert_close(block.points[0].s11.re, 0.2);
    assert_eq!(block.noise.len(), 1);
    assert_close(block.noise[0].frequency_hz, 1000.0);
    assert_close(block.noise[0].fmin_db, 1.5);
    assert_close(block.noise[0].rn_ohm, 5.0);
}

#[test]
fn smith_chart_analysis_emits_s2p_stability_circles() {
    let block = parse_touchstone(
        "# GHz S MA R 50
1.0 0.5 0 2.0 90 0.1 0 0.4 -45",
    )
    .unwrap();
    let circuit = vec![SmithChartElement::SParameter(block)];
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

    assert_eq!(result.stability_circles.len(), 1);
    assert!(result.stability_circles[0].source_radius.is_finite());
    assert!(result.stability_circles[0].load_radius.is_finite());
}

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
    assert_eq!(noise.len(), 1);
    assert!(noise[0].radius.is_finite());
}

#[test]
fn smith_chart_analysis_uses_single_active_s_parameter_point_for_zero_span() {
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

    assert_close(result.active_frequency_hz, 1.0e9);
    assert_eq!(result.frequency_results.len(), 1);
    assert_close(result.frequency_results[0].frequency_hz, 1.0e9);
    assert_eq!(result.s_parameter_gain.len(), 1);
    assert_close(result.s_parameter_gain[0].frequency_hz, 1.0e9);
    assert_eq!(result.stability_circles.len(), 1);
    assert_close(result.stability_circles[0].frequency_hz, 1.0e9);
}

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
    assert_eq!(block.noise.len(), 2);
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

#[test]
fn online_smith_chart_url_codec_round_trips_major_element_families() {
    let sparam = parse_touchstone(
        "# MHz S MA R 50
1000 0.1 0 2.0 0 0.02 0 0.1 180",
    )
    .unwrap();
    let circuit = vec![
        SmithChartElement::LoadTermination {
            impedance: Complex::new(40.0, 5.0),
            tolerance_percent: Some(2.0),
        },
        SmithChartElement::SeriesCapacitor {
            capacitance_f: 1.0e-12,
            esr_ohm: 0.1,
            esl_h: 0.2e-9,
            tolerance_percent: Some(5.0),
        },
        SmithChartElement::ShuntInductor {
            inductance_h: 10.0e-9,
            esr_ohm: 0.2,
            tolerance_percent: Some(10.0),
        },
        SmithChartElement::SeriesResistor {
            resistance_ohm: 12.5,
            esl_h: 0.0,
            tolerance_percent: None,
        },
        SmithChartElement::SeriesParallelRlc {
            resistance_ohm: 500.0,
            inductance_h: 3.0e-9,
            capacitance_f: 2.0e-12,
        },
        SmithChartElement::TransmissionLine {
            length_m: 0.01,
            characteristic_impedance_ohm: 75.0,
            effective_dielectric: 2.2,
            tolerance_percent: Some(1.0),
        },
        SmithChartElement::OpenStub {
            length_m: 0.005,
            characteristic_impedance_ohm: 50.0,
            effective_dielectric: 1.5,
            tolerance_percent: None,
        },
        SmithChartElement::ShortedStub {
            length_m: 0.004,
            characteristic_impedance_ohm: 45.0,
            effective_dielectric: 1.4,
            tolerance_percent: Some(3.0),
        },
        SmithChartElement::Transformer {
            model: TransformerModel::Ideal,
            l1_h: 0.0,
            l2_h: 0.0,
            coupling_or_turns_ratio: 2.0,
        },
        SmithChartElement::Transformer {
            model: TransformerModel::CoupledInductor,
            l1_h: 10.0e-9,
            l2_h: 12.0e-9,
            coupling_or_turns_ratio: 0.8,
        },
        SmithChartElement::Custom {
            interpolation: CustomInterpolation::Linear,
            points: vec![CustomPoint {
                frequency_hz: 1.0e9,
                impedance: Complex::new(42.0, -3.0),
            }],
        },
        SmithChartElement::Custom {
            interpolation: CustomInterpolation::SampleAndHold,
            points: vec![CustomPoint {
                frequency_hz: 2.0e9,
                impedance: Complex::new(52.0, 4.0),
            }],
        },
        SmithChartElement::SParameter(sparam),
    ];
    let settings = SmithChartSettings {
        frequency_hz: 1.0e9,
        frequency_unit: ScalarUnit::MegaHertz,
        reference_impedance_ohm: 75.0,
        span_hz: 10.0e6,
        span_unit: ScalarUnit::MegaHertz,
        resolution: 17,
        show_ideal: false,
    };
    let overlays = SmithChartOverlays {
        z_markers: vec![Complex::new(25.0, 10.0), Complex::new(80.0, -15.0)],
        vswr_circles: vec![1.5, 3.0],
        q_circles: vec![0.5, 1.0, 2.0],
        noise_figure_circles: vec![1.2],
        gain_input_circles: vec![8.0, 10.0],
        gain_output_circles: vec![6.0],
    };

    let query = serialize_online_smith_chart_query(&circuit, &settings, &overlays);
    assert!(query.contains("frequency=1000"));
    assert!(!query.contains("frequencyUnit="));
    assert!(query.contains("zo=75"));
    assert!(query.contains("fSpan=10"));
    assert!(!query.contains("fSpanUnit="));
    assert!(query.contains("fRes=17"));
    assert!(!query.contains("reference="));
    assert!(!query.contains("span="));
    assert!(!query.contains("resolution="));
    assert!(!query.contains("showIdeal="));
    assert!(!query.contains("_-_"));
    assert!(query.contains("custom_linear_%7B"));
    assert!(query.contains("custom_sah_%7B"));
    assert!(!query.contains("custom_sampleAndHold_%7B"));
    assert!(!query.contains("custom_stepped_%7B"));
    assert!(!query.contains("customZ"));
    assert!(query.contains("loadTerm_40_5_2"));
    assert!(query.contains("seriesCap_1_pF_5_0.1_0.0000000002"));
    assert!(query.contains("shortedInd_10_nH_10_0.2"));
    assert!(query.contains("seriesRes_12.5_%CE%A9_0_0"));
    assert!(query.contains("seriesRlc_500_%CE%A9_3_nH_2_pF"));
    assert!(query.contains("transmissionLine_10_mm_1_75_2.2"));
    assert!(!query.contains("_ohm_"));
    assert!(query.contains("stub_5_mm_0_50_1.5"));
    assert!(query.contains("shortedStub_4_mm_3_45_1.4"));
    assert!(query.contains("transformer_0_H_0_H_2_ideal"));
    assert!(query.contains("transformer_10_nH_12_nH_0.8_coupledInductor"));
    assert!(query.contains("sparam_s2p_MHz_50_1000_0.1_0_2_0_0.02_0_0.1_180"));
    let decoded = parse_online_smith_chart_query(&query).unwrap();

    assert_eq!(decoded.settings, settings);
    assert_eq!(decoded.overlays, overlays);
    assert_eq!(decoded.circuit.len(), circuit.len());
    assert_eq!(decoded.circuit[0], circuit[0]);
    assert_eq!(decoded.circuit[10], circuit[10]);
    assert_eq!(decoded.circuit[11], circuit[11]);
    assert!(matches!(
        decoded.circuit[12],
        SmithChartElement::SParameter(SParameterBlock {
            kind: SParameterKind::S2P,
            ..
        })
    ));
}

#[test]
fn online_smith_chart_url_serializer_omits_hm_defaults() {
    let default_circuit = vec![SmithChartElement::BlackBox {
        impedance: Complex::new(50.0, 0.0),
        tolerance_percent: None,
    }];
    let query = serialize_online_smith_chart_query(
        &default_circuit,
        &SmithChartSettings::default(),
        &SmithChartOverlays::default(),
    );
    assert_eq!(query, "");

    let tolerated_default_circuit = vec![SmithChartElement::BlackBox {
        impedance: Complex::new(50.0, 0.0),
        tolerance_percent: Some(5.0),
    }];
    let tolerated_query = serialize_online_smith_chart_query(
        &tolerated_default_circuit,
        &SmithChartSettings::default(),
        &SmithChartOverlays::default(),
    );
    assert_eq!(tolerated_query, "");

    let non_default_query = serialize_online_smith_chart_query(
        &default_circuit,
        &SmithChartSettings {
            frequency_hz: 1.0e9,
            span_hz: 10.0e6,
            resolution: 11,
            ..SmithChartSettings::default()
        },
        &SmithChartOverlays::default(),
    );
    assert_eq!(non_default_query, "frequency=1000&fSpan=10&fRes=11");
    let parsed = parse_online_smith_chart_query(&non_default_query).unwrap();
    assert_close(parsed.settings.frequency_hz, 1.0e9);
    assert_close(parsed.settings.span_hz, 10.0e6);
    assert_eq!(parsed.settings.resolution, 11);
    assert_eq!(parsed.circuit, default_circuit);
}

#[test]
fn online_smith_chart_url_codec_preserves_frequency_unit_settings() {
    let circuit = vec![SmithChartElement::BlackBox {
        impedance: Complex::new(25.0, 5.0),
        tolerance_percent: None,
    }];
    let settings = SmithChartSettings {
        frequency_hz: 2.4e9,
        frequency_unit: ScalarUnit::GigaHertz,
        span_hz: 125.0e3,
        span_unit: ScalarUnit::KiloHertz,
        resolution: 25,
        ..SmithChartSettings::default()
    };
    let query =
        serialize_online_smith_chart_query(&circuit, &settings, &SmithChartOverlays::default());

    assert!(query.contains("frequency=2.4"));
    assert!(query.contains("frequencyUnit=GHz"));
    assert!(query.contains("fSpan=125"));
    assert!(query.contains("fSpanUnit=kHz"));

    let parsed = parse_online_smith_chart_query(&query).unwrap();
    assert_close(parsed.settings.frequency_hz, 2.4e9);
    assert_eq!(parsed.settings.frequency_unit, ScalarUnit::GigaHertz);
    assert_close(parsed.settings.span_hz, 125.0e3);
    assert_eq!(parsed.settings.span_unit, ScalarUnit::KiloHertz);
}

#[test]
fn online_smith_chart_url_codec_preserves_s_parameter_source_frequency_unit() {
    let s_parameter = parse_touchstone(
        "# GHz S MA R 50
0.9 0.1 0
1 0.2 10
1.4 0.3 20",
    )
    .unwrap();
    assert_eq!(s_parameter.source_frequency_unit, ScalarUnit::GigaHertz);

    let query = serialize_online_smith_chart_query(
        &[SmithChartElement::SParameter(s_parameter)],
        &SmithChartSettings::default(),
        &SmithChartOverlays::default(),
    );

    assert!(query.contains("sparam_s1p_GHz_50_0.9_0.1_0_1_0.2_10_1.4_0.3_20"));
    let parsed = parse_online_smith_chart_query(&query).unwrap();
    match &parsed.circuit[0] {
        SmithChartElement::SParameter(block) => {
            assert_eq!(block.source_frequency_unit, ScalarUnit::GigaHertz);
            assert_close(block.points[1].frequency_hz, 1.0e9);
        }
        element => panic!("expected S-parameter block, got {element:?}"),
    }
}

#[test]
fn online_smith_chart_url_codec_does_not_persist_show_ideal() {
    let circuit = vec![SmithChartElement::BlackBox {
        impedance: Complex::new(50.0, 0.0),
        tolerance_percent: None,
    }];
    let settings = SmithChartSettings {
        show_ideal: true,
        ..SmithChartSettings::default()
    };

    let query =
        serialize_online_smith_chart_query(&circuit, &settings, &SmithChartOverlays::default());
    assert!(!query.contains("showIdeal="));

    let parsed = parse_online_smith_chart_query("showIdeal=1&circuit=blackBox_50_0_0").unwrap();
    assert!(!parsed.settings.show_ideal);
}

#[test]
fn online_smith_chart_url_serializer_uses_zero_for_missing_tolerance() {
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
        SmithChartElement::OpenStub {
            length_m: 0.01,
            characteristic_impedance_ohm: 50.0,
            effective_dielectric: 1.0,
            tolerance_percent: None,
        },
    ];

    let tokens = serialize_online_smith_chart_circuit_tokens(&circuit);
    assert_eq!(
        tokens,
        "blackBox_50_0_0__seriesRes_25_Ω_0_0__stub_10_mm_0_50_1"
    );
    assert!(!tokens.contains("-"));
    assert_eq!(
        parse_online_smith_chart_circuit_tokens(&tokens).unwrap(),
        circuit
    );

    let legacy_tokens = "blackBox_50_0_-__seriesRes_25_ohm_-_0__stub_0.01_m_-_50_1";
    assert_eq!(
        parse_online_smith_chart_circuit_tokens(legacy_tokens).unwrap(),
        circuit
    );
}

#[test]
fn online_smith_chart_url_parser_accepts_encoded_native_resistance_units() {
    let parsed = parse_online_smith_chart_query(
        "circuit=seriesRes_2_%CE%A9_0_0__shortedRes_3_K%CE%A9_0_0__seriesRlc_4_M%CE%A9_5_nH_6_pF",
    )
    .unwrap();

    assert_eq!(
        parsed.circuit,
        vec![
            SmithChartElement::SeriesResistor {
                resistance_ohm: 2.0,
                esl_h: 0.0,
                tolerance_percent: None,
            },
            SmithChartElement::ShuntResistor {
                resistance_ohm: 3.0e3,
                esl_h: 0.0,
                tolerance_percent: None,
            },
            SmithChartElement::SeriesParallelRlc {
                resistance_ohm: 4.0e6,
                inductance_h: 5.0e-9,
                capacitance_f: 6.0e-12,
            },
        ]
    );
}

#[test]
fn online_smith_chart_url_parser_accepts_blank_numeric_fields() {
    let parsed = parse_online_smith_chart_query(
            "frequency=&fSpan=&fRes=&zMarkers=25_&qCircles=1__2&circuit=blackBox_50__0__seriesRes__ohm_0_",
        )
        .unwrap();

    assert_close(parsed.settings.frequency_hz, 0.0);
    assert_close(parsed.settings.span_hz, 0.0);
    assert_eq!(parsed.settings.resolution, 0);
    assert_eq!(parsed.overlays.z_markers, vec![Complex::new(25.0, 0.0)]);
    assert_eq!(parsed.overlays.q_circles, vec![1.0, 0.0, 2.0]);
    assert_eq!(
        parsed.circuit,
        vec![
            SmithChartElement::BlackBox {
                impedance: Complex::new(50.0, 0.0),
                tolerance_percent: None,
            },
            SmithChartElement::SeriesResistor {
                resistance_ohm: 0.0,
                esl_h: 0.0,
                tolerance_percent: None,
            },
        ]
    );
}

#[test]
fn online_smith_chart_url_parser_uses_default_s_parameter_for_too_long_payloads() {
    let s1p = parse_online_smith_chart_query("circuit=sparam_s1p_Hz_50_tooLong").unwrap();
    assert_eq!(
        s1p.circuit,
        vec![SmithChartElement::SParameter(default_s1p_block())]
    );

    let s2p = parse_online_smith_chart_query("circuit=sparam_s2p_GHz_50_tooLong").unwrap();
    assert_eq!(
        s2p.circuit,
        vec![SmithChartElement::SParameter(default_s2p_block())]
    );
}

#[test]
fn online_smith_chart_url_parser_accepts_blank_s_parameter_payload_fields() {
    let s1p = parse_online_smith_chart_query("circuit=sparam_s1p_MHz_50_1000__").unwrap();
    match &s1p.circuit[0] {
        SmithChartElement::SParameter(block) => {
            assert_eq!(block.kind, SParameterKind::S1P);
            assert_close(block.points[0].frequency_hz, 1.0e9);
            assert_close(block.points[0].s11.magnitude(), 0.0);
            assert_close(block.points[0].s11.phase_degrees(), 0.0);
        }
        _ => panic!("expected S1P block"),
    }

    let s2p =
        parse_online_smith_chart_query("circuit=sparam_s2p_MHz_50_1000_________noise_1000____")
            .unwrap();
    match &s2p.circuit[0] {
        SmithChartElement::SParameter(block) => {
            assert_eq!(block.kind, SParameterKind::S2P);
            assert_close(block.points[0].s11.magnitude(), 0.0);
            assert_close(block.points[0].s21.unwrap().magnitude(), 0.0);
            assert_close(block.points[0].s12.unwrap().phase_degrees(), 0.0);
            assert_close(block.points[0].s22.unwrap().magnitude(), 0.0);
            assert_eq!(block.noise.len(), 1);
            assert_close(block.noise[0].fmin_db, 0.0);
            assert_close(block.noise[0].optimum_gamma.magnitude(), 0.0);
            assert_close(block.noise[0].rn_ohm, 0.0);
        }
        _ => panic!("expected S2P block"),
    }
}

#[test]
fn online_smith_chart_url_parser_accepts_documented_tokens() {
    let parsed = parse_online_smith_chart_query(
            "frequency=1&frequencyUnit=GHz&zo=50&fSpan=200&fSpanUnit=MHz&fRes=51&showIdeal=1&circuit=blackBox_25_5_10__seriesInd_10_nH_5_0.2__shortedCap_2_pF_1_0.1_0.3__transmissionLine_10_mm_2_75_2.2__transformer_10_nH_12_nH_0.8_coupledInductor__custom_linear_%7B%221000000000%22%3A%7B%22real%22%3A42%2C%22imaginary%22%3A-3%7D%7D__custom_sah_%7B%222000000000%22%3A%7B%22real%22%3A52%2C%22imaginary%22%3A4%7D%7D",
        )
        .unwrap();

    assert_close(parsed.settings.frequency_hz, 1.0e9);
    assert_close(parsed.settings.span_hz, 200.0e6);
    assert_eq!(parsed.settings.resolution, 51);
    assert!(!parsed.settings.show_ideal);
    assert_eq!(parsed.circuit.len(), 7);
    assert!(matches!(
        parsed.circuit[0],
        SmithChartElement::BlackBox { .. }
    ));
    match &parsed.circuit[1] {
        SmithChartElement::SeriesInductor { inductance_h, .. } => {
            assert_close(*inductance_h, 10.0e-9)
        }
        _ => panic!("expected series inductor"),
    }
    match &parsed.circuit[2] {
        SmithChartElement::ShuntCapacitor { capacitance_f, .. } => {
            assert_close(*capacitance_f, 2.0e-12)
        }
        _ => panic!("expected shunt capacitor"),
    }
    match &parsed.circuit[3] {
        SmithChartElement::TransmissionLine {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => {
            assert_close(*length_m, 0.01);
            assert_close(*characteristic_impedance_ohm, 75.0);
            assert_close(*effective_dielectric, 2.2);
        }
        _ => panic!("expected transmission line"),
    }
    match &parsed.circuit[4] {
        SmithChartElement::Transformer {
            model,
            l1_h,
            l2_h,
            coupling_or_turns_ratio,
        } => {
            assert!(matches!(model, TransformerModel::CoupledInductor));
            assert_close(*l1_h, 10.0e-9);
            assert_close(*l2_h, 12.0e-9);
            assert_close(*coupling_or_turns_ratio, 0.8);
        }
        _ => panic!("expected transformer"),
    }
    match &parsed.circuit[5] {
        SmithChartElement::Custom {
            points,
            interpolation,
        } => {
            assert_eq!(*interpolation, CustomInterpolation::Linear);
            assert_eq!(points.len(), 1);
            assert_close(points[0].frequency_hz, 1.0e9);
            assert_close(points[0].impedance.re, 42.0);
            assert_close(points[0].impedance.im, -3.0);
        }
        _ => panic!("expected custom Z"),
    }
    match &parsed.circuit[6] {
        SmithChartElement::Custom {
            points,
            interpolation,
        } => {
            assert_eq!(*interpolation, CustomInterpolation::SampleAndHold);
            assert_eq!(points.len(), 1);
            assert_close(points[0].frequency_hz, 2.0e9);
            assert_close(points[0].impedance.re, 52.0);
            assert_close(points[0].impedance.im, 4.0);
        }
        _ => panic!("expected custom sample-and-hold Z"),
    }
}

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

#[test]
fn online_smith_chart_url_parser_accepts_legacy_rust_scalar_keys_and_default_circuit() {
    let parsed = parse_online_smith_chart_query(
        "frequency=1000000000&reference=75&span=200000000&resolution=51",
    )
    .unwrap();

    assert_close(parsed.settings.frequency_hz, 1.0e9);
    assert_close(parsed.settings.reference_impedance_ohm, 75.0);
    assert_close(parsed.settings.span_hz, 200.0e6);
    assert_eq!(parsed.settings.resolution, 51);
    assert_eq!(
        parsed.circuit,
        vec![SmithChartElement::BlackBox {
            impedance: Complex::new(50.0, 0.0),
            tolerance_percent: None,
        }]
    );
}

#[test]
fn online_smith_chart_url_parser_applies_s1p_query_reordering() {
    let parsed = parse_online_smith_chart_query(
            "frequency=1000&frequencyUnit=MHz&circuit=blackBox_25_5_10__seriesInd_10_nH_0_0__shortedCap_2_pF_0_0_0__sparam_s1p_MHz_50_1000_0.1_0",
        )
        .unwrap();

    assert_eq!(parsed.circuit.len(), 4);
    assert!(matches!(
        parsed.circuit[0],
        SmithChartElement::SParameter(SParameterBlock {
            kind: SParameterKind::S1P,
            ..
        })
    ));
    assert!(matches!(
        parsed.circuit[1],
        SmithChartElement::ShuntCapacitor { .. }
    ));
    assert!(matches!(
        parsed.circuit[2],
        SmithChartElement::SeriesInductor { .. }
    ));
    match &parsed.circuit[3] {
        SmithChartElement::LoadTermination {
            impedance,
            tolerance_percent,
        } => {
            assert_close(impedance.re, 25.0);
            assert_close(impedance.im, 5.0);
            assert_eq!(*tolerance_percent, Some(10.0));
        }
        _ => panic!("expected synthesized load termination"),
    }
}

#[test]
fn online_smith_chart_url_parser_accepts_electrical_length_units() {
    let parsed = parse_online_smith_chart_query(
            "frequency=1000&frequencyUnit=MHz&circuit=transmissionLine_0.5_λ_0_75_4__stub_90_deg_0_50_1__shortedStub_10_mm_0_50_1",
        )
        .unwrap();

    match &parsed.circuit[0] {
        SmithChartElement::TransmissionLine {
            length_m,
            characteristic_impedance_ohm,
            effective_dielectric,
            ..
        } => {
            assert_close(*length_m, SPEED_OF_LIGHT_M_PER_S / (4.0 * 1.0e9));
            assert_close(*characteristic_impedance_ohm, 75.0);
            assert_close(*effective_dielectric, 4.0);
        }
        _ => panic!("expected transmission line"),
    }
    match &parsed.circuit[1] {
        SmithChartElement::OpenStub { length_m, .. } => {
            assert_close(*length_m, SPEED_OF_LIGHT_M_PER_S / (4.0 * 1.0e9));
        }
        _ => panic!("expected open stub"),
    }
    match &parsed.circuit[2] {
        SmithChartElement::ShortedStub { length_m, .. } => {
            assert_close(*length_m, 0.01);
        }
        _ => panic!("expected shorted stub"),
    }
}

#[test]
fn online_smith_chart_url_parser_accepts_exact_resistance_unit_spellings() {
    let parsed = parse_online_smith_chart_query(
        "circuit=seriesRes_2_KΩ_0_0__shortedRes_3_MΩ_0_0__loadTerm_50_0_0",
    )
    .unwrap();

    match &parsed.circuit[0] {
        SmithChartElement::SeriesResistor { resistance_ohm, .. } => {
            assert_close(*resistance_ohm, 2.0e3);
        }
        _ => panic!("expected series resistor"),
    }
    match &parsed.circuit[1] {
        SmithChartElement::ShuntResistor { resistance_ohm, .. } => {
            assert_close(*resistance_ohm, 3.0e6);
        }
        _ => panic!("expected shunt resistor"),
    }
}

#[test]
fn online_smith_chart_url_parser_uses_active_s_parameter_frequency_for_electrical_lengths() {
    let parsed = parse_online_smith_chart_query(
            "frequency=900&frequencyUnit=MHz&circuit=sparam_s1p_MHz_50_1000_0.1_0__transmissionLine_0.5_λ_0_50_4",
        )
        .unwrap();

    match &parsed.circuit[1] {
        SmithChartElement::TransmissionLine { length_m, .. } => {
            assert_close(*length_m, SPEED_OF_LIGHT_M_PER_S / (4.0 * 1.0e9));
        }
        _ => panic!("expected transmission line"),
    }
}

#[test]
fn compact_circuit_token_api_preserves_repeated_ordered_rows() {
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

    let tokens = serialize_online_smith_chart_circuit_tokens(&circuit);
    assert!(tokens.contains("transmissionLine"));
    assert!(tokens.contains("__shortedRes"));
    assert_eq!(
        split_online_smith_chart_circuit_tokens(&tokens).len(),
        circuit.len()
    );
    let decoded = parse_online_smith_chart_circuit_tokens(&tokens).unwrap();

    assert_eq!(decoded, circuit);

    let legacy_blank_tokens = "loadTerm_50_0___transmissionLine_10_mm__75_2.2__shortedRes_100_ohm_";
    assert_eq!(
        split_online_smith_chart_circuit_tokens(legacy_blank_tokens).len(),
        3
    );
    let decoded = parse_online_smith_chart_circuit_tokens(legacy_blank_tokens).unwrap();
    assert_eq!(decoded.len(), 3);
}

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

#[test]
fn length_units_convert_wavelength_and_degrees() {
    let wavelength = length_to_meters(1.0, ScalarUnit::Wavelength, 1.0e9, 1.0).unwrap();
    let degrees = length_to_meters(180.0, ScalarUnit::Degree, 1.0e9, 1.0).unwrap();

    assert_close(wavelength, SPEED_OF_LIGHT_M_PER_S / 1.0e9);
    assert_close(degrees, SPEED_OF_LIGHT_M_PER_S / (2.0 * 1.0e9));
}
