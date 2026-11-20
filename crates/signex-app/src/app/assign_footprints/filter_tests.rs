//! Graphite #80: the filter chips and their counts, the ticked bar's shared
//! footprints, the reason pills and the preview's pin map.

use std::path::PathBuf;

use signex_i18n::ids;
use uuid::Uuid;

use super::filter::AssignFilter;
use super::preview::{PinMap, pin_map};
use super::state::{AssignFootprintsState, FootprintOffer, FootprintPart, PartAccess};

fn part(designator: &str, row: u128, offers: &[&str], access: PartAccess) -> FootprintPart {
    FootprintPart {
        sheet: PathBuf::from("a.snxsch"),
        sheet_label: "a.snxsch".to_string(),
        symbol_id: Uuid::now_v7(),
        units: Vec::new(),
        designator: designator.to_string(),
        value: format!("{designator}-value"),
        row_label: Some(format!("ROW-{row}")),
        row_key: Some((Uuid::from_u128(1), Uuid::from_u128(row))),
        current: "FP".to_string(),
        current_binding: offers.first().map(|b| (*b).to_string()),
        offers: offers
            .iter()
            .map(|b| FootprintOffer {
                binding: (*b).to_string(),
                label: format!("FP {b}"),
            })
            .collect(),
        access,
    }
}

/// R1, R2 (row 1: 0402, 0603), C1 (row 2: 0603, 0805), U1 with no
/// alternates and D1 on a closed sheet.
fn state() -> AssignFootprintsState {
    let mut state = AssignFootprintsState::reading(1, 0);
    state.show(vec![
        part("R1", 1, &["0402", "0603"], PartAccess::Editable),
        part("R2", 1, &["0402", "0603"], PartAccess::Editable),
        part("C1", 2, &["0603", "0805"], PartAccess::Editable),
        part("U1", 3, &["SOIC8"], PartAccess::NoAlternates),
        part("D1", 4, &["SOT23"], PartAccess::SheetClosed),
    ]);
    state
}

#[test]
fn the_changed_chip_counts_the_parts_apply_changes() {
    let mut state = state();
    assert_eq!(state.filter_count(AssignFilter::All), 5);
    assert_eq!(state.filter_count(AssignFilter::Changed), 0);
    assert_eq!(state.filter_count(AssignFilter::NoAlternates), 1);
    state.pick(0, "0603");
    state.pick(2, "0805");
    assert_eq!(state.filter_count(AssignFilter::Changed), 2);
    state.filter = AssignFilter::Changed;
    assert_eq!(state.visible_rows(), [0, 2]);
    state.pick(0, "0402");
    assert_eq!(
        state.filter_count(AssignFilter::Changed),
        1,
        "picking the current footprint again drops the change"
    );
}

#[test]
fn the_filter_field_matches_designator_value_or_row() {
    let mut state = state();
    state.query = "row-1".to_string();
    assert_eq!(state.visible_rows(), [0, 1]);
    state.query = "c1-VALUE".to_string();
    assert_eq!(state.visible_rows(), [2]);
    state.query = "u1".to_string();
    state.filter = AssignFilter::Changed;
    assert!(state.visible_rows().is_empty(), "the chip applies too");
}

#[test]
fn the_ticked_bar_offers_what_every_ticked_row_shares() {
    let mut state = state();
    assert!(state.ticked_bar().is_none(), "no bar with nothing ticked");
    state.toggle(0);
    state.toggle(1);
    let bar = state.ticked_bar().expect("bar");
    assert_eq!(bar.count, 2);
    let shared: Vec<&str> = bar.offers.iter().map(|o| o.binding.as_str()).collect();
    assert_eq!(shared, ["0402", "0603"]);
    state.toggle(2);
    let bar = state.ticked_bar().expect("bar");
    assert_eq!(bar.count, 3);
    let shared: Vec<&str> = bar.offers.iter().map(|o| o.binding.as_str()).collect();
    assert_eq!(shared, ["0603"], "C1's row has only 0603 in common");
    state.select_all(false);
    assert!(state.ticked_bar().is_none(), "Clear ticks hides the bar");
}

#[test]
fn ticked_parts_with_no_footprint_in_common_get_an_empty_bar() {
    let mut state = AssignFootprintsState::reading(1, 0);
    state.show(vec![
        part("R1", 1, &["0402", "0603"], PartAccess::Editable),
        part("C1", 2, &["0805", "1206"], PartAccess::Editable),
    ]);
    state.select_all(true);
    let bar = state.ticked_bar().expect("bar");
    assert_eq!(bar.count, 2);
    assert!(bar.offers.is_empty(), "the bar says they share none");
}

#[test]
fn ticking_all_ticks_only_the_listed_parts() {
    let mut state = state();
    state.query = "row-1".to_string();
    state.select_all(true);
    assert_eq!(state.selected.iter().copied().collect::<Vec<_>>(), [0, 1]);
    assert!(state.all_selected());
}

#[test]
fn each_part_that_cannot_change_has_its_reason_pill() {
    assert_eq!(PartAccess::Editable.reason(), None);
    assert_eq!(
        PartAccess::SheetClosed.reason(),
        Some(ids::DIALOG_ASSIGN_FOOTPRINTS_SHEET_CLOSED)
    );
    assert_eq!(
        PartAccess::ReadOnly.reason(),
        Some(ids::DIALOG_ASSIGN_FOOTPRINTS_READ_ONLY)
    );
    assert_eq!(
        PartAccess::RowNotOpen.reason(),
        Some(ids::DIALOG_ASSIGN_FOOTPRINTS_ROW_NOT_OPEN)
    );
    assert_eq!(
        PartAccess::NoLink.reason(),
        Some(ids::DIALOG_ASSIGN_FOOTPRINTS_NO_LINK)
    );
    assert_eq!(
        PartAccess::NoAlternates.reason(),
        Some(ids::DIALOG_ASSIGN_FOOTPRINTS_NO_ALTERNATES)
    );
}

#[test]
fn the_preview_reads_the_current_and_the_picked_footprint() {
    let mut state = state();
    assert_eq!(state.preview_row, Some(0), "the first row is previewed");
    assert_eq!(state.preview_bindings(), ["0402"]);
    state.pick(0, "0603");
    assert_eq!(state.preview_bindings(), ["0402", "0603"]);
}

#[test]
fn the_pin_map_names_pads_the_new_footprint_lacks() {
    let pads = |list: &[&str]| list.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
    assert_eq!(
        pin_map(&pads(&["1", "2"]), &pads(&["1", "2"])),
        PinMap::Matches(pads(&["1", "2"]))
    );
    assert_eq!(
        pin_map(&pads(&["1", "2", "3"]), &pads(&["1", "2"])),
        PinMap::Missing(pads(&["3"]))
    );
    assert_eq!(
        pin_map(&pads(&["1", "2"]), &pads(&["2", "1", "EP"])),
        PinMap::Matches(pads(&["1", "2"])),
        "an extra pad on the new footprint loses no pin"
    );
}
