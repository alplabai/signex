//! #2688 — a board sketch's linear pattern with a suppression list writes
//! `SNXPCB_FORMAT_V1_13`, which a build that knows only 1.12 opens read-only. A
//! board without one keeps its token, byte for byte, and restoring the last
//! suppressed instance writes the lower token again. Policy:
//! `docs/FORMAT_VERSIONS.md`.

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::PcbBoard;
use crate::pcb_sketch::BoardSketch;
use crate::sketch::SketchData;
use crate::sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use crate::sketch::entity::{Entity, EntityKind};
use crate::sketch::id::SketchEntityId;
use crate::sketch::plane::{Plane, PlaneId, PlaneKind};

fn save(board: &PcbBoard) -> String {
    SnxPcb::new(board.clone()).write_string().unwrap()
}

fn token_of(text: &str) -> &str {
    let first = text.lines().next().unwrap_or_default();
    first
        .strip_prefix("format = \"")
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

/// A board whose sketch holds a free point and a 4-long linear pattern of
/// it, nothing suppressed.
fn board() -> PcbBoard {
    let plane = PlaneId::new();
    let point = SketchEntityId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    sketch.entities.push(Entity::new(
        point,
        plane,
        EntityKind::Point { x: 1.0, y: 1.0 },
    ));
    sketch.arrays.push(Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source: point,
            count_expr: "4".into(),
            dx_expr: "5mm".into(),
            dy_expr: "0mm".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });
    let mut board = empty_board();
    board.sketch = Some(BoardSketch {
        sketch,
        ..BoardSketch::default()
    });
    board
}

fn set_suppressed(board: &mut PcbBoard, suppressed: bool) {
    let stored = board.sketch.as_mut().expect("a board sketch");
    for array in &mut stored.sketch.arrays {
        array.kind.set_instance_suppressed(2, 0, suppressed);
    }
}

/// The policy of a build that stops at `snxpcb/1.12`.
fn one_twelve() -> FormatVersion {
    FormatVersion {
        current: SNXPCB_FORMAT_V1_12,
        readable: &[
            "snxpcb/1",
            "snxpcb/1.1",
            "snxpcb/1.2",
            "snxpcb/1.3",
            SNXPCB_FORMAT_V1,
            SNXPCB_FORMAT_LAYER_COLORS,
            SNXPCB_FORMAT_ROOMS,
            SNXPCB_FORMAT_BOARD_SKETCH_TARGETS,
            SNXPCB_FORMAT_V1_8,
            SNXPCB_FORMAT_V1_9,
            SNXPCB_FORMAT_V1_10,
            SNXPCB_FORMAT_V1_11,
            SNXPCB_FORMAT_V1_12,
        ],
        ..SNXPCB_VERSION
    }
}

#[test]
fn a_linear_suppression_writes_snxpcb_1_13_and_round_trips() {
    let mut board = board();
    set_suppressed(&mut board, true);
    let text = save(&board);
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_13, "{text}");
    let file = SnxPcb::parse(&text).unwrap();
    assert!(!file.is_from_newer_minor(), "this build reads 1.13");
    assert_eq!(file.board.sketch, board.sketch, "the list round-trips");
    assert_eq!(
        file.write_string().unwrap(),
        text,
        "a re-save is byte-identical"
    );
}

#[test]
fn a_board_without_a_linear_suppression_keeps_its_token_and_bytes() {
    let mut board = board();
    let before = save(&board);
    assert_eq!(token_of(&before), SNXPCB_FORMAT_V1, "{before}");
    assert!(!before.contains("depopulation"), "{before}");
    set_suppressed(&mut board, true);
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_13);
    set_suppressed(&mut board, false);
    assert_eq!(save(&board), before, "restoring it writes the old bytes");
}

#[test]
fn a_build_that_knows_only_1_12_opens_a_linear_suppression_read_only() {
    let mut board = board();
    set_suppressed(&mut board, true);
    let text = save(&board);
    let token = token_of(&text);
    let older = one_twelve();
    assert!(older.is_newer_minor(token), "{token}");
    assert!(older.refuse_save(token).is_err(), "{token}");
    assert!(!SNXPCB_VERSION.is_newer_minor(token), "this build reads it");
}
