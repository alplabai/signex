//! Mockup row 90: the class-to-class clearance matrix and its object-pair
//! card. The matrix messages are handled whichever Clearance view is shown,
//! and the Matrix view itself cannot be named from a test.

use signex_app::app::RulesTabMsg;
use signex_drc_dsl::table::LimitField;
use signex_drc_dsl::table::matrix::{self, CellKey};
use signex_widgets::constraint_grid::{ClassPair, MatrixMessage};

use super::{Rig, SIG_RULE};

fn rig() -> Rig {
    Rig::new(Some(SIG_RULE), &["HV", "PWR"])
}

/// The live (lower-triangle) HV x PWR cell.
fn hv_pwr() -> ClassPair {
    ClassPair::new(1, 0)
}

fn whole() -> CellKey {
    CellKey::new("PWR", "HV")
}

fn track_pad() -> CellKey {
    whole().with_pair("track-pad").expect("a known object pair")
}

/// The clearance the cell at `key` writes.
fn cell_value(rig: &Rig, key: &CellKey) -> Option<String> {
    let rows = rig.table();
    matrix::cell(&rows, key)
        .and_then(|row| row.limit(LimitField::MinClearance))
        .map(|limit| limit.value.clone())
}

fn type_in_cell(rig: &mut Rig, pair: ClassPair, value: &str) {
    rig.matrix(MatrixMessage::CellPressed(pair));
    rig.matrix(MatrixMessage::EditTextChanged(value.to_string()));
    rig.matrix(MatrixMessage::EditSubmitted);
}

fn type_in_pair_row(rig: &mut Rig, key: CellKey, text: &str) {
    let project_id = rig.id;
    rig.send(RulesTabMsg::MatrixPairTyped {
        project_id,
        key,
        text: text.to_string(),
    });
    rig.send(RulesTabMsg::MatrixCellCommitted { project_id });
}

#[test]
fn a_value_typed_in_a_matrix_cell_writes_its_class_pair_rule_and_undo_restores() {
    let mut rig = rig();
    type_in_cell(&mut rig, hv_pwr(), "0.8");

    assert_eq!(cell_value(&rig, &whole()).as_deref(), Some("0.8mm"));
    assert!(rig.stored().starts_with(SIG_RULE), "{}", rig.stored());
    assert!(rig.tab().matrix.edit().is_none(), "Enter closes the cell");
    assert!(rig.is_dirty());

    rig.undo();
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn pressing_a_matrix_cell_pins_it_and_opens_its_object_pairs() {
    let mut rig = rig();
    rig.matrix(MatrixMessage::CellPressed(hv_pwr()));

    assert_eq!(rig.tab().matrix.pinned(), Some(hv_pwr()));
    assert_eq!(
        rig.tab().matrix_open,
        Some(("PWR".to_string(), "HV".to_string()))
    );
    assert_eq!(rig.tab().matrix_pin, Some(whole()));
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn the_mirror_half_of_the_matrix_lands_on_the_same_live_cell() {
    let mut rig = rig();
    let mirrored = ClassPair { row: 0, col: 1 };
    type_in_cell(&mut rig, mirrored, "0.8");

    assert_eq!(rig.tab().matrix.pinned(), Some(hv_pwr()));
    assert_eq!(matrix::cells(&rig.table()).len(), 1, "one cell, not two");
    assert_eq!(cell_value(&rig, &whole()).as_deref(), Some("0.8mm"));
}

#[test]
fn escape_in_a_matrix_cell_writes_nothing() {
    let mut rig = rig();
    rig.matrix(MatrixMessage::CellPressed(hv_pwr()));
    rig.matrix(MatrixMessage::EditTextChanged("0.8".to_string()));
    rig.matrix(MatrixMessage::EditCancelled);

    assert!(rig.tab().matrix.edit().is_none());
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn enter_on_an_untouched_matrix_cell_writes_nothing() {
    let mut rig = rig();
    rig.matrix(MatrixMessage::CellPressed(hv_pwr()));
    rig.matrix(MatrixMessage::EditSubmitted);

    assert!(rig.tab().matrix.edit().is_none());
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn a_matrix_cell_refuses_text_a_negative_and_a_comma_decimal_and_stays_open() {
    for bad in ["abc", "-0.5", "0,8"] {
        let mut rig = rig();
        type_in_cell(&mut rig, hv_pwr(), bad);

        assert!(
            !rig.refusal().is_empty(),
            "'{bad}' is refused with a reason"
        );
        let open = rig.tab().matrix.edit().map(|edit| edit.text.clone());
        assert_eq!(open.as_deref(), Some(bad), "the cell keeps what was typed");
        rig.assert_untouched(SIG_RULE);
    }
}

#[test]
fn clearing_a_written_matrix_cell_removes_its_rule_and_undo_brings_it_back() {
    let mut rig = rig();
    type_in_cell(&mut rig, hv_pwr(), "0.8");

    rig.matrix(MatrixMessage::CellCleared(hv_pwr()));
    assert_eq!(cell_value(&rig, &whole()), None);
    assert!(rig.table().row("sig").is_some(), "other rules stay");

    rig.undo();
    assert_eq!(cell_value(&rig, &whole()).as_deref(), Some("0.8mm"));
    rig.undo();
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn an_object_pair_value_writes_a_narrowed_rule_and_leaves_the_whole_cell_empty() {
    let mut rig = rig();
    rig.matrix(MatrixMessage::CellPressed(hv_pwr()));
    type_in_pair_row(&mut rig, track_pad(), "0.4");

    assert_eq!(cell_value(&rig, &track_pad()).as_deref(), Some("0.4mm"));
    assert_eq!(cell_value(&rig, &whole()), None, "All objects stays empty");
    assert_eq!(rig.tab().matrix_edit, None, "Enter closes the row");
    assert_eq!(rig.tab().matrix_pin, Some(track_pad()), "the row is pinned");
    assert!(rig.is_dirty());

    rig.undo();
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn typing_in_an_object_pair_row_closes_the_matrix_cell_being_typed_in() {
    let mut rig = rig();
    rig.matrix(MatrixMessage::CellPressed(hv_pwr()));
    assert!(rig.tab().matrix.edit().is_some());

    let project_id = rig.id;
    rig.send(RulesTabMsg::MatrixPairTyped {
        project_id,
        key: track_pad(),
        text: "0.4".to_string(),
    });
    assert!(
        rig.tab().matrix.edit().is_none(),
        "one cell is typed in at a time"
    );
    assert!(rig.tab().matrix_edit.is_some());
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn a_bad_object_pair_value_is_refused_and_its_row_stays_open() {
    let mut rig = rig();
    rig.matrix(MatrixMessage::CellPressed(hv_pwr()));
    type_in_pair_row(&mut rig, track_pad(), "wide");

    assert!(!rig.refusal().is_empty());
    let open = rig.tab().matrix_edit.as_ref().map(|edit| edit.text.clone());
    assert_eq!(open.as_deref(), Some("wide"));
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn the_per_row_clear_removes_only_that_object_pair() {
    let mut rig = rig();
    type_in_cell(&mut rig, hv_pwr(), "0.8");
    type_in_pair_row(&mut rig, track_pad(), "0.4");

    let project_id = rig.id;
    rig.send(RulesTabMsg::MatrixCellCleared {
        project_id,
        key: track_pad(),
    });
    assert_eq!(cell_value(&rig, &track_pad()), None);
    assert_eq!(cell_value(&rig, &whole()).as_deref(), Some("0.8mm"));

    rig.undo();
    assert_eq!(cell_value(&rig, &track_pad()).as_deref(), Some("0.4mm"));
}

#[test]
fn a_cell_of_a_class_the_project_does_not_have_is_refused() {
    let mut rig = rig();
    let project_id = rig.id;
    rig.send(RulesTabMsg::MatrixPairTyped {
        project_id,
        key: CellKey::new("HV", "NOPE"),
        text: "0.4".to_string(),
    });
    rig.send(RulesTabMsg::MatrixCellCommitted { project_id });

    assert!(rig.refusal().contains("NOPE"), "{}", rig.refusal());
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn close_shuts_the_object_pair_card_and_the_preview_stays_on_the_class_pair() {
    let mut rig = rig();
    rig.matrix(MatrixMessage::CellPressed(hv_pwr()));
    let project_id = rig.id;
    rig.send(RulesTabMsg::MatrixPairPinned {
        project_id,
        key: track_pad(),
    });
    assert_eq!(rig.tab().matrix_pin, Some(track_pad()));

    rig.send(RulesTabMsg::MatrixCollapsed { project_id });

    assert_eq!(rig.tab().matrix_open, None);
    assert_eq!(rig.tab().matrix_edit, None);
    assert_eq!(rig.tab().matrix_hover, None);
    assert_eq!(rig.tab().matrix_pin, Some(whole()));
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn hovering_previews_a_cell_and_leaving_it_keeps_the_pin() {
    let mut rig = rig();
    rig.matrix(MatrixMessage::CellPressed(hv_pwr()));
    let project_id = rig.id;

    rig.send(RulesTabMsg::MatrixHovered {
        project_id,
        key: Some(track_pad()),
    });
    assert_eq!(rig.tab().matrix_hover, Some(track_pad()));
    assert_eq!(rig.tab().matrix_pin, Some(whole()), "a hover does not pin");

    rig.send(RulesTabMsg::MatrixHovered {
        project_id,
        key: None,
    });
    assert_eq!(rig.tab().matrix_hover, None);
    assert_eq!(rig.tab().matrix_pin, Some(whole()));
    rig.assert_untouched(SIG_RULE);
}

#[test]
fn picking_the_list_view_closes_the_card_and_drops_the_pin_and_hover() {
    let mut rig = rig();
    rig.matrix(MatrixMessage::CellPressed(hv_pwr()));
    let project_id = rig.id;
    rig.send(RulesTabMsg::MatrixHovered {
        project_id,
        key: Some(whole()),
    });

    // `ClearanceView` defaults to its List view.
    rig.send(RulesTabMsg::ClearanceViewPicked {
        project_id,
        view: Default::default(),
    });

    assert!(!rig.tab().is_matrix_shown());
    assert_eq!(rig.tab().matrix_open, None);
    assert_eq!(rig.tab().matrix_pin, None);
    assert_eq!(rig.tab().matrix_hover, None);
    rig.assert_untouched(SIG_RULE);
}
