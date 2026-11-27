//! #3060 — a pad its symbol has no pin for (a SOT-223 tab) that holds a
//! net keeps it and only gains its lock: no row, so a board that matches
//! its schematic lists nothing. The lock is taken with the rows that apply.

use signex_types::pcb::PadNet;

use super::fixture::*;
use super::net_tests::{execute, net_of};
use super::*;

const U1: u128 = 0x103;

fn sot223() -> signex_types::part::ResolvedFootprint {
    resolved("SOT-223", 0x223, "1.0.0", &["1", "2", "3", "4"])
}

/// U1 with pins 1..3 only; its footprint has the tab pad 4.
fn regulator(value: &str) -> Design {
    let parts = vec![part("U1", &[U1], value, Ok(sot223()))];
    let nets = netlist(&[
        ("GND", &[(U1, "U1", "1")]),
        ("+3V3", &[(U1, "U1", "2")]),
        ("+5V", &[(U1, "U1", "3")]),
    ]);
    let placed = board(vec![board_fp(
        0xa3,
        "U1",
        "LDO",
        &[U1],
        &sot223(),
        (10.0, 10.0),
    )]);
    Design::new(placed, parts, nets)
}

/// The board after a first sync, with the tab tied to +3V3 by hand as the
/// sample board has it: in sync with its schematic.
fn in_sync() -> Design {
    let mut design = regulator("LDO");
    let list = design.diff();
    execute(&mut design, &list);
    let fp = &mut design.board.footprints[0];
    let net = fp.pads[1].net.clone();
    assert_eq!(net.as_ref().map(|n| n.name.as_str()), Some("+3V3"));
    fp.pads[3].net = net;
    fp.pads[3].net_locked = false;
    design
}

fn tab(design: &Design) -> &signex_types::pcb::Pad {
    &design.board.footprints[0].pads[3]
}

#[test]
fn an_off_symbol_pad_that_keeps_its_net_is_no_row() {
    let design = in_sync();
    let list = design.diff();
    assert!(list.rows.is_empty(), "{:?}", list.rows);
    assert_eq!(list.pad_locks.len(), 1, "{:?}", list.pad_locks);
    assert_eq!(list.pad_locks[0].pad, tab(&design).uuid);
    assert_eq!(list.applied_pad_locks().count(), 0, "no row applies");
    assert!(
        lower(&list, &design.board).is_empty(),
        "Execute does nothing"
    );
}

/// A real difference is still listed, and Execute takes the lock with it.
#[test]
fn a_real_difference_is_listed_and_the_lock_rides_along() {
    let mut design = in_sync();
    design.parts[0].value = "AMS1117".into();
    let list = design.diff();
    assert_eq!(list.rows.len(), 1, "{:?}", list.rows);
    assert!(matches!(
        &list.rows[0].change,
        EcoChange::SetValue { from, to, .. } if from == "LDO" && to == "AMS1117"
    ));
    assert_eq!(list.applied_pad_locks().count(), 1);
    execute(&mut design, &list);
    assert!(tab(&design).net_locked, "locked with the value change");
    assert_eq!(net_of(&design.board, "U1", "4"), Some("+3V3"));
    let after = design.diff();
    assert!(after.rows.is_empty() && after.pad_locks.is_empty());
}

/// The lock is part of Execute's one undo step: undo restores the value
/// and the unlocked tab, redo takes both again.
#[test]
fn undo_takes_back_the_lock_with_the_row_and_redo_takes_it_again() {
    let mut design = in_sync();
    design.parts[0].value = "AMS1117".into();
    let list = design.diff();
    let mut engine = crate::pcb::BoardEngine::new(design.board.clone(), design.revision);
    let commands = lower(&list, engine.board());
    engine
        .execute_labelled(commands, "Update PCB", list.revision)
        .unwrap();
    let state = |engine: &crate::pcb::BoardEngine| {
        let fp = &engine.board().footprints[0];
        (fp.value.clone(), fp.pads[3].net_locked)
    };
    assert_eq!(state(&engine), ("AMS1117".to_string(), true));
    assert!(engine.undo().unwrap().is_some(), "one undo step");
    assert_eq!(state(&engine), ("LDO".to_string(), false));
    assert_eq!(engine.board().footprints[0].pads[3].net, tab(&design).net);
    assert!(engine.redo().unwrap().is_some());
    assert_eq!(state(&engine), ("AMS1117".to_string(), true));
}

/// A pad net that differs is still a row, whose from and to differ.
#[test]
fn a_pad_net_difference_is_still_listed() {
    let mut design = in_sync();
    let gnd = design.board.footprints[0].pads[0].net.clone();
    design.board.footprints[0].pads[2].net = gnd;
    let list = design.diff();
    let pad = list
        .rows
        .iter()
        .find_map(|row| match &row.change {
            EcoChange::PadNet {
                pad_number,
                from,
                to,
                ..
            } if pad_number == "3" => Some((from.clone(), to.clone())),
            _ => None,
        })
        .expect("U1.3 is listed");
    let name = |n: &Option<PadNet>| n.as_ref().map(|n| n.name.clone());
    assert_eq!(name(&pad.0).as_deref(), Some("GND"));
    assert_eq!(name(&pad.1).as_deref(), Some("+5V"));
    for row in &list.rows {
        if let EcoChange::PadNet { from, to, .. } = &row.change {
            assert_ne!(from, to, "no row reads the same on both sides");
        }
    }
}

/// Unticking every row leaves the lock out too.
#[test]
fn the_lock_waits_while_no_row_is_ticked() {
    let mut design = in_sync();
    design.parts[0].value = "AMS1117".into();
    let mut list = design.diff();
    let key = list.rows[0].key.clone();
    list.set_on(&key, false);
    assert_eq!(list.applied_pad_locks().count(), 0);
    assert!(lower(&list, &design.board).is_empty());
}
