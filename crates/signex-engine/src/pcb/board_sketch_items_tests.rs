//! Decision 2 (Caner, 2026-11-20): a board sketch bake that writes pours,
//! mechanical drawings and component positions is one exact undo step, and
//! a re-bake replaces only what the sketch owns.

use signex_types::pcb::{BoardGraphic, Point, Zone};
use signex_types::pcb_sketch::BakedBoard;
use signex_types::sketch::SketchData;
use signex_types::sketch::array::ArrayId;
use signex_types::sketch::baked_kinds::BakedKind;
use signex_types::sketch::board_attr::{
    BakedAnchor, ComponentAnchorAttr, MechanicalAttr, SketchBoardItems,
};
use signex_types::sketch::entity::{Entity, EntityKind};
use signex_types::sketch::id::SketchEntityId;
use signex_types::sketch::pattern_instance::pattern_instance_uuid;
use signex_types::sketch::plane::PlaneId;
use uuid::Uuid;

use crate::pcb::tests::{FP_A, FP_B, ZONE_1, board, id, snapshot};
use crate::pcb::{BoardCommand, BoardCommandError, BoardEngine, BoardPatchKind};

const POUR: Uuid = Uuid::from_u128(0x0e01);
const DRAWING: Uuid = Uuid::from_u128(0x0e02);
const ANCHOR: Uuid = Uuid::from_u128(0x0e03);

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// A sketch holding the pour edge, the mechanical edge and the anchor
/// point the items below came from.
fn sketch() -> SketchData {
    let plane = PlaneId::new();
    let point = |uuid| {
        Entity::new(
            SketchEntityId(uuid),
            plane,
            EntityKind::Point { x: 0.1, y: 0.2 },
        )
    };
    let edge = |uuid| {
        Entity::new(
            SketchEntityId(uuid),
            plane,
            EntityKind::Line {
                start: SketchEntityId(ANCHOR),
                end: SketchEntityId(ANCHOR),
            },
        )
    };
    let mut pour = edge(POUR);
    pour.pour = Some(signex_types::sketch::attr::PourAttr {
        layer: signex_types::pcb_layer::PcbLayer::Copper(signex_types::pcb_layer::CopperLayer::Top),
        net: None,
        fill_type: Default::default(),
        thermal_relief: Default::default(),
        clearance_expr: None,
        min_thickness_expr: None,
        priority: 0,
        corners: Default::default(),
        corner_size: 0.0,
    });
    let mut drawing = edge(DRAWING);
    drawing.mechanical = Some(MechanicalAttr::default());
    let mut anchor = point(ANCHOR);
    anchor.component_anchor = Some(ComponentAnchorAttr {
        footprint: Some(id(FP_A)),
    });
    SketchData {
        entities: vec![anchor, pour, drawing],
        ..SketchData::default()
    }
}

fn zone(uuid: Uuid, at: f64) -> Zone {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid,
        "layer": "TopCopper",
        "fill_type": "pour",
        "outline": [{"x": at, "y": 0.1}, {"x": at + 1.0, "y": 0.1}, {"x": at, "y": 1.0 / 3.0}],
    }))
    .unwrap()
}

fn drawing(uuid: Uuid, layer: &str) -> BoardGraphic {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid,
        "graphic_type": "line",
        "layer": layer,
        "width": 0.1,
        "start": {"x": 0.1, "y": 0.2},
        "end": {"x": 1.0 / 3.0, "y": 0.2},
    }))
    .unwrap()
}

fn items(at: f64) -> SketchBoardItems {
    SketchBoardItems {
        zones: vec![zone(POUR, at)],
        graphics: vec![drawing(DRAWING, "Mechanical2")],
        anchors: vec![BakedAnchor {
            entity: ANCHOR,
            footprint: id(FP_A),
            reference: "U1".into(),
            position: p(5.5, 1.0 / 3.0),
        }],
    }
}

fn bake(sketch: SketchData, items: SketchBoardItems) -> BoardCommand {
    BoardCommand::set_board_sketch_with_items(sketch, BakedBoard::default(), items)
}

/// A board with a zone and a drawing of its own beside the fixture's.
fn engine() -> BoardEngine {
    let mut board = board();
    board
        .graphics
        .push(drawing(Uuid::from_u128(0x0f01), "TopSilkscreen"));
    BoardEngine::new(board, 1)
}

#[test]
fn one_step_writes_the_pours_the_drawings_and_moves_the_component() {
    let mut engine = engine();
    let patch = engine.execute(bake(sketch(), items(2.0))).unwrap();
    assert_eq!(patch.kind, BoardPatchKind::BoardSketchReplaced);
    assert_eq!(engine.undo_len(), 1, "one undo step");

    let board = engine.board();
    let zones: Vec<Uuid> = board.zones.iter().map(|z| z.uuid).collect();
    assert_eq!(
        zones,
        [id(ZONE_1), POUR],
        "the board's own zone stays first"
    );
    assert_eq!(board.graphics.len(), 2);
    assert_eq!(board.graphics[1], drawing(DRAWING, "Mechanical2"));
    let u1 = board
        .footprints
        .iter()
        .find(|fp| fp.uuid == id(FP_A))
        .unwrap();
    assert_eq!(u1.position, p(5.5, 1.0 / 3.0));
    let stored = board.sketch.as_ref().expect("a sketch");
    for kind in [
        BakedKind::BoardPours,
        BakedKind::Mechanical,
        BakedKind::ComponentAnchors,
    ] {
        assert!(stored.sketch.was_baked(kind), "{kind:?} is recorded");
    }
}

#[test]
fn undo_puts_back_zones_drawings_and_positions_bit_for_bit() {
    let mut engine = engine();
    let before = snapshot(engine.board());
    engine.execute(bake(sketch(), items(2.0))).unwrap();
    let after = snapshot(engine.board());
    assert_ne!(before, after);

    engine.undo().unwrap().expect("a step to undo");
    assert_eq!(snapshot(engine.board()), before);
    engine.redo().unwrap().expect("a step to redo");
    assert_eq!(snapshot(engine.board()), after);
    engine.undo().unwrap();
    assert_eq!(snapshot(engine.board()), before, "and again");
}

#[test]
fn a_re_bake_replaces_only_what_the_sketch_owns() {
    let mut engine = engine();
    engine.execute(bake(sketch(), items(2.0))).unwrap();
    engine.execute(bake(sketch(), items(7.0))).unwrap();
    let board = engine.board();
    let pours: Vec<&Zone> = board.zones.iter().filter(|z| z.uuid == POUR).collect();
    assert_eq!(pours.len(), 1, "the old pour is replaced, not doubled");
    assert_eq!(pours[0].outline[0].x, 7.0);
    assert!(board.zones.iter().any(|z| z.uuid == id(ZONE_1)));
    assert_eq!(board.graphics.len(), 2, "the board's own drawing stays");
    assert_eq!(engine.undo_len(), 2);
}

/// #2517: the instances of a patterned pour carry derived uuids. A repeat
/// bake replaces them, and a shorter pattern takes the extra one off.
#[test]
fn a_re_bake_replaces_pattern_instances_and_never_doubles_them() {
    let pattern = ArrayId(Uuid::from_u128(0x0e10));
    let patterned = |count: u32| {
        let mut baked = items(2.0);
        baked.zones.extend((1..count).map(|i| {
            zone(
                pattern_instance_uuid(POUR, pattern, i, 0),
                2.0 + f64::from(i),
            )
        }));
        baked.graphics.extend(
            (1..count)
                .map(|i| drawing(pattern_instance_uuid(DRAWING, pattern, i, 0), "Mechanical2")),
        );
        baked
    };
    let own_zones = |engine: &BoardEngine| {
        engine
            .board()
            .zones
            .iter()
            .filter(|z| z.uuid != id(ZONE_1))
            .count()
    };
    let mut engine = engine();
    let drawings_before = engine.board().graphics.len();

    engine.execute(bake(sketch(), patterned(3))).unwrap();
    assert_eq!(own_zones(&engine), 3);
    engine.execute(bake(sketch(), patterned(3))).unwrap();
    assert_eq!(own_zones(&engine), 3, "a repeat bake adds nothing");
    assert_eq!(engine.board().graphics.len(), drawings_before + 3);

    engine.execute(bake(sketch(), patterned(2))).unwrap();
    assert_eq!(own_zones(&engine), 2, "the dropped instance goes");
    assert_eq!(engine.board().graphics.len(), drawings_before + 2);
    assert!(engine.board().zones.iter().any(|z| z.uuid == id(ZONE_1)));
}

/// The pour entity deleted: its zone goes with the next bake, because the
/// sketch it replaces owned it.
#[test]
fn an_entity_deleted_from_the_sketch_takes_its_item_off_the_board() {
    let mut engine = engine();
    engine.execute(bake(sketch(), items(2.0))).unwrap();
    let mut without = engine.board().sketch.as_ref().unwrap().sketch.clone();
    without.entities.retain(|e| e.id.0 != POUR);
    let mut rest = items(2.0);
    rest.zones.clear();
    engine.execute(bake(without, rest)).unwrap();
    let board = engine.board();
    assert!(board.zones.iter().all(|z| z.uuid != POUR));
    assert!(board.zones.iter().any(|z| z.uuid == id(ZONE_1)));
    let stored = &board.sketch.as_ref().unwrap().sketch;
    assert!(!stored.was_baked(BakedKind::BoardPours));
    assert!(stored.was_baked(BakedKind::Mechanical));
}

#[test]
fn a_locked_component_stays_where_it_is() {
    let mut board = board();
    for fp in &mut board.footprints {
        fp.locked = fp.uuid == id(FP_A);
    }
    let locked_at = board.footprints[0].position;
    let items = items(2.0);
    let locked = items.locked_anchors(&board.footprints);
    assert_eq!(locked.len(), 1, "the caller warns about U1");
    assert_eq!(locked[0].reference, "U1");

    let mut engine = BoardEngine::new(board, 1);
    let before = snapshot(engine.board());
    engine.execute(bake(sketch(), items)).unwrap();
    let u1 = engine
        .board()
        .footprints
        .iter()
        .find(|fp| fp.uuid == id(FP_A));
    assert_eq!(u1.unwrap().position, locked_at);
    engine.undo().unwrap();
    assert_eq!(snapshot(engine.board()), before);
}

#[test]
fn two_anchors_on_one_component_end_at_the_last_and_undo_to_the_first() {
    let mut engine = engine();
    let before = snapshot(engine.board());
    let mut twice = items(2.0);
    let mut second = twice.anchors[0].clone();
    second.position = p(9.0, 9.0);
    twice.anchors.push(second);
    engine.execute(bake(sketch(), twice)).unwrap();
    let u1 = engine
        .board()
        .footprints
        .iter()
        .find(|fp| fp.uuid == id(FP_A));
    assert_eq!(u1.unwrap().position, p(9.0, 9.0));
    let after = snapshot(engine.board());
    engine.undo().unwrap();
    assert_eq!(snapshot(engine.board()), before);
    engine.redo().unwrap();
    assert_eq!(snapshot(engine.board()), after);
}

#[test]
fn an_anchor_to_a_component_that_is_gone_is_refused_whole() {
    let mut engine = engine();
    let before = snapshot(engine.board());
    let mut stray = items(2.0);
    stray.anchors[0].footprint = Uuid::from_u128(0xdead);
    assert_eq!(
        engine.execute(bake(sketch(), stray)),
        Err(BoardCommandError::FootprintNotFound(Uuid::from_u128(
            0xdead
        )))
    );
    assert_eq!(snapshot(engine.board()), before, "nothing written");
    assert!(!engine.can_undo());
}

#[test]
fn a_non_finite_item_is_refused_whole() {
    let mut engine = engine();
    let before = snapshot(engine.board());
    let mut bad = items(2.0);
    bad.zones[0].outline[1].y = f64::NAN;
    assert!(matches!(
        engine.execute(bake(sketch(), bad)),
        Err(BoardCommandError::NonFiniteSketch(_))
    ));
    let mut far = items(2.0);
    far.anchors[0].position.x = f64::INFINITY;
    assert_eq!(
        engine.execute(bake(sketch(), far)),
        Err(BoardCommandError::NonFinite { field: "position" })
    );
    assert_eq!(snapshot(engine.board()), before);
}

/// The other footprint and a sketch without items: the plain command's
/// behaviour, so the old flow could take this one too.
#[test]
fn without_items_it_writes_what_the_plain_command_writes() {
    let mut plain = engine();
    plain
        .execute(BoardCommand::set_board_sketch(
            SketchData::default(),
            BakedBoard::default(),
        ))
        .unwrap();
    let mut with_items = engine();
    with_items
        .execute(bake(SketchData::default(), SketchBoardItems::default()))
        .unwrap();
    assert_eq!(snapshot(plain.board()), snapshot(with_items.board()));
    let r1 = with_items
        .board()
        .footprints
        .iter()
        .find(|fp| fp.uuid == id(FP_B));
    assert_eq!(r1.unwrap().position, p(0.30000000000000004, 7.0));
}

/// The board after a first bake, with the baked pour set by `edit` as a
/// board edit would (shelve, unfill), in a fresh engine.
fn baked_then(edit: impl FnOnce(&mut Zone)) -> BoardEngine {
    let mut engine = engine();
    engine.execute(bake(sketch(), items(2.0))).unwrap();
    let mut board = engine.board().clone();
    edit(board.zones.iter_mut().find(|z| z.uuid == POUR).unwrap());
    BoardEngine::new(board, 1)
}

fn pour(engine: &BoardEngine) -> &Zone {
    engine
        .board()
        .zones
        .iter()
        .find(|z| z.uuid == POUR)
        .unwrap()
}

/// #2706 (Caner, 2026-11-21): a re-bake keeps a shelved pour shelved and
/// takes the new outline; undo puts the old zone back.
#[test]
fn a_re_bake_keeps_a_shelved_pour_shelved() {
    let mut engine = baked_then(|z| z.shelved = true);
    let before = snapshot(engine.board());
    engine.execute(bake(sketch(), items(7.0))).unwrap();
    assert!(pour(&engine).shelved);
    assert!(!pour(&engine).unfilled);
    assert_eq!(pour(&engine).outline[0].x, 7.0, "the new outline");
    engine.undo().unwrap();
    assert_eq!(snapshot(engine.board()), before);
}

/// #2706, decision 8 (Caner, 2026-11-27): the shelved pour keeps its
/// `unfilled` flag too.
#[test]
fn a_re_bake_keeps_a_shelved_pours_unfilled_flag() {
    let mut engine = baked_then(|z| {
        z.shelved = true;
        z.unfilled = true;
    });
    engine.execute(bake(sketch(), items(7.0))).unwrap();
    assert!(pour(&engine).shelved);
    assert!(pour(&engine).unfilled);
}

/// A shelved pour whose entity left the sketch still goes.
#[test]
fn a_shelved_pour_removed_from_the_sketch_still_goes() {
    let mut engine = baked_then(|z| z.shelved = true);
    let mut without = engine.board().sketch.as_ref().unwrap().sketch.clone();
    without.entities.retain(|e| e.id.0 != POUR);
    let mut rest = items(2.0);
    rest.zones.clear();
    engine.execute(bake(without, rest)).unwrap();
    assert!(engine.board().zones.iter().all(|z| z.uuid != POUR));
    assert!(engine.board().zones.iter().any(|z| z.uuid == id(ZONE_1)));
}

/// #2706, decision 8 (Caner, 2026-11-27, answer (a)): a pour that is not
/// shelved keeps its `unfilled` flag too, and stays live.
#[test]
fn a_re_bake_keeps_an_unshelved_pours_unfilled_flag() {
    let mut engine = baked_then(|z| z.unfilled = true);
    engine.execute(bake(sketch(), items(7.0))).unwrap();
    assert!(!pour(&engine).shelved);
    assert!(pour(&engine).unfilled);
    assert_eq!(pour(&engine).outline[0].x, 7.0, "the new outline");
}

/// A pour neither shelved nor unfilled re-bakes as before: exactly what
/// the bake wrote, and a pour new in a bake starts unshelved and filled.
#[test]
fn an_unshelved_pour_re_bakes_as_before() {
    let mut engine = baked_then(|_| {});
    assert!(!pour(&engine).shelved, "a new pour starts unshelved");
    assert!(!pour(&engine).unfilled, "and filled");
    engine.execute(bake(sketch(), items(7.0))).unwrap();
    assert_eq!(*pour(&engine), zone(POUR, 7.0));
}
