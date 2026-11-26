//! #2942 — the board's layout plan in `[board_options.layout_plan]`: written
//! only when it sets something, read back as saved, and a board with one
//! writes `snxpcb/1.12`, which a build that knows only 1.11 opens read-only.
//! A board without a plan keeps its token, byte for byte.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::PcbBoard;
use crate::pcb_layout_plan::{LayoutPlan, LayoutSetting};

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

/// A plan setting every stage.
fn plan() -> LayoutPlan {
    let mut plan = LayoutPlan::default();
    let pour = Uuid::from_u128(0x77).to_string();
    for (setting, text) in [
        (LayoutSetting::Weights, "connection:3,edges:5.5"),
        (LayoutSetting::Rooms, "Power,\"Analog\" front"),
        (LayoutSetting::Keep, "J1,H1"),
        (LayoutSetting::Nets, "class:Power,GND"),
        (LayoutSetting::Skip, "NC_1"),
        (LayoutSetting::Pours, pour.as_str()),
        (LayoutSetting::Rules, "builtin::clearance,user::hv gap"),
        (LayoutSetting::Requirements, "size"),
    ] {
        plan.set(setting, text).unwrap();
    }
    plan
}

/// The policy of a build that stops at `snxpcb/1.11`, before the plan.
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
fn a_layout_plan_writes_snxpcb_1_12_and_round_trips() {
    let mut board = empty_board();
    board.options.layout_plan = plan();
    let text = save(&board);
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_12, "{text}");
    assert!(
        text.contains("[board_options.layout_plan.placement"),
        "{text}"
    );
    assert!(text.contains("class:Power"), "{text}");
    let file = SnxPcb::parse(&text).unwrap();
    assert!(!file.is_from_newer_minor(), "this build reads 1.12");
    assert_eq!(file.board.options.layout_plan, board.options.layout_plan);
    assert_eq!(
        file.write_string().unwrap(),
        text,
        "a re-save is byte-identical"
    );
}

/// One stage alone writes only its own table.
#[test]
fn only_the_stages_that_set_something_are_written() {
    let mut board = empty_board();
    board
        .options
        .layout_plan
        .set(LayoutSetting::Skip, "NC_1")
        .unwrap();
    let text = save(&board);
    assert!(
        text.contains("[board_options.layout_plan.routing]"),
        "{text}"
    );
    for stage in ["placement", "pours", "checks"] {
        let table = format!("[board_options.layout_plan.{stage}");
        assert!(!text.contains(&table), "{stage}: {text}");
    }
    let file = SnxPcb::parse(&text).unwrap();
    assert_eq!(file.board.options.layout_plan.routing.skip, ["NC_1"]);
}

/// A board without a plan writes no table and keeps its token; clearing the
/// last setting writes the bytes it had before.
#[test]
fn a_board_without_a_plan_keeps_its_token() {
    let mut board = empty_board();
    let before = save(&board);
    assert!(!before.contains("layout_plan"), "{before}");
    assert_eq!(token_of(&before), SNXPCB_FORMAT_V1);
    board.options.layout_plan = plan();
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_12);
    board.options.layout_plan = LayoutPlan::default();
    assert_eq!(save(&board), before, "the file is what it was");
}

/// A build whose policy stops at `snxpcb/1.11` sees a board with a plan as
/// newer: it opens it read-only and refuses its saves instead of dropping
/// the plan.
#[test]
fn a_layout_plan_opens_read_only_in_a_build_that_knows_only_1_11() {
    let mut board = empty_board();
    board.options.layout_plan = plan();
    let token = token_of(&save(&board)).to_string();
    let older = one_eleven();
    assert!(older.is_newer_minor(&token), "{token}");
    assert!(older.refuse_save(&token).is_err(), "{token}");
    assert!(
        !SNXPCB_VERSION.is_newer_minor(&token),
        "this build reads it"
    );
    assert!(SNXPCB_VERSION.refuse_save(&token).is_ok());
}

/// A plan this build cannot read refuses the file, saying what is wrong.
#[test]
fn a_plan_that_does_not_read_refuses_the_file() {
    let head = format!(
        "format = \"{SNXPCB_FORMAT_V1_12}\"\npcb_id = \"{}\"\n",
        Uuid::nil()
    );
    for (body, said) in [
        (
            "[board_options.layout_plan.placement.weights]\nheat = 1.0\n",
            "`heat` is not a priority",
        ),
        (
            "[board_options.layout_plan.placement.weights]\nedges = -1.0\n",
            "weight of `edges`",
        ),
        (
            "[board_options.layout_plan.pours]\npours = [\"x\"]\n",
            "`x` is not a uuid",
        ),
        (
            "[board_options.layout_plan.routing]\nnets = [\"class:\"]\n",
            "names no net or class",
        ),
        (
            "[board_options.layout_plan.checks]\nrules = [\"short\"]\n",
            "`short` is not a DRC rule id",
        ),
        (
            "[board_options.layout_plan.placement]\nrooms = [\"A\", \"A\"]\n",
            "`A` is listed twice",
        ),
    ] {
        let error = SnxPcb::parse(&format!("{head}{body}")).unwrap_err();
        let text = error.to_string();
        assert!(text.contains(said), "{body}: {text}");
        assert!(text.contains("board_options.layout_plan"), "{body}: {text}");
    }
}
