//! Graphite #78 and #80, driven through the real dialogs.
//!
//! #78 Number Schematic Sheets: Apply is greyed while a clash, a gap or a
//! number above the page count stands, and Apply pressed anyway changes
//! nothing. #80 Assign Footprints: the Changed chip counts the picks, the
//! ticked bar offers what every ticked part's row shares, and a part that
//! cannot change carries its reason pill.

use signex_app::app::{AssignFilter, AssignFootprintsMsg, PartAccess, SheetNumberingMsg, Signex};
use signex_i18n::ids;

use crate::assign_footprints as af;
use crate::library_footprint_pick::binding_key;
use crate::number_schematic_sheets as sn;
use crate::settle_support::settle;

fn can_apply(app: &Signex) -> bool {
    app.ui_state
        .sheet_numbering
        .as_ref()
        .expect("dialog open")
        .can_apply()
}

/// The numbering rig with its dialog read: `/`, `/A`, `/A/L` and `/C`.
fn numbering() -> sn::Rig {
    let mut rig = sn::rig();
    settle(&mut rig.app, crate::menu_support::menu_row(sn::COMMAND));
    rig
}

#[test]
fn a_clash_greys_apply_and_apply_pressed_anyway_changes_nothing() {
    let mut rig = numbering();
    sn::type_number(&mut rig.app, 3, "3");
    assert!(!can_apply(&rig.app), "/A/L and /C are both 3");
    sn::send(&mut rig.app, SheetNumberingMsg::ApplyRequested);
    assert!(
        rig.app.ui_state.sheet_numbering.is_some(),
        "the dialog stays open"
    );
    assert!(
        rig.app.document_state.dirty_document_paths().is_empty(),
        "no sheet changed"
    );
}

#[test]
fn a_gap_with_a_high_number_greys_apply() {
    let mut rig = numbering();
    // 1, 2, 3, 9: no page 4, and 9 is above the four pages.
    sn::type_number(&mut rig.app, 3, "9");
    assert!(!can_apply(&rig.app));
}

#[test]
fn a_number_above_the_page_count_greys_apply() {
    let mut rig = numbering();
    sn::type_number(&mut rig.app, 2, "4");
    sn::type_number(&mut rig.app, 3, "5");
    assert!(!can_apply(&rig.app), "5 is above the four pages");
}

#[test]
fn clean_numbers_offer_apply() {
    let mut rig = numbering();
    sn::type_number(&mut rig.app, 2, "4");
    sn::type_number(&mut rig.app, 3, "3");
    assert!(can_apply(&rig.app));
    sn::send(&mut rig.app, SheetNumberingMsg::ApplyRequested);
    assert!(
        rig.app.ui_state.sheet_numbering.is_none(),
        "Apply closes it"
    );
}

/// The Assign Footprints rig with its dialog read: R1 and R2 on open sheets,
/// R3 on a closed one.
fn footprints() -> af::Rig {
    let mut rig = af::rig();
    settle(&mut rig.app, crate::menu_support::menu_row(af::COMMAND));
    af::assert_dialog_read(&rig.app);
    rig
}

#[test]
fn the_changed_chip_counts_the_picked_parts() {
    let mut rig = footprints();
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert_eq!(state.filter_count(AssignFilter::All), 3);
    assert_eq!(state.filter_count(AssignFilter::Changed), 0);
    let row = state
        .parts()
        .iter()
        .position(|p| p.sheet == rig.a)
        .expect("R1 is listed");
    af::send(
        &mut rig.app,
        AssignFootprintsMsg::FootprintPicked {
            row,
            binding: binding_key(&rig.alt.0),
        },
    );
    af::send(
        &mut rig.app,
        AssignFootprintsMsg::FilterPicked(AssignFilter::Changed),
    );
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert_eq!(state.filter_count(AssignFilter::Changed), 1);
    assert_eq!(state.visible_rows(), [row], "only the changed part listed");
}

#[test]
fn the_ticked_bar_offers_the_footprints_the_ticked_rows_share() {
    let mut rig = footprints();
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert!(state.ticked_bar().is_none(), "no bar before a tick");
    af::send(&mut rig.app, AssignFootprintsMsg::AllToggled(true));
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    let bar = state.ticked_bar().expect("the bar shows");
    assert_eq!(bar.count, 2, "the closed sheet's part is not ticked");
    let editable = state
        .parts()
        .iter()
        .find(|p| p.access == PartAccess::Editable)
        .expect("an editable part");
    assert_eq!(
        bar.offers, editable.offers,
        "both resistors share the Stock row's footprints"
    );
    af::send(&mut rig.app, AssignFootprintsMsg::AllToggled(false));
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert!(state.ticked_bar().is_none(), "Clear ticks hides the bar");
}

#[test]
fn a_part_on_a_closed_sheet_carries_its_reason_pill() {
    let rig = footprints();
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    let closed = state
        .parts()
        .iter()
        .find(|p| p.access == PartAccess::SheetClosed)
        .expect("R3 is listed");
    assert_eq!(
        closed.access.reason(),
        Some(ids::DIALOG_ASSIGN_FOOTPRINTS_SHEET_CLOSED)
    );
    for part in state
        .parts()
        .iter()
        .filter(|p| p.access == PartAccess::Editable)
    {
        assert_eq!(part.access.reason(), None, "{} can change", part.designator);
    }
}
