//! #1312 review of #2271: net sync never guesses a pin map, never reuses a
//! number it frees, and never drops a net a kept pad still names.

use signex_types::part::{FootprintResolveError, PinPadMap};
use signex_types::pcb::Point;
use signex_types::pcb_sketch::{BoardSketch, PcbHole};

use super::fixture::*;
use super::net_tests::{
    assert_consistent, execute, net_of, number_of, pad_key, parts, placed, synced, wired,
};
use super::*;

/// The wired design with R1's pins swapped: pin 2 on VCC, pin 1 on SIG.
fn swapped_r1() -> signex_types::net::Netlist {
    netlist(&[
        ("VCC", &[(0x101, "R1", "2")]),
        ("SIG", &[(0x101, "R1", "1"), (0x102, "R2", "1")]),
        ("GND", &[(0x102, "R2", "2")]),
    ])
}

fn pad_rows_of(list: &EcoChangeList, footprint: u128) -> usize {
    list.rows
        .iter()
        .filter(|r| matches!(r.key, EcoKey::PadNet { footprint: f, .. } if f == u(footprint)))
        .count()
}

/// Blocker 1: R1 maps pin 1 to pad 2. Once its library is gone the part's
/// pin map is unknown: nothing renames or re-nets its pads, and a note says
/// why.
#[test]
fn an_unresolvable_footprint_keeps_its_pads_out_of_net_sync() {
    let mut design = Design::new(placed(), parts(), wired());
    let mut mapped = r0603();
    mapped.pin_map = PinPadMap::from_pairs([("1", "2"), ("2", "1")]);
    design.parts[0].footprint = Ok(mapped);
    let list = design.diff();
    execute(&mut design, &list);
    assert!(design.diff().rows.is_empty());
    assert_eq!(net_of(&design.board, "R1", "1"), Some("SIG"));

    design.parts[0].footprint = Err(FootprintResolveError::NotFound {
        library: u(LIB),
        footprint: u(0x603),
        version: "1.0.0".into(),
    });
    let list = design.diff();
    assert!(list.rows.is_empty(), "{}", report_text(&list));
    assert!(
        list.notes.iter().any(|n| matches!(
            n,
            EcoNote::FootprintUnresolved { footprint, reference, .. }
                if *footprint == u(0xa1) && reference == "R1"
        )),
        "{}",
        report_text(&list)
    );
    assert!(
        !list
            .notes
            .iter()
            .any(|n| matches!(n, EcoNote::Net(text) if text.contains("R1."))),
        "the note covers the part's pins: {}",
        report_text(&list)
    );
}

/// A part that binds no footprint, matched to a board footprint with no
/// library link, has no pin map to lose: its pads still sync pin n on pad n.
#[test]
fn a_part_binding_no_footprint_still_syncs_one_to_one() {
    let mut design = synced();
    for f in &mut design.board.footprints {
        f.library_ref = None;
    }
    design.parts[0].footprint = Err(FootprintResolveError::NoFootprint);
    design.netlist = swapped_r1();
    let list = design.diff();
    assert_eq!(pad_rows_of(&list, 0xa1), 2, "{}", report_text(&list));
    assert!(
        !list
            .notes
            .iter()
            .any(|n| matches!(n, EcoNote::FootprintUnresolved { .. }))
    );
}

/// The same holds for a part that could not be read back.
#[test]
fn an_unread_parts_pads_are_left_out_of_net_sync() {
    let mut design = synced();
    design.parts[0].read = false;
    design.parts[0].footprint = Err(FootprintResolveError::NoFootprint);
    design.netlist = swapped_r1();
    let list = design.diff();
    assert_eq!(pad_rows_of(&list, 0xa1), 0, "{}", report_text(&list));
}

/// Finding 2: GND loses its last pad and a new net NEW arrives with GND's
/// id. NEW takes a fresh number, so GND is removed and NEW added, never a
/// rename; unticking the pad row that empties GND keeps the board
/// consistent.
#[test]
fn a_number_freed_in_a_sync_is_not_reused_as_a_rename() {
    let mut design = synced();
    let gnd = number_of(&design.board, "GND");
    design.parts.push(part("C1", &[0x301], "100n", Ok(r0603())));
    design.netlist = netlist(&[
        ("VCC", &[(0x101, "R1", "1"), (0x102, "R2", "2")]),
        ("SIG", &[(0x101, "R1", "2"), (0x102, "R2", "1")]),
        ("NEW", &[(0x301, "C1", "1")]),
    ]);
    assert_eq!(design.netlist.nets[2].id.0, gnd, "NEW has GND's id");
    let mut list = design.diff();
    assert!(
        list.row(&EcoKey::RenameNet(gnd)).is_none(),
        "{}",
        report_text(&list)
    );
    assert!(list.row(&EcoKey::RemoveNet(gnd)).is_some());
    let add = list
        .row(&EcoKey::AddNet("NEW".into()))
        .expect("NEW is added");
    assert!(matches!(add.change, EcoChange::AddNet { number, .. } if number != gnd));

    list.set_on(&pad_key(&design.board, "R2", "2"), false);
    execute(&mut design, &list);
    assert_eq!(net_of(&design.board, "R2", "2"), Some("GND"));
    assert_eq!(net_of(&design.board, "C1", "1"), Some("NEW"));
    assert_consistent(&design.board);
}

/// Finding 3: unticking a removal without a refresh keeps the net its pads
/// hold, so no pad names a net the table dropped.
#[test]
fn a_net_removal_needs_the_footprint_removal_that_took_its_pads() {
    let mut design = synced();
    design.parts[1].on_board = false;
    design
        .netlist
        .nets
        .iter_mut()
        .for_each(|n| n.terminals.retain(|t| t.reference != "R2"));
    design.netlist.nets.retain(|n| !n.terminals.is_empty());
    let gnd = number_of(&design.board, "GND");
    let mut list = design.diff();
    let remove_fp = EcoKey::RemoveFootprint(u(0xa2));
    assert!(list.row(&remove_fp).is_some_and(|r| r.on));
    let remove_net = list.row(&EcoKey::RemoveNet(gnd)).expect("GND goes").clone();
    assert!(list.applies(&remove_net));

    list.set_on(&remove_fp, false);
    assert!(!list.applies(&remove_net), "the removal needs R2's removal");
    execute(&mut design, &list);
    assert_eq!(net_of(&design.board, "R2", "2"), Some("GND"));
    assert_consistent(&design.board);
}

/// Minor: a blocked row forced on (`on` is public) is still never lowered.
#[test]
fn a_blocked_row_is_never_lowered_even_when_ticked() {
    let mut design = synced();
    design.parts.push(part(
        "J1",
        &[0x401],
        "conn",
        Err(FootprintResolveError::NoFootprint),
    ));
    let mut list = design.diff();
    let row = list
        .rows
        .iter_mut()
        .find(|r| r.blocked.is_some())
        .expect("J1's add is blocked");
    row.on = true;
    let row = row.clone();
    assert!(!list.applies(&row));
    assert!(lower(&list, &design.board).is_empty());
}

/// Minor: a plated sketch hole on GND keeps GND in the table after its last
/// pad leaves it.
#[test]
fn a_sketch_hole_keeps_its_net() {
    let mut design = synced();
    let gnd = number_of(&design.board, "GND");
    design.board.sketch = Some(BoardSketch {
        holes: vec![PcbHole {
            uuid: u(0x3001),
            position: Point { x: 2.0, y: 2.0 },
            drill: 3.2,
            plated: true,
            net: Some(gnd),
            unknown_columns: Default::default(),
        }],
        ..Default::default()
    });
    design.netlist = netlist(&[
        ("VCC", &[(0x101, "R1", "1"), (0x102, "R2", "2")]),
        ("SIG", &[(0x101, "R1", "2"), (0x102, "R2", "1")]),
    ]);
    let list = design.diff();
    assert!(
        list.row(&EcoKey::RemoveNet(gnd)).is_none(),
        "{}",
        report_text(&list)
    );
    execute(&mut design, &list);
    assert!(design.board.nets.iter().any(|n| n.number == gnd));
}

/// Review N1: a part whose row no longer binds a footprint, matched to a
/// placed footprint built from a library footprint, keeps that footprint's
/// pads out of net sync: its pin map may differ from 1:1.
#[test]
fn a_part_binding_no_footprint_keeps_a_library_footprints_pads() {
    let mut design = synced();
    assert!(
        design
            .board
            .footprints
            .iter()
            .all(|f| f.library_ref.is_some()),
        "the synced footprints are built from the library"
    );
    design.parts[0].footprint = Err(FootprintResolveError::NoFootprint);
    design.netlist = swapped_r1();
    let list = design.diff();
    assert_eq!(pad_rows_of(&list, 0xa1), 0, "{}", report_text(&list));
    assert!(
        list.notes.iter().any(
            |n| matches!(n, EcoNote::FootprintUnresolved { footprint, .. } if *footprint == u(0xa1))
        ),
        "{}",
        report_text(&list)
    );
}
