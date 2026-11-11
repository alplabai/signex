//! The commit seam of a cross-sheet group (#2178): a commit that fails after
//! another sheet's landed takes that one back.

use signex_engine::Command;
use signex_types::schematic::{NoConnect, Point};

use super::group_run::commit_all;
use crate::app::command::agent_edit_cross_sheet_tests::{kid, top, two_sheets};

fn place() -> Command {
    Command::PlaceNoConnect {
        no_connect: NoConnect {
            uuid: uuid::Uuid::now_v7(),
            position: Point::new(40.0, 40.0),
        },
    }
}

#[test]
fn a_commit_that_fails_on_the_second_sheet_takes_the_first_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let sheets = vec![top.clone(), kid.clone()];
    let held = |app: &crate::app::Signex, path: &std::path::Path| {
        let engine = &app.document_state.engines[path];
        (
            engine.document().no_connects.len(),
            engine.can_undo(),
            engine.can_redo(),
        )
    };
    let before = (held(&app, &top), held(&app, &kid));
    for path in &sheets {
        let engine = app.document_state.engines.get_mut(path).unwrap();
        engine.begin_transaction().unwrap();
        engine.execute(place()).unwrap();
    }
    // The second sheet's transaction is spoiled, as a failing command does.
    app.document_state
        .engines
        .get_mut(&kid)
        .unwrap()
        .fail_transaction();
    let group = app.mint_undo_group();
    let failed = commit_all(&mut app, &sheets, group, |engine, _, group| {
        engine.commit_transaction(Some(group))
    });
    assert!(failed.is_err(), "the second commit failed");
    // The first sheet landed its entry and had it taken back: no entry, and
    // nothing on the redo stack; the second rolled back.
    assert_eq!((held(&app, &top), held(&app, &kid)), before);
    for path in &sheets {
        assert!(!app.document_state.engines[path].in_transaction());
    }
    assert!(!app.document_state.is_document_dirty(&top));
}
