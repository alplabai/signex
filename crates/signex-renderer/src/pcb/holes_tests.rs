//! Zone holes survive the PCB scene translation and stay unfilled (#861).

use super::*;
use crate::theme::ResolvedTheme;
use signex_gfx::pipeline::polygon::polygon_fill_triangles;
use signex_gfx::scene::{DirtyFlags, Scene};

fn centroid(t: &[[f32; 2]; 3]) -> [f32; 2] {
    [
        (t[0][0] + t[1][0] + t[2][0]) / 3.0,
        (t[0][1] + t[1][1] + t[2][1]) / 3.0,
    ]
}

/// Strict interior of the fixture's triangular hole (25,5) (32.5,5) (28.75,15).
fn in_triangle_hole([x, y]: [f32; 2]) -> bool {
    y > 5.0 && (y - 5.0) * 3.75 > (x - 25.0) * 10.0 && (y - 5.0) * -3.75 < (x - 32.5) * 10.0
}

#[test]
fn zone_with_holes_fixture_emits_holes_and_leaves_them_unfilled() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb/zone-with-holes.snxpcb");
    let text = std::fs::read_to_string(path).expect("read fixture");
    let board = signex_types::format::SnxPcb::parse(&text)
        .expect("parse fixture")
        .board;
    let snapshot = PcbSnapshot::from_board(&board);
    assert_eq!(snapshot.zones.len(), 1);
    assert_eq!(snapshot.zones[0].holes.len(), 2, "both holes carried");

    let theme = ResolvedTheme::builtin_default();
    let mut scene = Scene::default();
    PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::POLYGONS, &mut scene);
    let polygon = scene
        .polygons
        .iter()
        .find(|p| !p.holes.is_empty())
        .expect("emitted zone polygon carries its holes");
    assert_eq!(polygon.holes.len(), 2);

    let triangles = polygon_fill_triangles(polygon);
    assert!(!triangles.is_empty());

    // Outline minus both holes: pentagon 40x20 + triangle cap (shoelace),
    // square hole 10x7, triangular hole 7.5x10/2.
    let area: f32 = triangles
        .iter()
        .map(|t| {
            ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]))
                .abs()
                * 0.5
        })
        .sum();
    let outline = 40.0 * 20.0 + 40.0 * 15.0 * 0.5;
    let expected = outline - 10.0 * 7.0 - 7.5 * 10.0 * 0.5;
    assert!(
        (area - expected).abs() < 1e-2,
        "fill area {area}, want {expected}"
    );
    for t in &triangles {
        let c = centroid(t);
        let in_square = c[0] > 5.0 && c[0] < 15.0 && c[1] > 5.0 && c[1] < 12.0;
        assert!(
            !in_square && !in_triangle_hole(c),
            "fill triangle in a hole: {t:?}"
        );
    }
}
