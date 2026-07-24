use crate::transmission_line_calculator::{
    Complex, CustomInterpolation, DEFAULT_REFERENCE_IMPEDANCE_OHM, ScalarUnit, SmithChartAnalysis,
    SmithChartElement, SmithViewTransform, TransformerModel,
};
use iced::Point;

use crate::transmission_line_calculator::tool::component_editor::{
    CircuitComponentKind, CircuitEditorComponent,
};
use crate::transmission_line_calculator::tool::results::estimated_s21_summary;

use super::*;

/// Verifies that complete tool view can be constructed.
#[test]
fn complete_tool_view_can_be_constructed() {
    let state = SmithChartState::default();

    let _view = crate::transmission_line_calculator::tool::view(&state);
}

/// Verifies that tool view supports s parameter controls in every chart mode.
#[test]
fn tool_view_supports_s_parameter_controls_in_every_chart_mode() {
    let mut state = SmithChartState::default();
    state.circuit_components.push(CircuitEditorComponent::new(
        CircuitComponentKind::SParameters,
    ));

    for mode in SmithChartDiagramMode::ALL {
        state.diagram_mode = mode;
        let _view = crate::transmission_line_calculator::tool::view(&state);
    }
}

/// Verifies that default frequency span populates final result diagrams.
#[test]
fn default_frequency_span_populates_final_result_diagrams() {
    let state = SmithChartState::default();

    let result = state.solve().unwrap();

    assert_eq!(state.span_mhz, "0.5");
    assert_eq!(result.frequency_results.len(), 21);
    assert_close(
        result.frequency_results.first().unwrap().frequency_hz,
        2.4395e9,
    );
    assert_close(
        result.frequency_results.last().unwrap().frequency_hz,
        2.4405e9,
    );
}

/// Verifies that CSV export uses the selected inclusive range and sample count.
#[test]
fn csv_export_uses_the_selected_inclusive_range_and_sample_count() {
    let mut state = SmithChartState::default();
    state.update(SmithChartMessage::OpenCsvExport(
        crate::transmission_line_calculator::tool::ResultDiagramKind::ImpedanceMagnitude,
    ));
    state.update(SmithChartMessage::CsvExportStartFrequencyUnitChanged(
        ScalarUnit::MegaHertz,
    ));
    state.update(SmithChartMessage::CsvExportStopFrequencyUnitChanged(
        ScalarUnit::MegaHertz,
    ));
    state.update(SmithChartMessage::CsvExportOutputFrequencyUnitChanged(
        ScalarUnit::MegaHertz,
    ));
    state.update(SmithChartMessage::CsvExportStartFrequencyChanged(
        "2400".to_string(),
    ));
    state.update(SmithChartMessage::CsvExportStopFrequencyChanged(
        "2500".to_string(),
    ));
    state.update(SmithChartMessage::CsvExportSamplesChanged("5".to_string()));

    let (file_name, csv) = state.generated_csv_export().unwrap();
    let lines = csv.lines().collect::<Vec<_>>();

    assert_eq!(file_name, "smith_chart_impedance.csv");
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[0], "Frequency [MHz],|Z| [Ω]");
    assert!(lines[1].starts_with("2400.000000000000,"));
    assert!(lines[5].starts_with("2500.000000000000,"));
}

/// Verifies that CSV export defaults to one hertz and converts each range unit independently.
#[test]
fn csv_export_defaults_to_one_hertz_and_converts_frequency_units() {
    let mut state = SmithChartState::default();
    state.update(SmithChartMessage::OpenCsvExport(
        crate::transmission_line_calculator::tool::ResultDiagramKind::S11Db,
    ));

    let configuration = state.csv_export_configuration.as_ref().unwrap();
    assert_eq!(configuration.start_frequency_unit, ScalarUnit::Hertz);
    assert_eq!(configuration.stop_frequency_unit, ScalarUnit::Hertz);
    assert_eq!(configuration.output_frequency_unit, ScalarUnit::Hertz);
    assert_eq!(configuration.start_frequency, "1");
    assert_eq!(configuration.stop_frequency, "2440500000");

    state.update(SmithChartMessage::CsvExportStartFrequencyUnitChanged(
        ScalarUnit::GigaHertz,
    ));
    state.update(SmithChartMessage::CsvExportStopFrequencyUnitChanged(
        ScalarUnit::MegaHertz,
    ));
    state.update(SmithChartMessage::CsvExportOutputFrequencyUnitChanged(
        ScalarUnit::KiloHertz,
    ));

    let configuration = state.csv_export_configuration.as_ref().unwrap();
    assert_eq!(configuration.start_frequency_unit, ScalarUnit::GigaHertz);
    assert_eq!(configuration.stop_frequency_unit, ScalarUnit::MegaHertz);
    assert_eq!(configuration.output_frequency_unit, ScalarUnit::KiloHertz);
    assert_eq!(configuration.start_frequency, "0.000000001");
    assert_eq!(configuration.stop_frequency, "2440.5");
}

/// Verifies that CSV export rejects a reversed frequency range.
#[test]
fn csv_export_rejects_a_reversed_frequency_range() {
    let mut state = SmithChartState::default();
    state.update(SmithChartMessage::OpenCsvExport(
        crate::transmission_line_calculator::tool::ResultDiagramKind::S11Db,
    ));
    state.update(SmithChartMessage::CsvExportStartFrequencyChanged(
        "2500".to_string(),
    ));
    state.update(SmithChartMessage::CsvExportStopFrequencyChanged(
        "2400".to_string(),
    ));

    let error = state.generated_csv_export().unwrap_err();

    assert_eq!(
        error,
        "Stop frequency must be greater than start frequency."
    );
}

/// Verifies that marker list accepts rectangular and polar entries.
#[test]
fn marker_list_accepts_rectangular_and_polar_entries() {
    let markers = parse_marker_list("25,10;polar:50,90;p:10,180").unwrap();

    assert_eq!(markers.len(), 3);
    assert_close(markers[0].re, 25.0);
    assert_close(markers[0].im, 10.0);
    assert_close(markers[1].re, 0.0);
    assert_close(markers[1].im, 50.0);
    assert_close(markers[2].re, -10.0);
    assert_close(markers[2].im, 0.0);
}

/// Verifies that scalar lists accept comma and semicolon separators.
#[test]
fn scalar_lists_accept_comma_and_semicolon_separators() {
    let values = parse_scalar_list("0.2, 0.5;1,2").unwrap();
    assert_eq!(values.len(), 4);
    assert_close(values[0], 0.2);
    assert_close(values[1], 0.5);
    assert_close(values[2], 1.0);
    assert_close(values[3], 2.0);

    let vswr = parse_vswr_circle_list("6, 12", true).unwrap();
    assert_eq!(vswr.len(), 2);
    assert_close(vswr[0], 10.0_f64.powf(6.0 / 20.0));
    assert_close(vswr[1], 10.0_f64.powf(12.0 / 20.0));
}

/// Verifies that numeric fields match source numeric sanitizer.
#[test]
fn numeric_fields_match_source_numeric_sanitizer() {
    assert_close(parse_field("resistance", "50 Ω").unwrap(), 50.0);
    assert_close(parse_field("length", "2.5mm").unwrap(), 2.5);
    assert_close(parse_field("scientific", "1e+3 Hz").unwrap(), 1.0e3);
    assert_close(parse_field("scientific", "1e-3 F").unwrap(), 1.0e-3);

    let markers = parse_marker_list("25Ω, 10j; polar:50 Ω, +90").unwrap();
    assert_eq!(markers.len(), 2);
    assert_close(markers[0].re, 25.0);
    assert_close(markers[0].im, 10.0);
    assert_close(markers[1].re, 0.0);
    assert_close(markers[1].im, 50.0);

    let circles = parse_scalar_list("1 dB, 2 pts; 3x").unwrap();
    assert_eq!(circles, vec![1.0, 2.0, 3.0]);
}

/// Verifies that estimated S21 summary uses sampled default trace.
#[test]
fn estimated_s21_summary_uses_sampled_default_trace() {
    let result = SmithChartAnalysis {
        nominal: dummy_solve_result(Complex::new(50.0, 0.0), Complex::new(0.0, 0.0)),
        tolerance_results: Vec::new(),
        impedance_arcs: Vec::new(),
        frequency_results: vec![
            frequency_point(0.0, 0.8),
            frequency_point(1.0e9, 0.2),
            frequency_point(2.0e9, 0.0),
            frequency_point(3.0e9, 0.2),
            frequency_point(4.0e9, 0.8),
        ],
        frequency_result_variants: Vec::new(),
        s1p_reflection_variants: Vec::new(),
        s_parameter_gain: Vec::new(),
        s_parameter_gain_variants: Vec::new(),
        two_port_s_parameters: Vec::new(),
        noise_figure: Vec::new(),
        stability_circles: Vec::new(),
        active_frequency_hz: 2.0e9,
    };

    let summary = estimated_s21_summary(&result).unwrap();

    assert_close(summary.max_db, 0.0);
    assert_close(summary.frequency_hz, 2.0e9);
    assert_close(summary.bandwidth_hz.unwrap(), 4.0e9);
    assert_eq!(
        format_frequency(summary.frequency_hz, ScalarUnit::GigaHertz),
        "2 GHz"
    );
}

/// Verifies that compact default uses a black box load.
#[test]
fn compact_default_uses_a_black_box_load() {
    let state = SmithChartState::default();
    let circuit = state.compact_circuit().unwrap();

    assert!(matches!(
        circuit.as_slice(),
        [SmithChartElement::BlackBox { .. }]
    ));
}

/// Verifies that reset returns to calculator defaults.
#[test]
fn reset_returns_to_calculator_defaults() {
    let mut state = SmithChartState::new();
    state.reference_ohm = "75".to_string();
    state.marker_list = "25,10".to_string();
    state.q_circle_list = "1;2".to_string();
    state.vswr_circle_list = "2;3".to_string();
    state.short_stub_warning_seen = true;
    state.file_status = Some("dirty".to_string());

    state.update(SmithChartMessage::Reset);

    assert_eq!(state.reference_ohm, "50");
    assert!(state.marker_list.is_empty());
    assert!(state.q_circle_list.is_empty());
    assert!(state.vswr_circle_list.is_empty());
    assert!(!state.short_stub_warning_seen);
    assert_eq!(state.file_status, None);
}

/// Verifies that shorted stub warning is shown once for compact and ordered adds.
#[test]
fn shorted_stub_warning_is_shown_once_for_compact_and_ordered_adds() {
    let mut compact = SmithChartState::default();
    compact.update(SmithChartMessage::ShortStubEnabled(true));
    assert_eq!(compact.file_status.as_deref(), Some(SHORTED_STUB_WARNING));
    assert!(compact.short_stub_warning_seen);

    compact.file_status = None;
    compact.update(SmithChartMessage::ShortStubEnabled(false));
    compact.update(SmithChartMessage::ShortStubEnabled(true));
    assert_eq!(compact.file_status, None);

    let mut ordered = SmithChartState::default();
    ordered.update(SmithChartMessage::AddOrderedCircuitRow(
        "shortedStub_10_mm_-_50_1",
    ));
    assert_eq!(ordered.file_status.as_deref(), Some(SHORTED_STUB_WARNING));
    assert!(ordered.short_stub_warning_seen);
    assert!(ordered.ordered_circuit_tokens.contains("shortedStub"));

    ordered.file_status = None;
    ordered.update(SmithChartMessage::AddOrderedCircuitRow(
        "shortedStub_5_mm_-_50_1",
    ));
    assert_eq!(ordered.file_status, None);
}

/// Verifies that diagram mode switch and 3d rotation reset are stateful.
#[test]
fn diagram_mode_switch_and_3d_rotation_reset_are_stateful() {
    let mut state = SmithChartState::default();
    assert_eq!(state.diagram_mode, SmithChartDiagramMode::TwoDimensional);

    state.update(SmithChartMessage::DiagramModeChanged(
        SmithChartDiagramMode::ThreeDimensional,
    ));
    state.update(SmithChartMessage::SmithSphereRotationChanged {
        yaw: 1.25,
        pitch: -0.5,
    });

    assert_eq!(state.diagram_mode, SmithChartDiagramMode::ThreeDimensional);
    assert_eq!(state.smith_sphere_yaw, 1.25);
    assert_eq!(state.smith_sphere_pitch, -0.5);

    state.update(SmithChartMessage::ResetSmithSphereRotation);

    assert_eq!(state.smith_sphere_yaw, -0.65);
    assert_eq!(state.smith_sphere_pitch, 0.35);
}

/// Verifies that smith view transform is shared across modes and can be reset.
#[test]
fn smith_view_transform_is_shared_across_modes_and_can_be_reset() {
    let mut state = SmithChartState::default();
    let transform = SmithViewTransform::identity()
        .zoomed_at(Point::new(120.0, 80.0), 2.0)
        .translated(iced::Vector::new(16.0, -9.0));

    state.update(SmithChartMessage::SmithViewTransformChanged(transform));
    state.update(SmithChartMessage::DiagramModeChanged(
        SmithChartDiagramMode::ThreeDimensional,
    ));

    assert_eq!(state.smith_view_transform, transform);

    state.update(SmithChartMessage::ResetSmithViewTransform);

    assert!(state.smith_view_transform.is_identity());
}

/// Verifies that admittance smith chart is available as a dedicated diagram mode.
#[test]
fn admittance_smith_chart_is_available_as_a_dedicated_diagram_mode() {
    let mut state = SmithChartState::default();

    state.update(SmithChartMessage::DiagramModeChanged(
        SmithChartDiagramMode::AdmittanceTwoDimensional,
    ));

    assert_eq!(
        state.diagram_mode,
        SmithChartDiagramMode::AdmittanceTwoDimensional
    );
    assert_eq!(state.diagram_mode.to_string(), "2D Y Smith Chart");
    let _view = crate::transmission_line_calculator::tool::view(&state);
}

/// Verifies that compact s parameters follow source row normalization.
#[test]
fn compact_s_parameters_follow_source_row_normalization() {
    let s1p_state = SmithChartState {
        s_parameter_enabled: true,
        s_parameter_text: "# MHz S MA R 50\n1000 0.1 0".to_string(),
        series_resistance_enabled: true,
        series_resistance_ohm: "10".to_string(),
        load_re: "75".to_string(),
        load_im: "5".to_string(),
        load_tolerance: "2".to_string(),
        ..SmithChartState::default()
    };
    let s1p_circuit = s1p_state.compact_circuit().unwrap();
    assert!(matches!(
        s1p_circuit.as_slice(),
        [
            SmithChartElement::SParameter(_),
            SmithChartElement::SeriesResistor { .. },
            SmithChartElement::LoadTermination { .. }
        ]
    ));
    match s1p_circuit.last().unwrap() {
        SmithChartElement::LoadTermination {
            impedance,
            tolerance_percent,
        } => {
            assert_close(impedance.re, 75.0);
            assert_close(impedance.im, 5.0);
            assert_eq!(*tolerance_percent, Some(2.0));
        }
        element => panic!("expected trailing load termination, got {element:?}"),
    }

    let s2p_state = SmithChartState {
        s_parameter_enabled: true,
        series_resistance_enabled: true,
        series_resistance_ohm: "10".to_string(),
        load_re: "75".to_string(),
        ..SmithChartState::default()
    };
    let s2p_circuit = s2p_state.compact_circuit().unwrap();
    assert!(matches!(
        s2p_circuit.as_slice(),
        [
            SmithChartElement::BlackBox { .. },
            SmithChartElement::SeriesResistor { .. },
            SmithChartElement::SParameter(_),
            SmithChartElement::LoadTermination { .. }
        ]
    ));
    match s2p_circuit.last().unwrap() {
        SmithChartElement::LoadTermination { impedance, .. } => {
            assert_close(impedance.re, DEFAULT_REFERENCE_IMPEDANCE_OHM);
            assert_close(impedance.im, 0.0);
        }
        element => panic!("expected default trailing load termination, got {element:?}"),
    }
}

/// Verifies that compact s parameter text updates source sweep settings.
#[test]
fn compact_s_parameter_text_updates_source_sweep_settings() {
    let mut state = SmithChartState::default();

    state.update(SmithChartMessage::SParameterTextChanged(
        "# GHz S MA R 50\n0.9 0.1 0\n1 0.2 10\n1.4 0.3 20".to_string(),
    ));

    assert_eq!(state.frequency_unit, ScalarUnit::GigaHertz);
    assert_eq!(state.span_unit, ScalarUnit::GigaHertz);
    assert_eq!(state.frequency_mhz, "1");
    assert_eq!(state.span_mhz, "0.8");
}

/// Verifies that imported touchstone file enables s parameter and syncs sweep.
#[test]
fn imported_touchstone_file_enables_s_parameter_and_syncs_sweep() {
    let mut state = SmithChartState::default();

    state.update(SmithChartMessage::SParameterFileLoaded(Ok(Some(
        "# GHz S MA R 50\n0.8 0.1 0\n1.2 0.2 10\n1.6 0.3 20".to_string(),
    ))));

    assert!(state.s_parameter_enabled);
    assert_eq!(state.frequency_unit, ScalarUnit::GigaHertz);
    assert_eq!(state.span_unit, ScalarUnit::GigaHertz);
    assert_eq!(state.frequency_mhz, "1.2");
    assert_eq!(state.span_mhz, "0.8");
    assert_eq!(
        state.file_status.as_deref(),
        Some("Imported Touchstone data")
    );
    assert!(state.s_parameter_content.text().contains("# GHz S MA R 50"));
}

/// Verifies that generated SVG export can be saved by dispatcher.
#[test]
fn generated_svg_export_can_be_saved_by_dispatcher() {
    let mut state = SmithChartState::default();

    let svg = state.generated_svg_export().unwrap();
    state.update(SmithChartMessage::SvgFileSaved(Ok(Some(
        "C:\\temp\\smith_chart.svg".to_string(),
    ))));

    assert!(svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg""#));
    assert!(svg.contains(r#"aria-label="Smith chart""#));
    assert_eq!(
        state.file_status.as_deref(),
        Some("Saved SVG to C:\\temp\\smith_chart.svg")
    );
}

/// Verifies that compact custom interpolation preserves sample and hold.
#[test]
fn compact_custom_interpolation_preserves_sample_and_hold() {
    let state = SmithChartState {
        custom_enabled: true,
        custom_interpolation: CustomInterpolation::SampleAndHold,
        custom_points: "1000,42,-3;2000,55,4".to_string(),
        ..SmithChartState::default()
    };

    let circuit = state.compact_circuit().unwrap();
    match circuit.last().unwrap() {
        SmithChartElement::Custom {
            interpolation,
            points,
        } => {
            assert_eq!(*interpolation, CustomInterpolation::SampleAndHold);
            assert_eq!(points.len(), 2);
            assert_close(points[0].frequency_hz, 1.0e9);
        }
        element => panic!("expected custom Z(f), got {element:?}"),
    }
}

/// Verifies that compact resistor esl preserves source field.
#[test]
fn compact_resistor_esl_preserves_source_field() {
    let state = SmithChartState {
        series_resistance_enabled: true,
        series_resistance_ohm: "12".to_string(),
        series_resistance_esl_nh: "0.4".to_string(),
        shunt_resistance_enabled: true,
        shunt_resistance_ohm: "80".to_string(),
        shunt_resistance_esl_nh: "0.7".to_string(),
        ..SmithChartState::default()
    };

    let circuit = state.compact_circuit().unwrap();
    match &circuit[1] {
        SmithChartElement::SeriesResistor { esl_h, .. } => assert_close(*esl_h, 0.4e-9),
        element => panic!("expected series resistor, got {element:?}"),
    }
    match &circuit[2] {
        SmithChartElement::ShuntResistor { esl_h, .. } => assert_close(*esl_h, 0.7e-9),
        element => panic!("expected shunt resistor, got {element:?}"),
    }
}

/// Verifies that compact transformer model matches source controls.
#[test]
fn compact_transformer_model_matches_source_controls() {
    let mut state = SmithChartState {
        transformer_enabled: true,
        ..SmithChartState::default()
    };

    let circuit = state.compact_circuit().unwrap();
    match circuit.last().unwrap() {
        SmithChartElement::Transformer {
            model,
            l1_h,
            l2_h,
            coupling_or_turns_ratio,
        } => {
            assert_eq!(*model, TransformerModel::CoupledInductor);
            assert_close(*l1_h, 1.0e-9);
            assert_close(*l2_h, 1.0e-9);
            assert_close(*coupling_or_turns_ratio, 1.0);
        }
        element => panic!("expected coupled transformer, got {element:?}"),
    }

    state.transformer_model = TransformerModel::Ideal;
    state.transformer_ratio = "2".to_string();
    let circuit = state.compact_circuit().unwrap();
    match circuit.last().unwrap() {
        SmithChartElement::Transformer {
            model,
            coupling_or_turns_ratio,
            ..
        } => {
            assert_eq!(*model, TransformerModel::Ideal);
            assert_close(*coupling_or_turns_ratio, 2.0);
        }
        element => panic!("expected ideal transformer, got {element:?}"),
    }
}

/// Creates a representative frequency-result test fixture.
fn frequency_point(
    frequency_hz: f64,
    reflection_magnitude: f64,
) -> crate::transmission_line_calculator::FrequencyPointResult {
    let reflection_coefficient = Complex::new(reflection_magnitude, 0.0);
    crate::transmission_line_calculator::FrequencyPointResult {
        frequency_hz,
        impedance: Complex::new(50.0, 0.0),
        reflection_coefficient,
    }
}

/// Creates a minimal solve-result test fixture.
fn dummy_solve_result(
    impedance: Complex,
    reflection_coefficient: Complex,
) -> crate::transmission_line_calculator::SolveResult {
    crate::transmission_line_calculator::SolveResult {
        impedance,
        normalized_impedance: impedance * (1.0 / DEFAULT_REFERENCE_IMPEDANCE_OHM),
        reflection_coefficient,
        admittance: Complex::new(1.0 / impedance.re, 0.0),
        normalized_admittance: Complex::new(DEFAULT_REFERENCE_IMPEDANCE_OHM / impedance.re, 0.0),
        return_loss_db: f64::INFINITY,
        vswr: 1.0,
        chart_x: reflection_coefficient.re,
        chart_y: reflection_coefficient.im,
        steps: Vec::new(),
    }
}

/// Asserts that close.
fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-9,
        "expected {actual} to be close to {expected}"
    );
}
