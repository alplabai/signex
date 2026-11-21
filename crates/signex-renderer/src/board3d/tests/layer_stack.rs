//! The board's physical stack in the scene: what a ray straight at a face
//! meets first, and where each layer sits.

use pcb_geom::PointNm;
use signex_types::pcb::{LayerStack, Segment};
use signex_types::pcb_stack::{StackLayer, StackLayerKind};

use super::*;
use crate::board3d::layers::{ArtInput, board_art, mask_over_copper};
use crate::board3d::stack::StackHeights;
use crate::pcb::{GraphicInput, GraphicShapeInput};

/// A place on the test board with nothing on it but the mask.
const BARE: [f32; 2] = [25.0, -27.0];
/// On U1's top pad, U2's bottom pad and the via's copper ring.
const TOP_PAD: [f32; 2] = [10.0, -10.0];
const BOTTOM_PAD: [f32; 2] = [30.0, -15.0];
const VIA_RING: [f32; 2] = [40.2, -20.0];
const VIA_CENTRE: [f32; 2] = [40.0, -20.0];
/// On the tracks of [`routed`].
const TOP_TRACK: [f32; 2] = [12.0, -25.0];
const BOTTOM_TRACK: [f32; 2] = [12.0, -5.0];

fn track(id: u128, layer: &str, y: f64) -> Segment {
    Segment {
        uuid: Uuid::from_u128(id),
        start: Point::new(5.0, y),
        end: Point::new(20.0, y),
        width: 0.5,
        layer: layer.into(),
        net: 1,
        locked: false,
        attributes: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// The test board with a track on each outer layer.
fn routed() -> PcbBoard {
    let mut board = board();
    board.segments = vec![
        track(20, "TopCopper", 25.0),
        track(21, "BottomCopper", 5.0),
    ];
    board
}

/// `board`'s scene with the part bodies hidden, so a ray meets the layers.
fn layers_of(board: &PcbBoard) -> BoardScene3d {
    let mut view = PcbLayerView::default();
    view.toggle_visibility(PcbLayer::Technical(TechnicalLayer::Body3dTop));
    view.toggle_visibility(PcbLayer::Technical(TechnicalLayer::Body3dBottom));
    build(board, &view, &HashMap::new())
}

/// The colour of the first triangle a ray straight at `side` meets at `at`.
fn seen(scene: &BoardScene3d, at: [f32; 2], side: Face) -> [f32; 4] {
    let (z, dir) = match side {
        Face::Top => (10.0, DOWN),
        Face::Bottom => (-10.0, UP),
    };
    let origin = [at[0], at[1], z];
    let mut nearest: Option<(f32, [f32; 4])> = None;
    for first in (0..scene.scene.indices.len() as u32).step_by(3) {
        if let Some(t) = scene.scene.ray_hit(first..first + 3, origin, dir)
            && nearest.is_none_or(|(n, _)| t < n)
        {
            let vertex = scene.scene.indices[first as usize] as usize;
            nearest = Some((t, scene.scene.vertices[vertex].color));
        }
    }
    nearest.expect("the ray meets the board").1
}

#[test]
fn a_track_under_the_mask_reads_as_raised_mask_on_both_sides() {
    let scene = layers_of(&routed());
    let raised = mask_over_copper(colors().mask);
    assert_ne!(raised, colors().mask, "copper lifts the mask's colour");
    for (at, side) in [(TOP_TRACK, Face::Top), (BOTTOM_TRACK, Face::Bottom)] {
        let color = seen(&scene, at, side);
        assert_eq!(color, raised, "{side:?}");
        assert_ne!(color, colors().copper, "{side:?}");
    }
    assert_eq!(seen(&scene, BARE, Face::Top), colors().mask);
    assert_eq!(seen(&scene, BARE, Face::Bottom), colors().mask);
}

#[test]
fn a_pad_opening_shows_copper_in_its_finish() {
    let scene = layers_of(&routed());
    assert_eq!(seen(&scene, TOP_PAD, Face::Top), colors().finish);
    assert_eq!(seen(&scene, BOTTOM_PAD, Face::Bottom), colors().finish);
    assert_eq!(
        seen(&scene, TOP_PAD, Face::Bottom),
        colors().mask,
        "a top pad does not open the bottom mask"
    );
}

#[test]
fn an_untented_via_shows_its_annulus_on_both_sides() {
    let scene = layers_of(&board());
    assert_eq!(seen(&scene, VIA_RING, Face::Top), colors().finish);
    assert_eq!(seen(&scene, VIA_RING, Face::Bottom), colors().finish);
}

#[test]
fn a_tented_via_has_no_exposed_copper_on_its_tented_side() {
    let mut board = board();
    board.vias[0].attributes.top.tented = true;
    let scene = layers_of(&board);
    let raised = mask_over_copper(colors().mask);
    assert_eq!(seen(&scene, VIA_RING, Face::Top), raised);
    assert_eq!(seen(&scene, VIA_CENTRE, Face::Top), raised, "the mask spans the drill");
    assert_eq!(
        seen(&scene, VIA_RING, Face::Bottom),
        colors().finish,
        "tenting is per side"
    );
}

#[test]
fn copper_lies_between_the_laminate_and_the_mask_on_both_sides() {
    let board = routed();
    let scene = layers_of(&board);
    let heights = StackHeights::of(&board, 1.6);
    assert!(heights.bottom_core < heights.top_core);
    let (lo, hi) = z_span(&scene, Layer3d::Copper(Face::Top));
    assert!(lo > heights.top_core && hi < heights.mask(Face::Top), "{lo} {hi}");
    let (lo, hi) = z_span(&scene, Layer3d::Copper(Face::Bottom));
    assert!(hi < heights.bottom_core && lo > heights.mask(Face::Bottom), "{lo} {hi}");
}

#[test]
fn silk_stands_on_the_mask_and_stays_out_of_its_openings() {
    let board = board();
    let every = PcbSnapshot::from_board(&board);
    let view = PcbLayerView::default();
    let mut shown = PcbSnapshot::from_board_with_view(&board, &view);
    // A line across U1's pad, which spans x 9 to 11 at y 10.
    shown.graphics.push(GraphicInput {
        shape: GraphicShapeInput::Line {
            p0: [5.0, 10.0],
            p1: [15.0, 10.0],
        },
        layer: PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
        width_mm: 0.2,
        filled: false,
        dimmed: false,
    });
    let heights = StackHeights::of(&board, 1.6);
    let outline: [[f32; 2]; 4] = [[0.0, 0.0], [50.0, 0.0], [50.0, -30.0], [0.0, -30.0]];
    let art = board_art(&ArtInput {
        board: &board,
        every: &every,
        shown: &shown,
        view: &view,
        outline: &outline,
        heights: &heights,
    });
    let inked = |x_mm: i64| {
        let at = PointNm::new(x_mm * 1_000_000, -10_000_000);
        art.top.silk.iter().any(|piece| piece.contains_point(at))
    };
    assert!(inked(7), "beside the pad");
    assert!(!inked(10), "not over the opening");
    assert!(heights.silk(Face::Top) > heights.mask(Face::Top));
    assert!(heights.silk(Face::Bottom) < heights.mask(Face::Bottom));
}

/// Every vertex is one of the materials: nothing carries a layer colour.
#[test]
fn every_vertex_is_drawn_in_a_material() {
    let c = colors();
    let rgb = |color: [f32; 4]| [color[0], color[1], color[2]];
    let materials = [
        rgb(c.mask),
        rgb(mask_over_copper(c.mask)),
        rgb(c.substrate),
        rgb(c.copper),
        rgb(c.finish),
        rgb(c.silk),
        rgb(c.body),
    ];
    let scene = scene_of(&routed());
    for vertex in &scene.scene.vertices {
        assert!(materials.contains(&rgb(vertex.color)), "{:?}", vertex.color);
    }
}

fn four_layers() -> LayerStack {
    let copper = |layer: &str| StackLayer {
        layer: layer.into(),
        ..StackLayer::new(StackLayerKind::Signal)
    };
    let core = || StackLayer {
        thickness_mm: 0.5,
        ..StackLayer::new(StackLayerKind::Dielectric)
    };
    let mask = || StackLayer::new(StackLayerKind::SolderMask);
    LayerStack {
        layers: vec![
            mask(),
            copper("TopCopper"),
            core(),
            copper("Inner1Copper"),
            core(),
            copper("Inner2Copper"),
            core(),
            copper("BottomCopper"),
            mask(),
        ],
    }
}

/// Inner copper stays inside the laminate until the board is taken apart;
/// apart, every real layer stands in stack order on its side.
#[test]
fn the_explode_keeps_every_layer_in_stack_order() {
    let mut board = board();
    board.stackup = Some(four_layers());
    let scene = scene_of(&board);
    let heights = StackHeights::of(&board, 1.6);
    let upper = Layer3d::InnerCopper {
        face: Face::Top,
        rank: 1,
    };
    let lower = Layer3d::InnerCopper {
        face: Face::Bottom,
        rank: 1,
    };
    for layer in [upper, lower] {
        let (lo, hi) = z_span(&scene, layer);
        assert!(lo > heights.bottom_core && hi < heights.top_core, "{layer:?}");
    }

    let apart = scene.exploded(Explode {
        amount: 1.0,
        ..Explode::ASSEMBLED
    });
    let top = [
        z_span(&apart, upper),
        z_span(&apart, Layer3d::Copper(Face::Top)),
        z_span(&apart, Layer3d::Mask(Face::Top)),
        z_span(&apart, Layer3d::Part(Face::Top)),
    ];
    assert!(top[0].0 > heights.top_core, "inner copper leaves the core");
    assert!(top.windows(2).all(|p| p[0].1 < p[1].0), "{top:?}");
    let bottom = [
        z_span(&apart, lower),
        z_span(&apart, Layer3d::Copper(Face::Bottom)),
        z_span(&apart, Layer3d::Mask(Face::Bottom)),
        z_span(&apart, Layer3d::Part(Face::Bottom)),
    ];
    assert!(bottom[0].1 < heights.bottom_core, "inner copper leaves the core");
    assert!(bottom.windows(2).all(|p| p[0].0 > p[1].1), "{bottom:?}");
}
