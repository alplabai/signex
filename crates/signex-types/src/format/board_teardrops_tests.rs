//! The board's teardrop rule in `[board_options.teardrops]`: written only
//! when it is not the default, read back as saved, and a board carrying one
//! writes `snxpcb/1.12`, which a build that knows only 1.11 opens
//! read-only. A board with the default rule keeps its token, byte for byte.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::PcbBoard;
use crate::pcb_teardrop_rule::TeardropRule;

const PAD: Uuid = Uuid::from_u128(0x77);

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

/// The policy of a build that stops at `snxpcb/1.11`, before the rule.
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

fn custom_rule() -> TeardropRule {
    let mut rule = TeardropRule {
        pads: false,
        vias: true,
        width_share: 0.75,
        length_share: 1.5,
        ..TeardropRule::default()
    };
    rule.set_pad_override(PAD, Some(true));
    rule
}

#[test]
fn a_teardrop_rule_writes_snxpcb_1_12_and_round_trips() {
    let mut board = empty_board();
    board.options.teardrops = custom_rule();
    let text = save(&board);
    assert!(text.contains("[board_options.teardrops]"), "{text}");
    assert!(text.contains("width_share = 0.75"), "{text}");
    assert!(text.contains(&PAD.hyphenated().to_string()), "{text}");
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_12, "{text}");
    let file = SnxPcb::parse(&text).unwrap();
    assert!(!file.is_from_newer_minor(), "this build reads 1.12");
    assert_eq!(file.board.options.teardrops, board.options.teardrops);
    assert!(file.board.options.teardrops.pad_wants(PAD));
    assert_eq!(
        file.write_string().unwrap(),
        text,
        "a re-save is byte-identical"
    );
}

/// The default rule writes no table and keeps the token; going back to the
/// default writes the bytes the board had before.
#[test]
fn the_default_rule_writes_nothing_and_keeps_the_token() {
    let mut board = empty_board();
    let before = save(&board);
    assert!(!before.contains("teardrops"), "{before}");
    assert_eq!(token_of(&before), SNXPCB_FORMAT_V1);
    board.options.teardrops = custom_rule();
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_12);
    board.options.teardrops = TeardropRule::default();
    assert_eq!(save(&board), before, "the file is what it was");
}

/// A build that stops at 1.11 would drop the rule on save; it opens the
/// board read-only instead.
#[test]
fn a_teardrop_rule_opens_read_only_in_a_build_that_knows_only_1_11() {
    let mut board = empty_board();
    board.options.teardrops = custom_rule();
    let text = save(&board);
    let token = token_of(&text);
    let older = one_eleven();
    assert!(older.is_newer_minor(token), "{token}");
    assert!(older.refuse_save(token).is_err(), "{token}");
    assert!(!SNXPCB_VERSION.is_newer_minor(token), "this build reads it");
}

/// A key left out reads as the default's; a share out of range or a key
/// that is not a pad uuid refuses the file rather than guessing.
#[test]
fn a_partial_table_reads_defaults_and_a_bad_value_refuses_the_file() {
    let mut board = empty_board();
    board.options.teardrops = custom_rule();
    let text = save(&board);
    let partial = text.replace("width_share = 0.75\n", "");
    let read = SnxPcb::parse(&partial).expect("a partial table reads");
    assert_eq!(read.board.options.teardrops.width_share, 0.9);
    assert_eq!(read.board.options.teardrops.length_share, 1.5);

    let wide = text.replace("width_share = 0.75", "width_share = 1.5");
    let Err(err) = SnxPcb::parse(&wide) else {
        panic!("refused");
    };
    assert!(err.to_string().contains("teardrops"), "{err}");

    let key = PAD.hyphenated().to_string();
    let bad_key = text.replace(&key, "not-a-pad");
    assert!(SnxPcb::parse(&bad_key).is_err());
}
