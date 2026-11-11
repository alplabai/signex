//! The commit seam of a cross-sheet group (#2178): a commit that fails after
//! another sheet's landed takes that one back.

use signex_engine::Command;
use signex_types::schematic::{NoConnect, Point};

use iced::Task;

use super::apply::Prepared;
use super::group_run::{commit_all, run_group};
use crate::app::Signex;
use crate::app::command::agent_edit_cross_sheet_tests::{kid, top, two_sheets};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

/// An edit that spoils every open transaction, as a command that fails the
/// engine's transaction and still says it ran.
fn spoil(app: &mut Signex, _: &CommandArgs) -> (CommandOutcome, Task<crate::app::Message>) {
    for engine in app.document_state.engines.values_mut() {
        engine.fail_transaction();
    }
    (CommandOutcome::Ran, Task::none())
}

fn spoiling_edit(path: &std::path::Path) -> Prepared {
    Prepared {
        id: AppCommandId::new("place_junction").unwrap(),
        args: CommandArgs::none(),
        handler: spoil,
        path: path.to_path_buf(),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

/// One sheet: the commit itself refuses, with the engine's own words, as it
/// always did; nothing is left open or changed.
#[test]
fn a_one_sheet_call_whose_transaction_failed_is_refused_with_the_engines_words() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let top = top(&dir);
    let before = app.document_state.engines[&top].revision();
    let ran = run_group(
        &mut app,
        &top,
        std::slice::from_ref(&top),
        &[spoiling_edit(&top)],
        "Agent: x".into(),
    );
    let reason = refusal(ran.0);
    let engine_says = signex_engine::EngineError::TransactionFailed.to_string();
    assert_eq!(reason, format!("{engine_says}. Nothing was applied"));
    assert!(!app.document_state.engines[&top].in_transaction());
    assert_eq!(app.document_state.engines[&top].revision(), before);
}

/// Several sheets: no engine commits until every one can, so none landed.
#[test]
fn a_cross_sheet_call_with_a_spoiled_transaction_commits_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let before = (
        app.document_state.engines[&top].revision(),
        app.document_state.engines[&kid].revision(),
    );
    let ran = run_group(
        &mut app,
        &top,
        &[top.clone(), kid.clone()],
        &[spoiling_edit(&top)],
        "Agent: x".into(),
    );
    assert_eq!(
        refusal(ran.0),
        "a sheet's transaction cannot commit. Nothing was applied"
    );
    for path in [&top, &kid] {
        let engine = &app.document_state.engines[path];
        assert!(!engine.in_transaction() && !engine.can_undo());
    }
    assert_eq!(
        (
            app.document_state.engines[&top].revision(),
            app.document_state.engines[&kid].revision()
        ),
        before
    );
}

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
