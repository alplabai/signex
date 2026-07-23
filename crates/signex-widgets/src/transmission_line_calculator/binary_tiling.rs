use std::sync::OnceLock;

use crate::transmission_line_calculator::{Complex, chart_point_from_normalized_impedance};

pub(crate) const MINIMUM_BINARY_TILING_LEVEL: i32 = -4;
pub(crate) const MAXIMUM_BINARY_TILING_LEVEL: i32 = 4;

const MAXIMUM_COLUMN: i32 = 12;
const HOROCYCLE_SEGMENTS: usize = 96;
const GEODESIC_SEGMENTS: usize = 12;
const MINIMUM_EDGE_LENGTH: f64 = 0.004;

/// Selects the visual hierarchy used when drawing a binary-tiling edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinaryTilingHierarchy {
    Major,
    Minor,
}

/// Identifies a horocycle or geodesic in the binary tiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinaryTilingEdgeKind {
    Horocycle { level: i32 },
    Geodesic { level: i32, column: i32 },
}

/// Stores one sampled binary-tiling edge in normalized Smith-chart coordinates.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BinaryTilingEdge {
    pub(crate) kind: BinaryTilingEdgeKind,
    pub(crate) hierarchy: BinaryTilingHierarchy,
    pub(crate) points: Vec<(f64, f64)>,
}

/// Returns the lazily generated, shared Smith-chart binary tiling.
pub(crate) fn smith_binary_tiling() -> &'static [BinaryTilingEdge] {
    static TILING: OnceLock<Vec<BinaryTilingEdge>> = OnceLock::new();
    TILING.get_or_init(generate_smith_binary_tiling)
}

/// Generates smith binary tiling from the supplied configuration.
fn generate_smith_binary_tiling() -> Vec<BinaryTilingEdge> {
    let mut edges = Vec::new();

    for level in MINIMUM_BINARY_TILING_LEVEL..=MAXIMUM_BINARY_TILING_LEVEL {
        edges.push(BinaryTilingEdge {
            kind: BinaryTilingEdgeKind::Horocycle { level },
            hierarchy: hierarchy_for_level(level),
            points: horocycle_points(2.0_f64.powi(level)),
        });
    }

    for level in MINIMUM_BINARY_TILING_LEVEL..MAXIMUM_BINARY_TILING_LEVEL {
        let resistance = 2.0_f64.powi(level);
        for column in -MAXIMUM_COLUMN..=MAXIMUM_COLUMN {
            let points = geodesic_segment_points(resistance, column);
            if polyline_length(&points) < MINIMUM_EDGE_LENGTH {
                continue;
            }
            edges.push(BinaryTilingEdge {
                kind: BinaryTilingEdgeKind::Geodesic { level, column },
                hierarchy: hierarchy_for_level(level),
                points,
            });
        }
    }

    edges
}

/// Classifies a binary-tiling level as a major or minor guide.
fn hierarchy_for_level(level: i32) -> BinaryTilingHierarchy {
    if level.rem_euclid(2) == 0 {
        BinaryTilingHierarchy::Major
    } else {
        BinaryTilingHierarchy::Minor
    }
}

/// Samples a constant-resistance horocycle inside the unit disk.
fn horocycle_points(resistance: f64) -> Vec<(f64, f64)> {
    let center = resistance / (resistance + 1.0);
    let radius = 1.0 / (resistance + 1.0);
    (0..=HOROCYCLE_SEGMENTS)
        .map(|index| {
            let angle = index as f64 * std::f64::consts::TAU / HOROCYCLE_SEGMENTS as f64;
            (center + radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

/// Samples one geodesic seam between adjacent power-of-two levels.
fn geodesic_segment_points(resistance: f64, column: i32) -> Vec<(f64, f64)> {
    let reactance = f64::from(column) * resistance;
    (0..=GEODESIC_SEGMENTS)
        .map(|index| {
            let interpolation = index as f64 / GEODESIC_SEGMENTS as f64;
            let resistance = resistance * 2.0_f64.powf(interpolation);
            chart_point_from_normalized_impedance(Complex::new(resistance, reactance))
        })
        .collect()
}

/// Computes the Euclidean length of a sampled polyline.
fn polyline_length(points: &[(f64, f64)]) -> f64 {
    points
        .windows(2)
        .map(|points| (points[1].0 - points[0].0).hypot(points[1].1 - points[0].1))
        .sum()
}
