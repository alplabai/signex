//! #2366: swapping the footprint of a component placed on a board, driven
//! through `Signex::update` with the messages the UI sends. The Component
//! view's footprint button and the board menu's Change Footprint... open the
//! footprint picker, the component's own library footprints first; the pick
//! replaces the footprint as one undo step, keeping its place and designator
//! and its pad nets by pad number; an old pad with no match is listed in
//! Messages; the Library row names the library; a locked component and a
//! footprint the board cannot hold are refused with the reason.
//!
//! The real `Stock.snxlib`: STOCK-R offers a default footprint and six
//! alternates, all two-pad.

use std::path::PathBuf;

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{
    ContextAction, ContextMenuMsg, EditMsg, FileMsg, FootprintSwapMsg, Message, PcbMsg, Signex,
    TabDocument,
};
use signex_app::diagnostics;
use signex_app::dock::DockMessage;
use signex_app::keymap::AppCommandId;
use signex_app::library::LibraryMessage;
use signex_app::library::messages::PrimitivePickerMsg;
use signex_app::library::state::PrimitivePickerTarget;
use signex_app::panels::{FootprintLibrary, PanelMsg, PcbComponentFacts, PcbComponentMsg, Shared};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_library::ComponentRow;
use signex_library::primitive::PrimitiveRef;
use signex_types::format::SnxPcb;
use signex_types::pcb::{Footprint, PcbBoard, Point};
use uuid::Uuid;

use crate::settle_support::settle;

const R1: Uuid = Uuid::from_u128(0x10);

fn stock_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

fn pad(n: u128, number: &str, x: f64, y: f64, net: (u32, &str)) -> serde_json::Value {
    serde_json::json!({
        "uuid": Uuid::from_u128(n), "number": number, "pad_type": "smd", "shape": "rect",
        "position": {"x": x, "y": y}, "size": {"x": 0.8, "y": 0.8}, "drill": null,
        "layers": ["TopCopper"], "net": {"number": net.0, "name": net.1}
    })
}

/// R1 at (10, 10), turned 90°, with pads 1 (GND), 2 (VCC) and 3 (SIG) away
/// from its origin, built from `source` of the stock library.
fn board(source: &PrimitiveRef) -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "version": 1,
        "nets": [
            {"number": 1, "name": "GND"},
            {"number": 2, "name": "VCC"},
            {"number": 3, "name": "SIG"}
        ],
        "footprints": [{
            "uuid": R1, "reference": "R1", "value": "10k", "footprint_id": "Old-3-Pad",
            "position": {"x": 10.0, "y": 10.0}, "rotation": 90.0, "layer": "TopCopper",
            "library_ref": {
                "library": source.library_id, "footprint": source.uuid, "version": "1"
            },
            "pads": [
                pad(0x11, "1", -2.0, 0.0, (1, "GND")),
                pad(0x12, "2", 2.0, 0.0, (2, "VCC")),
                pad(0x13, "3", 0.0, 3.0, (3, "SIG"))
            ]
        }]
    }))
    .unwrap()
}

/// STOCK-R's library row.
fn stock_r(app: &Signex) -> ComponentRow {
    app.library
        .open_libraries
        .iter()
        .flat_map(|lib| lib.tables.values().flatten())
        .find(|row| row.internal_pn.as_str() == "STOCK-R")
        .expect("Stock lists STOCK-R")
        .clone()
}

/// The name of the stock footprint `reference`.
fn footprint_name(app: &Signex, reference: PrimitiveRef) -> String {
    app.library
        .open_libraries
        .iter()
        .find(|lib| lib.library_id == reference.library_id)
        .and_then(|lib| {
            lib.cached_footprints
                .iter()
                .find(|s| s.uuid == reference.uuid)
        })
        .expect("a stock footprint")
        .name
        .clone()
}

struct Fixture {
    app: Signex,
    path: PathBuf,
    row: ComponentRow,
    _tmp: tempfile::TempDir,
}

/// Stock mounted, then the board, with R1 built from STOCK-R's default
/// footprint, open in the active tab with R1 selected.
fn open() -> Fixture {
    let _ = diagnostics::init_logging();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let stock = stock_file();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock)
        .expect("mount stock");
    let row = stock_r(&app);
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("board.snxpcb");
    let source = row.footprint_ref.expect("STOCK-R has a footprint");
    std::fs::write(&path, SnxPcb::new(board(&source)).write_string().unwrap()).unwrap();
    settle(&mut app, Message::File(FileMsg::Opened(Some(path.clone()))));
    settle(
        &mut app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(10.0, 10.0),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
    let selected: Vec<ItemRef> = app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect();
    assert_eq!(selected, [ItemRef::Footprint(R1)]);
    Fixture {
        app,
        path,
        row,
        _tmp: tmp,
    }
}

fn live(app: &Signex) -> PcbBoard {
    let docs = &app.document_state;
    docs.tabs[docs.active_tab]
        .cached_document
        .as_ref()
        .and_then(TabDocument::as_pcb)
        .expect("a board is open")
        .clone()
}

fn snapshot(app: &Signex) -> String {
    serde_json::to_string(&live(app)).unwrap()
}

fn r1(app: &Signex) -> Footprint {
    live(app)
        .footprints
        .into_iter()
        .find(|f| f.uuid == R1)
        .expect("R1")
}

fn facts(app: &Signex) -> PcbComponentFacts {
    app.document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|b| b.component.clone())
        .expect("the Component view")
}

/// The footprint button of the Component view, pressed.
fn press_footprint_button(f: &mut Fixture) {
    settle(
        &mut f.app,
        Message::Dock(DockMessage::Panel(PanelMsg::PcbComponent(
            PcbComponentMsg::FootprintPickRequested {
                board: f.path.clone(),
                footprints: vec![R1],
            },
        ))),
    );
}

fn pick(app: &mut Signex, footprint: PrimitiveRef) {
    settle(
        app,
        Message::Library(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Pick(
            footprint,
        ))),
    );
}

/// A notice logged after `since` that contains `wanted`.
fn noticed(since: u64, wanted: &str) -> bool {
    diagnostics::recent_entries().iter().any(|e| {
        e.id > since
            && e.message
                .replace(['\u{2068}', '\u{2069}'], "")
                .contains(wanted)
    })
}

/// The net names of `fp`'s pads by pad number.
fn pad_nets(fp: &Footprint) -> Vec<(String, Option<String>)> {
    fp.pads
        .iter()
        .map(|p| (p.number.clone(), p.net.as_ref().map(|n| n.name.clone())))
        .collect()
}

#[test]
fn the_footprint_button_swaps_the_footprint_in_one_step_keeping_place_and_nets() {
    let mut f = open();
    let alternate = f.row.alternate_footprints[0].footprint;
    let name = footprint_name(&f.app, alternate);
    press_footprint_button(&mut f);
    let picker = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .expect("the picker opened");
    let PrimitivePickerTarget::BoardFootprint {
        footprints,
        suggested,
        ..
    } = &picker.target
    else {
        panic!("the picker is for the board footprint");
    };
    assert_eq!(footprints, &[R1]);
    assert_eq!(
        suggested.len(),
        1 + f.row.alternate_footprints.len(),
        "STOCK-R's default and alternates are listed first"
    );
    assert!(suggested.contains(&alternate));

    let before = snapshot(&f.app);
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    pick(&mut f.app, alternate);
    assert!(
        f.app.library.primitive_picker.is_none(),
        "the pick closes it"
    );
    let after = snapshot(&f.app);
    let swapped = r1(&f.app);
    assert_eq!(swapped.footprint_id, name);
    assert_eq!(swapped.position, Point::new(10.0, 10.0));
    assert_eq!(swapped.rotation, 90.0);
    assert_eq!(swapped.layer, "TopCopper");
    assert_eq!(swapped.reference, "R1");
    assert_eq!(
        swapped.library_ref.as_ref().map(|s| s.footprint),
        Some(alternate.uuid)
    );
    let mut nets = pad_nets(&swapped);
    nets.sort();
    assert_eq!(
        nets,
        [
            ("1".to_string(), Some("GND".to_string())),
            ("2".to_string(), Some("VCC".to_string()))
        ],
        "the pads keep their nets by number"
    );
    assert!(
        noticed(
            since,
            "R1: pad 3 has no match on the new footprint and was removed with its net SIG"
        ),
        "Messages lists the unmatched pad"
    );

    settle(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), before, "one undo restores the footprint");
    settle(&mut f.app, Message::Edit(EditMsg::Redo));
    assert_eq!(snapshot(&f.app), after, "redo replays the swap");
    settle(&mut f.app, Message::Edit(EditMsg::Undo));
    settle(&mut f.app, Message::Edit(EditMsg::Undo));
    assert_eq!(snapshot(&f.app), before, "the swap was a single step");
}

#[test]
fn the_library_row_names_the_library() {
    let f = open();
    let stock = f
        .app
        .library
        .open_libraries
        .iter()
        .find(|lib| Some(lib.library_id) == f.row.footprint_ref.map(|r| r.library_id))
        .expect("stock")
        .display_name
        .clone();
    assert_eq!(
        facts(&f.app).library,
        Shared::Same(Some(FootprintLibrary::Open(stock)))
    );
}

#[test]
fn the_board_menu_change_footprint_row_opens_the_picker_and_swaps() {
    let mut f = open();
    let alternate = f.row.alternate_footprints[1].footprint;
    let name = footprint_name(&f.app, alternate);
    settle(
        &mut f.app,
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
            id: AppCommandId::new("change_footprint").expect("a catalog id"),
            project: None,
        })),
    );
    let picker = f
        .app
        .library
        .primitive_picker
        .as_ref()
        .expect("the picker opened");
    assert!(matches!(
        &picker.target,
        PrimitivePickerTarget::BoardFootprint { footprints, .. } if footprints == &[R1]
    ));
    pick(&mut f.app, alternate);
    assert_eq!(r1(&f.app).footprint_id, name);
}

#[test]
fn a_locked_component_is_refused_with_the_reason() {
    let mut f = open();
    settle(
        &mut f.app,
        Message::Dock(DockMessage::Panel(PanelMsg::PcbComponent(
            PcbComponentMsg::LockToggled {
                board: f.path.clone(),
                footprints: vec![R1],
                locked: true,
            },
        ))),
    );
    let before = snapshot(&f.app);
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    let alternate = f.row.alternate_footprints[0].footprint;
    settle(
        &mut f.app,
        Message::Pcb(PcbMsg::FootprintSwap(FootprintSwapMsg::Picked {
            board: f.path.clone(),
            footprints: vec![R1],
            footprint: alternate,
        })),
    );
    assert_eq!(snapshot(&f.app), before, "nothing changed");
    assert!(
        noticed(since, "Change Footprint: R1 is locked"),
        "Messages says why"
    );
}

#[test]
fn a_footprint_the_board_cannot_hold_is_refused_with_the_reason() {
    let mut f = open();
    let before = snapshot(&f.app);
    // A template on the bottom side is not a library footprint's board form.
    let mut template = r1(&f.app);
    template.layer = "BottomCopper".into();
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    settle(
        &mut f.app,
        Message::Pcb(PcbMsg::FootprintSwap(FootprintSwapMsg::Resolved {
            board: f.path.clone(),
            footprints: vec![R1],
            result: Ok(Box::new(template)),
        })),
    );
    assert_eq!(snapshot(&f.app), before, "nothing changed");
    assert!(noticed(since, "Change Footprint: "), "Messages says why");

    let since = diagnostics::newest_entry_id().unwrap_or(0);
    settle(
        &mut f.app,
        Message::Pcb(PcbMsg::FootprintSwap(FootprintSwapMsg::Resolved {
            board: f.path.clone(),
            footprints: vec![R1],
            result: Err("the footprint has a pad shape the board cannot draw".into()),
        })),
    );
    assert_eq!(snapshot(&f.app), before);
    assert!(noticed(
        since,
        "Change Footprint: the footprint has a pad shape the board cannot draw; nothing changed"
    ));
}
