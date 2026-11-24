//! Mockup row 162: Net differences lists only real differences. A pad that
//! keeps its net and only gains its lock is not one.

use signex_types::pcb::PadNet;
use uuid::Uuid;

use super::*;
use crate::pcb::eco::{EcoChange, EcoGroup, EcoRow};

fn net(number: u32, name: &str) -> Option<PadNet> {
    Some(PadNet {
        number,
        name: name.into(),
    })
}

fn pad_key(n: u128) -> EcoKey {
    EcoKey::PadNet {
        footprint: Uuid::from_u128(n),
        pad: Uuid::from_u128(n + 1),
    }
}

fn pad_row(n: u128, from: Option<PadNet>, to: Option<PadNet>, locked: bool) -> EcoRow {
    EcoRow::new(
        pad_key(n),
        EcoGroup::PadNets,
        EcoChange::PadNet {
            footprint: Uuid::from_u128(n),
            pad: Uuid::from_u128(n + 1),
            reference: "J1".into(),
            pad_number: "3".into(),
            from,
            to,
            locked,
        },
        true,
    )
}

fn report(rows: Vec<EcoRow>) -> CompareReport {
    let eco = EcoChangeList {
        revision: 1,
        rows,
        notes: Vec::new(),
    };
    compare(&eco, &BackChangeList::default())
}

#[test]
fn a_pad_that_only_gains_its_lock_is_not_a_difference() {
    let report = report(vec![pad_row(0x10, net(1, "GND"), net(1, "GND"), true)]);
    assert!(report.in_sync(), "{:?}", report.rows);
}

/// The names match, but the pad moves to another net number: without this
/// row it would stay on a net the sync renames.
#[test]
fn a_pad_moving_between_numbers_of_one_name_stays_listed() {
    let report = report(vec![pad_row(0x10, net(2, "+5V"), net(1, "+5V"), false)]);
    assert_eq!(report.rows.len(), 1);
    assert_eq!(report.rows[0].pcb_action, Some(CompareAction::MovePin));
}

fn is_on(list: &EcoChangeList, key: &EcoKey) -> bool {
    list.row(key).is_some_and(|row| row.on)
}

/// A lock row another row needs reads GND / GND: not drawn (mockup row
/// 162), but applied with that row, or that row could never be applied.
#[test]
fn a_lock_row_another_row_needs_is_not_drawn_and_follows_it() {
    let lock = pad_row(0x10, net(1, "GND"), net(1, "GND"), true);
    let needing = pad_row(0x20, net(2, "VIN"), net(1, "GND"), false).needing(vec![pad_key(0x10)]);
    let mut report = report(vec![lock, needing]);
    assert_eq!(report.rows.len(), 1, "{:?}", report.rows);
    assert_eq!(report.rows[0].pcb, Some(pad_key(0x20)));
    assert_eq!(report.rows[0].direction, Direction::UpdatePcb);
    assert!(is_on(report.eco(), &pad_key(0x10)), "the lock rides along");
    assert!(!report.waits(&report.rows[0]), "the needing row applies");
    assert_eq!(report.counts().to_pcb, 1);

    let id = report.rows[0].id();
    report.set_direction(&id, Direction::Skip);
    assert!(!is_on(report.eco(), &pad_key(0x10)), "skipped with it");
    assert!(!is_on(&report.ticks().0, &pad_key(0x10)));
    report.set_direction(&id, Direction::UpdatePcb);
    assert!(is_on(report.eco(), &pad_key(0x10)), "ticked again with it");
    assert!(is_on(&report.ticks().0, &pad_key(0x10)));
}

/// Mockup row 162: U1 pin 3 moving between two +5V net numbers, tied to
/// the row it needs, reads +5V / +5V and is not drawn; it is applied while
/// that row is.
#[test]
fn an_equal_named_pad_row_that_needs_a_row_is_not_drawn_and_follows_it() {
    let needed = pad_row(0x20, net(3, "VIN"), net(1, "+5V"), false);
    let same = pad_row(0x10, net(2, "+5V"), net(1, "+5V"), false).needing(vec![pad_key(0x20)]);
    let mut report = report(vec![same, needed]);
    assert_eq!(report.rows.len(), 1, "{:?}", report.rows);
    let drawn = &report.rows[0];
    assert_ne!(drawn.schematic, drawn.board, "every drawn row differs");
    assert!(is_on(report.eco(), &pad_key(0x10)));

    let id = report.rows[0].id();
    report.set_shown(None, Direction::Skip);
    assert!(!is_on(report.eco(), &pad_key(0x10)));
    assert!(!is_on(report.eco(), &pad_key(0x20)));
    report.set_direction(&id, Direction::UpdatePcb);
    assert!(is_on(report.eco(), &pad_key(0x10)));
}

/// A folded row that is blocked stays unticked, whatever needs it.
#[test]
fn a_blocked_folded_row_stays_unticked() {
    let lock = pad_row(0x10, net(1, "GND"), net(1, "GND"), true).blocked(EcoBlock::PartNotRead);
    let needing = pad_row(0x20, net(2, "VIN"), net(1, "GND"), false).needing(vec![pad_key(0x10)]);
    let report = report(vec![lock, needing]);
    assert_eq!(report.rows.len(), 1);
    assert!(!is_on(report.eco(), &pad_key(0x10)));
    assert!(report.waits(&report.rows[0]), "the needing row waits");
}
