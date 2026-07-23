use crate::transmission_line_calculator::smith_chart_grid::*;

/// Verifies that conventional structural contours are always present and emphasized.
#[test]
fn conventional_structural_contours_are_major() {
    let grid = smith_chart_grid(
        &[0.2, 0.5, 1.0, 2.0, 5.0],
        &[-5.0, -2.0, -1.0, -0.5, -0.2, 0.2, 0.5, 1.0, 2.0, 5.0],
    );

    for kind in [
        SmithChartGridLineKind::Resistance { value: 0.0 },
        SmithChartGridLineKind::Resistance { value: 1.0 },
        SmithChartGridLineKind::Reactance { value: -1.0 },
        SmithChartGridLineKind::Reactance { value: 0.0 },
        SmithChartGridLineKind::Reactance { value: 1.0 },
    ] {
        let line = grid.iter().find(|line| line.kind == kind).unwrap();
        assert_eq!(line.hierarchy, SmithChartGridHierarchy::Major);
    }
}

/// Verifies that a constant-resistance contour has the conventional circle geometry.
#[test]
fn constant_resistance_contour_is_a_circle_tangent_to_open_circuit() {
    let resistance = 0.5;
    let grid = smith_chart_grid(&[resistance], &[]);
    let line = grid
        .iter()
        .find(|line| line.kind == SmithChartGridLineKind::Resistance { value: resistance })
        .unwrap();
    let center = resistance / (1.0 + resistance);
    let radius = 1.0 / (1.0 + resistance);

    assert!(line.points.iter().all(|(x, y)| {
        ((x - center).hypot(*y) - radius).abs() < 1.0e-12 && x.hypot(*y) <= 1.0 + 1.0e-12
    }));
    assert!(
        line.points
            .iter()
            .any(|(x, y)| (*x - 1.0).abs() < 1.0e-12 && y.abs() < 1.0e-12)
    );
}

/// Verifies that constant-reactance contours remain inside the chart and end at open circuit.
#[test]
fn constant_reactance_contours_run_from_boundary_to_open_circuit() {
    let grid = smith_chart_grid(&[], &[-0.5, 0.5]);

    for reactance in [-0.5, 0.5] {
        let line = grid
            .iter()
            .find(|line| line.kind == SmithChartGridLineKind::Reactance { value: reactance })
            .unwrap();

        assert!(
            line.points
                .iter()
                .all(|(x, y)| x.hypot(*y) <= 1.0 + 1.0e-12)
        );
        assert_eq!(line.points.last().copied(), Some((1.0, 0.0)));
        assert_eq!(line.points[0].1.signum(), reactance.signum());
    }
}

/// Verifies that configured decimal contours are deduplicated and classified as minor.
#[test]
fn configured_decimal_contours_are_deduplicated_minor_lines() {
    let grid = smith_chart_grid(&[0.2, 0.2, f64::NAN, -1.0], &[0.5, 0.5, f64::INFINITY]);

    assert_eq!(
        grid.iter()
            .filter(|line| { line.kind == SmithChartGridLineKind::Resistance { value: 0.2 } })
            .count(),
        1
    );
    assert_eq!(
        grid.iter()
            .find(|line| { line.kind == SmithChartGridLineKind::Reactance { value: 0.5 } })
            .unwrap()
            .hierarchy,
        SmithChartGridHierarchy::Minor
    );
}
