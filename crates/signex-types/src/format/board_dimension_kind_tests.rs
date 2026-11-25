//! #2507 — the kind keys of a `[[free.dimensions]]` row: an aligned
//! dimension writes none and keeps the board's token; any other kind
//! writes `kind` (a radial one also `measures`) and `snxpcb/1.11`.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::Point;
use crate::pcb_dimension::LinearDimension;
use crate::pcb_dimension_kind::DimensionKind;

fn dimension(kind: DimensionKind) -> LinearDimension {
    let mut d = LinearDimension::new(
        Uuid::from_u128(31),
        Point::new(0.0, 0.0),
        Point::new(20.0, 5.0),
        -2.5,
    );
    d.kind = kind;
    if kind == DimensionKind::Radial {
        d.offset = 0.0;
        d.measures = Some(Uuid::from_u128(32));
    }
    d
}

fn saved(d: LinearDimension) -> String {
    let mut board = empty_board();
    board.free.dimensions.push(d);
    SnxPcb::new(board).write_string().expect("writes")
}

/// The keys of the `[[free.dimensions]]` table of `text`.
fn row_keys(text: &str) -> Vec<String> {
    let start = text.find("[[free.dimensions]]").expect("a dimension row");
    text[start..]
        .lines()
        .skip(1)
        .take_while(|l| !l.starts_with('['))
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim().to_string()))
        .collect()
}

fn token_of(text: &str) -> &str {
    let first = text.lines().next().unwrap_or_default();
    first
        .strip_prefix("format = \"")
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

/// An aligned dimension, the one every board had before the kinds, writes
/// no kind key and keeps the token, so an older board re-saves
/// byte-identical.
#[test]
fn an_aligned_dimension_writes_no_kind_and_keeps_its_token() {
    let d = dimension(DimensionKind::Aligned);
    assert!(!dimension_uses_style(&d));
    let text = saved(d.clone());
    let keys = row_keys(&text);
    assert!(!keys.iter().any(|k| k == "kind" || k == "measures"), "{text}");
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1, "{text}");
    let file = SnxPcb::parse(&text).expect("reads");
    assert_eq!(file.board.free.dimensions, vec![d]);
    assert_eq!(file.write_string().expect("writes again"), text);
}

/// Each other kind writes its `kind` key and `snxpcb/1.11`, reads back
/// whole and re-saves byte-identical.
#[test]
fn each_other_kind_writes_snxpcb_1_11_and_round_trips() {
    for (kind, key) in [
        (DimensionKind::Horizontal, "kind = \"horizontal\""),
        (DimensionKind::Vertical, "kind = \"vertical\""),
        (DimensionKind::Radial, "kind = \"radial\""),
    ] {
        let d = dimension(kind);
        assert!(dimension_uses_style(&d), "{kind:?}");
        let text = saved(d.clone());
        assert!(text.contains(key), "{key} missing from {text}");
        let measures = row_keys(&text).iter().any(|k| k == "measures");
        assert_eq!(measures, kind == DimensionKind::Radial, "{text}");
        assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_11, "{text}");
        let file = SnxPcb::parse(&text).expect("reads");
        assert!(!file.is_from_newer_minor(), "this build reads 1.11");
        assert_eq!(file.board.free.dimensions, vec![d], "{kind:?}");
        assert_eq!(file.write_string().expect("writes again"), text);
    }
}

/// The token goes up with the first dimension of another kind and back
/// down, to the byte, when it is aligned again.
#[test]
fn the_token_follows_the_kind_both_ways() {
    let mut board = empty_board();
    board.free.dimensions = vec![dimension(DimensionKind::Aligned)];
    let before = SnxPcb::new(board.clone()).write_string().unwrap();
    assert_eq!(token_of(&before), SNXPCB_FORMAT_V1);
    board.free.dimensions[0].kind = DimensionKind::Vertical;
    assert!(board_uses_dimension_style(&board));
    let raised = SnxPcb::new(board.clone()).write_string().unwrap();
    assert_eq!(token_of(&raised), SNXPCB_FORMAT_V1_11);
    board.free.dimensions[0].kind = DimensionKind::Aligned;
    assert!(!board_uses_dimension_style(&board));
    let back = SnxPcb::new(board).write_string().unwrap();
    assert_eq!(back, before, "the file is what it was");
}

/// A build whose policy stops at `snxpcb/1.10` opens a board with a radial
/// dimension read-only and refuses its saves, instead of dropping its kind.
#[test]
fn a_dimension_kind_opens_read_only_in_a_build_that_knows_only_1_10() {
    let one_ten = FormatVersion {
        current: SNXPCB_FORMAT_V1_10,
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
        ],
        ..SNXPCB_VERSION
    };
    let text = saved(dimension(DimensionKind::Radial));
    let token = token_of(&text);
    assert_eq!(token, SNXPCB_FORMAT_V1_11);
    assert!(one_ten.is_newer_minor(token), "{token}");
    assert!(one_ten.refuse_save(token).is_err(), "{token}");
    assert!(SNXPCB_VERSION.refuse_save(token).is_ok());
}
