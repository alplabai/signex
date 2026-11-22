//! AV-11 (#2533): a rename that would leave a designator on two parts is
//! flagged on its own row and starts unticked.

use signex_engine::pcb::eco::{BackChange, BackChangeList, BackGroup, BackKey, BackNote, BackRow};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::{duplicate_renames, untick_duplicate_renames};

fn unit(n: u128) -> Vec<Uuid> {
    vec![Uuid::from_u128(n)]
}

/// The rename of part `n` from `from` to `to`, ticked.
fn rename(n: u128, from: &str, to: &str) -> BackRow {
    BackRow {
        key: BackKey::Designator(Uuid::from_u128(0x100 + n)),
        group: BackGroup::Designators,
        change: BackChange::Designator {
            footprint: Uuid::from_u128(0x100 + n),
            sheet: InstancePath::root(),
            units: unit(n),
            from: from.into(),
            to: to.into(),
        },
        on: true,
        blocked: None,
    }
}

/// A list of `rows` over parts `(n, designator)`.
fn list(parts: &[(u128, &str)], rows: Vec<BackRow>) -> BackChangeList {
    BackChangeList {
        revision: 1,
        rows,
        notes: Vec::new(),
        designators: parts
            .iter()
            .map(|(n, d)| (InstancePath::root(), unit(*n), (*d).to_string()))
            .collect(),
    }
}

fn key(n: u128) -> BackKey {
    BackKey::Designator(Uuid::from_u128(0x100 + n))
}

#[test]
fn a_rename_onto_a_held_designator_is_flagged_and_starts_unticked() {
    let mut l = list(&[(1, "R17"), (2, "R12")], vec![rename(1, "R17", "R12")]);
    let flagged = duplicate_renames(&l);
    let flag = flagged.get(&key(1)).expect("R17 to R12 is flagged");
    assert_eq!(flag.designator, "R12");
    assert_eq!(flag.holder, (InstancePath::root(), unit(2)));

    untick_duplicate_renames(&mut l, None);
    assert!(!l.row(&key(1)).expect("row").on, "it starts unticked");
    assert!(
        !l.notes
            .iter()
            .any(|n| matches!(n, BackNote::SharedDesignator { .. })),
        "nothing ticked shares a designator"
    );
    assert!(
        duplicate_renames(&l).contains_key(&key(1)),
        "the row stays flagged while unticked"
    );
    l.set_on(&key(1), true);
    assert!(
        l.row(&key(1)).expect("row").on,
        "the user may still tick it"
    );
}

#[test]
fn a_swap_is_not_a_duplicate() {
    let mut l = list(
        &[(1, "R17"), (2, "R12")],
        vec![rename(1, "R17", "R12"), rename(2, "R12", "R17")],
    );
    assert!(duplicate_renames(&l).is_empty());
    untick_duplicate_renames(&mut l, None);
    assert!(l.rows.iter().all(|r| r.on));
}

/// Unticking one rename keeps its part's designator, which can make
/// another rename a duplicate in turn.
#[test]
fn unticking_cascades_to_the_rename_it_uncovers() {
    let mut l = list(
        &[(1, "R5"), (2, "R6"), (3, "R7")],
        vec![rename(1, "R5", "R6"), rename(2, "R6", "R7")],
    );
    let first = duplicate_renames(&l);
    assert!(first.contains_key(&key(2)) && !first.contains_key(&key(1)));
    untick_duplicate_renames(&mut l, None);
    assert!(!l.row(&key(2)).expect("row").on);
    assert!(
        !l.row(&key(1)).expect("row").on,
        "R5 to R6 now collides too"
    );
}

/// Mockup row 82: two renames onto one free designator both start unticked
/// and keep their reason; ticking one leaves the other flagged by its holder.
#[test]
fn two_renames_onto_a_free_designator_stay_flagged_once_unticked() {
    let mut l = list(
        &[(1, "J1"), (2, "C2")],
        vec![rename(1, "J1", "J19"), rename(2, "C2", "J19")],
    );
    untick_duplicate_renames(&mut l, None);
    assert!(l.rows.iter().all(|r| !r.on), "both start unticked");
    let flagged = duplicate_renames(&l);
    for (me, other) in [(1, 2), (2, 1)] {
        let flag = flagged.get(&key(me)).expect("flagged while unticked");
        assert_eq!(flag.designator, "J19");
        assert_eq!(flag.holder, (InstancePath::root(), unit(other)));
        assert!(flag.contested, "no part holds J19 yet");
    }

    l.set_on(&key(1), true);
    let flagged = duplicate_renames(&l);
    assert!(!flagged.contains_key(&key(1)), "J1 to J19 is now unique");
    let left = flagged.get(&key(2)).expect("C2 to J19 still collides");
    assert_eq!(left.holder, (InstancePath::root(), unit(1)));
    assert!(!left.contested, "J1 holds J19 once its row applies");
}

/// A Refresh keeps the user's choice: a flagged rename ticked before stays
/// ticked; a new one starts unticked.
#[test]
fn a_refresh_keeps_a_chosen_tick() {
    let parts = [(1, "R17"), (2, "R12"), (3, "C1"), (4, "C2")];
    let previous = list(&parts, vec![rename(1, "R17", "R12")]);
    let mut l = list(&parts, vec![rename(1, "R17", "R12"), rename(3, "C1", "C2")]);
    untick_duplicate_renames(&mut l, Some(&previous));
    assert!(l.row(&key(1)).expect("row").on, "chosen before");
    assert!(!l.row(&key(3)).expect("row").on, "new and flagged");
}
