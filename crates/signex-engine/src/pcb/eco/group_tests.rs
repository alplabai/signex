//! #1312 — the group checkbox of the Update PCB dialog: its tick state and
//! ticking a whole group, blocked rows left alone.

use signex_types::part::FootprintResolveError;

use super::fixture::*;
use super::*;

fn add_key(unit: u128) -> EcoKey {
    EcoKey::AddFootprint {
        sheet: root(),
        part: u(unit),
    }
}

/// Three parts to add, the third blocked (no footprint).
fn three_adds() -> EcoChangeList {
    let design = Design::new(
        board(Vec::new()),
        vec![
            part("R1", &[0x101], "10k", Ok(r0603())),
            part("R2", &[0x102], "4k7", Ok(r0603())),
            part(
                "J1",
                &[0x103],
                "conn",
                Err(FootprintResolveError::NoFootprint),
            ),
        ],
        netlist(&[]),
    );
    design.diff()
}

#[test]
fn a_group_counts_only_rows_that_can_be_ticked() {
    let list = three_adds();
    let ticks = list.group_ticks(EcoGroup::AddFootprints);
    assert_eq!(ticks, GroupTicks { on: 2, open: 2 });
    assert!(ticks.all());
    assert!(!ticks.mixed());
}

#[test]
fn unticking_one_row_makes_the_group_mixed() {
    let mut list = three_adds();
    assert!(list.set_on(&add_key(0x101), false));
    let ticks = list.group_ticks(EcoGroup::AddFootprints);
    assert!(ticks.mixed());
    assert!(!ticks.all());
}

#[test]
fn the_group_box_ticks_every_open_row_and_never_a_blocked_one() {
    let mut list = three_adds();
    assert!(list.set_group_on(EcoGroup::AddFootprints, false));
    assert_eq!(
        list.group_ticks(EcoGroup::AddFootprints),
        GroupTicks { on: 0, open: 2 }
    );
    assert!(list.set_group_on(EcoGroup::AddFootprints, true));
    assert!(list.row(&add_key(0x101)).unwrap().on);
    assert!(list.row(&add_key(0x102)).unwrap().on);
    assert!(!list.row(&add_key(0x103)).unwrap().on, "blocked stays off");
    assert!(
        !list.set_group_on(EcoGroup::AddFootprints, true),
        "nothing changed the second time"
    );
}

#[test]
fn an_empty_group_is_neither_all_nor_mixed() {
    let list = three_adds();
    let ticks = list.group_ticks(EcoGroup::RemoveFootprints);
    assert_eq!(ticks, GroupTicks { on: 0, open: 0 });
    assert!(!ticks.all());
    assert!(!ticks.mixed());
}
