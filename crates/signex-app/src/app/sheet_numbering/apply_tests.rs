//! Apply's guards that need a read the public API cannot build: a closed
//! parent sheet missing from the read (the part-way failure and its rollback)
//! and a closed parent written by a newer Signex. Driven through
//! `Signex::update`.
//!
//! Two parents, in Apply's path order: `/w/a.snxsch` (open, places X) then
//! `/w/z.snxsch` (closed, places Y).

use std::path::PathBuf;
use std::sync::Arc;

use signex_types::schematic::{ChildSheet, FillType, InstancePath, Point, SchematicSheet};
use uuid::Uuid;

use super::SheetNumberingMsg;
use super::read::{NumberingRead, NumberingRow, RowPlacement};
use super::state::{NumberingProblem, SheetNumberingState};
use crate::app::{Message, Signex};

const X: u128 = 0x10;
const Y: u128 = 0x20;

fn open_parent() -> PathBuf {
    PathBuf::from("/w/a.snxsch")
}

fn closed_parent() -> PathBuf {
    PathBuf::from("/w/z.snxsch")
}

fn symbol(id: u128, filename: &str) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::from_u128(id),
        name: filename.trim_end_matches(".snxsch").to_string(),
        filename: filename.to_string(),
        position: Point::new(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn placed(position: usize, parent: PathBuf, symbol: u128) -> NumberingRow {
    NumberingRow {
        name: format!("page{position}"),
        sheet: format!("page{position}.snxsch"),
        depth: 1,
        number: position,
        position,
        manual: false,
        placement: Some(RowPlacement {
            parent,
            parent_path: InstancePath::root(),
            symbol: Uuid::from_u128(symbol),
        }),
    }
}

/// The open parent as an engine, and the dialog showing a read with both
/// rows renumbered; `loaded` is what the read kept of the closed parent.
fn app_with_dialog(loaded: Option<SchematicSheet>) -> Signex {
    let (mut app, _task) = Signex::new();
    let sheet = SchematicSheet {
        child_sheets: vec![symbol(X, "x.snxsch")],
        ..Default::default()
    };
    let engine = signex_engine::Engine::new_with_path(sheet, Some(open_parent())).expect("engine");
    app.document_state.engines.insert(open_parent(), engine);
    let read = NumberingRead {
        rows: vec![placed(1, open_parent(), X), placed(2, closed_parent(), Y)],
        loaded: loaded
            .map(|sheet| [(closed_parent(), sheet)].into_iter().collect())
            .unwrap_or_default(),
        unreadable: Vec::new(),
    };
    let mut state = SheetNumberingState::reading(
        1,
        app.document_state.documents_epoch,
        app.child_sheet_signature(),
        "p".into(),
    );
    state.show(Arc::new(read));
    state.edit(0, "7".into());
    state.edit(1, "8".into());
    app.ui_state.sheet_numbering = Some(state);
    let _ = app.update(Message::from(SheetNumberingMsg::ApplyRequested));
    app
}

fn problem(app: &Signex) -> Option<NumberingProblem> {
    app.ui_state
        .sheet_numbering
        .as_ref()
        .and_then(|state| state.problem().cloned())
}

fn open_parent_untouched(app: &Signex) -> bool {
    let engine = &app.document_state.engines[&open_parent()];
    engine.document().child_sheets[0].instances.is_empty() && engine.next_undo_group().is_none()
}

#[test]
fn a_failure_part_way_puts_the_sheets_already_changed_back() {
    // The closed parent is missing from the read, so it cannot be loaded
    // after the open one was already renumbered.
    let app = app_with_dialog(None);
    assert_eq!(
        problem(&app),
        Some(NumberingProblem::NotLoaded(closed_parent()))
    );
    assert!(
        open_parent_untouched(&app),
        "the open sheet's change and its undo entry are rolled back"
    );
    assert!(!app.document_state.engines.contains_key(&closed_parent()));
}

#[test]
fn a_closed_parent_from_a_newer_signex_is_refused_before_anything_changes() {
    let mut newer = SchematicSheet {
        child_sheets: vec![symbol(Y, "y.snxsch")],
        ..Default::default()
    };
    newer.unknown.newer_minor_source = Some("1.99".into());
    let app = app_with_dialog(Some(newer));
    assert_eq!(
        problem(&app),
        Some(NumberingProblem::ReadOnly(closed_parent())),
        "the dialog names the read-only file"
    );
    assert!(open_parent_untouched(&app));
    assert!(!app.document_state.engines.contains_key(&closed_parent()));
}
