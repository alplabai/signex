//! #2366 — a footprint swapped on the board sets the linked schematic part's
//! footprint too, in the same undo step, so a later Update PCB lists no
//! footprint change for it. Driven through `Signex::update`: the Component
//! view's footprint button, the picker's pick, Update PCB, Undo and Redo.

use std::cell::Cell;

use signex_app::dock::DockMessage;
use signex_app::library::messages::{LibraryMessage, PrimitivePickerMsg};
use signex_app::panels::{PanelMsg, PcbComponentMsg};
use signex_library::FootprintBinding;

use super::*;
use crate::settle_support::settle;

/// R0805, pads 1 and 2, saved in the library and offered by its row as an
/// alternate.
fn offer_alternate(lib: &Library) -> PrimitiveRef {
    let adapter = LocalGitAdapter::open(&lib.snxlib).expect("open");
    let mut footprint = Footprint::empty("R0805");
    for (number, x) in [("1", -1.0), ("2", 1.0)] {
        footprint.pads.push(Pad {
            number: number.to_string(),
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            size: [0.8, 1.2],
            position: [x, 0.0],
            layers: vec![LayerId::new("F.Cu")],
            ..Pad::default()
        });
    }
    let reference = PrimitiveRef::new(lib.library_id, footprint.uuid);
    adapter
        .save_footprint(footprint, "seed alternate")
        .expect("save footprint");
    let id = signex_library::RowId(lib.row_id);
    let mut row = adapter.read_row("ICs", id).expect("row");
    row.alternate_footprints.push(FootprintBinding {
        footprint: reference,
        version: String::new(),
        pin_map_overrides: Vec::new(),
    });
    adapter
        .update_row("ICs", row, "offer alternate")
        .expect("offer");
    reference
}

/// The project with R1 and R2 placed on a new board by Update PCB, and the
/// alternate footprint the row offers.
fn placed() -> (Fixture, PrimitiveRef) {
    let alternate = Cell::new(None);
    let mut f = fixture_preparing(
        |lib| alternate.set(Some(offer_alternate(lib))),
        |_, _| {},
        None,
        ProjectDefaults::default(),
    );
    new_board_and_list(&mut f);
    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert_eq!(references(&f), ["R1", "R2"]);
    (f, alternate.get().expect("alternate"))
}

fn board_r1(f: &Fixture) -> signex_types::pcb::Footprint {
    engine(f)
        .board()
        .footprints
        .iter()
        .find(|fp| fp.reference == "R1")
        .expect("R1 on the board")
        .clone()
}

fn sheet_r1_footprint(f: &Fixture) -> String {
    f.app.document_state.engines[&f.sheet]
        .document()
        .symbols
        .iter()
        .find(|s| s.uuid == R1)
        .expect("R1 on the sheet")
        .footprint
        .clone()
}

/// The footprint button for R1, then the alternate picked.
fn swap_r1(f: &mut Fixture, footprint: PrimitiveRef) {
    let r1 = board_r1(f).uuid;
    settle(
        &mut f.app,
        Message::Dock(DockMessage::Panel(PanelMsg::PcbComponent(
            PcbComponentMsg::FootprintPickRequested {
                board: f.board.clone(),
                footprints: vec![r1],
            },
        ))),
    );
    settle(
        &mut f.app,
        Message::Library(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Pick(
            footprint,
        ))),
    );
}

#[test]
fn a_board_swap_sets_the_schematic_footprint_and_update_pcb_stays_quiet() {
    let (mut f, alternate) = placed();
    let sheet_before = sheet_r1_footprint(&f);
    assert_eq!(board_r1(&f).footprint_id, "R0603");

    swap_r1(&mut f, alternate);
    assert_eq!(board_r1(&f).footprint_id, "R0805", "the board swapped");
    assert_eq!(
        sheet_r1_footprint(&f),
        "R0805",
        "the linked part took the footprint"
    );

    let sheet = f.sheet.clone();
    show(&mut f, &sheet);
    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    let r1 = board_r1(&f).uuid;
    assert!(
        list(&f).row(&EcoKey::ChangeFootprint(r1)).is_none(),
        "Update PCB lists no footprint change for R1: {:#?}",
        list(&f).rows
    );
    send(
        &mut f.app,
        Message::Input(signex_app::app::InputMsg::EscapePressed { window: None }),
    );

    let board = f.board.clone();
    show(&mut f, &board);
    send(&mut f.app, menu_row("undo"));
    assert_eq!(
        board_r1(&f).footprint_id,
        "R0603",
        "Undo restores the board"
    );
    assert_eq!(
        sheet_r1_footprint(&f),
        sheet_before,
        "and the schematic, in the same step"
    );
    assert_eq!(engine(&f).undo_len(), 1, "only Update PCB's step is left");

    send(&mut f.app, menu_row("redo"));
    assert_eq!(board_r1(&f).footprint_id, "R0805");
    assert_eq!(sheet_r1_footprint(&f), "R0805", "Redo brings both back");

    // From the sheet, one Undo also takes the board back.
    show(&mut f, &sheet);
    send(&mut f.app, menu_row("undo"));
    assert_eq!(sheet_r1_footprint(&f), sheet_before);
    assert_eq!(board_r1(&f).footprint_id, "R0603");
}
