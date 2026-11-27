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
            fingerprint: String::new(),
            author: "C. Alp".into(),
        }],
        panel: Some(panel()),
        layer_colors: Vec::new(),
        length_groups: Vec::new(),
        teardrops: Default::default(),
        layout_plan: Default::default(),
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
        fingerprint: String::new(),
        author: String::new(),
    }];
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(text.contains("drc_waivers"), "{text}");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.options.drc_waivers, board.options.drc_waivers);
    assert_eq!(SnxPcb::new(back).write_string().expect("again"), text);
}

/// #2307: a waiver saved before the fingerprint (no `fingerprint` key)
/// loads with an empty one and re-saves byte-identical, without the key.
#[test]
fn a_waiver_without_a_fingerprint_loads_and_re_saves_unchanged() {
    let mut board = empty_board();
    board.options.drc_waivers = vec![DrcWaiver {
        key: "k1".into(),
        rule: "builtin::clearance".into(),
        note: "old".into(),
        fingerprint: String::new(),
        author: String::new(),
    }];
    let text = SnxPcb::new(board).write_string().expect("writes");
    assert!(!text.contains("fingerprint"), "{text}");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.options.drc_waivers[0].fingerprint, "");
    assert_eq!(SnxPcb::new(back).write_string().expect("again"), text);
}

/// #2307: a waiver's geometry fingerprint is written and read back.
#[test]
fn a_waiver_fingerprint_round_trips() {
    let mut board = empty_board();
    board.options.drc_waivers = vec![DrcWaiver {
        key: "k1".into(),
        rule: "builtin::clearance".into(),
        note: String::new(),
        fingerprint: "00ff00ff00ff00ff00ff00ff00ff00ff".into(),
        author: String::new(),
    }];
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(
        text.contains("fingerprint = \"00ff00ff00ff00ff00ff00ff00ff00ff\""),
        "{text}"
    );
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.options.drc_waivers, board.options.drc_waivers);
    assert_eq!(SnxPcb::new(back).write_string().expect("again"), text);
}

/// AV-13: a waiver's author (multi-line reason and all) is written and read
/// back, and a second save is byte-identical.
#[test]
fn a_waiver_author_round_trips() {
    let mut board = empty_board();
    board.options.drc_waivers = vec![DrcWaiver {
        key: "k1".into(),
        rule: "builtin::clearance".into(),
        note: "Checked on the prototype\nby the layout review".into(),
        fingerprint: "00ff00ff00ff00ff00ff00ff00ff00ff".into(),
        author: "C. Alp".into(),
    }];
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(text.contains("author = \"C. Alp\""), "{text}");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.options.drc_waivers, board.options.drc_waivers);
    assert_eq!(SnxPcb::new(back).write_string().expect("again"), text);
}

/// AV-13: a board saved before waivers had an author (no `author` key)
/// loads with an empty one and re-saves without the key.
#[test]
fn a_waiver_without_an_author_loads_and_re_saves_unchanged() {
    let mut board = empty_board();
    board.options.drc_waivers = vec![DrcWaiver {
        key: "k1".into(),
        rule: "builtin::clearance".into(),
        note: "old".into(),
        fingerprint: "00ff00ff00ff00ff00ff00ff00ff00ff".into(),
        author: String::new(),
    }];
    let text = SnxPcb::new(board).write_string().expect("writes");
    assert!(!text.contains("author"), "{text}");
    let back = SnxPcb::parse(&text).expect("an old board loads").board;
    assert_eq!(back.options.drc_waivers[0].author, "");
    assert_eq!(back.options.drc_waivers[0].note, "old");
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

/// 2026-11-18: layer colours round-trip in `[board_options.layer_colors]`
/// under `snxpcb/1.5`; a board whose colours are all reset writes 1.4 again.
#[test]
fn layer_colours_round_trip_and_raise_the_token() {
    use crate::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
    let silk = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    let top = PcbLayer::Copper(CopperLayer::Top);
    let mut board = empty_board();
    let plain = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(plain.starts_with(&format!("format = \"{SNXPCB_FORMAT_V1}\"")));
    assert!(board.options.set_layer_color(silk, Some(MARK_COLORS[1])));
    assert!(board.options.set_layer_color(top, Some(MARK_COLORS[0])));
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    assert!(
        text.starts_with(&format!("format = \"{SNXPCB_FORMAT_LAYER_COLORS}\"")),
        "{text}"
    );
    assert!(text.contains("[board_options.layer_colors]"), "{text}");
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.options.layer_colors, board.options.layer_colors);
    assert_eq!(back.options.layer_color(silk), Some(MARK_COLORS[1]));
    assert_eq!(SnxPcb::new(back).write_string().expect("again"), text);

    assert!(board.options.set_layer_color(silk, None));
    assert!(board.options.set_layer_color(top, None));
    let reset = SnxPcb::new(board).write_string().expect("writes");
    assert_eq!(reset, plain, "no colours: byte-identical to before");
}

/// A colour row naming a layer this build does not know refuses the file
/// instead of dropping the colour on the next save.
#[test]
fn an_unknown_layer_colour_token_refuses_the_file() {
    let mut board = empty_board();
    board.options.set_layer_color(
        crate::pcb_layer::PcbLayer::Technical(crate::pcb_layer::TechnicalLayer::PasteTop),
        Some(MARK_COLORS[2]),
    );
    let text = SnxPcb::new(board).write_string().expect("writes");
    let broken = text.replacen("TopPaste =", "NoSuchLayer =", 1);
    assert_ne!(broken, text, "{text}");
    assert!(SnxPcb::parse(&broken).is_err());
}
