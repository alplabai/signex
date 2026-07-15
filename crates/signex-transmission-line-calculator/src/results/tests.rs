use super::*;

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

fn frequency_point(
    frequency_hz: f64,
    reflection_coefficient: Complex,
) -> crate::FrequencyPointResult {
    crate::FrequencyPointResult {
        frequency_hz,
        impedance: Complex::new(50.0, 0.0),
        reflection_coefficient,
    }
}

fn solve_result(reflection_coefficient: Complex) -> crate::SolveResult {
    crate::SolveResult {
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
