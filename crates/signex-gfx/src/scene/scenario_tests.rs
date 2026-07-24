//! Full-board scenario coverage for the CPU side of the GPU render path.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.
//!
//! We cannot visually verify actual GPU shader pixel output without hardware,
//! so this module pins everything up to the GPU boundary instead: a single
//! [`Scene`] populated across *every* primitive bucket, the CPU-side
//! predicates (`is_dashed`/`is_filled`/`is_stroked`) that decide how each
//! primitive is meant to render, the CPU-side vertex generation
//! (`triangulate_polygons`) the GPU polygon pipeline uploads verbatim, and
//! the draw order both renderers are supposed to share. It deliberately does
//! NOT duplicate `order`'s own tests (the const-array parity checks and
//! `overlays_composite_above_base_buckets`) — it builds on top of them with
//! real, concretely-populated scene content.

use super::{CPU_PCB_DRAW_ORDER, GPU_SCENE_DRAW_ORDER, Scene, SceneBucket};
use crate::pipeline::polygon::triangulate_polygons;
use crate::primitive::arc::Arc;
use crate::primitive::circle::Circle;
use crate::primitive::line::LineSegment;
use crate::primitive::polygon::GpuPolygon;
use crate::primitive::text::{TextHAlign, TextItem, TextVAlign};

fn solid_line(x1: f32, y1: f32, x2: f32, y2: f32) -> LineSegment {
    LineSegment {
        p0: [x1, y1],
        p1: [x2, y2],
        width: 0.2,
        color: [0.1, 0.8, 0.1, 1.0],
        style: 0,
        _pad: 0,
    }
}

fn dashed_line(x1: f32, y1: f32, x2: f32, y2: f32) -> LineSegment {
    LineSegment {
        style: LineSegment::STYLE_DASHED,
        ..solid_line(x1, y1, x2, y2)
    }
}

fn filled_circle(cx: f32, cy: f32, r: f32) -> Circle {
    Circle {
        center: [cx, cy],
        radius: r,
        stroke_width: 0.0,
        color: [0.9, 0.2, 0.2, 1.0],
    }
}

fn outline_circle(cx: f32, cy: f32, r: f32) -> Circle {
    Circle {
        center: [cx, cy],
        radius: r,
        stroke_width: 0.15,
        color: [0.2, 0.2, 0.9, 1.0],
    }
}

/// Convex quad standing in for a filled SMD pad: fill only, no stroke.
fn convex_filled_pad() -> GpuPolygon {
    GpuPolygon {
        vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
        fill_color: [0.85, 0.65, 0.13, 1.0],
        stroke_color: None,
        stroke_width: 0.0,
    }
}

/// A genuinely non-convex, 6-vertex notched contour standing in for a real
/// copper pour/zone routed around an obstacle. Concave at `[12.0, 2.0]` (an
/// inward corner) — an L-shape, not a fan-friendly convex polygon.
fn concave_zone() -> GpuPolygon {
    GpuPolygon {
        vertices: vec![
            [10.0, 0.0],
            [14.0, 0.0],
            [14.0, 4.0],
            [12.0, 4.0],
            [12.0, 2.0],
            [10.0, 2.0],
        ],
        fill_color: [0.2, 0.5, 0.9, 0.5],
        stroke_color: None,
        stroke_width: 0.0,
    }
}

/// Filled AND stroked polygon (e.g. a courtyard/silkscreen shape with both a
/// fill and an outline).
fn filled_and_stroked_polygon() -> GpuPolygon {
    GpuPolygon {
        vertices: vec![[20.0, 0.0], [22.0, 0.0], [21.0, 2.0]],
        fill_color: [0.3, 0.3, 0.3, 1.0],
        stroke_color: Some([1.0, 1.0, 1.0, 1.0]),
        stroke_width: 0.1,
    }
}

/// Outline-only rule/keepout area: fully transparent fill (alpha 0), the
/// stroke carries all the visible signal.
fn outline_only_rule_area() -> GpuPolygon {
    GpuPolygon {
        vertices: vec![[30.0, 0.0], [32.0, 0.0], [32.0, 2.0], [30.0, 2.0]],
        fill_color: [0.0, 0.0, 0.0, 0.0],
        stroke_color: Some([0.9, 0.1, 0.1, 1.0]),
        stroke_width: 0.08,
    }
}

/// One [`Scene`], every bucket populated: this is the shape a real board with
/// an active selection/highlight and ERC markers hands to both the CPU
/// canvas renderer and the GPU scene shader.
fn build_full_board_scene() -> Scene {
    let mut scene = Scene::default();

    scene.lines = vec![
        solid_line(0.0, 0.0, 10.0, 0.0),
        solid_line(0.0, 5.0, 10.0, 5.0),
        dashed_line(0.0, 10.0, 10.0, 10.0),
    ];
    scene.circles = vec![filled_circle(0.0, 0.0, 1.0), outline_circle(5.0, 5.0, 1.0)];
    scene.arcs = vec![Arc {
        center: [0.0, 0.0],
        radius: 3.0,
        start_angle: 0.0,
        end_angle: std::f32::consts::FRAC_PI_2,
        width: 0.2,
        color: [0.5, 0.5, 0.5, 1.0],
        _pad: [0.0; 3],
    }];
    scene.polygons = vec![
        convex_filled_pad(),
        concave_zone(),
        filled_and_stroked_polygon(),
        outline_only_rule_area(),
    ];
    scene.texts = vec![TextItem {
        content: "R1".to_string(),
        position: [1.0, 1.0],
        size_mm: 1.0,
        color: [1.0, 1.0, 1.0, 1.0],
        bold: false,
        italic: false,
        rotation: 0.0,
        h_align: TextHAlign::Center,
        v_align: TextVAlign::Center,
    }];
    scene.overlay_lines = vec![solid_line(0.0, -1.0, 10.0, -1.0)];
    scene.overlay_circles = vec![filled_circle(-5.0, -5.0, 0.5)];
    scene.overlay_polygons = vec![GpuPolygon {
        vertices: vec![[40.0, 0.0], [42.0, 0.0], [42.0, 2.0], [40.0, 2.0]],
        fill_color: [0.6, 0.6, 1.0, 0.4],
        stroke_color: None,
        stroke_width: 0.0,
    }];
    scene.erc_marker_lines = vec![solid_line(-1.0, -1.0, -2.0, -2.0)];
    scene.erc_marker_circles = vec![outline_circle(-3.0, -3.0, 0.3)];
    scene.erc_marker_polygons = vec![GpuPolygon {
        vertices: vec![[50.0, 0.0], [52.0, 0.0], [51.0, 2.0]],
        fill_color: [1.0, 0.0, 0.0, 1.0],
        stroke_color: None,
        stroke_width: 0.0,
    }];

    scene
}

#[test]
fn full_board_scenario_populates_every_bucket() {
    let scene = build_full_board_scene();
    assert_eq!(scene.lines.len(), 3, "2 solid + 1 dashed");
    assert_eq!(scene.circles.len(), 2, "1 filled + 1 outline");
    assert_eq!(scene.arcs.len(), 1);
    assert_eq!(
        scene.polygons.len(),
        4,
        "convex pad, concave zone, filled+stroked, outline-only"
    );
    assert_eq!(scene.texts.len(), 1);
    assert_eq!(scene.overlay_lines.len(), 1);
    assert_eq!(scene.overlay_circles.len(), 1);
    assert_eq!(scene.overlay_polygons.len(), 1);
    assert_eq!(scene.erc_marker_lines.len(), 1);
    assert_eq!(scene.erc_marker_circles.len(), 1);
    assert_eq!(scene.erc_marker_polygons.len(), 1);
    assert!(!scene.is_empty());
}

#[test]
fn line_style_bit_is_preserved_in_the_scene_ir() {
    // The dash style bit is now honoured on both engines (GPU pixel-verified
    // by `debug_pass::tests::line_wgsl_actually_renders_a_dashed_pattern`) —
    // not re-tested here. This only asserts the Rust-side Scene IR faithfully
    // carries the style bit for every line either renderer has to honour.
    let scene = build_full_board_scene();
    let dashed = scene.lines.iter().filter(|l| l.is_dashed()).count();
    let solid = scene.lines.iter().filter(|l| !l.is_dashed()).count();
    assert_eq!(dashed, 1);
    assert_eq!(solid, 2);
}

#[test]
fn circle_and_polygon_predicates_split_correctly_across_the_scenario() {
    let scene = build_full_board_scene();

    assert!(
        scene.circles[0].is_filled(),
        "stroke_width == 0.0 => filled"
    );
    assert!(
        !scene.circles[1].is_filled(),
        "stroke_width > 0.0 => outline"
    );

    let pad = &scene.polygons[0];
    let zone = &scene.polygons[1];
    let filled_stroked = &scene.polygons[2];
    let outline_only = &scene.polygons[3];

    assert!(!pad.is_stroked(), "plain pad: fill only, no outline");
    assert!(!zone.is_stroked(), "zone in this scenario: fill only");
    assert!(filled_stroked.is_stroked());
    assert!(
        outline_only.is_stroked(),
        "transparent fill does not stop the stroke predicate: colour+width is all that matters"
    );
}

#[test]
fn triangulate_convex_pad_fans_exactly_n_minus_2_fill_triangles() {
    let pad = convex_filled_pad();
    let n = pad.vertices.len();
    let vertices = triangulate_polygons(std::slice::from_ref(&pad));
    assert_eq!(vertices.len(), (n - 2) * 3);
}

/// Shoelace area of a closed contour — the ground truth a correct
/// triangulation's total triangle area must equal exactly, whether the
/// contour is convex or concave.
fn shoelace_area(points: &[[f32; 2]]) -> f64 {
    let mut sum = 0.0f64;
    for i in 0..points.len() {
        let [x0, y0] = points[i].map(f64::from);
        let [x1, y1] = points[(i + 1) % points.len()].map(f64::from);
        sum += x0 * y1 - x1 * y0;
    }
    (sum / 2.0).abs()
}

fn triangle_area(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f64 {
    shoelace_area(&[a, b, c])
}

#[test]
fn concave_zone_fills_exactly_its_area() {
    // #3 (fixed): `append_fill` triangulates via `signex_sketch::ear_clip`,
    // which partitions a concave contour exactly instead of fanning it from
    // vertex 0 — the fan's over-fill-across-the-notch bug no longer applies.
    // A vertex count alone can't prove this (ear-clip still emits
    // `(n - 2)` triangles for this contour, same as a fan would), so this
    // checks the actual covered area against the contour's shoelace area,
    // matching the CPU `frame.fill` (lyon) result exactly.
    let zone = concave_zone();
    let n = zone.vertices.len();
    assert!(
        n >= 6,
        "scenario requires a genuinely non-trivial concave contour"
    );

    let vertices = triangulate_polygons(std::slice::from_ref(&zone));
    assert_eq!(vertices.len(), (n - 2) * 3);

    let expected_area = shoelace_area(&zone.vertices);
    let total_area: f64 = vertices
        .chunks_exact(3)
        .map(|tri| triangle_area(tri[0].position, tri[1].position, tri[2].position))
        .sum();
    assert!(
        (total_area - expected_area).abs() < 1e-6,
        "concave fill must cover exactly the contour's area: expected \
         {expected_area}, got {total_area} (a fan would over-fill across \
         the notch)"
    );
}

#[test]
fn triangulate_filled_and_stroked_polygon_appends_stroke_after_fill() {
    let polygon = filled_and_stroked_polygon();
    let n = polygon.vertices.len();
    let fill_count = (n - 2) * 3;
    let stroke_count = n * 6; // one edge-quad (6 vertices) per closed-contour edge
    let vertices = triangulate_polygons(std::slice::from_ref(&polygon));
    assert_eq!(vertices.len(), fill_count + stroke_count);
}

#[test]
fn triangulate_outline_only_rule_area_still_emits_a_stroke() {
    // A "no visible fill" rule area (fill alpha 0) is a colour choice, not a
    // vertex-count difference: `triangulate_polygons` decides purely off
    // `stroke_color`/`stroke_width` (via `is_stroked`), never off fill alpha,
    // so it still emits both the fan fill and the stroke outline.
    let rule_area = outline_only_rule_area();
    assert_eq!(
        rule_area.fill_color[3], 0.0,
        "scenario fixture must have an invisible fill"
    );
    assert!(rule_area.is_stroked());

    let n = rule_area.vertices.len();
    let fill_count = (n - 2) * 3;
    let stroke_count = n * 6;
    let vertices = triangulate_polygons(std::slice::from_ref(&rule_area));
    assert_eq!(vertices.len(), fill_count + stroke_count);
}

#[test]
fn triangulate_the_full_scenario_polygon_batch_matches_the_per_polygon_sum() {
    let scene = build_full_board_scene();
    let per_polygon_total: usize = scene
        .polygons
        .iter()
        .map(|polygon| triangulate_polygons(std::slice::from_ref(polygon)).len())
        .sum();
    let batch_total = triangulate_polygons(&scene.polygons).len();
    assert_eq!(
        batch_total, per_polygon_total,
        "batching polygons must not change per-polygon vertex counts"
    );
    // Concretely: pad(6, fill only) + zone(12, over-filled fill only)
    // + filled_and_stroked(3 fill + 18 stroke = 21)
    // + outline_only(6 fill + 24 stroke = 30) = 69.
    assert_eq!(batch_total, 69);
}

fn bucket_count(scene: &Scene, bucket: SceneBucket) -> usize {
    match bucket {
        SceneBucket::Lines => scene.lines.len(),
        SceneBucket::Circles => scene.circles.len(),
        SceneBucket::Arcs => scene.arcs.len(),
        SceneBucket::Polygons => scene.polygons.len(),
        SceneBucket::Texts => scene.texts.len(),
        SceneBucket::OverlayLines => scene.overlay_lines.len(),
        SceneBucket::OverlayCircles => scene.overlay_circles.len(),
        SceneBucket::OverlayPolygons => scene.overlay_polygons.len(),
        SceneBucket::ErcMarkerLines => scene.erc_marker_lines.len(),
        SceneBucket::ErcMarkerCircles => scene.erc_marker_circles.len(),
        SceneBucket::ErcMarkerPolygons => scene.erc_marker_polygons.len(),
    }
}

/// The buckets of `order`, in the order they appear, restricted to the ones
/// this concrete scene actually has geometry in -- the real, drawn paint
/// sequence rather than the abstract bucket-name list.
fn nonempty_bucket_sequence(scene: &Scene, order: &[SceneBucket]) -> Vec<SceneBucket> {
    order
        .iter()
        .copied()
        .filter(|bucket| bucket_count(scene, *bucket) > 0)
        .collect()
}

/// #4 (fixed for overlays), tied to real content: `scene::order`'s own
/// `overlays_composite_above_base_buckets` pins the abstract per-bucket
/// constants; this walks the SAME full-board scene used above — every bucket
/// actually holds geometry, including the overlay trio — and asserts the
/// concrete consequence now holds: an overlay meant to sit on top of copper
/// (the active-layer zone highlight, selection highlight, or a DRC marker)
/// lands there on BOTH engines. On GPU this holds structurally — no overlay
/// bucket is ever a member of `GPU_SCENE_DRAW_ORDER`, so the scene shader's
/// dedicated overlay pass runs strictly after every base bucket regardless of
/// scene content — and on CPU, every populated overlay bucket's position in
/// `CPU_PCB_DRAW_ORDER` is after every populated base bucket's. (The base
/// `Polygons`-vs-`Lines` order — pad/pour geometry, not overlays — remains a
/// separate, deliberately open question pinned exactly by `scene::order`'s
/// own `gpu_scene_draw_order_is_fills_then_strokes_then_text` /
/// `cpu_pcb_draw_order_is_main_geometry_then_overlays`, unaffected by this
/// fix.)
#[test]
fn overlays_composite_above_base_content_in_a_fully_populated_scene() {
    let scene = build_full_board_scene();
    let base_buckets = [
        SceneBucket::Lines,
        SceneBucket::Circles,
        SceneBucket::Polygons,
    ];
    let overlay_buckets = [
        SceneBucket::OverlayLines,
        SceneBucket::OverlayCircles,
        SceneBucket::OverlayPolygons,
    ];

    // Sanity: this scenario actually populates every overlay bucket, so the
    // parity claim below is exercised against real content, not empty ones.
    for bucket in overlay_buckets {
        assert!(
            bucket_count(&scene, bucket) > 0,
            "scenario must populate every overlay bucket"
        );
    }

    // GPU: no overlay bucket is ever a member of the base draw order -- the
    // overlay trio composites in its own pass strictly after, unconditionally.
    for bucket in overlay_buckets {
        assert!(!GPU_SCENE_DRAW_ORDER.contains(&bucket));
    }

    // CPU: every populated overlay bucket's position in the real draw order
    // is after every populated base bucket's.
    let cpu_sequence = nonempty_bucket_sequence(&scene, CPU_PCB_DRAW_ORDER);
    let max_base_idx = base_buckets
        .iter()
        .filter_map(|b| cpu_sequence.iter().position(|x| x == b))
        .max()
        .expect("scenario populates the base buckets");
    let min_overlay_idx = overlay_buckets
        .iter()
        .filter_map(|b| cpu_sequence.iter().position(|x| x == b))
        .min()
        .expect("overlay buckets are populated above");
    assert!(
        min_overlay_idx > max_base_idx,
        "CPU must draw every populated overlay bucket after every populated base bucket"
    );
}
