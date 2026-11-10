//! Apply's guards that need a read the public API cannot build: a closed
//! parent sheet missing from the read (the part-way failure and its
//! rollback), a closed parent written by a newer Signex, and a closed parent
//! rewritten on disk after the read. Driven through `Signex::update`, with
//! the re-stat task run to completion.
//!
//! Two parents in a temp folder, in Apply's path order: `a.snxsch` (open,
//! places X) then `z.snxsch` (closed, places Y; its file exists so it can be
//! stated).

use std::path::PathBuf;
use std::sync::Arc;

use signex_types::schematic::{ChildSheet, FillType, InstancePath, Point, SchematicSheet};
use uuid::Uuid;

use super::SheetNumberingMsg;
use super::read::{NumberingRead, NumberingRow, RowPlacement};
use super::state::{NumberingProblem, SheetNumberingState};
use crate::app::{Message, Signex};
use crate::library::closed_sheets::FileStamp;

const X: u128 = 0x10;
const Y: u128 = 0x20;

/// The closed parent's file content when the read took it.
const AS_READ: &str = "as read";

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

struct Rig {
    app: Signex,
    open_parent: PathBuf,
    closed_parent: PathBuf,
    _dir: tempfile::TempDir,
}

/// The open parent as an engine, the closed parent's file on disk, and the
/// dialog showing a read with both rows renumbered. `loaded` is what the
/// read kept of the closed parent; its stamp is the file as written now.
fn rig(loaded: Option<SchematicSheet>) -> Rig {
    let dir = tempfile::tempdir().expect("tempdir");
    let open_parent = dir.path().join("a.snxsch");
    let closed_parent = dir.path().join("z.snxsch");
    std::fs::write(&closed_parent, AS_READ).expect("closed parent on disk");
    let (mut app, _task) = Signex::new();
    let sheet = SchematicSheet {
        child_sheets: vec![symbol(X, "x.snxsch")],
        ..Default::default()
    };
    let engine =
        signex_engine::Engine::new_with_path(sheet, Some(open_parent.clone())).expect("engine");
    app.document_state
        .engines
        .insert(open_parent.clone(), engine);
    let read = NumberingRead {
        rows: vec![
            placed(1, open_parent.clone(), X),
            placed(2, closed_parent.clone(), Y),
        ],
        loaded: loaded
            .map(|sheet| [(closed_parent.clone(), sheet)].into_iter().collect())
            .unwrap_or_default(),
        stamps: [(closed_parent.clone(), FileStamp::of(&closed_parent))]
            .into_iter()
            .collect(),
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
    Rig {
        app,
        open_parent,
        closed_parent,
        _dir: dir,
    }
}

fn closed_parent_sheet() -> SchematicSheet {
    SchematicSheet {
        child_sheets: vec![symbol(Y, "y.snxsch")],
        ..Default::default()
    }
}

/// Press Apply and run the re-stat it starts.
fn apply(rig: &mut Rig) {
    crate::task_driver::TaskDriver::new()
        .send(
            &mut rig.app,
            Message::from(SheetNumberingMsg::ApplyRequested),
        )
        .expect("the apply settled");
}

fn problem(rig: &Rig) -> Option<NumberingProblem> {
    rig.app
        .ui_state
        .sheet_numbering
        .as_ref()
        .and_then(|state| state.problem().cloned())
}

/// Nothing changed: the open parent has no entry and no undo step, and the
/// closed parent was not loaded.
fn assert_untouched(rig: &Rig) {
    let engine = &rig.app.document_state.engines[&rig.open_parent];
    assert!(engine.document().child_sheets[0].instances.is_empty());
    assert!(engine.next_undo_group().is_none(), "no undo step left");
    assert!(
        !rig.app
            .document_state
            .engines
            .contains_key(&rig.closed_parent),
        "the closed parent was not loaded"
    );
}

#[test]
fn a_failure_part_way_puts_the_sheets_already_changed_back() {
    // The closed parent is missing from the read, so it cannot be loaded
    // after the open one was already renumbered.
    let mut rig = rig(None);
    apply(&mut rig);
    assert_eq!(
        problem(&rig),
        Some(NumberingProblem::NotLoaded(rig.closed_parent.clone()))
    );
    assert_untouched(&rig);
}

#[test]
fn a_closed_parent_from_a_newer_signex_is_refused_before_anything_changes() {
    let mut newer = closed_parent_sheet();
    newer.unknown.newer_minor_source = Some("1.99".into());
    let mut rig = rig(Some(newer));
    apply(&mut rig);
    assert_eq!(
        problem(&rig),
        Some(NumberingProblem::ReadOnly(rig.closed_parent.clone())),
        "the dialog names the read-only file"
    );
    assert_untouched(&rig);
}

#[test]
fn a_closed_parent_rewritten_after_the_read_is_refused_before_anything_changes() {
    let mut rig = rig(Some(closed_parent_sheet()));
    // Something else saves the file between the read and Apply (a longer
    // file, so the stamp differs whatever the timestamp resolution).
    std::fs::write(&rig.closed_parent, "written by another program").expect("rewrite");
    apply(&mut rig);
    assert_eq!(
        problem(&rig),
        Some(NumberingProblem::DiskChanged(rig.closed_parent.clone())),
        "the dialog says the file changed on disk"
    );
    assert_untouched(&rig);
    assert_eq!(
        std::fs::read_to_string(&rig.closed_parent).expect("file"),
        "written by another program",
        "the other program's write is left alone"
    );
}

#[test]
fn an_unchanged_closed_parent_is_renumbered_in_memory() {
    let mut rig = rig(Some(closed_parent_sheet()));
    apply(&mut rig);
    assert!(
        rig.app.ui_state.sheet_numbering.is_none(),
        "Apply closed it"
    );
    let closed = &rig.app.document_state.engines[&rig.closed_parent];
    assert_eq!(closed.document().child_sheets[0].instances[0].page, "8");
    assert_eq!(
        std::fs::read_to_string(&rig.closed_parent).expect("file"),
        AS_READ,
        "nothing is written until the user saves"
    );
}
