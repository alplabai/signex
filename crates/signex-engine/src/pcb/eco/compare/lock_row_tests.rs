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

/// A row another row needs stays, or that row could never be applied.
#[test]
fn a_lock_row_another_row_needs_stays_listed() {
    let lock = pad_row(0x10, net(1, "GND"), net(1, "GND"), true);
    let needing = pad_row(0x20, net(2, "VIN"), net(1, "GND"), false).needing(vec![pad_key(0x10)]);
    let report = report(vec![lock, needing]);
    assert_eq!(report.rows.len(), 2);
}
