//! Forward-compatibility guards for TSV bulk blocks: a column a newer
//! Signex added must neither refuse the file nor be dropped, and must
//! stay attached to its own row through a load -> edit -> save cycle.

use super::tests::{empty_sheet, sample_symbol};
use super::tsv::{encode_cell, escape_tsv_body_for_toml};
use super::*;
use crate::pcb::{Point as PcbPoint, Segment, Via, ViaType};
use crate::schematic::{
    HAlign, Junction, Label, LabelType, Point as SchPoint, SchematicSheet, VAlign, Wire,
};

/// Awkward cell content: whitespace, quotes, backslash, a lone `-`, empty.
const NASTY: [&str; 3] = ["a b \"q\" C:\\p\tz", "-", ""];

fn uuid_n(n: u32) -> Uuid {
    Uuid::parse_str(&format!("0192a8c0-00aa-7000-8000-{n:012}")).unwrap()
}

fn two_row_sheet() -> SchematicSheet {
    let mut sheet = empty_sheet();
    for n in 0..2u32 {
        let mut s = sample_symbol();
        s.uuid = uuid_n(n);
        s.footprint.clear();
        s.reference = format!("R{n}");
        sheet.symbols.push(s);
        sheet.wires.push(Wire {
            unknown_columns: Default::default(),
            uuid: uuid_n(10 + n),
            start: SchPoint {
                x: 1.0,
                y: f64::from(n),
            },
            end: SchPoint { x: 2.0, y: 3.0 },
            stroke_width: 0.0,
        });
        sheet.junctions.push(Junction {
            unknown_columns: Default::default(),
            uuid: uuid_n(20 + n),
            position: SchPoint {
                x: f64::from(n),
                y: 1.0,
            },
            diameter: 0.0,
            minted: false,
        });
        sheet.labels.push(Label {
            unknown_columns: Default::default(),
            uuid: uuid_n(30 + n),
            text: format!("NET{n}"),
            position: SchPoint { x: 1.0, y: 1.0 },
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 1.27,
            justify: HAlign::Left,
            justify_v: VAlign::Bottom,
        });
    }
    sheet
}

fn two_row_board() -> PcbBoard {
    let mut board = super::unknown_tests::plain_board();
    let mut fp2 = board.footprints[0].clone();
    fp2.uuid = uuid_n(40);
    fp2.reference = "U2".into();
    fp2.pads[0].uuid = uuid_n(41);
    board.footprints.push(fp2);
    for n in 0..2u32 {
        board.segments.push(Segment {
            unknown_columns: Default::default(),
            uuid: uuid_n(50 + n),
            start: PcbPoint {
                x: 0.0,
                y: f64::from(n),
            },
            end: PcbPoint { x: 1.0, y: 1.0 },
            width: 0.2,
            layer: "TopCopper".into(),
            net: 1,
        });
        board.vias.push(Via {
            unknown_columns: Default::default(),
            uuid: uuid_n(60 + n),
            position: PcbPoint {
                x: f64::from(n),
                y: 2.0,
            },
            diameter: 0.6,
            drill: 0.3,
            layers: vec!["TopCopper".into(), "BottomCopper".into()],
            net: 1,
            via_type: ViaType::Through,
        });
    }
    board
}

/// Rewrite the `[section]` TSV body line by line (header first).
fn map_block(text: &str, section: &str, mut f: impl FnMut(usize, &str) -> String) -> String {
    let open = format!("[{section}]\ncontent = \"\"\"\n");
    let start = text.find(&open).expect("section present") + open.len();
    let len = text[start..].find("\"\"\"").expect("section closes");
    let body: Vec<String> = text[start..start + len]
        .lines()
        .enumerate()
        .map(|(i, l)| f(i, l))
        .collect();
    format!(
        "{}{}\n{}",
        &text[..start],
        body.join("\n"),
        &text[start + len..]
    )
}

/// Add column `name` (cell per data row from `NASTY`) after the first
/// column when `middle`, else at the end.
fn inject(text: &str, section: &str, name: &str, middle: bool) -> String {
    map_block(text, section, |i, line| {
        let cell = if i == 0 {
            name.to_string()
        } else {
            escape_tsv_body_for_toml(&encode_cell(NASTY[(i - 1) % NASTY.len()]))
        };
        match (middle, line.split_once(char::is_whitespace)) {
            (true, Some((first, rest))) => format!("{first}  {cell}  {}", rest.trim_start()),
            _ => format!("{line}  {cell}"),
        }
    })
}

fn sheet_text() -> String {
    SnxSchematic::new(two_row_sheet()).write_string().unwrap()
}

fn board_text() -> String {
    SnxPcb::new(two_row_board()).write_string().unwrap()
}

const SHEET_SECTIONS: [&str; 4] = [
    "sheets.components",
    "sheets.wires",
    "sheets.junctions",
    "sheets.labels",
];

const BOARD_SECTIONS: [&str; 4] = ["footprints", "pads", "tracks", "vias"];

/// The saved block's rows must carry every injected cell, escaped.
fn assert_cells_present(saved: &str) {
    for cell in NASTY {
        let enc = escape_tsv_body_for_toml(&encode_cell(cell));
        assert!(saved.contains(&format!("  {enc}")), "cell {cell:?} lost");
    }
}

#[test]
fn sheet_unknown_columns_round_trip_in_every_section() {
    for middle in [false, true] {
        let mut text = sheet_text();
        for section in SHEET_SECTIONS {
            text = inject(&text, section, "vendor_col", middle);
            text = inject(&text, section, "later_col", false);
        }
        let first = SnxSchematic::parse(&text).unwrap().sheet;
        let saved = SnxSchematic::new(first).write_string().unwrap();
        let second = SnxSchematic::parse(&saved).unwrap().sheet;
        assert_eq!(SnxSchematic::new(second).write_string().unwrap(), saved);
        assert_cells_present(&saved);
        assert_eq!(squash(&saved).matches("vendor_col later_col").count(), 4);
    }
}

#[test]
fn board_unknown_columns_round_trip_in_every_section() {
    for middle in [false, true] {
        let mut text = board_text();
        for section in BOARD_SECTIONS {
            text = inject(&text, section, "vendor_col", middle);
            text = inject(&text, section, "later_col", false);
        }
        let first = SnxPcb::parse(&text).unwrap().board;
        let saved = SnxPcb::new(first).write_string().unwrap();
        let second = SnxPcb::parse(&saved).unwrap().board;
        assert_eq!(SnxPcb::new(second).write_string().unwrap(), saved);
        assert_cells_present(&saved);
        assert_eq!(squash(&saved).matches("vendor_col later_col").count(), 4);
    }
}

/// Collapse alignment padding so header assertions ignore column widths.
fn squash(text: &str) -> String {
    let parts: Vec<&str> = text.split(' ').filter(|p| !p.is_empty()).collect();
    parts.join(" ")
}

fn cell(cols: &UnknownColumns, name: &str) -> String {
    cols.get(name).unwrap_or("<absent>").to_string()
}

/// Both rows of every section carry their own cells, in file order.
#[test]
fn unknown_columns_attach_to_their_own_row() {
    let mut text = sheet_text();
    for section in SHEET_SECTIONS {
        text = inject(&text, section, "vendor_col", true);
    }
    let sheet = SnxSchematic::parse(&text).unwrap().sheet;
    for (i, want) in NASTY.iter().take(2).enumerate() {
        assert_eq!(cell(&sheet.symbols[i].unknown_columns, "vendor_col"), *want);
        assert_eq!(cell(&sheet.wires[i].unknown_columns, "vendor_col"), *want);
        assert_eq!(
            cell(&sheet.junctions[i].unknown_columns, "vendor_col"),
            *want
        );
        assert_eq!(cell(&sheet.labels[i].unknown_columns, "vendor_col"), *want);
    }

    let mut text = board_text();
    for section in BOARD_SECTIONS {
        text = inject(&text, section, "vendor_col", true);
    }
    let board = SnxPcb::parse(&text).unwrap().board;
    for (i, want) in NASTY.iter().take(2).enumerate() {
        let fp = &board.footprints[i];
        assert_eq!(cell(&fp.unknown_columns, "vendor_col"), *want);
        assert_eq!(cell(&fp.pads[0].unknown_columns, "vendor_col"), *want);
        assert_eq!(
            cell(&board.segments[i].unknown_columns, "vendor_col"),
            *want
        );
        assert_eq!(cell(&board.vias[i].unknown_columns, "vendor_col"), *want);
    }
}

#[test]
fn unknown_columns_survive_editing_an_unrelated_row() {
    let text = inject(&sheet_text(), "sheets.wires", "vendor_col", false);
    let mut sheet = SnxSchematic::parse(&text).unwrap().sheet;
    sheet.wires[1].end.x = 9.0;
    sheet.wires.remove(0);
    let again = SnxSchematic::parse(&SnxSchematic::new(sheet).write_string().unwrap())
        .unwrap()
        .sheet;
    assert_eq!(again.wires.len(), 1);
    assert_eq!(
        cell(&again.wires[0].unknown_columns, "vendor_col"),
        NASTY[1]
    );
}

/// A row that lacks a column another row carries writes an empty cell.
#[test]
fn rows_missing_a_foreign_column_write_an_empty_cell() {
    let text = inject(&sheet_text(), "sheets.wires", "vendor_col", false);
    let mut sheet = SnxSchematic::parse(&text).unwrap().sheet;
    let mut extra = sheet.wires[0].clone();
    extra.uuid = uuid_n(99);
    extra.unknown_columns = UnknownColumns::default();
    sheet.wires.push(extra);
    let again = SnxSchematic::parse(&SnxSchematic::new(sheet).write_string().unwrap())
        .unwrap()
        .sheet;
    assert_eq!(again.wires.len(), 3);
    assert_eq!(cell(&again.wires[2].unknown_columns, "vendor_col"), "");
    assert_eq!(
        cell(&again.wires[0].unknown_columns, "vendor_col"),
        NASTY[0]
    );
}

const JUNCTION_UUID: &str = "0192a8c0-00aa-7000-8000-000000000001";

#[test]
fn reordered_known_header_parses_by_name() {
    let text = format!("diameter  uuid  pos_y  pos_x\n0  {JUNCTION_UUID}  7  5\n");
    let rows = parse_tsv_block::<SchJunctionRow>("t", &text).unwrap();
    assert_eq!((rows[0].pos_x, rows[0].pos_y), (5, 7));
    assert!(rows[0].unknown_columns.is_empty());
}

#[test]
fn missing_known_column_still_errors() {
    let text = format!("uuid  pos_x  vendor_col\n{JUNCTION_UUID}  5  x\n");
    let err = parse_tsv_block::<SchJunctionRow>("t", &text).unwrap_err();
    assert!(
        matches!(err, FormatError::TsvHeaderMismatch { .. }),
        "{err}"
    );
}

#[test]
fn row_width_is_checked_against_the_actual_header() {
    let text = format!("uuid  pos_x  pos_y  diameter  vendor_col\n{JUNCTION_UUID}  1  2  0\n");
    let err = parse_tsv_block::<SchJunctionRow>("t", &text).unwrap_err();
    assert!(
        matches!(
            err,
            FormatError::TsvCellCountMismatch {
                expected: 5,
                got: 4,
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn repeated_header_column_errors() {
    let text = format!("uuid  pos_x  pos_y  diameter  v  v\n{JUNCTION_UUID}  1  2  0  a  b\n");
    let err = parse_tsv_block::<SchJunctionRow>("t", &text).unwrap_err();
    assert!(
        matches!(err, FormatError::TsvDuplicateColumn { .. }),
        "{err}"
    );
}

#[test]
fn foreign_column_name_needing_quotes_round_trips() {
    let text = format!("uuid  pos_x  pos_y  diameter  \"my col\"\n{JUNCTION_UUID}  1  2  0  v\n");
    let rows = parse_tsv_block::<SchJunctionRow>("t", &text).unwrap();
    assert_eq!(rows[0].unknown_columns.get("my col"), Some("v"));
    let out = write_tsv_block(&rows);
    assert!(
        squash(&out).starts_with("uuid pos_x pos_y diameter \"my col\"\n"),
        "{out}"
    );
}

#[test]
fn files_without_foreign_columns_write_unchanged_json_and_text() {
    let sheet = two_row_sheet();
    let json = serde_json::to_string(&sheet.wires[0]).unwrap();
    assert!(!json.contains("unknown_columns"));
    let first = SnxSchematic::new(sheet).write_string().unwrap();
    let loaded = SnxSchematic::parse(&first).unwrap().sheet;
    assert!(loaded.wires.iter().all(|w| w.unknown_columns.is_empty()));
    let saved = SnxSchematic::new(loaded).write_string().unwrap();
    let again = SnxSchematic::parse(&saved).unwrap().sheet;
    assert_eq!(SnxSchematic::new(again).write_string().unwrap(), saved);
    assert!(squash(&saved).contains("uuid pos_x pos_y diameter\n"));
}
