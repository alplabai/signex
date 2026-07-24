use super::*;
use crate::transmission_line_calculator::{
    FrequencyScale, SParameterMatrix, TwoPortSParameterPoint,
};

/// Verifies that every default final-result diagram receives a visible finite sweep.
#[test]
fn default_final_result_diagrams_receive_finite_sweep_points() {
    let state = SmithChartState::default();
    let result = state.solve().unwrap();
    let diagram_result = diagram_analysis(&state, &result).unwrap();

    let impedance = impedance_magnitude_points(&diagram_result);
    let s11 = s11_db_points(&diagram_result);
    let s21 = estimated_s21_points(&diagram_result);

    assert_eq!(impedance.len(), 21);
    assert_eq!(s11.len(), 21);
    assert_eq!(s21.len(), 21);
    assert!(impedance.iter().all(|point| point.1.is_finite()));
    assert!(s11.iter().all(|point| point.1.is_finite()));
    assert!(s21.iter().all(|point| point.1.is_finite()));
    assert_eq!(impedance.first().unwrap().0, 1.0);
    assert_eq!(impedance.last().unwrap().0, 2.4405e9);
}

/// Verifies that diagram frequencies cover their complete range for every scale.
#[test]
fn diagram_frequencies_cover_the_complete_range_for_every_scale() {
    for scale in FrequencyScale::ALL {
        let frequencies = diagram_frequencies(1.0, 100.0, 3, scale);

        assert_eq!(frequencies.len(), 3);
        assert!((frequencies[0] - 1.0).abs() < 1.0e-12);
        assert!((frequencies[2] - 100.0).abs() < 1.0e-9);
        assert!(frequencies[1] > frequencies[0]);
        assert!(frequencies[1] < frequencies[2]);
    }
}

/// Verifies the expected midpoint for linear and logarithmic frequency scales.
#[test]
fn diagram_frequency_scale_controls_sample_distribution() {
    let linear = diagram_frequencies(1.0, 100.0, 3, FrequencyScale::Linear);
    let log10 = diagram_frequencies(1.0, 100.0, 3, FrequencyScale::Logarithmic);
    let natural = diagram_frequencies(1.0, 100.0, 3, FrequencyScale::NaturalLogarithm);

    assert!((linear[1] - 50.5).abs() < 1.0e-12);
    assert!((log10[1] - 10.0).abs() < 1.0e-12);
    assert!((natural[1] - 10.0).abs() < 1.0e-12);
}

/// Verifies that estimated S21 diagram keeps every finite sweep point.
#[test]
fn estimated_s21_diagram_keeps_every_finite_sweep_point() {
    let result = SmithChartAnalysis {
        nominal: solve_result(Complex::ZERO),
        tolerance_results: Vec::new(),
        impedance_arcs: Vec::new(),
        frequency_results: vec![
            frequency_point(2.4395e9, Complex::ZERO),
            frequency_point(2.44e9, Complex::new(1.0, 0.0)),
            frequency_point(2.4405e9, Complex::new(1.2, 0.0)),
        ],
        frequency_result_variants: Vec::new(),
        s1p_reflection_variants: Vec::new(),
        s_parameter_gain: Vec::new(),
        s_parameter_gain_variants: Vec::new(),
        two_port_s_parameters: Vec::new(),
        noise_figure: Vec::new(),
        stability_circles: Vec::new(),
        active_frequency_hz: 2.44e9,
    };

    let points = estimated_s21_points(&result);

    assert_eq!(points.len(), 3);
    assert_eq!(points[0].1, 0.0);
    assert_eq!(points[1].1, -120.0);
    assert_eq!(points[2].1, -120.0);
}

/// Verifies that solved two-port S21 replaces the reflected-power fallback.
#[test]
fn s21_diagram_prefers_solved_two_port_parameters() {
    let result = SmithChartAnalysis {
        nominal: solve_result(Complex::ZERO),
        tolerance_results: Vec::new(),
        impedance_arcs: Vec::new(),
        frequency_results: vec![frequency_point(1.0e9, Complex::ZERO)],
        frequency_result_variants: Vec::new(),
        s1p_reflection_variants: Vec::new(),
        s_parameter_gain: Vec::new(),
        s_parameter_gain_variants: Vec::new(),
        two_port_s_parameters: vec![TwoPortSParameterPoint {
            frequency_hz: 1.0e9,
            s_parameters: SParameterMatrix::new(
                Complex::ZERO,
                Complex::new(0.8, 0.0),
                Complex::new(0.8, 0.0),
                Complex::ZERO,
            ),
        }],
        noise_figure: Vec::new(),
        stability_circles: Vec::new(),
        active_frequency_hz: 1.0e9,
    };

    let points = estimated_s21_points(&result);

    assert_eq!(points.len(), 1);
    assert!((points[0].1 - 20.0 * 0.8_f64.log10()).abs() < 1.0e-9);
}

/// Creates a representative frequency-result test fixture.
fn frequency_point(
    frequency_hz: f64,
    reflection_coefficient: Complex,
) -> crate::transmission_line_calculator::FrequencyPointResult {
    crate::transmission_line_calculator::FrequencyPointResult {
        frequency_hz,
        impedance: Complex::new(50.0, 0.0),
        reflection_coefficient,
    }
}

/// Solves result from the supplied circuit and settings.
fn solve_result(
    reflection_coefficient: Complex,
) -> crate::transmission_line_calculator::SolveResult {
    crate::transmission_line_calculator::SolveResult {
        impedance: Complex::new(50.0, 0.0),
        normalized_impedance: Complex::ONE,
        reflection_coefficient,
        admittance: Complex::new(0.02, 0.0),
        normalized_admittance: Complex::ONE,
        return_loss_db: f64::INFINITY,
        vswr: 1.0,
        chart_x: reflection_coefficient.re,
        chart_y: reflection_coefficient.im,
        steps: Vec::new(),
    }
}
