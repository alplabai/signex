use super::*;
use iced::Vector;

/// Verifies that transformed hit testing recovers chart coordinates.
#[test]
fn transformed_hit_testing_recovers_chart_coordinates() {
    let center = Point::new(400.0, 300.0);
    let radius = 250.0;
    let transform = SmithViewTransform::identity()
        .zoomed_at(Point::new(330.0, 260.0), 2.25)
        .translated(Vector::new(48.0, -31.0));
    let chart_point = (0.35, -0.42);
    let screen_point = transform.transform_point(Point::new(
        center.x + chart_point.0 * radius,
        center.y - chart_point.1 * radius,
    ));

    let recovered = chart_coordinates_from_screen(screen_point, center, radius, transform).unwrap();

    assert!((recovered.0 - chart_point.0).abs() < 1.0e-5);
    assert!((recovered.1 - chart_point.1).abs() < 1.0e-5);
}

/// Verifies that matched frequency point hover matches reference readout.
#[test]
fn matched_frequency_point_hover_matches_reference_readout() {
    let lines = hover_readout_lines(
        Complex::new(50.0, 0.0),
        Complex::ZERO,
        Some(2.44e9),
        ScalarUnit::MegaHertz,
    );

    assert_eq!(
        lines,
        vec![
            "Frequency = 2440 MHz",
            "Impedance = 50.00 + 0.00j (50.00 ∠ 0.00°)",
            "Admittance = 0.0200 + 0.00j",
            "Refl-Coeff = 0.000 + 0.000j (0.000 ∠ 0.0°)",
            "VSWR = 1.00",
            "Q-Factor = 0.0e+0",
        ]
    );
}

/// Verifies that overlapping sweep points prefer the active frequency.
#[test]
fn overlapping_sweep_points_prefer_the_active_frequency() {
    let points = [
        frequency_point(2.4395e9),
        frequency_point(2.44e9),
        frequency_point(2.4405e9),
    ];

    let point = nearest_frequency_point(&points, Complex::ZERO, 2.44e9, 0.1).unwrap();

    assert_eq!(point.frequency_hz, 2.44e9);
}

/// Verifies that plot hover interpolates the value at the cursor frequency.
#[test]
fn plot_hover_interpolates_the_value_at_the_cursor_frequency() {
    let points = [(2.4e9, 40.0), (2.5e9, 60.0)];

    let value = interpolate_plot_value(&points, 2.45e9).unwrap();

    assert!((value - 50.0).abs() < f64::EPSILON);
}

/// Verifies that frequency plot ranges start at zero and include the zero value line.
#[test]
fn frequency_plot_ranges_start_at_zero_and_include_the_zero_value_line() {
    let positive_points = [(2.4e9, 40.0), (2.5e9, 60.0)];
    let negative_points = [(2.4e9, -20.0), (2.5e9, -10.0)];

    let positive_range = frequency_track_ranges(&positive_points).unwrap();
    let negative_range = frequency_track_ranges(&negative_points).unwrap();

    assert_eq!(positive_range, (0.0, 2.5e9, 0.0, 60.0));
    assert_eq!(negative_range, (0.0, 2.5e9, -20.0, 0.0));
}

/// Verifies that plot hover does not extend a trace into the empty zero to start range.
#[test]
fn plot_hover_does_not_extend_a_trace_into_the_empty_zero_to_start_range() {
    let points = [(2.4e9, 40.0), (2.5e9, 60.0)];

    assert_eq!(interpolate_plot_value(&points, 1.0e9), None);
}

/// Creates a representative frequency-result test fixture.
fn frequency_point(frequency_hz: f64) -> crate::transmission_line_calculator::FrequencyPointResult {
    crate::transmission_line_calculator::FrequencyPointResult {
        frequency_hz,
        impedance: Complex::new(50.0, 0.0),
        reflection_coefficient: Complex::ZERO,
    }
}
