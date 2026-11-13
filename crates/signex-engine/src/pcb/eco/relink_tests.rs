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
fn new_sheet_symbol_uuid_relinks_by_unit_without_add_rows() {
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
fn a_moved_sheet_relinks_by_unit_whatever_the_board_designators_say() {
    // Both board footprints say R1, but each shares a unit with one part:
    // both relink, and the second is renamed to its part's designator.
    let list = moved_sheet_design(["R1", "R1"]).diff();
    assert!(list.row(&EcoKey::Link(u(0xa1))).is_some());
    assert!(list.row(&EcoKey::Link(u(0xa2))).is_some());
    assert!(list.row(&EcoKey::SetDesignator(u(0xa2))).is_some());
    assert!(removals(&list).is_empty(), "{}", report_text(&list));
    assert!(
        !list
            .rows
            .iter()
            .any(|r| matches!(r.change, EcoChange::AddFootprint { .. }))
    );
}

#[test]
fn a_deleted_part_replaced_under_its_designator_is_not_relinked() {
    let d = Design::new(
        board(vec![board_fp(
            0xa1,
            "R5",
            "10k",
            &[0x101],
            &r0603(),
            (10.0, 10.0),
        )]),
        vec![part("R5", &[0x201], "100n", Ok(sot23()))],
        netlist(&[]),
    );
    let list = d.diff();
    assert!(
        list.row(&EcoKey::Link(u(0xa1))).is_none(),
        "{}",
        report_text(&list)
    );
}

#[test]
fn a_stale_link_does_not_take_an_unlinked_footprints_relink() {
    let mut f1 = board_fp(0xa1, "R1", "10k", &[], &r0603(), (10.0, 10.0));
    f1.schematic_link = None;
    let f2 = board_fp(0xa2, "R1", "10k", &[0x999], &r0603(), (20.0, 10.0));
    let d = Design::new(
        board(vec![f1, f2]),
        vec![part("R1", &[0x101], "10k", Ok(r0603()))],
        netlist(&[]),
    );
    let list = d.diff();
    assert!(
        list.row(&EcoKey::Link(u(0xa1))).is_some(),
        "{}",
        report_text(&list)
    );
}

#[test]
fn a_stale_link_whose_units_match_two_parts_is_not_relinked() {
    // A child sheet placed twice: the same unit uuid under two paths.
    let mut fp = board_fp(0xa1, "R1", "10k", &[0x101], &r0603(), (10.0, 10.0));
    fp.schematic_link = Some(SymbolLink::new(root().child(u(0xa)), [u(0x101)]));
    let mut first = part("R1", &[0x101], "10k", Ok(r0603()));
    first.part.sheet = root().child(u(0xb));
    let mut second = part("R2", &[0x101], "10k", Ok(r0603()));
    second.part.sheet = root().child(u(0xc));
    let list = Design::new(board(vec![fp]), vec![first, second], netlist(&[])).diff();
    assert!(
        list.row(&EcoKey::Link(u(0xa1))).is_none(),
        "{}",
        report_text(&list)
    );
    let removal = list
        .row(&EcoKey::RemoveFootprint(u(0xa1)))
        .expect("removal");
    assert!(removal.default_on);
}

#[test]
fn a_stale_link_under_an_unreadable_sheet_is_left_alone() {
    let gone = root().child(u(0xa));
    let mut fp = board_fp(0xa1, "R1", "10k", &[0x101], &r0603(), (10.0, 10.0));
    fp.schematic_link = Some(SymbolLink::new(gone.clone(), [u(0x101)]));
    let mut design = Design::new(
        board(vec![fp]),
        vec![part("R1", &[0x101], "10k", Ok(r0603()))],
        netlist(&[]),
    );
    design.unreadable = vec![gone];
    let list = design.diff();
    assert!(
        list.row(&EcoKey::Link(u(0xa1))).is_none(),
        "{}",
        report_text(&list)
    );
    assert!(list.row(&EcoKey::RemoveFootprint(u(0xa1))).is_none());
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
fn second_footprint_linked_to_a_part_off_the_board_keeps_both_removals_ticked() {
    let mut design = second_link_design();
    design.parts[0].on_board = false;
    let list = design.diff();
    for id in [0xa1, 0xa9] {
        let row = list.row(&EcoKey::RemoveFootprint(u(id))).expect("removal");
        assert!(row.default_on, "{}", report_text(&list));
    }
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
