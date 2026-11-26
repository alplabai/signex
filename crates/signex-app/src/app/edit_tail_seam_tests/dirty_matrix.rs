//! Block A tests 1 and 2: every in-crate edit entry point leaves its sheet
//! dirty, an undo back to the save point clean, a redo dirty again; the quit
//! prompt follows. The entry points a catalog id reaches are the rows of
//! `tests/regression/edit_tail_seam.rs`.

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_engine::Command;

use super::{after_replace, dirty, paper, quit_prompt_lists};
use crate::app::Signex;
use crate::app::command::queries::tests::{WIRE_A, app_with, path, uid};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::editing_commands::net_color_driver as colours;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
use crate::keymap::AppCommandId;

type Step = fn(&mut Signex, &Path);

/// One entry point: how to build its app, edit, undo and redo.
struct Row {
    name: &'static str,
    setup: fn(&tempfile::TempDir) -> (Signex, PathBuf),
    edit: Step,
    undo: Step,
    redo: Step,
    /// Also read the answer off the real quit prompt.
    asks_on_quit: bool,
}

fn one_sheet(_dir: &tempfile::TempDir) -> (Signex, PathBuf) {
    (app_with(&[("a.snxsch", 100)]), path("a.snxsch"))
}

fn gateway_undo(app: &mut Signex, _: &Path) {
    assert!(app.apply_engine_undo(true), "the gateway undo steps");
}

fn gateway_redo(app: &mut Signex, _: &Path) {
    assert!(app.apply_engine_redo(true), "the gateway redo steps");
}

fn router_undo(app: &mut Signex, _: &Path) {
    app.handle_undo_requested();
}

fn router_redo(app: &mut Signex, _: &Path) {
    app.handle_redo_requested();
}

/// The open root of a one-sheet project with two unnumbered parts, as the
/// active tab, annotated as one project-wide undo group.
fn annotated_root(dir: &tempfile::TempDir) -> (Signex, PathBuf) {
    let root = dir.path().join("a_root.snxsch");
    let mut sheet = sheet_with_net("R?", "NET", &[]);
    let second = signex_types::schematic::Symbol {
        uuid: uuid::Uuid::now_v7(),
        position: signex_types::schematic::Point::new(0.0, 20.0),
        ..sheet.symbols[0].clone()
    };
    sheet.symbols.push(second);
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a_root.snxsch"]);
    open_with(&mut app.document_state, &root, sheet);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "a_root.snxsch".into(),
        path: root.clone(),
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(root.clone());
    app.document_state.mark_document_clean(&root);
    app.ui_state.annotate_undo = crate::annotate_undo::AnnotateUndo::Project;
    (app, root)
}

fn annotate(app: &mut Signex, _: &Path) {
    let task = app.handle_annotate(signex_engine::AnnotateMode::ResetAndRenumber);
    crate::app::path_ops::settle_task(app, task);
    assert_eq!(
        app.document_state.undo_groups.len(),
        1,
        "the annotate is one project-wide group"
    );
}

/// Both sheets of the project open as tabs, `top` active: no sheet is parked,
/// so the undo drops none and the redo is not refused (a parked one is
/// `a_grouped_undo_takes_a_parked_sheet_back_to_disk_clean_and_drops_its_engine`).
fn project_with_both_sheets_open(dir: &tempfile::TempDir) -> (Signex, PathBuf) {
    let (mut app, top, child) =
        crate::app::handlers::menu::export::tests::current_placement::placed_twice_on_disk(dir);
    assert!(app.open_or_focus_sheet_path(child));
    assert!(app.open_or_focus_sheet_path(top.clone()));
    (app, top)
}

const ROWS: [Row; 6] = [
    Row {
        name: "gateway single",
        setup: one_sheet,
        edit: |app, _| assert!(app.apply_engine_command(paper("A3"), true, true)),
        undo: gateway_undo,
        redo: gateway_redo,
        asks_on_quit: true,
    },
    Row {
        name: "gateway batch",
        setup: one_sheet,
        edit: |app, _| {
            let font = Command::SetDocumentFontFamily {
                font_family: Some("Inter".to_string()),
            };
            assert!(app.apply_engine_commands(vec![paper("A3"), font], true, true));
        },
        undo: gateway_undo,
        redo: gateway_redo,
        asks_on_quit: false,
    },
    Row {
        name: "active undo and redo (the Edit menu route)",
        setup: one_sheet,
        edit: |app, _| assert!(app.apply_engine_command(paper("A3"), true, true)),
        undo: router_undo,
        redo: router_redo,
        asks_on_quit: false,
    },
    Row {
        name: "colour on the active sheet (display-only)",
        setup: one_sheet,
        edit: |app, path| {
            let wire = colours::wire_of(app, path);
            colours::paint(app, &[wire], colours::color(4));
        },
        undo: router_undo,
        redo: router_redo,
        asks_on_quit: false,
    },
    Row {
        name: "grouped undo and redo through annotate",
        setup: annotated_root,
        edit: annotate,
        undo: router_undo,
        redo: router_redo,
        asks_on_quit: false,
    },
    Row {
        name: "project Replace All (the active sheet)",
        setup: project_with_both_sheets_open,
        edit: |app, _| super::replace_in_project(app, "net", "X"),
        undo: router_undo,
        redo: router_redo,
        asks_on_quit: true,
    },
];

/// Test 1, the in-crate rows of the dirty and close-confirm matrix.
#[test]
fn every_edit_entry_point_is_dirty_after_the_edit_clean_after_undo_and_dirty_after_redo() {
    for row in &ROWS {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, sheet) = (row.setup)(&dir);
        assert!(!dirty(&app, &sheet), "{}: starts clean", row.name);
        (row.edit)(&mut app, &sheet);
        assert!(dirty(&app, &sheet), "{}: dirty after the edit", row.name);
        if row.asks_on_quit {
            assert!(
                quit_prompt_lists(&mut app, &sheet),
                "{}: quit asks after the edit",
                row.name
            );
        }
        (row.undo)(&mut app, &sheet);
        assert!(!dirty(&app, &sheet), "{}: clean after undo", row.name);
        if row.asks_on_quit {
            assert!(
                !quit_prompt_lists(&mut app, &sheet),
                "{}: quit does not ask after undo",
                row.name
            );
        }
        (row.redo)(&mut app, &sheet);
        assert!(dirty(&app, &sheet), "{}: dirty after redo", row.name);
    }
}

/// Test 1, the unrecorded gateway row: load-time maintenance lands with no
/// undo step, so there is nothing to undo; what it leaves is pinned here.
#[test]
fn an_unrecorded_gateway_edit_leaves_the_sheet_dirty_with_nothing_to_undo() {
    let (mut app, sheet) = one_sheet(&tempfile::tempdir().unwrap());
    let (outcome, refused) = app.apply_engine_command_unrecorded(paper("A3"));
    assert_eq!(
        outcome,
        crate::app::mutation_gateway::MutationOutcome::Changed,
        "{refused:?}"
    );
    assert!(dirty(&app, &sheet));
    assert!(!app.apply_engine_undo(true), "no undo step was recorded");
    assert!(dirty(&app, &sheet));
}

fn run_operands(app: &mut Signex, name: &str, operands: &[String]) -> CommandOutcome {
    let command = AppCommandId::new(name).unwrap();
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).unwrap();
    app.run_operands_settled(&command, &args).0
}

/// An agent's one-junction edit list on the sheet at `sheet`; its group.
fn agent_junction(app: &mut Signex, sheet: &Path) -> u64 {
    let revision = app.document_state.engines[sheet].revision();
    let ran = run_operands(
        app,
        "apply_edits",
        &[
            format!("document={}", sheet.display()),
            format!("expect_revision={revision}"),
            format!(
                "edits={}",
                json!([{"id": "place_junction", "args": {"at": [50.8, 50.8]}}])
            ),
        ],
    );
    let CommandOutcome::Answered(answer) = ran else {
        panic!("apply_edits was not answered: {ran:?}");
    };
    answer["group"].as_u64().expect("a group")
}

/// Test 1, the `revert` row, in-crate because the adapter does not hand back
/// the answer that names the group: clean after the revert, and no redo.
#[test]
fn an_agents_revert_leaves_the_sheet_clean_with_nothing_to_redo() {
    let (mut app, sheet) = one_sheet(&tempfile::tempdir().unwrap());
    let group = agent_junction(&mut app, &sheet);
    assert!(dirty(&app, &sheet), "dirty after the edit");
    let reverted = run_operands(
        &mut app,
        "revert",
        &[
            format!("document={}", sheet.display()),
            format!("group={group}"),
        ],
    );
    assert!(
        matches!(reverted, CommandOutcome::Answered(_)),
        "{reverted:?}"
    );
    assert!(!dirty(&app, &sheet), "clean after the revert");
    assert!(!app.apply_engine_redo(true), "a revert leaves no redo");
}

// ---- Test 2: an undo to the save point while a save is in flight.

/// Start a background write of the sheet at `path`, left unfinished.
fn save_in_flight(app: &mut Signex, path: &Path) {
    let _job = app
        .document_state
        .document_saves
        .begin(path.to_path_buf(), b"in flight".to_vec());
    assert!(app.document_state.document_saves.is_saving(path));
}

#[test]
fn an_active_undo_to_the_save_point_during_a_save_stays_dirty() {
    let (mut app, sheet) = one_sheet(&tempfile::tempdir().unwrap());
    assert!(app.apply_engine_command(paper("A3"), true, true));
    save_in_flight(&mut app, &sheet);
    assert!(app.apply_engine_undo(true));
    assert!(
        app.document_state.engines[&sheet].is_at_save_point(),
        "back at the save point"
    );
    assert!(dirty(&app, &sheet), "the write in flight holds it dirty");
}

#[test]
fn an_undo_by_path_to_the_save_point_during_a_save_stays_dirty() {
    let mut app = app_with(&[("a.snxsch", 100), ("b.snxsch", 101)]);
    let a = path("a.snxsch");
    let mut colors = std::collections::HashMap::new();
    colors.insert(uid(WIRE_A), colours::color(5));
    assert_eq!(
        app.execute_on_sheet(&a, Command::SetNetColors { colors }),
        Ok(true)
    );
    save_in_flight(&mut app, &a);
    assert_eq!(app.step_sheet_on(&a, true), Ok(true));
    assert!(app.document_state.engines[&a].is_at_save_point());
    assert!(dirty(&app, &a), "the write in flight holds it dirty");
}

#[test]
fn a_grouped_undo_to_the_save_point_during_a_save_stays_dirty() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let a = path("a.snxsch");
    agent_junction(&mut app, &a);
    assert!(
        app.document_state.engines[&a].next_undo_group().is_some(),
        "the agent's step is a group"
    );
    save_in_flight(&mut app, &a);
    app.handle_undo_requested();
    assert!(app.document_state.engines[&a].is_at_save_point());
    assert!(dirty(&app, &a), "the write in flight holds it dirty");
}

/// Test 2 for a non-active member of a group: the parked sheet a replace
/// loaded is neither dropped nor marked clean while its save runs.
#[test]
fn a_grouped_undo_keeps_a_parked_sheet_dirty_and_loaded_while_its_save_runs() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = after_replace(&dir);
    save_in_flight(&mut app, &child);
    app.handle_undo_requested();
    assert!(
        app.document_state.engines.contains_key(&child),
        "not dropped while its save runs"
    );
    assert!(dirty(&app, &child), "the write in flight holds it dirty");
}
