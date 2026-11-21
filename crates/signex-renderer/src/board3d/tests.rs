use std::collections::{BTreeSet, HashMap};

use signex_types::part::FootprintSourceRef;
use signex_types::pcb::{
    ComponentAttributes, Footprint, Pad, PadShape, PadType, PcbBoard, Point, Via, ViaType,
};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::*;

const U1: Uuid = Uuid::from_u128(1);
const U2: Uuid = Uuid::from_u128(2);
const U3: Uuid = Uuid::from_u128(3);
const DOWN: [f32; 3] = [0.0, 0.0, -1.0];
const UP: [f32; 3] = [0.0, 0.0, 1.0];

/// An opaque grey of `level`: the scene's colours do not matter here, and
/// the renderer source holds no colour literals.
fn grey(level: f32) -> [f32; 4] {
    [level, level, level, OPAQUE]
}

const OPAQUE: f32 = 1.0;

fn colors() -> Board3dColors {
    Board3dColors {
        mask: grey(0.3),
        edge: grey(0.2),
        top_copper: grey(0.8),
        bottom_copper: grey(0.7),
        multi_layer: grey(0.6),
        body: grey(0.5),
    }
}

fn source() -> FootprintSourceRef {
    FootprintSourceRef {
        library: Uuid::from_u128(10),
        footprint: Uuid::from_u128(11),
        version: "1".into(),
    }
}

fn pad(layer: &str) -> Pad {
    Pad {
        uuid: Uuid::now_v7(),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(2.0, 1.0),
        drill: None,
        layers: vec![layer.into()],
        net: None,
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        layer_sizes: None,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn footprint(uuid: Uuid, at: (f64, f64), layer: &str, height_mm: f64) -> Footprint {
    Footprint {
        uuid,
        reference: String::new(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(at.0, at.1),
        rotation: 0.0,
        layer: layer.into(),
        locked: false,
        pads: vec![pad(layer)],
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref: None,
        component: Box::new(ComponentAttributes {
            height_mm,
            ..ComponentAttributes::default()
        }),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// A 50 x 30 mm board, 1.6 mm thick; U1 on top at (10, 10), 2 mm tall;
/// U2 on the bottom at (30, 15), 1 mm tall; a through via at (40, 20).
fn board() -> PcbBoard {
    let outline = [(0.0, 0.0), (50.0, 0.0), (50.0, 30.0), (0.0, 30.0)];
    PcbBoard {
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: outline.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![
            footprint(U1, (10.0, 10.0), "TopCopper", 2.0),
            footprint(U2, (30.0, 15.0), "BottomCopper", 1.0),
        ],
        segments: Vec::new(),
        vias: vec![Via {
            attributes: Default::default(),
            uuid: Uuid::from_u128(4),
            position: Point::new(40.0, 20.0),
            diameter: 0.6,
            drill: 0.3,
            layers: vec!["TopCopper".into(), "BottomCopper".into()],
            net: 1,
            locked: false,
            layer_sizes: None,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
        }],
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        unknown: Default::default(),
    }
}

fn build(
    board: &PcbBoard,
    view: &PcbLayerView,
    models: &HashMap<FootprintSourceRef, ComponentModel>,
) -> BoardScene3d {
    build_board_scene(&Board3dInput {
        board,
        view,
        models,
        colors: colors(),
    })
}

fn scene_of(board: &PcbBoard) -> BoardScene3d {
    build(board, &PcbLayerView::default(), &HashMap::new())
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn the_board_is_its_outline_extruded_to_its_thickness() {
    let scene = scene_of(&board());
    let (lo, hi) = scene.scene.bounds_of(scene.board.clone()).expect("a slab");
    assert_eq!(lo, [0.0, -30.0, 0.0]);
    assert_eq!(hi, [50.0, 0.0, 1.6]);
    assert_eq!(scene.thickness_mm, 1.6);
    let into_board = scene
        .scene
        .ray_hit(scene.board.clone(), [5.0, -5.0, 10.0], DOWN);
    assert!(into_board.is_some_and(|t| close(t, 10.0 - 1.6)));
}

/// The active layer's size is a flat-view aid: 3D keeps the stored size.
#[test]
fn the_3d_copper_keeps_its_own_size_whatever_layer_is_active() {
    use signex_types::pcb::{DrillDef, LayerSizes};
    use signex_types::pcb_layer::CopperLayer;
    let mut board = board();
    board.vias[0].layer_sizes = Some(LayerSizes::TopMiddleBottom {
        top: 1.2,
        middle: 0.5,
        bottom: 0.9,
    });
    let mut through = pad("TopCopper");
    through.pad_type = PadType::Thru;
    through.shape = PadShape::Circle;
    through.size = Point::new(1.5, 1.5);
    through.drill = Some(DrillDef {
        diameter: 0.8,
        shape: String::new(),
    });
    through.layers = vec!["TopCopper".into(), "BottomCopper".into()];
    through.layer_sizes = Some(LayerSizes::TopMiddleBottom {
        top: Point::new(2.4, 2.4),
        middle: Point::new(1.2, 1.2),
        bottom: Point::new(1.8, 1.8),
    });
    board.footprints[0].pads.push(through);
    let mut view = PcbLayerView::default();
    view.set_active(CopperLayer::Top);
    let through_size = |snapshot: &PcbSnapshot| {
        let pad = snapshot.pads.iter().find(|p| p.pad_type == PadType::Thru);
        pad.expect("the through-hole pad").size_mm
    };

    let flat = PcbSnapshot::from_board_with_view(&board, &view);
    assert_eq!(flat.vias[0].diameter_mm, 1.2, "the flat view's active size");
    assert_eq!(through_size(&flat), [2.4, 2.4]);

    let in_3d = PcbSnapshot::from_board_with_view(&board, &all_layers_of(&view));
    assert_eq!(in_3d.vias[0].diameter_mm, 0.6);
    assert_eq!(through_size(&in_3d), [1.5, 1.5]);
}

#[test]
fn the_via_drill_goes_through_the_board() {
    let scene = scene_of(&board());
    let through = scene
        .scene
        .ray_hit(scene.board.clone(), [40.0, -20.0, 10.0], DOWN);
    assert_eq!(through, None, "nothing of the slab inside the drill");
    let beside = scene
        .scene
        .ray_hit(scene.board.clone(), [41.0, -20.0, 10.0], DOWN);
    assert!(beside.is_some());
}

/// One body per component, at the footprint's position: a top part stands
/// on the top face, a bottom part hangs below the bottom face.
#[test]
fn one_body_per_component_at_its_position_with_the_bottom_part_flipped() {
    let scene = scene_of(&board());
    assert_eq!(scene.parts.len(), 2);

    let top = scene.part(U1).expect("U1 has a part");
    assert_eq!(top.face, Face::Top);
    let (lo, hi) = scene.scene.bounds_of(top.indices.clone()).unwrap();
    assert!(close((lo[0] + hi[0]) / 2.0, 10.0) && close((lo[1] + hi[1]) / 2.0, -10.0));
    assert!(close(lo[2], 1.6) && close(hi[2], 3.6), "{lo:?} {hi:?}");

    let bottom = scene.part(U2).expect("U2 has a part");
    assert_eq!(bottom.face, Face::Bottom);
    let (lo, hi) = scene.scene.bounds_of(bottom.indices.clone()).unwrap();
    assert!(close((lo[0] + hi[0]) / 2.0, 30.0) && close((lo[1] + hi[1]) / 2.0, -15.0));
    assert!(close(lo[2], -1.0) && close(hi[2], 0.0), "{lo:?} {hi:?}");
}

#[test]
fn a_ray_picks_the_part_it_meets_before_the_board() {
    let scene = scene_of(&board());
    assert_eq!(scene.pick([10.0, -10.0, 50.0], DOWN), Some(U1));
    assert_eq!(scene.pick([30.0, -15.0, -50.0], UP), Some(U2));
    assert_eq!(
        scene.pick([30.0, -15.0, 50.0], DOWN),
        None,
        "from above the board hides the bottom part"
    );
    assert_eq!(scene.pick([5.0, -25.0, 50.0], DOWN), None);
}

#[test]
fn the_highlight_is_the_selected_parts_triangles() {
    let scene = scene_of(&board());
    let ranges = scene.highlight_ranges(&BTreeSet::from([U2]));
    assert_eq!(ranges, vec![scene.part(U2).unwrap().indices.clone()]);
    assert!(scene.highlight_ranges(&BTreeSet::new()).is_empty());
}

#[test]
fn a_hidden_3d_body_layer_hides_the_parts_of_its_face() {
    let mut view = PcbLayerView::default();
    view.toggle_visibility(PcbLayer::Technical(TechnicalLayer::Body3dTop));
    let scene = build(&board(), &view, &HashMap::new());
    assert!(scene.part(U1).is_none());
    assert!(scene.part(U2).is_some());
}

/// The library 3D Body outline and height make the body when the source
/// footprint has no model.
#[test]
fn a_body_outline_from_the_library_is_extruded() {
    let mut board = board();
    let mut part = footprint(U3, (20.0, 20.0), "TopCopper", 0.0);
    part.library_ref = Some(source());
    board.footprints.push(part);
    let models = HashMap::from([(
        source(),
        ComponentModel::Body(BodyExtrusion {
            outline: Some(vec![[-3.0, -1.0], [3.0, -1.0], [3.0, 1.0], [-3.0, 1.0]]),
            height_mm: 4.0,
            offset_z_mm: 0.5,
            colors: None,
        }),
    )]);
    let scene = build(&board, &PcbLayerView::default(), &models);
    let part = scene.part(U3).expect("U3 has a part");
    let (lo, hi) = scene.scene.bounds_of(part.indices.clone()).unwrap();
    assert_eq!([lo[0], hi[0]], [17.0, 23.0]);
    assert_eq!([lo[1], hi[1]], [-21.0, -19.0]);
    assert!(close(lo[2], 2.1) && close(hi[2], 6.1), "{lo:?} {hi:?}");
}

/// A model point lands where the footprint puts it: the model's y up the
/// board; on the bottom mirrored in x and below the board.
#[test]
fn a_model_is_placed_by_position_and_side() {
    let model = ComponentModel::Model(AttachedModel {
        meshes: vec![ModelMesh {
            positions: vec![[1.0, 1.0, 0.5], [1.0, 1.0, 0.5], [1.0, 1.0, 0.5]],
            triangles: vec![[0, 1, 2]],
            color: grey(1.0),
        }],
        offset_mm: [0.0; 3],
        rotation_deg: [0.0; 3],
    });
    let models = HashMap::from([(source(), model)]);
    for (layer, want) in [
        ("TopCopper", [21.0, -9.0, 2.1]),
        ("BottomCopper", [19.0, -9.0, -0.5]),
    ] {
        let mut board = board();
        let mut part = footprint(U3, (20.0, 10.0), layer, 0.0);
        part.library_ref = Some(source());
        board.footprints.push(part);
        let scene = build(&board, &PcbLayerView::default(), &models);
        let part = scene.part(U3).expect("U3 has a part");
        let (lo, _) = scene.scene.bounds_of(part.indices.clone()).unwrap();
        assert!(
            close(lo[0], want[0]) && close(lo[1], want[1]) && close(lo[2], want[2]),
            "{layer}: {lo:?}"
        );
    }
}

/// A board with no outline still gets a slab round its copper.
#[test]
fn a_board_without_an_outline_gets_a_slab_round_its_copper() {
    let mut board = board();
    board.outline.clear();
    let scene = scene_of(&board);
    let (lo, hi) = scene.scene.bounds_of(scene.board.clone()).expect("a slab");
    assert!(lo[0] < 9.0 && hi[0] > 40.0, "{lo:?} {hi:?}");
}

/// Single-layer mode is a flat-view aid: in 3D the parts of both faces and
/// the copper of every shown layer stay, and a click still picks a part.
#[test]
fn single_layer_mode_does_not_reach_the_3d_view() {
    let mut single = PcbLayerView::default();
    single.toggle_single_layer();
    let flat_aid = build(&board(), &single, &HashMap::new());
    let plain = scene_of(&board());
    assert!(flat_aid.part(U1).is_some(), "U1 keeps its part");
    assert!(flat_aid.part(U2).is_some(), "so does the bottom part");
    assert_eq!(flat_aid.pick([10.0, -10.0, 50.0], DOWN), Some(U1));
    assert_eq!(
        flat_aid.scene.triangle_count(),
        plain.scene.triangle_count(),
        "the same copper as with the mode off"
    );
}

fn via_at(id: u128, x: f64, y: f64) -> Via {
    Via {
        uuid: Uuid::from_u128(id),
        position: Point::new(x, y),
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 1,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        layer_sizes: None,
        locked: false,
    }
}

/// Two vias stacked at one point: the second drill is left out, and every
/// other hole is still cut through the board.
#[test]
fn coincident_vias_keep_every_other_hole() {
    let mut board = board();
    board.vias.push(via_at(5, 40.0, 20.0));
    board.vias.push(via_at(6, 20.0, 25.0));
    let scene = scene_of(&board);
    assert_eq!(scene.dropped_holes, 1);
    for (x, y) in [(40.0, -20.0), (20.0, -25.0)] {
        let through = scene.scene.ray_hit(scene.board.clone(), [x, y, 10.0], DOWN);
        assert_eq!(through, None, "the drill at ({x}, {y}) is open");
    }
    let beside = scene
        .scene
        .ray_hit(scene.board.clone(), [21.0, -25.0, 10.0], DOWN);
    assert!(beside.is_some());
}

#[test]
fn the_board_size_is_its_body_and_thickness() {
    assert_eq!(scene_of(&board()).board_size_mm(), Some([50.0, 30.0, 1.6]));
    assert_eq!(BoardScene3d::default().board_size_mm(), None);
}

/// The lowest and highest height of the vertices tagged `layer`.
fn z_span(scene: &BoardScene3d, layer: Layer3d) -> (f32, f32) {
    let zs: Vec<f32> = scene
        .scene
        .vertices
        .iter()
        .zip(&scene.vertex_layers)
        .filter(|(_, l)| **l == layer)
        .map(|(v, _)| v.position[2])
        .collect();
    assert!(!zs.is_empty(), "no {layer:?} vertices");
    let lo = zs.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = zs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    (lo, hi)
}

/// Every vertex is tagged: the two mask faces at the faces' heights, the
/// walls as core, copper by its face, parts by theirs.
#[test]
fn every_vertex_is_tagged_with_its_slice_of_the_board() {
    let scene = scene_of(&board());
    assert_eq!(scene.vertex_layers.len(), scene.scene.vertices.len());
    assert_eq!(z_span(&scene, Layer3d::Mask(Face::Top)), (1.6, 1.6));
    assert_eq!(z_span(&scene, Layer3d::Mask(Face::Bottom)), (0.0, 0.0));
    assert_eq!(z_span(&scene, Layer3d::Core), (0.0, 1.6));
    let (lo, _) = z_span(&scene, Layer3d::Copper(Face::Top));
    assert!(lo > 1.6, "top copper above the top face");
    let (_, hi) = z_span(&scene, Layer3d::Copper(Face::Bottom));
    assert!(hi < 0.0, "bottom copper below the bottom face");
    assert!(close(z_span(&scene, Layer3d::Part(Face::Top)).0, 1.6));
    assert!(close(z_span(&scene, Layer3d::Part(Face::Bottom)).1, 0.0));
}

#[test]
fn assembled_the_scene_is_unchanged() {
    let scene = scene_of(&board());
    assert_eq!(scene.exploded(Explode::ASSEMBLED), scene);
    assert_eq!(scene.exploded(Explode::default()), scene);
    assert!(Explode::default().is_assembled());
    let below = Explode {
        amount: -0.5,
        ..Explode::ASSEMBLED
    };
    assert!(below.is_assembled());
    assert_eq!(scene.exploded(below), scene);
    let nothing_ticked = Explode {
        amount: 1.0,
        separate_copper_and_mask: false,
        lift_components: false,
    };
    assert!(!nothing_ticked.is_assembled());
    assert_eq!(scene.exploded(nothing_ticked), scene);
}

/// Fully apart, the core stays, the mask and copper of each face move off
/// it on their side and the parts further; the mesh is the same mesh, so
/// a click still picks a lifted part.
#[test]
fn layers_apart_move_off_the_core_and_still_pick() {
    let scene = scene_of(&board());
    let apart = scene.exploded(Explode {
        amount: 1.0,
        ..Explode::ASSEMBLED
    });
    assert_eq!(apart.scene.indices, scene.scene.indices);
    assert_eq!(apart.scene.vertices.len(), scene.scene.vertices.len());
    assert_eq!(apart.board, scene.board);
    assert_eq!(apart.parts, scene.parts);
    assert_eq!(apart.thickness_mm, scene.thickness_mm);
    assert_eq!(apart.vertex_layers, scene.vertex_layers);
    for (a, b) in apart.scene.vertices.iter().zip(&scene.scene.vertices) {
        assert_eq!(a.position[..2], b.position[..2], "only heights move");
        assert_eq!(a.color, b.color);
    }

    // One step is 6% of the board's 50 mm side: 3 mm.
    assert_eq!(z_span(&apart, Layer3d::Core), (0.0, 1.6));
    let (_, top_mask) = z_span(&apart, Layer3d::Mask(Face::Top));
    assert!(close(top_mask, 1.6 + 3.0), "{top_mask}");
    let (top_copper, _) = z_span(&apart, Layer3d::Copper(Face::Top));
    assert!(top_copper > top_mask, "top copper above the top face");
    let (top_part, _) = z_span(&apart, Layer3d::Part(Face::Top));
    let (_, top_copper_hi) = z_span(&apart, Layer3d::Copper(Face::Top));
    assert!(top_part > top_copper_hi, "the top part above its copper");

    let (bottom_mask, _) = z_span(&apart, Layer3d::Mask(Face::Bottom));
    assert!(close(bottom_mask, -3.0), "{bottom_mask}");
    let (bottom_copper, _) = z_span(&apart, Layer3d::Copper(Face::Bottom));
    assert!(bottom_copper < bottom_mask);
    let (lo, hi) = apart
        .scene
        .bounds_of(apart.part(U2).unwrap().indices.clone())
        .unwrap();
    assert!(hi[2] < bottom_copper, "the bottom part below its copper");
    assert!(close(lo[2], -1.0 - 9.0), "{lo:?}");

    assert_eq!(apart.pick([10.0, -10.0, 50.0], DOWN), Some(U1));
    assert_eq!(apart.pick([30.0, -15.0, -50.0], UP), Some(U2));
    assert_eq!(
        apart.highlight_ranges(&BTreeSet::from([U2])),
        scene.highlight_ranges(&BTreeSet::from([U2]))
    );
}

/// Half way moves half as far; past 1 is 1.
#[test]
fn the_amount_scales_the_gaps_and_stops_at_one() {
    let scene = scene_of(&board());
    let at = |amount: f32| {
        let apart = scene.exploded(Explode {
            amount,
            ..Explode::ASSEMBLED
        });
        z_span(&apart, Layer3d::Mask(Face::Top)).0
    };
    assert!(close(at(0.5), 1.6 + 1.5));
    assert_eq!(at(4.0), at(1.0));
}

/// Lift only: the copper and mask stay on the core and the parts move one
/// step off it.
#[test]
fn lifting_parts_alone_leaves_the_copper_and_mask_on_the_core() {
    let scene = scene_of(&board());
    let lifted = scene.exploded(Explode {
        amount: 1.0,
        separate_copper_and_mask: false,
        lift_components: true,
    });
    for layer in [
        Layer3d::Mask(Face::Top),
        Layer3d::Mask(Face::Bottom),
        Layer3d::Copper(Face::Top),
        Layer3d::Copper(Face::Bottom),
    ] {
        assert_eq!(z_span(&lifted, layer), z_span(&scene, layer), "{layer:?}");
    }
    let (top_part, _) = z_span(&lifted, Layer3d::Part(Face::Top));
    assert!(close(top_part, 1.6 + 3.0), "{top_part}");
}
