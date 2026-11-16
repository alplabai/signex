//! #2374 — the optional `[board_options]` section of a `.snxpcb`.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::pcb::Point;
use crate::pcb_options::{
    BoardGrid, BoardGuide, BoardOptions, BoardOrigin, BoardSheet, DesignatorDisplay, DrcWaiver,
    GuideKind, MARK_COLORS, PolygonNaming,
};
use crate::pcb_panel::{BoardPanel, PanelSeparation};

const OLD_BOARD: &str = include_str!("../../tests/corpus/pcb/board-outline-only.snxpcb");

fn options() -> BoardOptions {
    let mut guide = BoardGuide::new(
        Uuid::from_u128(1),
        GuideKind::Plus45,
        Point::new(1.5, -2.25),
    );
    guide.enabled = false;
    guide.color = MARK_COLORS[3];
    BoardOptions {
        guides: vec![
            guide,
            BoardGuide::new(
                Uuid::from_u128(2),
                GuideKind::Vertical,
                Point::new(10.0, 0.0),
            ),
        ],
        grids: vec![BoardGrid {
            uuid: Uuid::from_u128(3),
            name: "Fine \"pads\"".into(),
            step_mm: 0.125,
            origin: Point::new(0.5, 0.75),
            color: MARK_COLORS[1],
            components: true,
            non_components: false,
        }],
        sheet: BoardSheet {
            from_board: false,
            origin: Point::new(-5.0, -5.0),
            width: 210.0,
            height: 297.0,
        },
        polygon_naming: PolygonNaming::LayerNetPour,
        designator_display: DesignatorDisplay::Logical,
        origin: BoardOrigin::new(Point::new(-12.5, 80.0)),
        drc_waivers: vec![DrcWaiver {
            key: "0123456789abcdef0123456789abcdef".into(),
            rule: "builtin::clearance".into(),
            note: "Checked by hand: \"tested\" on rev A\nsecond line".into(),
        }],
        panel: Some(panel()),
    }
}

/// A three by one V-score panel with every length set (#2410).
fn panel() -> BoardPanel {
    let mut panel = BoardPanel {
        columns: 3,
        rows: 1,
        spacing_x: 0.0,
        spacing_y: 1.5,
        separation: PanelSeparation::VScore,
        ..BoardPanel::default()
    };
    panel.rails.left = 2.5;
    panel.rails.right = 3.25;
    panel.bites.tabs_per_edge = 3;
    panel.bites.pitch = 0.75;
    panel
}

/// #2399: a board whose only option is a waived finding writes the section
/// and reads the waiver, note and all, back unchanged.
#[test]
fn a_waived_finding_round_trips_with_its_note() {
    let mut board = empty_board();
    board.options.drc_waivers = vec![DrcWaiver {
        key: "k1".into(),
        rule: "user::hv".into(),
        note: String::new(),
    }];
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(text.contains("drc_waivers"), "{text}");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.options.drc_waivers, board.options.drc_waivers);
    assert_eq!(SnxPcb::new(back).write_string().expect("again"), text);
}

#[test]
fn a_board_without_options_writes_no_section() {
    let board = SnxPcb::parse(OLD_BOARD).expect("parses").board;
    assert!(board.options.is_default());
    let text = SnxPcb::new(board).write_string().expect("writes");
    assert_eq!(text, OLD_BOARD, "no-edit save must keep every byte");
    assert!(!text.contains("board_options"), "{text}");
}

#[test]
fn every_option_round_trips() {
    let mut board = empty_board();
    board.options = options();
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(text.contains("[board_options]"), "{text}");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.options, board.options);
    let again = SnxPcb::new(back).write_string().expect("writes again");
    assert_eq!(again, text, "a second save is byte-identical");
}

#[test]
fn a_bad_colour_is_refused_on_read() {
    let mut board = empty_board();
    board.options = options();
    let text = SnxPcb::new(board).write_string().expect("writes");
    let broken = text.replacen(&MARK_COLORS[3].to_hex(), "teal", 1);
    assert!(matches!(
        SnxPcb::parse(&broken),
        Err(FormatError::TsvFieldParse { .. })
    ));
}

#[test]
fn a_non_finite_guide_is_refused_on_write() {
    let mut board = empty_board();
    board.options.guides.push(BoardGuide::new(
        Uuid::nil(),
        GuideKind::Horizontal,
        Point::new(0.0, f64::NAN),
    ));
    assert!(matches!(
        SnxPcb::new(board).write_string(),
        Err(FormatError::NonFiniteField { .. })
    ));
}

/// #2384: a board keeps the origin it was saved with, and one saved before
/// the origin existed reloads counting from the model's zero.
#[test]
fn the_saved_origin_reloads_unchanged() {
    let old = SnxPcb::parse(OLD_BOARD).expect("parses").board;
    assert_eq!(old.options.origin, BoardOrigin::default());
    let mut board = empty_board();
    board.options.origin = BoardOrigin::new(Point::new(3.25, 47.5));
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(text.contains("[board_options.origin]"), "{text}");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.options.origin, board.options.origin);
    assert_eq!(SnxPcb::new(back).write_string().expect("again"), text);
}

/// #2410: the panel reloads as saved, under its own table, and a panel that
/// breaks a rule (no columns) is refused on load.
#[test]
fn the_panel_round_trips_and_a_broken_one_is_refused() {
    let mut board = empty_board();
    board.options.panel = Some(panel());
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(text.contains("[board_options.panel]"), "{text}");
    assert!(text.contains("separation = \"v_score\""), "{text}");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.options.panel, board.options.panel);
    let broken = text.replacen("columns = 3", "columns = 0", 1);
    assert!(matches!(
        SnxPcb::parse(&broken),
        Err(FormatError::TsvFieldParse { .. })
    ));
}
