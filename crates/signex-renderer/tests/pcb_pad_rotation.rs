//! A footprint's rotation turns its pad shapes, not just the pad centres (#680).

use signex_gfx::scene::{DirtyFlags, Scene};
use signex_renderer::pcb::{PcbRenderer, PcbSnapshot};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;
use signex_types::pcb::PcbBoard;

const TOL: f32 = 1e-4;

/// One footprint at the origin holding one 2.0 x 1.0 mm pad at the origin.
fn pad_polygon(shape: &str, footprint_rotation_deg: f64) -> Vec<[f32; 2]> {
    let board: PcbBoard = serde_json::from_str(&format!(
        r#"{{
          "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
          "generator": "test", "thickness": 1.6, "outline": [],
          "layers": [{{ "id": 0, "name": "F.Cu", "layer_type": "signal" }}],
          "setup": null, "nets": [],
          "footprints": [{{
            "uuid": "8e98ba95-1d0f-4d9f-84e0-27ddf90e10b5",
            "position": {{ "x": 0.0, "y": 0.0 }},
            "rotation": {footprint_rotation_deg}, "layer": "F.Cu",
            "pads": [{{
              "uuid": "47ceef20-d496-4e62-b8d8-7c34195c6818",
              "pad_type": "smd", "shape": "{shape}",
              "position": {{ "x": 0.0, "y": 0.0 }},
              "size": {{ "x": 2.0, "y": 1.0 }},
              "drill": null, "layers": ["F.Cu"], "net": null
            }}]
          }}],
          "segments": [], "vias": [], "zones": []
        }}"#
    ))
    .expect("valid board");
    let snapshot = PcbSnapshot::from_board(&board);
    let mut scene = Scene::default();
    PcbRenderer::build_scene(
        &snapshot,
        &ResolvedTheme::builtin_default(),
        DirtyFlags::POLYGONS,
        &mut scene,
    );
    scene.top_polygons[0].vertices.clone()
}

fn extent(vertices: &[[f32; 2]]) -> [f32; 2] {
    let span = |axis: usize| {
        let max = vertices.iter().map(|v| v[axis]).fold(f32::MIN, f32::max);
        let min = vertices.iter().map(|v| v[axis]).fold(f32::MAX, f32::min);
        max - min
    };
    [span(0), span(1)]
}

/// Un-rotate a vertex into the pad's own frame.
fn to_pad_frame(vertex: [f32; 2], rotation_deg: f32) -> [f32; 2] {
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    [
        vertex[0] * cos + vertex[1] * sin,
        -vertex[0] * sin + vertex[1] * cos,
    ]
}

#[test]
fn rect_pad_quarter_turn_swaps_extent() {
    let extent = extent(&pad_polygon("rect", 90.0));
    assert!((extent[0] - 1.0).abs() < TOL, "width {}", extent[0]);
    assert!((extent[1] - 2.0).abs() < TOL, "height {}", extent[1]);
}

#[test]
fn rect_pad_unrotated_keeps_extent() {
    let extent = extent(&pad_polygon("rect", 0.0));
    assert!((extent[0] - 2.0).abs() < TOL && (extent[1] - 1.0).abs() < TOL);
}

#[test]
fn rect_pad_45_degrees_lies_on_the_diagonal() {
    let vertices = pad_polygon("rect", 45.0);
    for vertex in &vertices {
        let local = to_pad_frame(*vertex, 45.0);
        assert!((local[0].abs() - 1.0).abs() < TOL, "{local:?}");
        assert!((local[1].abs() - 0.5).abs() < TOL, "{local:?}");
    }
    let extent = extent(&vertices);
    let expected = 3.0 / std::f32::consts::SQRT_2;
    assert!((extent[0] - expected).abs() < TOL && (extent[1] - expected).abs() < TOL);
}

#[test]
fn oval_pad_follows_footprint_rotation() {
    for rotation in [90.0_f32, 45.0] {
        for vertex in pad_polygon("oval", f64::from(rotation)) {
            let local = to_pad_frame(vertex, rotation);
            let on_ellipse = local[0].powi(2) / 1.0 + local[1].powi(2) / 0.25;
            assert!((on_ellipse - 1.0).abs() < TOL, "{rotation}: {local:?}");
        }
    }
}
