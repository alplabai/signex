//! Forward-compatibility guards: keys a newer Signex wrote, which this
//! build does not recognise, must survive a load -> save cycle on the
//! in-memory model (not just on the discarded codec wrapper).

use super::tests::{empty_board, empty_sheet, sample_symbol};
use super::*;
use crate::pcb::{Footprint, Pad, PadNet, PadShape, PadType, PcbBoard, Point as PcbPoint};
use crate::schematic::SchematicSheet;

const SYMBOL_UUID: &str = "0192a8c0-0001-7000-8000-000000000001";

/// A sheet holding one all-default symbol, so its `[extras]` tree is
/// empty until a test injects foreign keys.
fn plain_sheet() -> SchematicSheet {
    let mut sheet = empty_sheet();
    let mut symbol = sample_symbol();
    symbol.footprint.clear();
    sheet.symbols.push(symbol);
    sheet
}

/// Parse `text`, panicking with the codec error on failure.
fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

/// Save a bare sheet.
fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

/// A plain file with `vendor_annotation` prepended to the manifest.
fn with_manifest_key() -> String {
    format!(
        "vendor_annotation = \"important-data\"\n{}",
        save(&plain_sheet())
    )
}

#[test]
fn unknown_manifest_key_survives() {
    let sheet = load(&with_manifest_key());
    assert_eq!(sheet.unknown.manifest.len(), 1);
    assert!(save(&sheet).contains("vendor_annotation = \"important-data\""));
}

#[test]
fn unknown_manifest_table_survives() {
    let text = format!(
        "{}\n[vendor]\nk = \"v\"\n[vendor.deep]\nn = 1\n",
        save(&plain_sheet())
    );
    let sheet = load(&text);
    assert!(sheet.unknown.manifest.contains_key("vendor"));
    let again = load(&save(&sheet));
    assert_eq!(again.unknown, sheet.unknown);
}

#[test]
fn unknown_symbol_extras_key_survives_and_is_not_elided() {
    // The symbol is otherwise all-default, so without the is_default()
    // guard its extras entry would be dropped on save.
    let text = format!(
        "{}\n[extras.symbols.{SYMBOL_UUID}]\nfuture_symbol_key = 42\n\n[extras.symbols.{SYMBOL_UUID}.vendor]\nnote = \"n\"\n",
        save(&plain_sheet())
    );
    let sheet = load(&text);
    assert_eq!(sheet.symbols[0].unknown.len(), 2);
    let out = save(&sheet);
    assert!(out.contains("future_symbol_key = 42"));
    assert!(out.contains("note = \"n\""));
    assert_eq!(load(&out).symbols[0].unknown, sheet.symbols[0].unknown);
}

#[test]
fn unknown_sheet_extras_key_survives_and_stays_apart_from_manifest() {
    let text = format!(
        "same_name = \"manifest\"\n{}\n[extras.sheet]\nsame_name = \"sheet\"\n[extras.sheet.vendor]\nz = 1\n",
        save(&plain_sheet())
    );
    let sheet = load(&text);
    assert_eq!(sheet.unknown.manifest.len(), 1);
    assert_eq!(sheet.unknown.extras.len(), 2);
    let again = load(&save(&sheet));
    assert_eq!(again.unknown, sheet.unknown);
}

#[test]
fn load_save_without_edit_is_byte_stable() {
    let text = format!(
        "{}\n[extras.sheet]\nfuture = 1\n[extras.symbols.{SYMBOL_UUID}]\nfuture_symbol_key = 2\n",
        with_manifest_key()
    );
    let first = save(&load(&text));
    let second = save(&load(&first));
    assert_eq!(first, second);
    assert!(first.contains("vendor_annotation"));
    assert!(first.contains("future = 1"));
    assert!(first.contains("future_symbol_key = 2"));
}

#[test]
fn unknowns_survive_editing_an_unrelated_symbol() {
    let text = format!(
        "{}
[extras.sheet]
future_sheet = 1

[extras.symbols.{SYMBOL_UUID}]
future_symbol_key = 42
",
        with_manifest_key()
    );
    let mut sheet = load(&text);
    let mut other = sample_symbol();
    other.uuid = Uuid::parse_str("0192a8c0-0001-7000-8000-000000000002").unwrap();
    other.reference = "R9".into();
    sheet.symbols.push(other);
    sheet.symbols[1].reference = "R10".into();
    let again = load(&save(&sheet));
    assert_eq!(again.symbols.len(), 2);
    assert_eq!(again.unknown, sheet.unknown);
    assert!(!again.unknown.extras.is_empty());
    assert!(!again.unknown.manifest.is_empty());
    assert_eq!(again.symbols[0].unknown, sheet.symbols[0].unknown);
    assert!(!again.symbols[0].unknown.is_empty());
    assert!(again.symbols[1].unknown.is_empty());
}

#[test]
fn file_without_unknowns_has_empty_bags_and_unchanged_json() {
    let sheet = load(&save(&plain_sheet()));
    assert!(sheet.unknown.is_empty());
    assert!(sheet.symbols[0].unknown.is_empty());
    let json = serde_json::to_string(&sheet).unwrap();
    assert!(!json.contains("\"unknown\""));
}

// ---------------------------------------------------------------------------
// .snxpcb
// ---------------------------------------------------------------------------

const FP_UUID: &str = "0192a8c0-0010-7000-8000-000000000001";
const PAD_UUID: &str = "0192a8c0-0011-7000-8000-000000000001";

/// A board with one all-default footprint and pad, and no stackup, nets
/// or board extras, so every section can be injected from scratch.
pub(super) fn plain_board() -> PcbBoard {
    let pad = Pad {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::parse_str(PAD_UUID).unwrap(),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::RoundRect,
        position: PcbPoint { x: 50.5, y: 25.0 },
        size: PcbPoint { x: 1.0, y: 0.6 },
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: 1,
            name: "VCC".into(),
        }),
        roundrect_ratio: 0.25,
    };
    let footprint = Footprint {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::parse_str(FP_UUID).unwrap(),
        reference: "U1".into(),
        value: "STM32F407".into(),
        footprint_id: String::new(),
        position: PcbPoint { x: 50.0, y: 25.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
    };
    let mut board = empty_board();
    board.footprints.push(footprint);
    board
}

fn load_pcb(text: &str) -> PcbBoard {
    SnxPcb::parse(text).unwrap().board
}

fn save_pcb(board: &PcbBoard) -> String {
    SnxPcb::new(board.clone()).write_string().unwrap()
}

/// A plain board file with a foreign key at all six levels, two of them
/// (`[stackup]`, `[nets]`) in sections that do not otherwise exist.
fn pcb_with_unknowns() -> String {
    format!(
        "vendor_annotation = \"important-data\"
{}
         [stackup]
stack_future = 1

         [nets]
nets_future = 2

         [extras.board]
board_future = 3
[extras.board.vendor]
z = 1

         [extras.footprints.{FP_UUID}]
fp_future = 4

         [extras.pads.{PAD_UUID}]
pad_future = 5
",
        save_pcb(&plain_board())
    )
}

#[test]
fn pcb_unknowns_survive_at_all_six_levels() {
    let board = load_pcb(&pcb_with_unknowns());
    assert_eq!(board.unknown.manifest.len(), 1);
    assert_eq!(board.unknown.stackup.len(), 1);
    assert_eq!(board.unknown.nets.len(), 1);
    assert_eq!(board.unknown.extras.len(), 2);
    assert_eq!(board.footprints[0].unknown.len(), 1);
    assert_eq!(board.footprints[0].pads[0].unknown.len(), 1);

    let out = save_pcb(&board);
    for needle in [
        "vendor_annotation = \"important-data\"",
        "stack_future = 1",
        "nets_future = 2",
        "board_future = 3",
        "fp_future = 4",
        "pad_future = 5",
    ] {
        assert!(out.contains(needle), "lost {needle}");
    }
    let again = load_pcb(&out);
    assert_eq!(again.unknown, board.unknown);
    assert_eq!(again.footprints[0].unknown, board.footprints[0].unknown);
    assert_eq!(
        again.footprints[0].pads[0].unknown,
        board.footprints[0].pads[0].unknown
    );
}

#[test]
fn pcb_load_save_without_edit_is_byte_stable() {
    let first = save_pcb(&load_pcb(&pcb_with_unknowns()));
    let second = save_pcb(&load_pcb(&first));
    assert_eq!(first, second);
}

#[test]
fn pcb_same_key_in_different_sections_stays_apart() {
    let text = format!(
        "dup = \"manifest\"
{}
[stackup]
dup = \"stackup\"
[extras.board]
dup = \"board\"
",
        save_pcb(&plain_board())
    );
    let board = load_pcb(&text);
    let again = load_pcb(&save_pcb(&board));
    assert_eq!(again.unknown, board.unknown);
    assert_eq!(again.unknown.manifest.len(), 1);
    assert_eq!(again.unknown.stackup.len(), 1);
    assert_eq!(again.unknown.extras.len(), 1);
}

#[test]
fn pcb_unknown_nested_table_survives() {
    let text = format!(
        "{}
[extras.footprints.{FP_UUID}.vendor]
k = \"v\"
[extras.footprints.{FP_UUID}.vendor.deep]
n = 1
",
        save_pcb(&plain_board())
    );
    let board = load_pcb(&text);
    assert!(board.footprints[0].unknown.contains_key("vendor"));
    let again = load_pcb(&save_pcb(&board));
    assert_eq!(again.footprints[0].unknown, board.footprints[0].unknown);
}

#[test]
fn pcb_unknowns_survive_editing_an_unrelated_footprint() {
    let mut board = load_pcb(&pcb_with_unknowns());
    let mut other = board.footprints[0].clone();
    other.uuid = Uuid::parse_str("0192a8c0-0010-7000-8000-000000000002").unwrap();
    other.reference = "R9".into();
    other.unknown = Default::default();
    other.pads.clear();
    board.footprints.push(other);
    board.footprints[1].value = "10k".into();
    let again = load_pcb(&save_pcb(&board));
    assert_eq!(again.unknown, board.unknown);
    assert_eq!(again.footprints[0].unknown, board.footprints[0].unknown);
    assert!(again.footprints[1].unknown.is_empty());
}

#[test]
fn orphan_pad_footprint_gets_an_empty_bag() {
    let mut board = plain_board();
    board.footprints[0].reference = "U1".into();
    let fp_uuid = board.footprints[0].uuid.to_string();
    let text = save_pcb(&board);
    // Re-key the footprint in the footprints block only, so the pad
    // row's footprint_ref (the footprint uuid) matches nothing.
    let text = text.replacen(&fp_uuid, "0192a8c0-ffff-7000-8000-00000000ffff", 1)
        + &format!(
            "
[extras.pads.{PAD_UUID}]
pad_future = 5
"
        );
    let board = load_pcb(&text);
    let orphan = board
        .footprints
        .iter()
        .find(|f| {
            f.pads.iter().any(|p| p.unknown.len() == 1)
                && f.value.is_empty()
                && f.footprint_id.is_empty()
        })
        .expect("synthetic orphan footprint");
    assert!(orphan.unknown.is_empty());
    assert_eq!(orphan.pads[0].unknown.len(), 1);
}

#[test]
fn board_without_unknowns_has_empty_bags_and_unchanged_json() {
    let board = load_pcb(&save_pcb(&plain_board()));
    assert!(board.unknown.is_empty());
    assert!(board.footprints[0].unknown.is_empty());
    let json = serde_json::to_string(&board).unwrap();
    assert!(!json.contains("\"unknown\""));
}

/// #883: zone keys this build does not know survive a load and save
/// instead of being dropped (for `holes` that would have turned a cut-out
/// back into solid copper). A later build may add a nested table as well as
/// scalars, so both are covered.
#[test]
fn unknown_zone_keys_survive_a_round_trip() {
    let mut board = plain_board();
    board.zones.push(crate::pcb::Zone {
        uuid: uuid::Uuid::parse_str("0192a8c0-0040-7000-8000-000000000003").unwrap(),
        net: 0,
        net_name: String::new(),
        layer: "TopCopper".into(),
        outline: vec![
            PcbPoint { x: 0.0, y: 0.0 },
            PcbPoint { x: 1.0, y: 0.0 },
            PcbPoint { x: 1.0, y: 1.0 },
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: String::new(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        unknown: Default::default(),
    });
    let text = save_pcb(&board);
    let marker = "min_thickness = 0.0\n";
    assert!(text.contains(marker), "{text}");
    let future = text.replacen(
        marker,
        &format!(
            "{marker}hatch_pitch = 0.5\nfuture_mode = \"stitched\"\n\n\
             [zones.future_sub]\nkind = \"hatch\"\nangle = 45\n"
        ),
        1,
    );

    let loaded = load_pcb(&future);
    let zone = &loaded.zones[0];
    let keys: Vec<&str> = zone.unknown.keys().map(String::as_str).collect();
    assert_eq!(keys, ["future_mode", "future_sub", "hatch_pitch"]);
    assert!(
        !zone.unknown.contains_key("min_thickness"),
        "a known key must not be captured as unknown"
    );

    let resaved = save_pcb(&loaded);
    let again = load_pcb(&resaved);
    assert_eq!(again.zones[0].unknown, zone.unknown);
    assert_eq!(resaved, save_pcb(&again), "re-save is stable");
}
