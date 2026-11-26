//! #2470: `annotate_schematic_quietly document=..` and
//! `force_annotate_all_schematics document=..` run the menu's numbering run
//! on the named sheet's project and answer with every change.

use std::path::Path;

use signex_engine::AnnotateMode::{Incremental, ResetAndRenumber};

use super::annotate_tests::{child_path, project, root_refs, unit};
use crate::annotate_undo::AnnotateUndo;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::path_ops::settle_task;
use crate::app::{AnnotateMsg, Message, Signex, TabInfo, TabKind};
use crate::keymap::AppCommandId;

fn show_root_tab(app: &mut Signex, dir: &Path) {
    app.document_state.tabs.push(TabInfo {
        title: "a_root.snxsch".into(),
        path: dir.join("a_root.snxsch"),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

/// Run `id` headless on the sheet `sheet`, its task chain to the end.
fn run_named(app: &mut Signex, id: &str, sheet: &Path) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a command id");
    let document = format!("document={}", sheet.display());
    let args = CommandArgs::parse_operands(&command, &[document.as_str()]).expect("operands parse");
    let (outcome, task) = app.run_headless_command_in_test(&command, args);
    settle_task(app, task);
    outcome
}

/// The designators of the sheet an engine holds at `path` (a parked one).
fn held_refs(app: &Signex, path: &Path) -> Vec<String> {
    app.document_state
        .engines
        .get(path)
        .map(|engine| {
            (engine.document().symbols.iter())
                .map(|symbol| symbol.reference.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Each change of the answer as (sheet file name, old, new).
fn changes(outcome: &CommandOutcome) -> Vec<(String, String, String)> {
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("the run answers with its changes: {outcome:?}");
    };
    let text = |value: &serde_json::Value| value.as_str().expect("text").to_string();
    answer["changes"]
        .as_array()
        .expect("a change list")
        .iter()
        .map(|change| {
            let sheet = text(&change["sheet"]);
            let name = Path::new(&sheet)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            (name, text(&change["old"]), text(&change["new"]))
        })
        .collect()
}

fn triple(sheet: &str, old: &str, new: &str) -> (String, String, String) {
    (sheet.to_string(), old.to_string(), new.to_string())
}

#[test]
fn the_quiet_annotate_answers_each_change_as_one_undo_group() {
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![unit("C?", 1, 0.0)]);
    app.ui_state.annotate_undo = AnnotateUndo::Project;
    show_root_tab(&mut app, &dir);
    let child = child_path(&dir);
    let on_disk = std::fs::read(&child).expect("child");

    let outcome = run_named(
        &mut app,
        "annotate_schematic_quietly",
        &dir.join("a_root.snxsch"),
    );

    let listed = changes(&outcome);
    let refs = (root_refs(&app, &dir), held_refs(&app, &child));
    let unchanged = std::fs::read(&child).expect("child") == on_disk;
    let parked: Vec<Vec<std::path::PathBuf>> = (app.document_state.undo_groups.values())
        .map(|group| group.parked.clone())
        .collect();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        listed,
        [
            triple("a_root.snxsch", "R?", "R1"),
            triple("z_child.snxsch", "C?", "C1"),
        ]
    );
    assert_eq!(refs, (vec!["R1".to_string()], vec!["C1".to_string()]));
    assert!(unchanged, "`edit` never writes a file");
    assert_eq!(parked, [vec![child]], "one undo step for the whole run");
}

/// The menu's run and the named run number the same project alike.
#[test]
fn the_menu_run_and_the_named_run_number_alike() {
    for (id, mode) in [
        ("annotate_schematic_quietly", Incremental),
        ("force_annotate_all_schematics", ResetAndRenumber),
    ] {
        let fixture = || {
            project(
                vec![unit("R7", 1, 0.0), unit("R?", 1, 10.0)],
                vec![unit("R?", 1, 0.0)],
            )
        };
        let (mut gui, gui_dir) = fixture();
        show_root_tab(&mut gui, &gui_dir);
        let task = gui.update(Message::Annotate(AnnotateMsg::Run(mode)));
        settle_task(&mut gui, task);
        let (mut cli, cli_dir) = fixture();
        show_root_tab(&mut cli, &cli_dir);
        let outcome = run_named(&mut cli, id, &cli_dir.join("a_root.snxsch"));

        let by_menu = (
            root_refs(&gui, &gui_dir),
            held_refs(&gui, &child_path(&gui_dir)),
        );
        let by_name = (
            root_refs(&cli, &cli_dir),
            held_refs(&cli, &child_path(&cli_dir)),
        );
        std::fs::remove_dir_all(&gui_dir).ok();
        std::fs::remove_dir_all(&cli_dir).ok();
        assert!(!changes(&outcome).is_empty(), "{id}: {outcome:?}");
        assert_eq!(by_menu, by_name, "{id}");
    }
}

/// The named sheet's project is numbered with no sheet in view.
#[test]
fn a_named_run_needs_no_sheet_in_view() {
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![]);
    show_root_tab(&mut app, &dir);
    app.document_state.active_path = None;

    let outcome = run_named(
        &mut app,
        "annotate_schematic_quietly",
        &dir.join("a_root.snxsch"),
    );

    let refs = root_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(changes(&outcome), [triple("a_root.snxsch", "R?", "R1")]);
    assert_eq!(refs, ["R1"]);
}

#[test]
fn a_named_document_that_is_not_a_sheet_is_refused() {
    let (mut app, dir) = project(vec![unit("R?", 1, 0.0)], vec![]);
    let outcome = run_named(
        &mut app,
        "annotate_schematic_quietly",
        &dir.join("nope.snxsch"),
    );
    let refs = root_refs(&app, &dir);
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert_eq!(refs, ["R?"], "nothing ran");
}
