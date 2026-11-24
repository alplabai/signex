//! Mockup row 179: the Bake panel's rows carry the values Bake writes, not
//! a count per target, and a slot is one Slot row.

use signex_i18n::I18n;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use super::write_lines;
use crate::panels::{BakedHole, BakedOutline, BoardBakeRows};

fn lines(rows: &BoardBakeRows) -> Vec<(String, String)> {
    let plain = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
    write_lines(rows, &I18n::en_us())
        .into_iter()
        .map(|(key, value)| (plain(key.into_string()), plain(value)))
        .collect()
}

fn pair(key: &str, value: &str) -> (String, String) {
    (key.to_string(), value.to_string())
}

#[test]
fn each_row_is_a_real_value() {
    let rows = BoardBakeRows {
        outline: Some(BakedOutline::Size(40.0, 30.0)),
        holes: vec![
            BakedHole {
                drill_mm: 3.2,
                plated: true,
            },
            BakedHole {
                drill_mm: 2.5,
                plated: false,
            },
        ],
        keepouts: vec![vec![
            PcbLayer::Copper(CopperLayer::Top),
            PcbLayer::Copper(CopperLayer::Bottom),
        ]],
        cutouts: 1,
        slots: vec![(12.0, 3.0)],
        v_scores: 0,
        pours: 2,
        mechanical: 0,
        anchors: vec!["U1".into()],
    };
    assert_eq!(
        lines(&rows),
        vec![
            pair("Outline", "40.00 × 30.00 mm"),
            pair("Mounting hole", "Ø 3.20 mm · plated"),
            pair("Mounting hole", "Ø 2.50 mm · unplated"),
            pair("Keepouts", "On Top Layer, Bottom Layer"),
            pair("Cutouts", "1"),
            pair("Slot", "12.00 × 3.00 · routed"),
            pair("Pours", "2"),
            pair("Anchors", "Moves U1"),
        ]
    );
}

#[test]
fn a_removed_outline_says_so_and_nothing_written_lists_nothing() {
    let removed = BoardBakeRows {
        outline: Some(BakedOutline::Removed),
        ..BoardBakeRows::default()
    };
    assert_eq!(
        lines(&removed),
        vec![pair("Outline", "Removed from the board")]
    );
    assert!(lines(&BoardBakeRows::default()).is_empty());
}
