//! Foreign keys in `[extras]` tables a newer Signex may write: an unknown
//! key under `[extras.junctions.<uuid>]` in a `.snxsch` (#1874) and an
//! unknown `[extras.<name>]` table in a `.snxpcb` (#1873) must survive a
//! load -> save cycle, and a second save must be byte-identical.

use super::tests::empty_sheet;
use super::unknown_tests::plain_board;
use super::*;
use crate::schematic::{Junction, Point, SchematicSheet};

const JUNCTION_UUID: &str = "0192a8c0-0020-7000-8000-000000000001";

/// A sheet holding one user-placed junction, whose extras table is empty
/// until a test injects foreign keys.
fn sheet_with_junction() -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.junctions.push(Junction {
        uuid: Uuid::parse_str(JUNCTION_UUID).unwrap(),
        position: Point::new(10.16, 5.08),
        ..Default::default()
    });
    sheet
}

fn resave_sheet(text: &str) -> String {
    let sheet = SnxSchematic::parse(text).unwrap().sheet;
    SnxSchematic::new(sheet).write_string().unwrap()
}

fn resave_pcb(text: &str) -> String {
    let board = SnxPcb::parse(text).unwrap().board;
    SnxPcb::new(board).write_string().unwrap()
}

fn plain_sheet_text() -> String {
    SnxSchematic::new(sheet_with_junction())
        .write_string()
        .unwrap()
}

#[test]
fn unknown_junction_extras_keys_survive_save() {
    // The junction is user-placed, so without the catch-all its extras
    // table is default and the writer elides it with the foreign keys.
    let text = format!(
        "{}
[extras.junctions.{JUNCTION_UUID}]
future_junction_key = 7

[extras.junctions.{JUNCTION_UUID}.vendor]
note = \"j\"
",
        plain_sheet_text()
    );
    let out = resave_sheet(&text);
    assert!(out.contains("future_junction_key = 7"), "{out}");
    assert!(out.contains("note = \"j\""), "{out}");
    assert!(
        out.contains(&format!("[extras.junctions.{JUNCTION_UUID}.vendor]")),
        "{out}"
    );
    assert!(!out.contains("minted = false"), "{out}");
}

#[test]
fn unknown_junction_extras_keys_sit_beside_minted() {
    let text = format!(
        "{}
[extras.junctions.{JUNCTION_UUID}]
minted = true
future_junction_key = 7
",
        plain_sheet_text()
    );
    let out = resave_sheet(&text);
    assert!(out.contains("minted = true"), "{out}");
    assert!(out.contains("future_junction_key = 7"), "{out}");
    let sheet = SnxSchematic::parse(&out).unwrap().sheet;
    assert!(sheet.junctions[0].minted);
    assert_eq!(sheet.junctions[0].unknown.len(), 1);
}

#[test]
fn unknown_junction_extras_save_is_byte_stable() {
    let text = format!(
        "{}
[extras.junctions.{JUNCTION_UUID}]
future_junction_key = 7
",
        plain_sheet_text()
    );
    let first = resave_sheet(&text);
    assert!(first.contains("future_junction_key = 7"), "{first}");
    assert_eq!(resave_sheet(&first), first);
}

#[test]
fn junction_without_unknowns_writes_no_extras() {
    let out = plain_sheet_text();
    assert!(!out.contains("[extras"), "{out}");
    assert_eq!(resave_sheet(&out), out);
}

/// A plain board with an unknown table under `[extras]`, with a nested
/// sub-table and a scalar beside it.
fn pcb_with_unknown_extras_table() -> String {
    let board = SnxPcb::new(plain_board()).write_string().unwrap();
    format!(
        "{board}
[extras]
future_extras_key = 3

[extras.mystery]
key = \"m\"
[extras.mystery.deep]
n = 4
"
    )
}

#[test]
fn pcb_unknown_extras_tables_survive_save() {
    let out = resave_pcb(&pcb_with_unknown_extras_table());
    assert!(out.contains("future_extras_key = 3"), "{out}");
    assert!(out.contains("[extras.mystery]"), "{out}");
    assert!(out.contains("[extras.mystery.deep]"), "{out}");
    let board = SnxPcb::parse(&out).unwrap().board;
    assert!(board.unknown.extras_root.contains_key("mystery"));
    assert!(board.unknown.extras_root.contains_key("future_extras_key"));
}

#[test]
fn pcb_unknown_extras_tables_survive_beside_known_extras() {
    let text = format!(
        "{}
[extras.board]
board_future = 1

[extras.footprints.0192a8c0-0010-7000-8000-000000000001]
locked = true
",
        pcb_with_unknown_extras_table()
    );
    let board = SnxPcb::parse(&text).unwrap().board;
    assert!(board.footprints[0].locked);
    let out = SnxPcb::new(board.clone()).write_string().unwrap();
    assert!(out.contains("[extras.mystery.deep]"), "{out}");
    assert!(out.contains("board_future = 1"), "{out}");
    let again = SnxPcb::parse(&out).unwrap().board;
    assert_eq!(again.unknown, board.unknown);
    assert!(again.footprints[0].locked);
}

#[test]
fn pcb_unknown_extras_save_is_byte_stable() {
    let first = resave_pcb(&pcb_with_unknown_extras_table());
    assert!(first.contains("[extras.mystery.deep]"), "{first}");
    assert_eq!(resave_pcb(&first), first);
}
