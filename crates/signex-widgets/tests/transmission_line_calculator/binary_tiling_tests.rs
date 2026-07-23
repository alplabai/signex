use std::collections::BTreeMap;

use crate::transmission_line_calculator::binary_tiling::*;

/// Verifies that horocycle levels follow a power of two hierarchy.
#[test]
fn horocycle_levels_follow_a_power_of_two_hierarchy() {
    let levels = smith_binary_tiling()
        .iter()
        .filter_map(|edge| match edge.kind {
            BinaryTilingEdgeKind::Horocycle { level } => Some(level),
            BinaryTilingEdgeKind::Geodesic { .. } => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        levels,
        (MINIMUM_BINARY_TILING_LEVEL..=MAXIMUM_BINARY_TILING_LEVEL).collect::<Vec<_>>()
    );
    for levels in levels.windows(2) {
        assert_eq!(2.0_f64.powi(levels[1]) / 2.0_f64.powi(levels[0]), 2.0);
    }
}

/// Verifies that generated edges are finite and clipped to the unit disk.
#[test]
fn generated_edges_are_finite_and_clipped_to_the_unit_disk() {
    let tiling = smith_binary_tiling();

    assert!(!tiling.is_empty());
    for edge in tiling {
        assert!(edge.points.len() >= 2);
        for (x, y) in &edge.points {
            assert!(x.is_finite());
            assert!(y.is_finite());
            assert!(x.hypot(*y) <= 1.0 + 1.0e-12);
        }
    }
}

/// Verifies that adjacent levels form binary seams with major and minor styling.
#[test]
fn adjacent_levels_form_binary_seams_with_major_and_minor_styling() {
    let tiling = smith_binary_tiling();
    let hierarchy_by_level = tiling
        .iter()
        .filter_map(|edge| match edge.kind {
            BinaryTilingEdgeKind::Horocycle { level } => Some((level, edge.hierarchy)),
            BinaryTilingEdgeKind::Geodesic { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(hierarchy_by_level[&0], BinaryTilingHierarchy::Major);
    assert_eq!(hierarchy_by_level[&1], BinaryTilingHierarchy::Minor);

    for level in MINIMUM_BINARY_TILING_LEVEL..MAXIMUM_BINARY_TILING_LEVEL - 1 {
        let parent = tiling
            .iter()
            .find(|edge| edge.kind == BinaryTilingEdgeKind::Geodesic { level, column: 2 })
            .unwrap();
        let child = tiling
            .iter()
            .find(|edge| {
                edge.kind
                    == BinaryTilingEdgeKind::Geodesic {
                        level: level + 1,
                        column: 1,
                    }
            })
            .unwrap();

        let parent_end = parent.points.last().unwrap();
        let child_start = child.points.first().unwrap();
        assert!((parent_end.0 - child_start.0).abs() < 1.0e-12);
        assert!((parent_end.1 - child_start.1).abs() < 1.0e-12);
    }
}
