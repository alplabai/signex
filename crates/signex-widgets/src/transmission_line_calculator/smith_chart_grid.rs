use crate::transmission_line_calculator::{Complex, chart_point_from_normalized_impedance};

const GRID_SEGMENTS: usize = 96;
const VALUE_EPSILON: f64 = 1.0e-12;

/// Selects the visual hierarchy used when drawing a Smith-chart grid line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SmithChartGridHierarchy {
    Major,
    Minor,
}

/// Identifies a normalized constant-resistance or constant-reactance contour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SmithChartGridLineKind {
    Resistance { value: f64 },
    Reactance { value: f64 },
}

/// Stores one conventional Smith-chart grid line in normalized chart coordinates.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SmithChartGridLine {
    pub(crate) kind: SmithChartGridLineKind,
    pub(crate) hierarchy: SmithChartGridHierarchy,
    pub(crate) points: Vec<(f64, f64)>,
}

/// Generates the conventional decimal Smith-chart grid.
pub(crate) fn smith_chart_grid(
    resistance_values: &[f64],
    reactance_values: &[f64],
) -> Vec<SmithChartGridLine> {
    let resistance_values = grid_values(&[0.0, 1.0], resistance_values, false);
    let reactance_values = grid_values(&[0.0, 1.0, -1.0], reactance_values, true);
    let mut lines = Vec::with_capacity(resistance_values.len() + reactance_values.len());

    for value in resistance_values {
        lines.push(SmithChartGridLine {
            kind: SmithChartGridLineKind::Resistance { value },
            hierarchy: hierarchy(SmithChartGridLineKind::Resistance { value }),
            points: constant_resistance_points(value),
        });
    }
    for value in reactance_values {
        lines.push(SmithChartGridLine {
            kind: SmithChartGridLineKind::Reactance { value },
            hierarchy: hierarchy(SmithChartGridLineKind::Reactance { value }),
            points: constant_reactance_points(value),
        });
    }

    lines
}

/// Normalizes, deduplicates, and orders configured grid values.
fn grid_values(required: &[f64], configured: &[f64], signed: bool) -> Vec<f64> {
    let mut values = required.to_vec();
    for value in configured
        .iter()
        .copied()
        .filter(|value| value.is_finite() && (signed || *value >= 0.0))
    {
        if !values
            .iter()
            .any(|existing| (*existing - value).abs() <= VALUE_EPSILON)
        {
            values.push(value);
        }
    }
    values.sort_by(f64::total_cmp);
    values
}

/// Classifies the familiar Smith-chart reference contours as major.
fn hierarchy(kind: SmithChartGridLineKind) -> SmithChartGridHierarchy {
    let value = match kind {
        SmithChartGridLineKind::Resistance { value } => value,
        SmithChartGridLineKind::Reactance { value } => value.abs(),
    };
    if value <= VALUE_EPSILON || (value - 1.0).abs() <= VALUE_EPSILON {
        SmithChartGridHierarchy::Major
    } else {
        SmithChartGridHierarchy::Minor
    }
}

/// Samples a normalized constant-resistance circle.
fn constant_resistance_points(resistance: f64) -> Vec<(f64, f64)> {
    let center = resistance / (resistance + 1.0);
    let radius = 1.0 / (resistance + 1.0);
    (0..=GRID_SEGMENTS)
        .map(|index| {
            let angle = index as f64 * std::f64::consts::TAU / GRID_SEGMENTS as f64;
            (center + radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

/// Samples a normalized constant-reactance arc from short to open circuit.
fn constant_reactance_points(reactance: f64) -> Vec<(f64, f64)> {
    (0..=GRID_SEGMENTS)
        .map(|index| {
            if index == GRID_SEGMENTS {
                return (1.0, 0.0);
            }
            let interpolation = index as f64 / GRID_SEGMENTS as f64;
            let resistance = interpolation / (1.0 - interpolation);
            chart_point_from_normalized_impedance(Complex::new(resistance, reactance))
        })
        .collect()
}
