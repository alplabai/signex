//! `PreviewFootprint::scene` against the editor's silk lowering (#3116): the
//! same graphics must give the same primitives, and the scene must stay
//! inside the extent `PreviewFootprint::bbox_mm` computes from the graphics
//! alone (the box the preview fits to).

use signex_library::primitive::Footprint;
use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

use super::PreviewFootprint;

fn footprint() -> Footprint {
    let mut fp = Footprint::empty("PARITY");
    fp.silk_f = [
        FpGraphicKind::Line {
            from: [-2.0, -1.0],
            to: [2.0, -1.0],
        },
        FpGraphicKind::Rectangle {
            from: [-1.0, -0.5],
            to: [1.0, 0.5],
        },
        FpGraphicKind::Circle {
            center: [0.0, 2.0],
            radius: 0.5,
        },
        FpGraphicKind::Arc {
            center: [3.0, 0.0],
            radius: 1.0,
            start_deg: 300.0,
            end_deg: 60.0,
        },
        FpGraphicKind::Polygon {
            vertices: vec![[4.0, 3.0], [5.0, 3.0], [5.0, 4.0]],
        },
    ]
    .into_iter()
    .map(|kind| FpGraphic::drawn(kind, 0.12, false))
    .collect();
    fp
}

#[test]
fn each_silk_graphic_lowers_to_the_primitives_the_canvas_strokes() {
    let preview = PreviewFootprint::build(&footprint());
    let scene = preview.scene();
    // Line 1 + rectangle 4 edges + triangle 3 edges, as the canvas strokes them.
    assert_eq!(scene.lines.len(), 1 + 4 + 3);
    assert_eq!(scene.circles.len(), 1);
    assert_eq!(
        (scene.circles[0].center, scene.circles[0].radius),
        ([0.0, 2.0], 0.5)
    );
    assert_eq!(scene.arcs.len(), 1);
    // 300 -> 60 degrees wraps through zero: a 120 degree sweep.
    let arc = scene.arcs[0];
    let sweep = (arc.end_angle - arc.start_angle).to_degrees();
    assert!((sweep - 120.0).abs() < 1e-3, "{sweep}");
    assert!(scene.lines.iter().all(|l| (l.width - 0.12).abs() < 1e-6));
}

#[test]
fn the_scene_stays_inside_the_preview_extent() {
    let preview = PreviewFootprint::build(&footprint());
    let (min_x, min_y, max_x, max_y) = preview.bbox_mm().expect("silk gives an extent");
    let scene = preview.scene();
    let eps = 1e-4;
    let points = scene
        .lines
        .iter()
        .flat_map(|l| [l.p0, l.p1])
        .chain(scene.circles.iter().flat_map(|c| {
            [
                [c.center[0] - c.radius, c.center[1] - c.radius],
                [c.center[0] + c.radius, c.center[1] + c.radius],
            ]
        }))
        .chain(scene.arcs.iter().flat_map(|a| {
            [
                [a.center[0] - a.radius, a.center[1] - a.radius],
                [a.center[0] + a.radius, a.center[1] + a.radius],
            ]
        }));
    for p in points {
        assert!(
            f64::from(p[0]) >= min_x - eps && f64::from(p[0]) <= max_x + eps,
            "x {} outside {min_x}..{max_x}",
            p[0]
        );
        assert!(
            f64::from(p[1]) >= min_y - eps && f64::from(p[1]) <= max_y + eps,
            "y {} outside {min_y}..{max_y}",
            p[1]
        );
    }
    // And the extent is the graphics': the far corner is the triangle's.
    assert!(
        (max_x - 5.0).abs() < eps && (max_y - 4.0).abs() < eps,
        "{max_x} {max_y}"
    );
}
