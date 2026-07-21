//! ERC's verdict must not depend on the order wires sit in the document.
//!
//! Every net rule reads `SheetConnectivity::root_of_anchored`, which used to
//! anchor a point into the *first* wire whose segment it touched. At a
//! junction-less T — a point that is one wire's own endpoint and another wire's
//! interior — that either bridged the two wires into one net or left them
//! separate, decided purely by which wire the slice yielded first. Two
//! conflicting labels straddling the T therefore reported a conflict or not
//! depending on draw order (issue #402).
//!
//! Asserting the whole violation multiset (not just one rule) covers every rule
//! that routes through the shared anchor, `derive_nets` included.

use signex_erc::{RuleKind, Violation};
use signex_types::schematic::{HAlign, Label, LabelType, Point, SchematicSheet, VAlign, Wire};
use std::collections::HashMap;
use uuid::Uuid;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn sheet(wires: Vec<Wire>, labels: Vec<Label>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::nil(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols: Vec::new(),
        wires,
        junctions: Vec::new(),
        labels,
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
    }
}

fn wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: Uuid::new_v4(),
        start: a,
        end: b,
        stroke_width: 0.0,
    }
}

fn net_label(text: &str, pos: Point) -> Label {
    Label {
        uuid: Uuid::new_v4(),
        text: text.to_string(),
        position: pos,
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

/// Violations reduced to an order-insensitive, uuid-free fingerprint. Rule
/// *order* within the list legitimately follows document order; what must not
/// change is the set of verdicts.
fn fingerprint(violations: &[Violation]) -> Vec<(RuleKind, String)> {
    let mut v: Vec<(RuleKind, String)> = violations
        .iter()
        .map(|x| (x.rule, x.message.clone()))
        .collect();
    v.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(format!("{:?}", a.0).cmp(&format!("{:?}", b.0)))
    });
    v
}

/// A = (0,0)–(10,0), B = (5,0)–(5,10), no junction dot. B *ends* on A's
/// interior, so (5,0) is simultaneously B's own endpoint and a point of A.
///
/// `ALPHA` sits exactly on that T point — the ambiguous one. `BETA` sits on A's
/// **pure** interior, away from every endpoint, so it always belongs to A. If
/// anchoring bridges A and B, both names land on one net and ERC reports a
/// `NetLabelConflict`; if it doesn't, they stay on separate nets and it
/// reports nothing. The old first-match anchor picked whichever wire came
/// first, so this one sheet produced both verdicts.
fn conflicting_t(reversed: bool) -> SchematicSheet {
    let mut wires = vec![
        wire(pt(0.0, 0.0), pt(10.0, 0.0)),
        wire(pt(5.0, 0.0), pt(5.0, 10.0)),
    ];
    if reversed {
        wires.reverse();
    }
    sheet(
        wires,
        vec![
            net_label("ALPHA", pt(5.0, 0.0)),
            net_label("BETA", pt(8.0, 0.0)),
        ],
    )
}

#[test]
fn erc_verdict_is_independent_of_wire_order_at_a_junction_less_t() {
    let forward = signex_erc::run(&conflicting_t(false));
    let reversed = signex_erc::run(&conflicting_t(true));

    assert_eq!(
        fingerprint(&forward),
        fingerprint(&reversed),
        "ERC changed its mind when the two wires were drawn in the other order"
    );
}

#[test]
fn net_label_conflict_count_is_independent_of_wire_order() {
    let count = |s: &SchematicSheet| {
        signex_erc::run(s)
            .iter()
            .filter(|v| v.rule == RuleKind::NetLabelConflict)
            .count()
    };

    assert_eq!(
        count(&conflicting_t(false)),
        count(&conflicting_t(true)),
        "a junction-less T flipped the NetLabelConflict verdict"
    );
}
