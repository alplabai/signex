//! #2578 / #2579 — a pour's `corners`, `corner_size`, `fill_mode`,
//! `hatch_width` and `hatch_gap` keys: written only when set, read back as
//! saved, and a board with a shaped pour writes `snxpcb/1.12`, which a build
//! that knows only 1.11 opens read-only. A board with none keeps its token,
//! byte for byte.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::{PcbBoard, Point, Zone};
use crate::zone_pour::POUR_FILL;
use crate::zone_pour_shape::{PourCorners, PourFillMode};

fn pour(n: u128) -> Zone {
    Zone {
        uuid: Uuid::from_u128(n),
        net: 0,
        net_name: String::new(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 3.0),
        ],
        holes: Vec::new(),
        priority: 1,
        fill_type: POUR_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.3,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled: false,
        corners: PourCorners::Sharp,
        corner_size: 0.0,
        fill_mode: PourFillMode::Solid,
        hatch_width: 0.0,
        hatch_gap: 0.0,
        unknown: Default::default(),
    }
}

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

/// The policy of a build that stops at `snxpcb/1.11`, before pour corners
/// and hatched fill.
fn one_eleven() -> FormatVersion {
    FormatVersion {
        current: SNXPCB_FORMAT_V1_11,
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
        ],
        ..SNXPCB_VERSION
    }
}

#[test]
fn a_rounded_hatched_pour_writes_snxpcb_1_12_and_round_trips() {
    let mut board = empty_board();
    let mut shaped = pour(1);
    shaped.corners = PourCorners::Rounded;
    shaped.corner_size = 0.5;
    shaped.fill_mode = PourFillMode::Hatched;
    shaped.hatch_width = 0.3;
    shaped.hatch_gap = 0.7;
    board.zones = vec![shaped, pour(2)];
    let text = save(&board);
    assert_eq!(text.matches("corners = \"rounded\"").count(), 1, "{text}");
    assert_eq!(text.matches("corner_size = 0.5").count(), 1, "{text}");
    assert_eq!(text.matches("fill_mode = \"hatched\"").count(), 1, "{text}");
    assert_eq!(text.matches("hatch_width = 0.3").count(), 1, "{text}");
    assert_eq!(text.matches("hatch_gap = 0.7").count(), 1, "{text}");
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_12, "{text}");
    let file = SnxPcb::parse(&text).unwrap();
    assert!(!file.is_from_newer_minor(), "this build reads 1.12");
    assert_eq!(file.board.zones, board.zones);
    assert_eq!(
        file.write_string().unwrap(),
        text,
        "a re-save is byte-identical"
    );
}

/// A board with sharp, solid pours writes none of the keys and keeps its
/// token; a chamfer alone raises it, and putting the pour back writes the
/// bytes it had before.
#[test]
fn a_board_without_a_shaped_pour_keeps_its_token() {
    let mut board = empty_board();
    board.zones = vec![pour(1)];
    let before = save(&board);
    for key in ["corners", "corner_size", "fill_mode", "hatch_"] {
        assert!(!before.contains(key), "{key}: {before}");
    }
    assert_eq!(token_of(&before), SNXPCB_FORMAT_V1);
    board.zones[0].corners = PourCorners::Chamfer;
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_12);
    board.zones[0].corners = PourCorners::Sharp;
    board.zones[0].fill_mode = PourFillMode::Hatched;
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_12);
    board.zones[0].fill_mode = PourFillMode::Solid;
    assert_eq!(save(&board), before, "the file is what it was");
}

/// A build whose policy stops at `snxpcb/1.11` sees a board with a shaped
/// pour as newer: it opens it read-only and refuses its saves instead of
/// filling the pour solid with sharp corners.
#[test]
fn a_shaped_pour_opens_read_only_in_a_build_that_knows_only_1_11() {
    let mut board = empty_board();
    let mut hatched = pour(1);
    hatched.fill_mode = PourFillMode::Hatched;
    board.zones = vec![hatched];
    let token = token_of(&save(&board)).to_string();
    assert_eq!(token, SNXPCB_FORMAT_V1_12);
    let older = one_eleven();
    assert!(older.is_newer_minor(&token), "{token}");
    assert!(older.refuse_save(&token).is_err(), "{token}");
    assert!(
        !SNXPCB_VERSION.is_newer_minor(&token),
        "this build reads it"
    );
    assert!(SNXPCB_VERSION.refuse_save(&token).is_ok());
}

/// A corner or fill token this build does not know is a load error, not a
/// silent Sharp or Solid.
#[test]
fn an_unknown_corner_or_fill_token_is_refused() {
    let mut board = empty_board();
    let mut shaped = pour(1);
    shaped.corners = PourCorners::Chamfer;
    shaped.fill_mode = PourFillMode::Hatched;
    board.zones = vec![shaped];
    let text = save(&board);
    let corners = text.replace("corners = \"chamfer\"", "corners = \"scalloped\"");
    assert!(SnxPcb::parse(&corners).is_err());
    let fill = text.replace("fill_mode = \"hatched\"", "fill_mode = \"none\"");
    assert!(SnxPcb::parse(&fill).is_err());
}
