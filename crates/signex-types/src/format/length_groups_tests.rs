//! #2797: length-matching groups round-trip in
//! `[[board_options.length_groups]]` under `snxpcb/1.12`; a board without
//! one keeps its lower token, byte for byte, and a build that stops at 1.11
//! opens a board with one read-only.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::PcbBoard;
use crate::pcb_length::{LengthGroup, LengthTopology};

fn group(n: u128, name: &str, members: &[&str]) -> LengthGroup {
    LengthGroup {
        uuid: Uuid::from_u128(n),
        name: name.into(),
        target_mm: 28.5,
        tolerance_mm: 0.25,
        topology: LengthTopology::FlyBy,
        members: members.iter().map(|m| (*m).to_string()).collect(),
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

/// The policy of a build that stops at `snxpcb/1.11`.
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
fn groups_round_trip_under_snxpcb_1_12() {
    let mut board = empty_board();
    board.options.length_groups = vec![
        group(1, "DDR_DQ", &["DDR_DQ0", "DDR_DQ1"]),
        group(2, "USB \"D\"", &["USB_DP", "USB_DM"]),
    ];
    let text = save(&board);
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_12, "{text}");
    assert!(text.contains("[[board_options.length_groups]]"), "{text}");
    assert!(text.contains("topology = \"fly_by\""), "{text}");
    assert!(text.contains("target = 28500000"), "lengths are nm: {text}");
    let file = SnxPcb::parse(&text).unwrap();
    assert!(!file.is_from_newer_minor(), "this build reads 1.12");
    assert_eq!(
        file.board.options.length_groups,
        board.options.length_groups
    );
    assert_eq!(file.write_string().unwrap(), text, "re-save is identical");
}

#[test]
fn removing_the_last_group_writes_the_lower_token_again() {
    let mut board = empty_board();
    let before = save(&board);
    board.options.length_groups.push(group(1, "CLK", &[]));
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_12);
    board.options.length_groups.clear();
    assert_eq!(save(&board), before, "the file is what it was");
    assert!(!before.contains("length_groups"), "{before}");
}

#[test]
fn a_group_this_build_cannot_hold_refuses_the_file() {
    let mut board = empty_board();
    board.options.length_groups.push(group(1, "CLK", &["CLK"]));
    let text = save(&board);
    for (from, to) in [
        ("topology = \"fly_by\"", "topology = \"ring\""),
        ("tolerance = 250000", "tolerance = -1"),
        ("target = 28500000", "target = 0"),
        ("name = \"CLK\"", "name = \" \""),
    ] {
        let broken = text.replacen(from, to, 1);
        assert_ne!(broken, text, "{from} is in {text}");
        assert!(
            matches!(
                SnxPcb::parse(&broken),
                Err(FormatError::TsvFieldParse { .. })
            ),
            "{to}"
        );
    }
}

#[test]
fn a_non_finite_target_is_refused_on_write() {
    let mut board = empty_board();
    let mut bad = group(1, "CLK", &[]);
    bad.target_mm = f64::NAN;
    board.options.length_groups.push(bad);
    assert!(matches!(
        SnxPcb::new(board).write_string(),
        Err(FormatError::NonFiniteField { .. })
    ));
}

#[test]
fn groups_open_read_only_in_a_build_that_knows_only_1_11() {
    let mut board = empty_board();
    board.options.length_groups.push(group(1, "CLK", &["CLK"]));
    let text = save(&board);
    let token = token_of(&text);
    assert_eq!(token, SNXPCB_FORMAT_V1_12);
    let older = one_eleven();
    assert!(older.is_newer_minor(token), "{token}");
    assert!(older.refuse_save(token).is_err(), "{token}");
    assert!(!SNXPCB_VERSION.is_newer_minor(token), "this build reads it");
}
