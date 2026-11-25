//! #2505 — a footprint placed on a project's board from Place ▸ Component
//! has no schematic part behind it, so the next Update PCB lists it as kept
//! unlinked (the existing unlinked path), never as a removal. Fixture and
//! helpers: `update_pcb.rs`.

use pcb_geom::ClickMode;
use signex_app::library::LibraryMessage;
use signex_app::library::messages::PickerMsg;
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_engine::pcb::eco::EcoNote;

use super::*;

/// Place ▸ Component on the board in view, pick the fixture row and click
/// once at (30, 30) mm.
fn place_from_place_component(f: &mut Fixture) {
    send(&mut f.app, menu_row("place_board_component"));
    assert!(f.app.library.picker.is_some(), "the picker opened");
    let row = f._lib.row_id;
    send(
        &mut f.app,
        Message::Library(LibraryMessage::Picker(PickerMsg::SelectComponent(row))),
    );
    send(
        &mut f.app,
        Message::Library(LibraryMessage::Picker(PickerMsg::PlaceSelected)),
    );
    send(
        &mut f.app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: signex_types::pcb::Point::new(30.0, 30.0),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
}

#[test]
fn a_footprint_placed_from_place_component_is_kept_unlinked_by_update_pcb() {
    let mut f = fixture();
    new_board_and_list(&mut f);
    up(&mut f.app, UpdatePcbMsg::ExecuteRequested);
    assert_eq!(references(&f), ["R1", "R2"]);

    place_from_place_component(&mut f);
    let unlinked: Vec<(Uuid, String)> = engine(&f)
        .board()
        .footprints
        .iter()
        .filter(|fp| fp.schematic_link.is_none())
        .map(|fp| (fp.uuid, fp.reference.clone()))
        .collect();
    let [(placed, reference)] = unlinked.as_slice() else {
        panic!("one unlinked footprint was placed: {unlinked:?}");
    };
    assert_eq!(references(&f).len(), 3, "R1, R2 and the placed one");

    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    let list = list(&f);
    assert!(
        list.notes.iter().any(|n| matches!(
            n,
            EcoNote::KeptUnlinked { footprint, reference: r }
                if footprint == placed && r == reference
        )),
        "Update PCB lists {reference} as kept unlinked: {:?}",
        list.notes
    );
    assert!(
        !list.rows.iter().any(|r| matches!(
            r.change,
            EcoChange::RemoveFootprint { footprint, .. } if footprint == *placed
        )),
        "the unlinked footprint is not removed"
    );
}
