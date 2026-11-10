//! The Assign Footprints draft: a part takes only its own row's offers, and
//! a bulk pick never gives a part a footprint its row does not offer.

use std::path::PathBuf;

use uuid::Uuid;

use super::state::{AssignFootprintsState, FootprintOffer, FootprintPart, PartAccess};

fn offer(binding: &str) -> FootprintOffer {
    FootprintOffer {
        binding: binding.to_string(),
        label: format!("FP {binding}"),
    }
}

fn part(designator: &str, row: u128, offers: &[&str], access: PartAccess) -> FootprintPart {
    FootprintPart {
        sheet: PathBuf::from("a.snxsch"),
        sheet_label: "a.snxsch".to_string(),
        symbol_id: Uuid::now_v7(),
        units: Vec::new(),
        designator: designator.to_string(),
        value: "10k".to_string(),
        row_label: Some(format!("ROW-{row}")),
        row_key: Some((Uuid::from_u128(1), Uuid::from_u128(row))),
        current: "FP 0805".to_string(),
        current_binding: offers.first().map(|b| (*b).to_string()),
        offers: offers.iter().map(|b| offer(b)).collect(),
        access,
    }
}

/// R1 and R2 from row 1 (0805, 0603), C1 from row 2 (0805, 1206), R3 on a
/// closed sheet.
fn state() -> AssignFootprintsState {
    let mut state = AssignFootprintsState::reading(1, 0);
    state.show(vec![
        part("R1", 1, &["0805", "0603"], PartAccess::Editable),
        part("R2", 1, &["0805", "0603"], PartAccess::Editable),
        part("C1", 2, &["0805", "1206"], PartAccess::Editable),
        part("R3", 1, &["0805", "0603"], PartAccess::SheetClosed),
    ]);
    state
}

#[test]
fn a_part_takes_only_a_footprint_its_row_offers() {
    let mut state = state();
    state.pick(0, "1206");
    assert!(state.changes().is_empty(), "R1's row does not offer 1206");
    state.pick(0, "0603");
    assert_eq!(state.changes(), vec![(0, "0603".to_string())]);
    assert!(state.can_apply());
}

#[test]
fn picking_what_a_part_has_drops_the_pick() {
    let mut state = state();
    state.pick(0, "0603");
    state.pick(0, "0805");
    assert!(state.changes().is_empty());
    assert!(!state.can_apply());
}

#[test]
fn a_part_on_a_closed_sheet_is_listed_only() {
    let mut state = state();
    state.pick(3, "0603");
    state.toggle(3);
    assert!(state.changes().is_empty());
    assert!(state.selected.is_empty());
}

#[test]
fn ticked_parts_share_only_what_every_row_offers() {
    let mut state = state();
    state.toggle(0);
    state.toggle(2);
    let shared: Vec<String> = state
        .shared_offers()
        .into_iter()
        .map(|o| o.binding)
        .collect();
    assert_eq!(shared, ["0805"], "R1 and C1 have only 0805 in common");
    state.toggle(2);
    state.toggle(1);
    let shared: Vec<String> = state
        .shared_offers()
        .into_iter()
        .map(|o| o.binding)
        .collect();
    assert_eq!(shared, ["0805", "0603"]);
}

#[test]
fn a_bulk_pick_sets_every_ticked_part_its_row_allows() {
    let mut state = state();
    state.select_all(true);
    assert_eq!(
        state.selected.len(),
        3,
        "the closed-sheet part is not ticked"
    );
    assert!(state.all_selected());
    state.pick_selected("0603");
    assert_eq!(
        state.changes(),
        vec![(0, "0603".to_string()), (1, "0603".to_string())],
        "C1's row does not offer 0603, so C1 keeps its footprint"
    );
}

#[test]
fn tick_all_from_this_row_ticks_the_rows_editable_parts() {
    let mut state = state();
    state.select_same_row(0);
    assert_eq!(state.selected.iter().copied().collect::<Vec<_>>(), [0, 1]);
    state.select_all(false);
    assert!(state.selected.is_empty());
}
