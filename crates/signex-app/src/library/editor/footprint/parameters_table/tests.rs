//! The table's own rules: the autocomplete offer, the shown value and unit,
//! and what Esc closes.

use std::collections::BTreeMap;

use signex_sketch::parameter_refs::RowValue;
use signex_sketch::unit::Quantity;

use super::SketchParametersMsg;
use super::state::{
    DisplayUnit, MAX_SUGGESTIONS, ParametersTableState, RenameDraft, display_value, suggestions,
};
use super::update::{escape, open_focused, start_rename, toggle, toggle_menu};

const NAMES: [&str; 5] = ["body", "pad_l", "pad_w", "pitch", "pitch2"];

fn offered(expr: &str, own: &str) -> Vec<String> {
    suggestions(expr, own, NAMES)
        .into_iter()
        .map(|s| s.completed)
        .collect()
}

#[test]
fn a_typed_letter_offers_the_names_that_start_with_it() {
    assert_eq!(offered("2 * pa", "span"), ["2 * pad_l", "2 * pad_w"]);
    assert_eq!(offered("b", "span"), ["body"]);
}

#[test]
fn a_complete_name_offers_only_the_longer_ones() {
    assert_eq!(offered("pitch", "span"), ["pitch2"]);
    assert!(offered("pitch2", "span").is_empty());
}

#[test]
fn the_row_itself_is_never_offered() {
    assert_eq!(offered("pitc", "pitch"), ["pitch2"]);
}

#[test]
fn a_number_a_unit_or_punctuation_offers_nothing() {
    assert!(offered("0.5", "span").is_empty());
    assert!(
        offered("0.5mm", "span").is_empty(),
        "a unit suffix is not a name"
    );
    assert!(offered("pitch * ", "span").is_empty());
    assert!(offered("", "span").is_empty());
}

#[test]
fn the_offer_is_capped() {
    let many: Vec<String> = (0..20).map(|i| format!("p{i}")).collect();
    let offer = suggestions("p", "x", many.iter().map(String::as_str));
    assert_eq!(offer.len(), MAX_SUGGESTIONS);
}

#[test]
fn values_show_in_mm_deg_or_count() {
    assert_eq!(
        display_value(Quantity::length(0.65)),
        ("0.65".to_string(), DisplayUnit::Mm)
    );
    assert_eq!(
        display_value(Quantity::angle(std::f64::consts::FRAC_PI_2)),
        ("90".to_string(), DisplayUnit::Deg)
    );
    assert_eq!(
        display_value(Quantity::count(4.0)),
        ("4".to_string(), DisplayUnit::Count)
    );
}

#[test]
fn only_rows_with_a_value_are_remembered() {
    let mut table = ParametersTableState::default();
    let mut rows = BTreeMap::new();
    rows.insert("a".to_string(), RowValue::Value(Quantity::length(1.0)));
    rows.insert(
        "b".to_string(),
        RowValue::Blocked {
            by: "a".to_string(),
        },
    );
    table.remember(&rows);
    rows.insert(
        "a".to_string(),
        RowValue::Blocked {
            by: "c".to_string(),
        },
    );
    table.remember(&rows);
    assert_eq!(table.last_good.get("a"), Some(&Quantity::length(1.0)));
    assert_eq!(table.last_good.get("b"), None);
}

#[test]
fn esc_closes_the_innermost_thing_then_the_table() {
    let mut slot = None;
    assert!(!escape(&mut slot), "a closed table lets Esc go on");
    toggle(&mut slot);
    toggle_menu(&mut slot, "a".into());
    assert!(escape(&mut slot));
    assert_eq!(slot.as_ref().unwrap().menu, None, "the menu first");
    start_rename(&mut slot, "a".into());
    assert!(escape(&mut slot));
    assert!(slot.as_ref().unwrap().rename.is_none(), "then the rename");
    assert!(escape(&mut slot));
    assert!(slot.is_none(), "then the table");
}

#[test]
fn opening_on_a_row_focuses_it_and_closes_what_was_open() {
    let mut slot = None;
    toggle(&mut slot);
    start_rename(&mut slot, "a".into());
    open_focused(&mut slot, "b");
    let table = slot.as_ref().unwrap();
    assert_eq!(table.focus.as_deref(), Some("b"));
    assert_eq!(table.rename, None);
    toggle(&mut slot);
    assert!(slot.is_none(), "Parameters again closes it");
}

#[test]
fn start_rename_holds_the_old_name() {
    let mut slot = None;
    toggle(&mut slot);
    toggle_menu(&mut slot, "pitch".into());
    start_rename(&mut slot, "pitch".into());
    let table = slot.as_ref().unwrap();
    assert_eq!(table.menu, None);
    assert_eq!(
        table.rename,
        Some(RenameDraft {
            from: "pitch".into(),
            draft: "pitch".into(),
            error: None,
        })
    );
}

#[test]
fn only_typing_is_recorded_by_the_router() {
    assert!(
        SketchParametersMsg::ExpressionEdited {
            name: "a".into(),
            expr: "1mm".into()
        }
        .records_undo_step()
    );
    for msg in [
        SketchParametersMsg::TableToggled,
        SketchParametersMsg::AddClicked,
        SketchParametersMsg::RenameSubmitted,
        SketchParametersMsg::DeleteConfirmed,
    ] {
        assert!(!msg.records_undo_step(), "{msg:?}");
    }
}
