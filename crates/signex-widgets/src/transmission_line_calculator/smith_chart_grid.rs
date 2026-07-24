use std::sync::OnceLock;

use crate::transmission_line_calculator::{Complex, chart_point_from_normalized_impedance};

const GRID_SEGMENTS: usize = 96;
const MAXIMUM_GRID_VALUE: f64 = 50.0;
const MINIMUM_MINOR_SPACING: f64 = 0.008;
const CLIP_SEARCH_ITERATIONS: usize = 48;

const DECIMAL_GRID_RANGES: [DecimalGridRange; 8] = [
    DecimalGridRange::new(0, 10, 1),
    DecimalGridRange::new(10, 60, 2),
    DecimalGridRange::new(60, 100, 5),
    DecimalGridRange::new(100, 200, 10),
    DecimalGridRange::new(200, 600, 20),
    DecimalGridRange::new(600, 1000, 100),
    DecimalGridRange::new(1000, 2000, 200),
    DecimalGridRange::new(2000, 5000, 1000),
];

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

#[derive(Debug, Clone, Copy)]
struct DecimalGridRange {
    start_hundredths: i32,
    end_hundredths: i32,
    step_hundredths: i32,
}

impl DecimalGridRange {
    const fn new(start_hundredths: i32, end_hundredths: i32, step_hundredths: i32) -> Self {
        Self {
            start_hundredths,
            end_hundredths,
            step_hundredths,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DecimalGridValue {
    value: f64,
    hierarchy: SmithChartGridHierarchy,
}

/// Returns the shared adaptive decimal Smith-chart grid.
pub(crate) fn smith_chart_grid() -> &'static [SmithChartGridLine] {
    static GRID: OnceLock<Vec<SmithChartGridLine>> = OnceLock::new();
    GRID.get_or_init(generate_smith_chart_grid)
}

/// Generates decimal contours with progressively wider steps at larger values.
fn generate_smith_chart_grid() -> Vec<SmithChartGridLine> {
    let values = decimal_grid_values();
    let mut lines = Vec::with_capacity(values.len() * 3 - 1);

    for hierarchy in [
        SmithChartGridHierarchy::Minor,
        SmithChartGridHierarchy::Major,
    ] {
        for (index, grid_value) in values.iter().copied().enumerate() {
            if grid_value.hierarchy != hierarchy {
                continue;
            }
            let reactance_limit =
                minor_resistance_reactance_limit(&values, index, grid_value.hierarchy);
            lines.push(SmithChartGridLine {
                kind: SmithChartGridLineKind::Resistance {
                    value: grid_value.value,
                },
                hierarchy,
                points: constant_resistance_points(grid_value.value, reactance_limit),
            });
        }

        if hierarchy == SmithChartGridHierarchy::Major {
            lines.push(SmithChartGridLine {
                kind: SmithChartGridLineKind::Reactance { value: 0.0 },
                hierarchy,
                points: constant_reactance_points(0.0, None),
            });
        }
        for (index, grid_value) in values.iter().copied().enumerate().skip(1) {
            if grid_value.hierarchy != hierarchy {
                continue;
            }
            let resistance_limit =
                minor_reactance_resistance_limit(&values, index, grid_value.hierarchy);
            for value in [-grid_value.value, grid_value.value] {
                lines.push(SmithChartGridLine {
                    kind: SmithChartGridLineKind::Reactance { value },
                    hierarchy,
                    points: constant_reactance_points(value, resistance_limit),
                });
            }
        }
    }

    lines
}

/// Builds exact hundredth-based values so each numerical range can use a suitable step.
fn decimal_grid_values() -> Vec<DecimalGridValue> {
    let mut values = Vec::new();
    for range in DECIMAL_GRID_RANGES {
        let mut hundredths = range.start_hundredths;
        while hundredths < range.end_hundredths {
            if values
                .last()
                .is_none_or(|value: &DecimalGridValue| value.value != f64::from(hundredths) / 100.0)
            {
                values.push(decimal_grid_value(hundredths));
            }
            hundredths += range.step_hundredths;
        }
    }
    values.push(decimal_grid_value(5000));
    values
}

/// Applies a 1-2-5 hierarchy to the decimal contour values.
fn decimal_grid_value(hundredths: i32) -> DecimalGridValue {
    let hierarchy = if matches!(
        hundredths,
        0 | 2 | 5 | 10 | 20 | 50 | 100 | 200 | 500 | 1000 | 2000 | 5000
    ) {
        SmithChartGridHierarchy::Major
    } else {
        SmithChartGridHierarchy::Minor
    };
    DecimalGridValue {
        value: f64::from(hundredths) / 100.0,
        hierarchy,
    }
}

/// Limits a minor resistance circle where adjacent resistance contours become too close.
fn minor_resistance_reactance_limit(
    values: &[DecimalGridValue],
    index: usize,
    hierarchy: SmithChartGridHierarchy,
) -> Option<f64> {
    if hierarchy == SmithChartGridHierarchy::Major {
        return None;
    }
    let previous = values[index - 1].value;
    let value = values[index].value;
    let next = values[index + 1].value;
    Some(find_clip_limit(|reactance| {
        minimum_resistance_spacing(previous, value, next, reactance)
    }))
}

/// Limits a minor reactance arc where adjacent reactance contours become too close.
fn minor_reactance_resistance_limit(
    values: &[DecimalGridValue],
    index: usize,
    hierarchy: SmithChartGridHierarchy,
) -> Option<f64> {
    if hierarchy == SmithChartGridHierarchy::Major {
        return None;
    }
    let previous = values[index - 1].value;
    let value = values[index].value;
    let next = values[index + 1].value;
    Some(find_clip_limit(|resistance| {
        minimum_reactance_spacing(previous, value, next, resistance)
    }))
}

/// Finds the largest parameter for which adjacent mapped contours remain legible.
fn find_clip_limit(mut spacing: impl FnMut(f64) -> f64) -> f64 {
    if spacing(MAXIMUM_GRID_VALUE) >= MINIMUM_MINOR_SPACING {
        return MAXIMUM_GRID_VALUE;
    }

    let mut lower = 0.0;
    let mut upper = MAXIMUM_GRID_VALUE;
    for _ in 0..CLIP_SEARCH_ITERATIONS {
        let middle = (lower + upper) / 2.0;
        if spacing(middle) >= MINIMUM_MINOR_SPACING {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    lower
}

/// Measures the nearest resistance-neighbor distance in reflection-coefficient space.
fn minimum_resistance_spacing(previous: f64, value: f64, next: f64, reactance: f64) -> f64 {
    let point = chart_point_from_normalized_impedance(Complex::new(value, reactance));
    let previous_point = chart_point_from_normalized_impedance(Complex::new(previous, reactance));
    let next_point = chart_point_from_normalized_impedance(Complex::new(next, reactance));
    point_distance(point, previous_point).min(point_distance(point, next_point))
}

/// Measures the nearest reactance-neighbor distance in reflection-coefficient space.
fn minimum_reactance_spacing(previous: f64, value: f64, next: f64, resistance: f64) -> f64 {
    let point = chart_point_from_normalized_impedance(Complex::new(resistance, value));
    let previous_point = chart_point_from_normalized_impedance(Complex::new(resistance, previous));
    let next_point = chart_point_from_normalized_impedance(Complex::new(resistance, next));
    point_distance(point, previous_point).min(point_distance(point, next_point))
}

/// Computes the Euclidean distance between normalized chart points.
fn point_distance(first: (f64, f64), second: (f64, f64)) -> f64 {
    (first.0 - second.0).hypot(first.1 - second.1)
}

/// Samples a normalized constant-resistance circle, optionally clipped by reactance.
fn constant_resistance_points(resistance: f64, reactance_limit: Option<f64>) -> Vec<(f64, f64)> {
    let center = resistance / (resistance + 1.0);
    let radius = 1.0 / (resistance + 1.0);
    let start_angle = reactance_limit.map_or(0.0, |reactance| {
        let point = chart_point_from_normalized_impedance(Complex::new(resistance, reactance));
        point.1.atan2(point.0 - center)
    });
    let angle_span = std::f64::consts::TAU - 2.0 * start_angle;

    (0..=GRID_SEGMENTS)
        .map(|index| {
            let interpolation = index as f64 / GRID_SEGMENTS as f64;
            let angle = start_angle + angle_span * interpolation;
            (center + radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

/// Samples a normalized constant-reactance arc, optionally clipped by resistance.
fn constant_reactance_points(reactance: f64, resistance_limit: Option<f64>) -> Vec<(f64, f64)> {
    (0..=GRID_SEGMENTS)
        .map(|index| {
            if resistance_limit.is_none() && index == GRID_SEGMENTS {
                return (1.0, 0.0);
            }
            let interpolation = index as f64 / GRID_SEGMENTS as f64;
            let resistance = resistance_limit.map_or_else(
                || interpolation / (1.0 - interpolation),
                |limit| limit * interpolation,
            );
            chart_point_from_normalized_impedance(Complex::new(resistance, reactance))
        })
        .collect()
}
