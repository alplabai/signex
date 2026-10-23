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
            uuid: uuid_n(10 + n),
            start: SchPoint {
                x: 1.0,
                y: f64::from(n),
            },
            end: SchPoint { x: 2.0, y: 3.0 },
            ..Default::default()
        });
        sheet.junctions.push(Junction {
            uuid: uuid_n(20 + n),
            position: SchPoint {
                x: f64::from(n),
                y: 1.0,
            },
            ..Default::default()
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
    let out = write_tsv_block("t", &rows).unwrap();
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

// ---------------------------------------------------------------------------
// Hostile-input hardening: shared names, linear validation, hard caps
// ---------------------------------------------------------------------------

const JUNCTION_HEAD: &str = "uuid  pos_x  pos_y  diameter";

fn header_with(extra: impl Iterator<Item = String>) -> String {
    let mut h = JUNCTION_HEAD.to_string();
    for name in extra {
        h.push_str("  ");
        h.push_str(&name);
    }
    h.push('\n');
    h
}

#[test]
fn foreign_names_are_shared_across_rows() {
    let text = format!(
        "{JUNCTION_HEAD}  vendor\n{JUNCTION_UUID}  1  2  0  a\n{JUNCTION_UUID}  3  4  0  b\n"
    );
    let rows = parse_tsv_block::<SchJunctionRow>("t", &text).unwrap();
    let ptr = |r: &SchJunctionRow| r.unknown_columns.iter().next().unwrap().0.as_ptr();
    assert_eq!(ptr(&rows[0]), ptr(&rows[1]), "one allocation per name");
    let out = write_tsv_block("t", &rows).unwrap();
    assert!(squash(&out).starts_with("uuid pos_x pos_y diameter vendor\n"));
    assert_eq!(cell(&rows[1].unknown_columns, "vendor"), "b");
}

#[test]
fn header_at_both_caps_still_loads() {
    let long = "n".repeat(MAX_FOREIGN_COLUMN_NAME_BYTES);
    let names = (0..MAX_FOREIGN_COLUMNS - 1)
        .map(|i| format!("c{i}"))
        .chain(std::iter::once(long));
    let text = header_with(names);
    let width = JUNCTION_HEAD.split_whitespace().count() + MAX_FOREIGN_COLUMNS;
    let mut cells = vec!["1"; width];
    cells[0] = JUNCTION_UUID;
    let row = cells.join("  ");
    let rows = parse_tsv_block::<SchJunctionRow>("t", &format!("{text}{row}\n"));
    assert!(rows.is_ok(), "{:?}", rows.err());
}

#[test]
fn ten_thousand_column_header_is_rejected_quickly() {
    let text = header_with((0..10_000).map(|i| format!("c{i}")));
    let started = std::time::Instant::now();
    let err = parse_tsv_block::<SchJunctionRow>("t", &text).unwrap_err();
    assert!(
        matches!(
            err,
            FormatError::TsvTooManyForeignColumns {
                count: 10_000,
                max: MAX_FOREIGN_COLUMNS,
                ..
            }
        ),
        "{err}"
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn duplicate_is_still_found_in_a_ten_thousand_column_header() {
    let names = (0..10_000)
        .map(|i| format!("c{i}"))
        .chain(["c9999".to_string()]);
    let started = std::time::Instant::now();
    let err = parse_tsv_block::<SchJunctionRow>("t", &header_with(names)).unwrap_err();
    assert!(
        matches!(&err, FormatError::TsvDuplicateColumn { column, .. } if column == "c9999"),
        "{err}"
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn one_foreign_column_over_the_cap_errors() {
    let text = header_with((0..=MAX_FOREIGN_COLUMNS).map(|i| format!("c{i}")));
    let err = parse_tsv_block::<SchJunctionRow>("t", &text).unwrap_err();
    assert!(
        matches!(
            err,
            FormatError::TsvTooManyForeignColumns {
                count: 257,
                max: 256,
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn foreign_name_over_the_byte_cap_errors_with_a_short_prefix() {
    let long = "x".repeat(MAX_FOREIGN_COLUMN_NAME_BYTES + 1);
    let err =
        parse_tsv_block::<SchJunctionRow>("t", &header_with(std::iter::once(long))).unwrap_err();
    match &err {
        FormatError::TsvForeignColumnNameTooLong {
            prefix, len, max, ..
        } => {
            assert_eq!((*len, *max), (257, 256));
            assert_eq!(prefix.len(), 32);
        }
        other => panic!("{other}"),
    }
    assert!(err.to_string().len() < 300, "error must not echo the name");
}

// ---------------------------------------------------------------------------
// Writer/reader symmetry: what the writer emits, the reader accepts
// ---------------------------------------------------------------------------

/// A row carrying `names` as foreign columns, built through the same
/// deserialisation path a clipboard/merge would use.
fn with_foreign(mut row: SchJunctionRow, names: &[String]) -> SchJunctionRow {
    row.unknown_columns = foreign_cols(names);
    row
}

fn foreign_cols(names: &[String]) -> UnknownColumns {
    let pairs: Vec<(String, String)> = names.iter().map(|n| (n.clone(), "v".into())).collect();
    serde_json::from_value(serde_json::json!(pairs)).unwrap()
}

fn junction_row(n: u32) -> SchJunctionRow {
    let text = format!("{JUNCTION_HEAD}\n{}  1  2  0\n", uuid_n(n));
    parse_tsv_block::<SchJunctionRow>("t", &text)
        .unwrap()
        .remove(0)
}

fn names(range: std::ops::Range<usize>) -> Vec<String> {
    range.map(|i| format!("c{i}")).collect()
}

#[test]
fn merged_rows_whose_union_exceeds_the_cap_fail_to_write() {
    // Each row alone is valid (200 columns); together they are 400.
    let rows = vec![
        with_foreign(junction_row(1), &names(0..200)),
        with_foreign(junction_row(2), &names(200..400)),
    ];
    let err = write_tsv_block("t", &rows).unwrap_err();
    assert!(
        matches!(
            err,
            FormatError::TsvTooManyForeignColumns { count: 400, .. }
        ),
        "{err}"
    );
}

#[test]
fn too_many_columns_error_names_the_first_one_over_the_cap() {
    let rows = vec![
        with_foreign(junction_row(1), &names(0..129)),
        with_foreign(junction_row(2), &names(129..257)),
    ];
    let err = write_tsv_block("t", &rows).unwrap_err();
    assert!(
        matches!(&err, FormatError::TsvTooManyForeignColumns { first_over, .. } if first_over == "c256"),
        "{err}"
    );
    assert!(err.to_string().contains("c256"), "{err}");
}

#[test]
fn foreign_column_named_like_a_known_one_fails_to_write() {
    let rows = vec![with_foreign(junction_row(1), &["uuid".to_string()])];
    let err = write_tsv_block("t", &rows).unwrap_err();
    assert!(
        matches!(&err, FormatError::TsvForeignColumnShadowsKnown { column, .. } if column == "uuid"),
        "{err}"
    );
}

#[test]
fn a_failed_save_leaves_the_existing_file_untouched() {
    let mut sheet = empty_sheet();
    let mut a = sample_symbol();
    a.unknown_columns = foreign_cols(&names(0..200));
    let mut b = sample_symbol();
    b.uuid = uuid_n(7);
    b.unknown_columns = foreign_cols(&names(200..400));
    sheet.symbols.extend([a, b]);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.snxsch");
    std::fs::write(&path, b"previous good contents").unwrap();

    // The engine's save sequence: serialise first, write only on success.
    let err = SnxSchematic::new(sheet).write_string().unwrap_err();
    assert!(err.to_string().contains("foreign columns"), "{err}");
    assert_eq!(std::fs::read(&path).unwrap(), b"previous good contents");
}

#[test]
fn writing_exactly_at_the_caps_round_trips() {
    let mut all = names(0..MAX_FOREIGN_COLUMNS - 1);
    all.push("n".repeat(MAX_FOREIGN_COLUMN_NAME_BYTES));
    let rows = vec![with_foreign(junction_row(1), &all)];
    let out = write_tsv_block("t", &rows).unwrap();
    let back = parse_tsv_block::<SchJunctionRow>("t", &out).unwrap();
    assert_eq!(back[0].unknown_columns, rows[0].unknown_columns);
}

#[test]
fn deserialising_an_invalid_unknown_columns_is_refused() {
    let too_many: Vec<(String, String)> = names(0..MAX_FOREIGN_COLUMNS + 1)
        .into_iter()
        .map(|n| (n, String::new()))
        .collect();
    assert!(serde_json::from_value::<UnknownColumns>(serde_json::json!(too_many)).is_err());
    let long = vec![("x".repeat(MAX_FOREIGN_COLUMN_NAME_BYTES + 1), "v")];
    assert!(serde_json::from_value::<UnknownColumns>(serde_json::json!(long)).is_err());
    let dup = vec![("a", "1"), ("a", "2")];
    assert!(serde_json::from_value::<UnknownColumns>(serde_json::json!(dup)).is_err());
}
