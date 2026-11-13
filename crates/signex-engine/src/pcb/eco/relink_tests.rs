//! #2318 — Update PCB never ticks the removal of a footprint whose part is
//! still in the design: a re-created sheet symbol relinks by designator, and
//! a second footprint linked to one part is listed unticked.

use signex_types::pcb::SymbolLink;

use super::fixture::*;
use super::*;

fn removals(list: &EcoChangeList) -> Vec<&EcoRow> {
    list.rows
        .iter()
        .filter(|r| matches!(r.change, EcoChange::RemoveFootprint { .. }))
        .collect()
}

/// R1 and R2 linked under sheet symbol 0xa; the design has them under 0xb.
fn moved_sheet_design(board_designators: [&str; 2]) -> Design {
    let old = root().child(u(0xa));
    let new = root().child(u(0xb));
    let mut fps = vec![
        board_fp(
            0xa1,
            board_designators[0],
            "10k",
            &[0x101],
            &r0603(),
            (10.0, 10.0),
        ),
        board_fp(
            0xa2,
            board_designators[1],
            "4k7",
            &[0x102],
            &r0603(),
            (20.0, 10.0),
        ),
    ];
    for fp in &mut fps {
        let units = fp.schematic_link.as_ref().unwrap().units.clone();
        fp.schematic_link = Some(SymbolLink::new(old.clone(), units));
    }
    let mut parts = vec![
        part("R1", &[0x101], "10k", Ok(r0603())),
        part("R2", &[0x102], "4k7", Ok(r0603())),
    ];
    for p in &mut parts {
        p.part.sheet = new.clone();
    }
    Design::new(board(fps), parts, netlist(&[]))
}

#[test]
fn new_sheet_symbol_uuid_does_not_tick_removal_of_still_present_parts() {
    let list = moved_sheet_design(["R1", "R2"]).diff();
    let ticked: Vec<_> = removals(&list)
        .into_iter()
        .filter(|r| r.default_on)
        .collect();
    assert!(
        ticked.is_empty(),
        "ticked RemoveFootprint rows:\n{}",
        report_text(&list)
    );
}

#[test]
fn new_sheet_symbol_uuid_relinks_by_designator_without_add_rows() {
    let list = moved_sheet_design(["R1", "R2"]).diff();
    let new = root().child(u(0xb));
    for id in [0xa1, 0xa2] {
        let row = list.row(&EcoKey::Link(u(id))).expect("a Link row");
        assert!(row.default_on);
        let EcoChange::Link { link, .. } = &row.change else {
            panic!("not a Link change");
        };
        assert_eq!(link.sheet, new);
    }
    assert!(removals(&list).is_empty(), "{}", report_text(&list));
    assert!(
        !list
            .rows
            .iter()
            .any(|r| matches!(r.change, EcoChange::AddFootprint { .. })),
        "{}",
        report_text(&list)
    );
}

#[test]
fn new_sheet_symbol_uuid_with_ambiguous_designator_keeps_removal_and_add() {
    // Both dangling footprints say R1: no unique match, so today's rows stay.
    let list = moved_sheet_design(["R1", "R1"]).diff();
    assert!(list.row(&EcoKey::Link(u(0xa1))).is_none());
    assert!(list.row(&EcoKey::Link(u(0xa2))).is_none());
    let rows = removals(&list);
    assert_eq!(rows.len(), 2, "{}", report_text(&list));
    assert!(rows.iter().all(|r| r.default_on));
    assert!(
        list.rows
            .iter()
            .any(|r| matches!(r.change, EcoChange::AddFootprint { .. }))
    );
}

fn second_link_design() -> Design {
    Design::new(
        board(vec![
            board_fp(0xa1, "R1", "10k", &[0x101], &r0603(), (10.0, 10.0)),
            board_fp(0xa9, "R1", "10k", &[0x101], &r0603(), (40.0, 10.0)),
        ]),
        vec![part("R1", &[0x101], "10k", Ok(r0603()))],
        netlist(&[]),
    )
}

#[test]
fn second_footprint_linked_to_same_part_is_not_a_ticked_removal() {
    let list = second_link_design().diff();
    let ticked: Vec<_> = removals(&list)
        .into_iter()
        .filter(|r| r.default_on)
        .collect();
    assert!(
        ticked.is_empty(),
        "ticked RemoveFootprint rows:\n{}",
        report_text(&list)
    );
}

#[test]
fn second_footprint_linked_to_same_part_is_listed_unticked_with_a_note() {
    let list = second_link_design().diff();
    assert!(list.row(&EcoKey::RemoveFootprint(u(0xa1))).is_none());
    let row = list.row(&EcoKey::RemoveFootprint(u(0xa9))).expect("listed");
    assert!(!row.default_on && !row.on && row.blocked.is_none());
    assert!(list.notes.iter().any(|n| matches!(
        n,
        EcoNote::ExtraLink { footprint, designator, .. }
            if *footprint == u(0xa9) && designator == "R1"
    )));
}
