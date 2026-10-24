//! Holes in a `GpuPolygon` must stay unfilled in the GPU triangulation (#861).

use super::*;

fn square(lo: f32, hi: f32) -> Vec<[f32; 2]> {
    vec![[lo, lo], [hi, lo], [hi, hi], [lo, hi]]
}

fn fill_triangles(vertices: &[PolygonVertex]) -> Vec<[[f32; 2]; 3]> {
    vertices
        .chunks_exact(3)
        .map(|t| [t[0].position, t[1].position, t[2].position])
        .collect()
}

fn area(t: &[[f32; 2]; 3]) -> f32 {
    ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])).abs()
        * 0.5
}

fn centroid(t: &[[f32; 2]; 3]) -> [f32; 2] {
    [
        (t[0][0] + t[1][0] + t[2][0]) / 3.0,
        (t[0][1] + t[1][1] + t[2][1]) / 3.0,
    ]
}

fn in_square(p: [f32; 2], lo: f32, hi: f32) -> bool {
    p[0] > lo && p[0] < hi && p[1] > lo && p[1] < hi
}

#[test]
fn square_with_square_hole_fills_outer_minus_hole() {
    let polygon = GpuPolygon {
        vertices: square(0.0, 10.0),
        holes: vec![square(4.0, 6.0)],
        fill_color: [1.0; 4],
        ..Default::default()
    };
    let triangles = fill_triangles(&triangulate_polygons(&[polygon]));
    let total: f32 = triangles.iter().map(area).sum();
    assert!((total - 96.0).abs() < 1e-3, "area {total}, want 96");
    for t in &triangles {
        assert!(!in_square(centroid(t), 4.0, 6.0), "triangle in hole: {t:?}");
    }
}

#[test]
fn malformed_hole_is_skipped_and_hole_is_stroked() {
    let skipped = GpuPolygon {
        vertices: square(0.0, 10.0),
        holes: vec![vec![[4.0, 4.0], [6.0, 6.0]]],
        fill_color: [1.0; 4],
        ..Default::default()
    };
    let total: f32 = fill_triangles(&triangulate_polygons(&[skipped]))
        .iter()
        .map(area)
        .sum();
    assert!((total - 100.0).abs() < 1e-3);

    let stroked = |holes| GpuPolygon {
        vertices: square(0.0, 10.0),
        holes,
        stroke_color: Some([1.0; 4]),
        stroke_width: 0.1,
        ..Default::default()
    };
    let stroke_vertices = |holes| {
        triangulate_polygons(&[stroked(holes)])
            .iter()
            .filter(|v| v.half_width > 0.0)
            .count()
    };
    // Six vertices per edge quad: 4 outer edges, plus 4 hole edges.
    assert_eq!(stroke_vertices(Vec::new()), 24);
    assert_eq!(stroke_vertices(vec![square(4.0, 6.0)]), 48);
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<[f32; 2]> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

fn filled_area(polygon: &GpuPolygon) -> f32 {
    fill_triangles(&triangulate_polygons(std::slice::from_ref(polygon)))
        .iter()
        .map(area)
        .sum()
}

#[test]
fn stacked_and_grid_holes_keep_their_area() {
    let stacked = GpuPolygon {
        vertices: rect(0.0, 0.0, 10.0, 20.0),
        holes: vec![rect(4.0, 2.0, 6.0, 4.0), rect(4.0, 8.0, 6.0, 10.0)],
        fill_color: [1.0; 4],
        ..Default::default()
    };
    assert!((filled_area(&stacked) - 192.0).abs() < 1e-3);

    let grid = GpuPolygon {
        vertices: rect(0.0, 0.0, 70.0, 70.0),
        holes: (0..6)
            .flat_map(|r| (0..6).map(move |c| (r as f32, c as f32)))
            .map(|(r, c)| {
                rect(
                    10.0 * c + 7.0,
                    10.0 * r + 7.0,
                    10.0 * c + 12.0,
                    10.0 * r + 12.0,
                )
            })
            .collect(),
        fill_color: [1.0; 4],
        ..Default::default()
    };
    assert!((filled_area(&grid) - (4900.0 - 36.0 * 25.0)).abs() < 1e-2);
}

#[test]
fn untriangulable_holes_fall_back_to_the_solid_outline() {
    // Overlapping holes break the disjoint-holes invariant, so the bridged
    // ring cannot partition outer-minus-holes. The zone must still draw.
    let polygon = GpuPolygon {
        vertices: rect(0.0, 0.0, 10.0, 10.0),
        holes: vec![rect(2.0, 2.0, 6.0, 6.0), rect(4.0, 4.0, 8.0, 8.0)],
        fill_color: [1.0; 4],
        ..Default::default()
    };
    let total = filled_area(&polygon);
    assert!(
        (total - 100.0).abs() < 1e-3,
        "expected the solid fill, got {total}"
    );
}

/// A hole that reaches the outline breaks the `Zone.holes` invariant
/// (strictly inside). It must still draw, as the solid outline, rather than
/// with a wrong partial fill.
#[test]
fn holes_touching_the_outline_fall_back_to_the_solid_outline() {
    let polygon = GpuPolygon {
        vertices: rect(0.0, 0.0, 10.0, 20.0),
        holes: vec![rect(6.0, 4.0, 10.0, 6.0), rect(6.0, 12.0, 10.0, 14.0)],
        fill_color: [1.0; 4],
        ..Default::default()
    };
    let total = filled_area(&polygon);
    assert!(
        (total - 200.0).abs() < 1e-3 || (total - 184.0).abs() < 1e-3,
        "either the solid fill or the exact cut-out fill, got {total}"
    );
}
