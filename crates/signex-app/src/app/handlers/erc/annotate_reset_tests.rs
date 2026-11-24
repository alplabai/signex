//! The project-wide numbering runs through `Signex::update` (#2760): unopened
//! sheets are parked unsaved, so undo restores them and the files never change.

use std::path::Path;

use signex_engine::AnnotateMode::{self, Incremental, ResetAndRenumber, ResetOnly};

use super::annotate_tests::{child_path, child_refs, logged_since, notice_detail, project};
use super::annotate_tests::{root_refs, unit};
use crate::annotate_undo::AnnotateUndo;
use crate::app::path_ops::settle_task;
use crate::app::{AnnotateMsg, EditMsg, Message, Signex, TabInfo, TabKind};

fn run(app: &mut Signex, mode: AnnotateMode) {
    let task = app.update(Message::Annotate(AnnotateMsg::Run(mode)));
    settle_task(app, task);
}

/// Ctrl+Z steps the sheet the window's tab shows.
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

fn parked_refs(app: &Signex, path: &Path) -> Option<Vec<String>> {
    let engine = app.document_state.engines.get(path)?;
    Some(
        (engine.document().symbols.iter())
            .map(|symbol| symbol.reference.clone())
            .collect(),
    )
}

/// Open root `root`, unopened child `child`: `act` changes both to `done`, the
/// file stays, and one undo restores both.
fn one_undo_restores_both(root: &str, child: &str, done: (&str, &str), act: impl Fn(&mut Signex)) {
    let (mut app, dir) = project(vec![unit(root, 1, 0.0)], vec![unit(child, 1, 0.0)]);
    app.ui_state.annotate_undo = AnnotateUndo::Project;
    show_root_tab(&mut app, &dir);
    let child_file = child_path(&dir);
    let on_disk = std::fs::read(&child_file).expect("child");

    act(&mut app);

    let after_run = (
        root_refs(&app, &dir),
        parked_refs(&app, &child_file),
        app.document_state.is_document_dirty(&child_file),
        std::fs::read(&child_file).expect("child") == on_disk,
    );
    let group_parked =
        (app.document_state.undo_groups.values()).all(|group| group.parked == [child_file.clone()]);

    let undo = app.update(Message::Edit(EditMsg::Undo));
    settle_task(&mut app, undo);

    let after_undo = (
        root_refs(&app, &dir),
        parked_refs(&app, &child_file),
        app.document_state.is_document_dirty(&child_file),
        std::fs::read(&child_file).expect("child") == on_disk,
        child_refs(&dir),
    );
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(
        after_run,
        (
            vec![done.0.to_string()],
            Some(vec![done.1.to_string()]),
            true,
            true,
        ),
        "changed in memory, unsaved, file untouched"
    );
    assert!(group_parked, "the undo group holds the parked sheet");
    assert_eq!(
        after_undo,
        (
            vec![root.to_string()],
            None,
            false,
            true,
            vec![child.to_string()],
        ),
        "one undo restores both; the child reads from its file again"
    );
}

#[test]
fn one_undo_restores_the_open_and_the_unopened_sheet_and_the_file_never_changes() {
    for (mode, reset_root, reset_child) in [(ResetOnly, "R?", "R?"), (ResetAndRenumber, "R1", "R2")]
    {
        one_undo_restores_both("R5", "R7", (reset_root, reset_child), |app| {
            run(app, mode);
        });
    }
}

#[test]
fn plain_annotate_leaves_the_file_and_one_undo_restores_both_sheets() {
    one_undo_restores_both("R?", "C?", ("R1", "C1"), |app| run(app, Incremental));
}

#[test]
fn reset_duplicates_leaves_the_file_and_one_undo_restores_both_sheets() {
    one_undo_restores_both("R1", "R1", ("R?", "R?"), |app| {
        let task = app.handle_reset_duplicate_designators();
        settle_task(app, task);
    });
}

/// A headless run holds `edit`, not `save`: the command must leave the file
/// alone and name the sheet as unsaved for the run's closing report.
#[test]
fn the_force_annotate_command_leaves_the_sheet_unsaved_for_a_headless_run() {
    let (mut app, dir) = project(vec![unit("R5", 1, 0.0)], vec![unit("R7", 1, 0.0)]);
    show_root_tab(&mut app, &dir);
    let child = child_path(&dir);
    let on_disk = std::fs::read(&child).expect("child");
    let id = crate::keymap::AppCommandId::new("force_annotate_all_schematics").unwrap();

    let (_outcome, task) =
        app.run_headless_command_in_test(&id, crate::app::command::CommandArgs::none());
    settle_task(&mut app, task);

    let unsaved = app.document_state.dirty_document_paths().contains(&child);
    let parked = parked_refs(&app, &child);
    let unchanged = std::fs::read(&child).expect("child") == on_disk;
    std::fs::remove_dir_all(&dir).ok();
    assert!(unsaved, "the run's end names the sheet as unsaved");
    assert_eq!(parked, Some(vec!["R2".to_string()]));
    assert!(unchanged, "`edit` never writes a file");
}

#[test]
fn a_per_sheet_reset_parks_the_unopened_sheet_with_its_own_undo() {
    let (mut app, dir) = project(vec![unit("R5", 1, 0.0)], vec![unit("R7", 1, 0.0)]);
    app.ui_state.annotate_undo = AnnotateUndo::PerSheet;
    let child = child_path(&dir);
    let on_disk = std::fs::read(&child).expect("child");

    run(&mut app, ResetOnly);

    let parked = parked_refs(&app, &child);
    let unchanged = std::fs::read(&child).expect("child") == on_disk;
    let engine = app.document_state.engines.get_mut(&child).expect("parked");
    let undone = engine.undo().expect("undo").is_some();
    let restored = parked_refs(&app, &child);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(parked, Some(vec!["R?".to_string()]));
    assert!(unchanged, "the file waits for the user's save");
    assert!(undone);
    assert_eq!(restored, Some(vec!["R7".to_string()]));
}

#[test]
fn a_sheet_that_fails_while_resetting_rolls_the_whole_run_back() {
    let _ = crate::diagnostics::init_logging();
    for undo in [AnnotateUndo::Project, AnnotateUndo::PerSheet] {
        // The locked R4294967295 seeds the counter, so numbering the child's
        // R7 overflows after the root's C5 has already changed.
        let locked = unit("R4294967295", 1, 10.0);
        let locked_id = locked.uuid;
        let (mut app, dir) = project(vec![unit("C5", 1, 0.0), locked], vec![unit("R7", 1, 0.0)]);
        app.ui_state.annotate_undo = undo;
        app.ui_state.annotate_locked.insert(locked_id);
        let child = child_path(&dir);
        let on_disk = std::fs::read(&child).expect("child");
        let since = crate::diagnostics::newest_entry_id();

        run(&mut app, ResetAndRenumber);

        let root = root_refs(&app, &dir);
        let engine = &app.document_state.engines[&dir.join("a_root.snxsch")];
        let no_history = !engine.can_undo() && !engine.can_redo();
        let parked = parked_refs(&app, &child);
        let unchanged = std::fs::read(&child).expect("child") == on_disk;
        let said = logged_since(since, "the whole run was undone");
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(root, ["C5", "R4294967295"], "{undo:?}");
        assert!(no_history, "{undo:?}: no half run left to redo");
        assert_eq!(parked, None, "{undo:?}: the parked sheet is forgotten");
        assert!(unchanged, "{undo:?}");
        assert!(said, "{undo:?}: the Messages panel says what happened");
    }
}

#[test]
fn reset_all_with_a_sheet_that_cannot_be_read_changes_nothing_and_names_it() {
    let (mut app, dir) = project(vec![unit("R5", 1, 0.0)], vec![unit("R7", 1, 0.0)]);
    let child = child_path(&dir);
    std::fs::write(&child, "not a schematic").expect("corrupt the child");
    let on_disk = std::fs::read(&child).expect("child");

    run(&mut app, ResetOnly);

    let root = root_refs(&app, &dir);
    let parked = parked_refs(&app, &child);
    let unchanged = std::fs::read(&child).expect("child") == on_disk;
    let notice = notice_detail(&app);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(root, ["R5"], "no partial reset");
    assert_eq!(parked, None);
    assert!(unchanged);
    assert!(notice.contains("z_child.snxsch"), "{notice}");
    assert!(notice.contains("Nothing was changed"), "{notice}");
}
