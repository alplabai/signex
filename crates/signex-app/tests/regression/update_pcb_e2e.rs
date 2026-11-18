//! #2473 — schematic to board, end to end: a two-sheet project whose nets
//! cross the sheet boundary goes through File ▸ New ▸ PCB and Design ▸
//! Update PCB Document. Every part with a footprint is placed, every net
//! of two or more pads shows as ratsnest, the part with no footprint is
//! reported, and cross probe resolves both ways, into the child sheet too.
//! Fixture and helpers: `update_pcb.rs`.

use std::collections::{BTreeMap, BTreeSet};

use pcb_geom::{ClickMode, ItemRef};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_types::schematic::{ChildSheet, FillType, Label, LabelType, SelectedItem, SelectedKind};

use super::*;

/// The child sheet's placement on the root sheet.
const AMP: Uuid = Uuid::from_u128(0x2473_0a00);
/// Parts on the child sheet.
const R10: Uuid = Uuid::from_u128(0x2473_0010);
const R11: Uuid = Uuid::from_u128(0x2473_0011);

/// The net both sheets name with a global label.
const SIG: &str = "SIG";

fn amp_placement() -> ChildSheet {
    ChildSheet {
        uuid: AMP,
        name: "Amp".to_string(),
        filename: "amp.snxsch".to_string(),
        position: Point::new(0.0, 50.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

/// The child sheet: R10 and R11 joined pin 2 to pin 1, and R10 pin 1 on
/// the global net SIG.
fn amp_sheet(lib: &Library) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::from_u128(0x2473_0b00),
        paper_size: "A4".to_string(),
        symbols: vec![
            part(lib, R10, "R10", 0.0, true),
            part(lib, R11, "R11", 20.0, true),
        ],
        wires: vec![Wire {
            uuid: Uuid::from_u128(0x2473_0b10),
            start: Point::new(10.0, 0.0),
            end: Point::new(20.0, 0.0),
            stroke_width: 0.0,
            color: None,
            locked: false,
            unknown_columns: Default::default(),
        }],
        labels: vec![Label::new(SIG, Point::new(0.0, 0.0), LabelType::Global)],
        ..Default::default()
    };
    sheet.lib_symbols.insert("R".to_string(), resistor_body());
    sheet
}

fn sheet_entry(filename: &str) -> SheetEntry {
    SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

/// The root sheet (R1, R2, R3 with no footprint, R2 pin 2 on SIG and the
/// Amp placement) open in front, amp.snxsch closed on disk, both in the
/// project, which has no board yet.
fn two_sheet_fixture() -> Fixture {
    let mut f = fixture_editing(
        |lib, sheet| {
            sheet.symbols.push(part(lib, R3, "R3", 60.0, false));
            sheet
                .labels
                .push(Label::new(SIG, Point::new(30.0, 0.0), LabelType::Global));
            sheet.child_sheets.push(amp_placement());
        },
        None,
        ProjectDefaults::default(),
    );
    let amp = f.sheet.with_file_name("amp.snxsch");
    let text = SnxSchematic::new(amp_sheet(&f._lib))
        .write_string()
        .unwrap();
    std::fs::write(&amp, text).unwrap();
    let project = &mut f.app.document_state.projects[0].data;
    project.sheets.push(sheet_entry("amp.snxsch"));
    f
}

fn footprint_of<'a>(f: &'a Fixture, reference: &str) -> &'a signex_types::pcb::Footprint {
    engine(f)
        .board()
        .footprints
        .iter()
        .find(|fp| fp.reference == reference)
        .unwrap_or_else(|| panic!("{reference} is on the board"))
}

/// The net number of `reference`'s pad `number`.
fn pad_net(f: &Fixture, reference: &str, number: &str) -> Option<u32> {
    footprint_of(f, reference)
        .pads
        .iter()
        .find(|p| p.number == number)
        .and_then(|p| p.net.as_ref())
        .map(|n| n.number)
}

/// Every net number on the board with its pad count.
fn pads_per_net(f: &Fixture) -> BTreeMap<u32, usize> {
    let mut nets = BTreeMap::new();
    for fp in &engine(f).board().footprints {
        for pad in &fp.pads {
            if let Some(net) = &pad.net {
                *nets.entry(net.number).or_insert(0) += 1;
            }
        }
    }
    nets
}

fn ratsnest_nets(f: &Fixture) -> BTreeSet<u32> {
    f.app
        .interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .expect("the board is in view")
        .ratsnest_lines
        .iter()
        .map(|line| line.net)
        .collect()
}

fn active_path(f: &Fixture) -> PathBuf {
    let docs = &f.app.document_state;
    docs.tabs[docs.active_tab].path.clone()
}

fn board_selection(f: &Fixture) -> BTreeSet<Uuid> {
    f.app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .filter_map(|item| match item {
            ItemRef::Footprint(uuid) => Some(uuid),
            _ => None,
        })
        .collect()
}

/// A click on `reference`'s footprint between its pads (a netted pad
/// stands for its net, not its part).
fn click_footprint(f: &mut Fixture, reference: &str) {
    let at = footprint_of(f, reference).position;
    send(
        &mut f.app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at,
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
}

/// The whole flow on a two-sheet design: placement, nets across the sheet
/// boundary, ratsnest, the part with no footprint, cross probe both ways.
#[test]
fn a_two_sheet_schematic_becomes_a_board_with_every_net_as_ratsnest() {
    let mut f = two_sheet_fixture();
    new_board_and_list(&mut f);

    // The part with no footprint is reported with its reason, not dropped.
    let r3 = add_row(&f, R3);
    assert!(
        matches!(r3.blocked, Some(EcoBlock::MissingFootprint(_))),
        "R3 is listed as blocked: {:?}",
        r3.blocked
    );
    let report = f.sheet.with_file_name("report.txt");
    up(
        &mut f.app,
        UpdatePcbMsg::ReportPathPicked(Some(report.clone())),
    );
    let text = std::fs::read_to_string(&report).expect("the report");
    assert!(text.contains("[-] Add R3"), "{text}");
    let r10 = list(&f)
        .row(&EcoKey::AddFootprint {
            sheet: InstancePath::root().child(AMP),
            part: R10,
        })
        .expect("R10 on the child sheet is listed under its placement");
    assert!(r10.on && r10.blocked.is_none());

    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert!(f.app.ui_state.update_pcb.is_none(), "Execute ran");
    assert_eq!(active_path(&f), f.board, "the board comes forward");

    // Every part with a footprint is placed, from both sheets; R3 is not.
    assert_eq!(references(&f), ["R1", "R10", "R11", "R2"]);
    let r10_link = footprint_of(&f, "R10")
        .schematic_link
        .clone()
        .expect("R10 is linked");
    assert_eq!(
        r10_link,
        SymbolLink::new(InstancePath::root().child(AMP), [R10])
    );

    // The nets: one on each sheet, one across the boundary by SIG.
    let within_root = pad_net(&f, "R1", "2").expect("R1.2 has a net");
    assert_eq!(pad_net(&f, "R2", "1"), Some(within_root));
    let across = pad_net(&f, "R2", "2").expect("R2.2 has a net");
    assert_eq!(
        pad_net(&f, "R10", "1"),
        Some(across),
        "SIG joins the sheets"
    );
    let within_amp = pad_net(&f, "R10", "2").expect("R10.2 has a net");
    assert_eq!(pad_net(&f, "R11", "1"), Some(within_amp));

    // Every net of two or more pads is in the ratsnest.
    let wanted: BTreeSet<u32> = pads_per_net(&f)
        .into_iter()
        .filter(|(_, pads)| *pads >= 2)
        .map(|(net, _)| net)
        .collect();
    assert_eq!(
        wanted,
        BTreeSet::from([within_root, across, within_amp]),
        "three nets have two pads or more"
    );
    let drawn = ratsnest_nets(&f);
    assert!(wanted.is_subset(&drawn), "{wanted:?} in {drawn:?}");

    // Board to sheet: R10's footprint probes to R10 on the closed child
    // sheet, which opens in front.
    click_footprint(&mut f, "R10");
    let fp_r10 = footprint_of(&f, "R10").uuid;
    assert_eq!(board_selection(&f), BTreeSet::from([fp_r10]));
    send(&mut f.app, menu_row("cross_probe"));
    let amp = f.sheet.with_file_name("amp.snxsch");
    assert_eq!(active_path(&f), amp, "the child sheet opens in front");
    assert_eq!(
        f.app.interaction_state.canvas.selected,
        vec![SelectedItem::new(R10, SelectedKind::Symbol)]
    );

    // Sheet to board: both parts of the child sheet probe to their
    // footprints.
    send(&mut f.app, menu_row("select_all"));
    send(&mut f.app, menu_row("cross_probe"));
    assert_eq!(active_path(&f), f.board, "the board comes forward");
    let fp_r11 = footprint_of(&f, "R11").uuid;
    assert_eq!(board_selection(&f), BTreeSet::from([fp_r10, fp_r11]));

    // And a root-sheet part probes to its own footprint.
    let root = f.sheet.clone();
    show(&mut f, &root);
    send(&mut f.app, menu_row("select_all"));
    send(&mut f.app, menu_row("cross_probe"));
    assert_eq!(active_path(&f), f.board);
    let root_parts: BTreeSet<Uuid> = ["R1", "R2"]
        .iter()
        .map(|r| footprint_of(&f, r).uuid)
        .collect();
    assert_eq!(board_selection(&f), root_parts);
}
