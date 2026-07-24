use crate::transmission_line_calculator::smith_chart_grid::*;

/// Verifies the requested fine decimal resistance steps around the chart center.
#[test]
fn decimal_resistance_grid_uses_range_dependent_steps() {
    let grid = smith_chart_grid();

    for value in [
        0.40, 0.42, 0.44, 0.46, 0.48, 0.50, 0.52, 0.54, 0.56, 0.58, 0.60, 0.65, 0.70, 0.75, 0.80,
        0.85, 0.90, 0.95, 1.00,
    ] {
        assert!(
            grid.iter()
                .any(|line| { line.kind == SmithChartGridLineKind::Resistance { value } })
        );
    }
}

/// Verifies that larger normalized values use progressively wider decimal steps.
#[test]
fn decimal_resistance_grid_widens_steps_at_larger_values() {
    let grid = smith_chart_grid();

    for value in [10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 30.0, 40.0, 50.0] {
        assert!(
            grid.iter()
                .any(|line| { line.kind == SmithChartGridLineKind::Resistance { value } })
        );
    }
    for value in [0.61, 11.0, 22.0] {
        assert!(
            grid.iter()
                .all(|line| { line.kind != SmithChartGridLineKind::Resistance { value } })
        );
    }
}

/// Verifies that complex-value contours use the same positive and negative decimal values.
#[test]
fn reactance_grid_mirrors_the_decimal_resistance_values() {
    let grid = smith_chart_grid();

    for value in [0.42, 0.95, 12.0, 40.0] {
        for signed_value in [-value, value] {
            assert!(grid.iter().any(|line| {
                line.kind
                    == SmithChartGridLineKind::Reactance {
                        value: signed_value,
                    }
            }));
        }
    }
}

/// Verifies that primary 1-2-5 contours remain complete and visually emphasized.
#[test]
fn one_two_five_contours_are_complete_major_lines() {
    let grid = smith_chart_grid();

    for kind in [
        SmithChartGridLineKind::Resistance { value: 0.2 },
        SmithChartGridLineKind::Resistance { value: 0.5 },
        SmithChartGridLineKind::Resistance { value: 1.0 },
        SmithChartGridLineKind::Reactance { value: -0.5 },
        SmithChartGridLineKind::Reactance { value: 0.0 },
        SmithChartGridLineKind::Reactance { value: 2.0 },
    ] {
        let line = grid.iter().find(|line| line.kind == kind).unwrap();
        assert_eq!(line.hierarchy, SmithChartGridHierarchy::Major);
        assert!(
            line.points
                .iter()
                .any(|(x, y)| (*x - 1.0).abs() < 1.0e-12 && y.abs() < 1.0e-12)
        );
    }
}

/// Verifies that fine contours stop before entering visually crowded chart regions.
#[test]
fn minor_decimal_contours_are_adaptively_clipped() {
    let grid = smith_chart_grid();

    for kind in [
        SmithChartGridLineKind::Resistance { value: 0.42 },
        SmithChartGridLineKind::Reactance { value: 0.42 },
        SmithChartGridLineKind::Reactance { value: -0.42 },
    ] {
        let line = grid.iter().find(|line| line.kind == kind).unwrap();
        assert_eq!(line.hierarchy, SmithChartGridHierarchy::Minor);
        assert_ne!(line.points.first(), line.points.last());
        assert!(
            line.points
                .iter()
                .all(|(x, y)| { (*x - 1.0).abs() > 1.0e-12 || y.abs() > 1.0e-12 })
        );
    }
}

/// Verifies that every generated contour is finite and stays inside the unit disk.
#[test]
fn adaptive_decimal_grid_stays_inside_the_smith_chart() {
    let grid = smith_chart_grid();

    assert!(!grid.is_empty());
    for line in grid {
        assert!(line.points.len() >= 2);
        for (x, y) in &line.points {
            assert!(x.is_finite());
            assert!(y.is_finite());
            assert!(x.hypot(*y) <= 1.0 + 1.0e-12);
        }
    }
}
