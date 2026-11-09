//! #2234 — the optional `[board_sketch]` section of a `.snxpcb`.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::pcb::{PcbBoard, Point};
use crate::pcb_layer::{CopperLayer, PcbLayer};
use crate::pcb_sketch::{BoardSketch, PcbCutout, PcbHole, PcbKeepout, PcbVScore};
use crate::sketch::SketchData;
use crate::sketch::attr::{KeepoutKinds, MountingHoleAttr, VScoreSide};
use crate::sketch::constraint::{Constraint, ConstraintKind};
use crate::sketch::entity::{Entity, EntityKind};
use crate::sketch::id::{ConstraintId, SketchEntityId};
use crate::sketch::plane::{Plane, PlaneId, PlaneKind};

const OLD_BOARD: &str = include_str!("../../tests/corpus/pcb/board-outline-only.snxpcb");

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn square(at: f64, side: f64) -> Vec<Point> {
    vec![
        p(at, at),
        p(at + side, at),
        p(at + side, at + side),
        p(at, at + side),
    ]
}

/// A sketch with an outlined edge, a mounting-hole circle and a fixed
/// point, so entities, attributes, constraints and parameters all appear.
fn sketch() -> SketchData {
    let plane = PlaneId::new();
    let a = SketchEntityId::new();
    let b = SketchEntityId::new();
    let mut edge = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Line { start: a, end: b },
    );
    edge.board_outline = true;
    let mut hole = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Circle {
            center: a,
            radius: 1.6,
        },
    );
    hole.mounting_hole = Some(MountingHoleAttr {
        drill_expr: Some("= hole_d".into()),
        plated: true,
        net: Some("GND".into()),
    });
    let mut data = SketchData::default();
    data.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    data.entities = vec![
        Entity::new(
            a,
            plane,
            EntityKind::Point {
                x: 0.1,
                y: 1.0 / 3.0,
            },
        ),
        Entity::new(b, plane, EntityKind::Point { x: 50.0, y: 0.0 }),
        edge,
        hole,
    ];
    data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: a },
    });
    data.parameters.insert("hole_d", "3.2mm");
    data
}

fn stored() -> BoardSketch {
    BoardSketch {
        sketch: sketch(),
        keepouts: vec![PcbKeepout {
            uuid: Uuid::now_v7(),
            boundary: square(10.0, 5.5),
            layers: vec![
                PcbLayer::Copper(CopperLayer::Top),
                PcbLayer::Copper(CopperLayer::Bottom),
            ],
            forbids: KeepoutKinds {
                no_routing: true,
                no_vias: true,
                ..KeepoutKinds::default()
            },
        }],
        cutouts: vec![PcbCutout {
            uuid: Uuid::now_v7(),
            boundary: square(20.0, 2.25),
            edge_radius: 0.5,
            through: true,
        }],
        holes: vec![
            PcbHole {
                uuid: Uuid::now_v7(),
                position: p(4.0, 4.0),
                drill: 3.2,
                plated: true,
                net: Some(7),
                unknown_columns: UnknownColumns::default(),
            },
            PcbHole {
                uuid: Uuid::now_v7(),
                position: p(46.0, 4.0),
                drill: 3.2,
                plated: false,
                net: None,
                unknown_columns: UnknownColumns::default(),
            },
        ],
        v_scores: vec![PcbVScore {
            uuid: Uuid::now_v7(),
            from: p(0.0, 15.0),
            to: p(50.0, 15.0),
            depth_fraction: 1.0 / 3.0,
            min_web: 0.4,
            side: VScoreSide::Top,
        }],
    }
}

fn sketched_board() -> PcbBoard {
    let mut board = empty_board();
    board.outline = square(0.0, 50.0);
    board.sketch = Some(stored());
    board
}

fn write(board: &PcbBoard) -> String {
    SnxPcb::new(board.clone()).write_string().expect("write")
}

fn read(text: &str) -> PcbBoard {
    SnxPcb::parse(text).expect("parse").board
}

fn stored_nm(point: Point) -> (Option<i64>, Option<i64>) {
    (mm_as_stored_nm(point.x), mm_as_stored_nm(point.y))
}

#[test]
fn a_board_without_a_sketch_resaves_byte_identical_and_writes_no_section() {
    let board = read(OLD_BOARD);
    assert!(board.sketch.is_none(), "an older file has no sketch");
    let text = write(&board);
    assert_eq!(text, OLD_BOARD, "no-edit save must keep every byte");
    assert!(!text.contains("board_sketch"), "{text}");

    let fresh = write(&empty_board());
    assert!(!fresh.contains("board_sketch"), "{fresh}");
}

#[test]
fn the_sketch_and_its_baked_geometry_round_trip() {
    let board = sketched_board();
    let text = write(&board);
    let back = read(&text);
    let (want, got) = (stored(), back.sketch.clone().expect("a sketch"));

    let original = board.sketch.as_ref().unwrap();
    assert_eq!(got.sketch, original.sketch);
    assert_eq!(got.keepouts, original.keepouts);
    assert_eq!(got.cutouts, original.cutouts);
    assert_eq!(got.v_scores, original.v_scores);
    assert_eq!(got.holes.len(), want.holes.len());
    for (read, written) in got.holes.iter().zip(&original.holes) {
        assert_eq!(read.uuid, written.uuid);
        assert_eq!(stored_nm(read.position), stored_nm(written.position));
        assert_eq!(mm_as_stored_nm(read.drill), mm_as_stored_nm(written.drill));
        assert_eq!((read.plated, read.net), (written.plated, written.net));
    }
    assert_eq!(back.outline, board.outline, "the outline stays in extras");
    assert_eq!(write(&back), text, "a second save is byte-identical");
}

#[test]
fn holes_are_a_tsv_block_in_integer_nanometres() {
    let text = write(&sketched_board());
    assert!(text.contains("\n[board_sketch.holes]\n"), "{text}");
    let block = text.split("[board_sketch.holes]").nth(1).unwrap();
    let header = block.lines().find(|l| l.starts_with("uuid")).unwrap();
    let columns: Vec<&str> = header.split_whitespace().collect();
    assert_eq!(
        columns,
        ["uuid", "pos_x", "pos_y", "drill", "plated", "net"]
    );
    assert!(block.contains("3200000"), "drill in nm: {block}");
    assert!(block.contains("46000000"), "position in nm: {block}");
    assert!(text.contains("[board_sketch.sketch]"), "{text}");
    assert!(text.contains("[[board_sketch.keepouts]]"), "{text}");
}

#[test]
fn a_sketch_without_baked_geometry_writes_only_the_sketch() {
    let mut board = empty_board();
    board.sketch = Some(BoardSketch {
        sketch: sketch(),
        ..BoardSketch::default()
    });
    let text = write(&board);
    assert!(!text.contains("[board_sketch.holes]"), "{text}");
    assert!(!text.contains("keepouts"), "{text}");
    assert_eq!(read(&text).sketch, board.sketch);
}

#[test]
fn foreign_keys_and_hole_columns_survive_a_save() {
    let mut board = sketched_board();
    if let Some(stored) = board.sketch.as_mut() {
        stored.holes[0]
            .unknown_columns
            .set("finish", "ENIG".to_string());
    }
    let text = format!("{}\n[board_sketch.mystery]\nanswer = 42\n", write(&board));
    let back = read(&text);
    assert!(back.unknown.board_sketch.contains_key("mystery"));
    let holes = &back.sketch.as_ref().unwrap().holes;
    assert_eq!(holes[0].unknown_columns.get("finish"), Some("ENIG"));

    let again = write(&back);
    assert!(again.contains("mystery"), "{again}");
    assert!(again.contains("ENIG"), "{again}");
    assert_eq!(read(&again).unknown, back.unknown);
}

#[test]
fn a_hole_with_an_unreadable_plated_cell_is_a_load_error() {
    let mut board = empty_board();
    board.sketch = Some(BoardSketch {
        holes: stored().holes[..1].to_vec(),
        ..BoardSketch::default()
    });
    let text = write(&board).replace("true", "maybe");
    match SnxPcb::parse(&text) {
        Err(FormatError::TsvFieldParse { block, field, .. }) => {
            assert_eq!(
                (block.as_str(), field.as_str()),
                ("board_sketch.holes", "plated")
            );
        }
        other => panic!("expected a plated cell error, got {other:?}"),
    }
}

#[test]
fn a_non_finite_sketch_value_fails_the_save() {
    let mut board = sketched_board();
    if let Some(stored) = board.sketch.as_mut() {
        stored.keepouts[0].boundary[0].x = f64::NAN;
    }
    assert!(matches!(
        SnxPcb::new(board).write_string(),
        Err(FormatError::NonFiniteTomlField { .. })
    ));
}
