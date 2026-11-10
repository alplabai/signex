//! The dialog's draft rules: what Apply writes for what is typed.

use std::path::PathBuf;
use std::sync::Arc;

use signex_types::schematic::InstancePath;

use super::read::{NumberingRead, NumberingRow, RowPlacement};
use super::state::{PageEdit, SheetNumberingState};

fn row(number: usize, position: usize, manual: bool, placed: bool) -> NumberingRow {
    NumberingRow {
        name: format!("page{position}"),
        sheet: format!("page{position}.snxsch"),
        depth: usize::from(placed),
        number,
        position,
        manual,
        placement: placed.then(|| RowPlacement {
            parent: PathBuf::from("/p/root.snxsch"),
            parent_path: InstancePath::root(),
            symbol: uuid::Uuid::from_u128(position as u128),
        }),
    }
}

/// The root, two placed pages, the second numbered 5 by hand.
fn shown() -> SheetNumberingState {
    let mut state = SheetNumberingState::reading(1, 0, Default::default(), "p".into());
    state.show(Arc::new(NumberingRead {
        rows: vec![
            row(1, 1, false, false),
            row(2, 2, false, true),
            row(5, 3, true, true),
        ],
        ..Default::default()
    }));
    state
}

#[test]
fn an_untouched_dialog_writes_nothing() {
    let state = shown();
    assert_eq!(state.drafts, ["1", "2", "5"]);
    assert!(state.edits().is_empty());
    assert!(!state.can_apply(), "nothing to apply");
}

#[test]
fn a_typed_number_is_written_and_its_position_clears_the_entry() {
    let mut state = shown();
    state.edit(1, "7".into());
    state.edit(2, "3".into());
    assert_eq!(
        state.edits(),
        vec![
            PageEdit {
                row: 1,
                page: Some(7)
            },
            PageEdit { row: 2, page: None },
        ]
    );
    assert!(state.can_apply());
}

#[test]
fn a_top_level_page_cannot_be_renumbered() {
    let mut state = shown();
    state.edit(0, "9".into());
    assert_eq!(state.drafts[0], "1");
    assert!(state.edits().is_empty());
}

#[test]
fn a_number_that_is_not_a_sheet_number_blocks_apply() {
    let mut state = shown();
    for bad in ["0", "", "+2", "x"] {
        state.edit(1, bad.into());
        assert!(state.has_invalid(), "{bad:?}");
        assert!(!state.can_apply(), "{bad:?}");
    }
}

#[test]
fn auto_number_puts_every_page_back_at_its_position() {
    let mut state = shown();
    state.edit(1, "7".into());
    state.auto_number();
    assert_eq!(state.drafts, ["1", "2", "3"]);
    // Only the hand-numbered page changes: its entry is cleared.
    assert_eq!(state.edits(), vec![PageEdit { row: 2, page: None }]);
}

#[test]
fn clashes_and_gaps_are_reported_from_the_typed_numbers() {
    let mut state = shown();
    state.edit(2, "2".into());
    let issues = state.issues();
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue, signex_net::NumberingIssue::Clash { number: 2, .. }))
    );
    assert!(
        state.can_apply(),
        "a clash warns; it does not block, as exports do not"
    );
}
