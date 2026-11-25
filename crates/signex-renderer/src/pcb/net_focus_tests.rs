//! A net in focus (#2514): its copper keeps full brightness while tracks,
//! vias, pads and zones on every other net dim by the preference amount.

use super::*;
use signex_gfx::scene::{DirtyFlags, Scene};

const SQUARE: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

fn trace(net: u32) -> TraceInput {
    TraceInput {
        p0: [0.0, net as f32],
        p1: [5.0, net as f32],
        width_mm: 0.25,
        net,
        dimmed: false,
        ink: CopperInk::Unknown,
        object: BoardObject::Tracks,
    }
}

fn via(net: u32) -> ViaInput {
    ViaInput {
        center: [net as f32, 0.0],
        diameter_mm: 0.8,
        drill_mm: 0.0,
        net,
        dimmed: false,
        ink: CopperInk::Unknown,
        shown_on: None,
    }
}

fn pad(net: u32) -> PadInput {
    PadInput {
        center: [0.5, 0.5],
        size_mm: [1.0, 1.0],
        outline: SQUARE.to_vec(),
        pad_type: PadType::Smd,
        drill_mm: 0.0,
        net,
        dimmed: false,
        ink: CopperInk::Unknown,
        shown_on: None,
    }
}

fn zone(net: u32) -> ZonePolygonInput {
    ZonePolygonInput {
        vertices: SQUARE.to_vec(),
        holes: Vec::new(),
        rule_area: false,
        net,
        priority: 0,
        layer_rank: 0,
        layer_name: "top".to_string(),
        source_order: net as usize,
        dimmed: false,
        ink: CopperInk::Unknown,
        fill: None,
        shelved: false,
        object: BoardObject::Polygons,
    }
}

/// Net 1 and net 2 of every copper kind, drawn with `paint`.
fn scene(paint: BoardPaint) -> Scene {
    let snapshot = PcbSnapshot {
        traces: vec![trace(1), trace(2)],
        vias: vec![via(1), via(2)],
        pads: vec![pad(1), pad(2)],
        zones: vec![zone(1), zone(2)],
        paint,
        ..PcbSnapshot::default()
    };
    let mut scene = Scene::default();
    PcbRenderer::build_scene(
        &snapshot,
        &ResolvedTheme::builtin_default(),
        DirtyFlags::LINES | DirtyFlags::CIRCLES | DirtyFlags::POLYGONS,
        &mut scene,
    );
    scene
}

/// The alpha of net 1's and net 2's copper of each kind: track, via, pad,
/// zone fill, zone edge.
fn alphas(scene: &Scene) -> [[f32; 2]; 5] {
    let edge = |p: &signex_gfx::primitive::polygon::GpuPolygon| p.stroke_color.unwrap()[3];
    [
        [scene.lines[0].color[3], scene.lines[1].color[3]],
        [scene.circles[0].color[3], scene.circles[1].color[3]],
        [
            scene.top_polygons[0].fill_color[3],
            scene.top_polygons[1].fill_color[3],
        ],
        [
            scene.polygons[0].fill_color[3],
            scene.polygons[1].fill_color[3],
        ],
        [edge(&scene.polygons[0]), edge(&scene.polygons[1])],
    ]
}

#[test]
fn the_net_in_focus_stays_bright_and_the_others_dim_by_the_amount() {
    let plain = alphas(&scene(BoardPaint::default()));
    for (percent, keep) in [(0u8, 1.0f32), (50, 0.5), (100, 0.0)] {
        let focused = alphas(&scene(BoardPaint::default().with_net_focus(&[1], percent)));
        for (kind, (lit, base)) in focused.iter().zip(plain.iter()).enumerate() {
            assert_eq!(lit[0], base[0], "active net, kind {kind}, {percent} %");
            assert!(
                (lit[1] - base[1] * keep).abs() < 1e-6,
                "other net, kind {kind}, {percent} %: {} vs {}",
                lit[1],
                base[1] * keep
            );
        }
    }
}

#[test]
fn without_a_net_in_focus_nothing_dims() {
    let plain = alphas(&scene(BoardPaint::default()));
    let unfocused = alphas(&scene(BoardPaint::default().with_net_focus(&[], 100)));
    assert_eq!(plain, unfocused);
}

/// A route or drag of two nets keeps both bright.
#[test]
fn every_net_in_focus_stays_bright() {
    let plain = alphas(&scene(BoardPaint::default()));
    let both = alphas(&scene(BoardPaint::default().with_net_focus(&[1, 2], 100)));
    assert_eq!(plain, both);
}
