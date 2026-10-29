//! #859 — a cancelled History restore keeps its claim until its worker
//! reports, so saves of the file stay refused while the worker can still
//! write it. These tests cancel while the worker is still pending: the task
//! is held unrun until after the "meanwhile" actions. Every fixture has its
//! own file stem, so the lines asserted in the (global) log are its own.

use std::path::{Path, PathBuf};

use super::open_read_tests::open_from_tree;
use super::restore_commit_tests::{
    collect_messages, commit_revision, edit_description, editor_name, file_name, finish_restore,
    footprint_fixture, logged, marker_editor, named_footprint_fixture, open_tab, run_task,
};
use crate::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use crate::persist::{DocumentSaveDone, SaveOutcome};

fn refused(path: &Path) -> String {
    format!(
        "A History restore of {} is still running, so the save was refused",
        file_name(path)
    )
}

/// The restoring file's tab is closed and reopened through the project tree,
/// as a user would.
fn reopen_from_tree(app: &mut Signex, path: &Path) {
    let task = open_from_tree(app, &file_name(path));
    run_task(app, task);
    assert!(app.document_state.tabs.iter().any(|t| t.path == path));
}

/// Every route that could write `path` is refused, and the file is untouched.
fn assert_saves_refused_and_file_untouched(app: &mut Signex, path: &Path, before: &str) {
    for err in [
        app.try_save_dirty_path(path).unwrap_err(),
        app.save_primitive_tab_at(path).unwrap_err(),
    ] {
        assert_eq!(err.to_string(), refused(path));
        assert!(err.to_string().len() < 157, "Messages-panel budget");
    }
    assert_eq!(std::fs::read_to_string(path).expect("read"), before);
}

fn assert_released_and_restored_on_disk(app: &Signex, path: &Path) {
    assert!(
        !app.document_state.pending_restores.contains(path),
        "claim released"
    );
    assert!(app.refuse_save_while_restoring(path).is_ok());
    assert!(
        std::fs::read_to_string(path)
            .expect("read")
            .contains("name = \"old\""),
        "the worker's write is the final content"
    );
}

fn close_all(app: &mut Signex) {
    let _ = app.update(Message::ContextMenu(crate::app::ContextMenuMsg::TabAction(
        crate::app::TabContextAction::CloseAll,
    )));
}

#[test]
fn closing_the_tab_mid_restore_keeps_saves_refused_and_a_clean_reopen_gets_the_restore() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, mut app) = footprint_fixture();
    let task = app.handle_history_restore_clicked(&sha);
    let before = std::fs::read_to_string(&path).expect("read");

    let _ = app.close_tab_now(0);
    assert!(
        app.document_state.pending_restores.contains(&path),
        "claim kept"
    );
    reopen_from_tree(&mut app, &path);
    assert_saves_refused_and_file_untouched(&mut app, &path, &before);

    finish_restore(&mut app, collect_messages(task));
    assert_released_and_restored_on_disk(&app, &path);
    assert_eq!(
        editor_name(&app, &path),
        "old",
        "clean tab follows the file"
    );
    assert_eq!(app.document_state.pending_git_commits.len(), 1);
    assert!(logged(&format!(
        "A cancelled restore of {} to {sha} finished after writing; saves are allowed again",
        file_name(&path)
    )));
    assert!(app.save_primitive_tab_at(&path).is_ok(), "saves work again");
}

#[test]
fn a_reopened_tab_with_edits_keeps_them_and_the_message_says_what_a_save_does() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, mut app) = footprint_fixture();
    let task = app.handle_history_restore_clicked(&sha);
    let _ = app.close_tab_now(0);
    reopen_from_tree(&mut app, &path);
    edit_description(&mut app, "mine");

    finish_restore(&mut app, collect_messages(task));
    assert_released_and_restored_on_disk(&app, &path);
    assert_eq!(editor_name(&app, &path), "new", "edits kept, not replaced");
    assert!(app.document_state.pending_git_commits.is_empty());
    let line = format!(
        "{} has unsaved edits but its file is now at {sha}; saving overwrites that restore",
        file_name(&path)
    );
    assert!(logged(&line));
    assert!(!logged(&format!(
        "{} was restored to {sha} on disk, but its tab or project was closed",
        path.display()
    )));
}

#[test]
fn close_all_mid_restore_keeps_saves_refused_until_the_worker_reports() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, mut app) = footprint_fixture();
    let task = app.handle_history_restore_clicked(&sha);
    let before = std::fs::read_to_string(&path).expect("read");

    close_all(&mut app);
    assert!(
        app.document_state.pending_restores.contains(&path),
        "claim kept"
    );
    reopen_from_tree(&mut app, &path);
    assert_saves_refused_and_file_untouched(&mut app, &path, &before);

    finish_restore(&mut app, collect_messages(task));
    assert_released_and_restored_on_disk(&app, &path);
}

#[test]
fn a_cancelled_restore_that_wrote_nothing_says_so() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, _sha, mut app) = footprint_fixture();
    let bad = "0000000000000000000000000000000000000000";
    let task = app.handle_history_restore_clicked(bad);
    let _ = app.close_tab_now(0);
    finish_restore(&mut app, collect_messages(task));
    assert!(logged(&format!(
        "A cancelled restore of {} to {bad} finished without writing",
        file_name(&path)
    )));
    assert!(!logged(&format!(
        "{} was restored to {bad}",
        path.display()
    )));
    assert!(!app.document_state.pending_restores.contains(&path));
}

#[test]
fn a_second_restore_while_a_cancelled_one_runs_says_why_it_did_nothing() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, mut app) = footprint_fixture();
    let task = app.handle_history_restore_clicked(&sha);
    let _ = app.close_tab_now(0);
    reopen_from_tree(&mut app, &path);
    assert_eq!(
        app.handle_history_restore_clicked(&sha).units(),
        0,
        "no second worker next to the cancelled one"
    );
    assert!(logged(&format!(
        "A History restore of {} is still running",
        file_name(&path)
    )));
    finish_restore(&mut app, collect_messages(task));
    assert!(!app.document_state.pending_restores.contains(&path));
}

fn schematic_in(app: &mut Signex, path: &Path) {
    let engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        ..Default::default()
    })
    .expect("engine");
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: file_name(path),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
}

#[test]
fn save_as_onto_a_file_whose_restore_is_cancelled_but_running_is_refused() {
    let _ = crate::diagnostics::init_logging();
    let (dir, path, sha, mut app) = footprint_fixture();
    let task = app.handle_history_restore_clicked(&sha);
    let before = std::fs::read_to_string(&path).expect("read");
    let _ = app.close_tab_now(0);
    let other = dir.path().join(format!("b-{}.snxsch", file_name(&path)));
    schematic_in(&mut app, &other);
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(other.clone());

    let next = app.update(Message::File(FileMsg::SaveAs {
        from_path: other.clone(),
        to_path: path.clone(),
    }));
    run_task(&mut app, next);
    assert!(logged(&refused(&path)));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), before);
    assert!(
        app.document_state.engines.contains_key(&other),
        "not re-keyed"
    );
    assert!(!app.document_state.engines.contains_key(&path));

    finish_restore(&mut app, collect_messages(task));
    assert_released_and_restored_on_disk(&app, &path);
}

#[test]
fn primitive_save_as_onto_a_restoring_file_is_refused_and_rolled_back() {
    let (dir, path, sha, mut app) = footprint_fixture();
    let task = app.handle_history_restore_clicked(&sha);
    let _ = app.close_tab_now(0);
    let other = dir.path().join(format!("o-{}", file_name(&path)));
    marker_editor(&mut app, &other, "other");
    app.document_state.tabs.push(TabInfo {
        title: "other".into(),
        path: other.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(other.clone()),
    });
    let err = app.handle_save_primitive_as(&other, &path).unwrap_err();
    assert!(format!("{err:#}").contains(&refused(&path)), "{err:#}");
    assert_eq!(app.document_state.tabs[0].path, other, "re-key rolled back");
    drop(task);
}

fn schematic_fixture(stem: &str) -> (tempfile::TempDir, PathBuf, String, uuid::Uuid, Signex) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(format!("{stem}.snxsch"));
    let restored_uuid = uuid::Uuid::new_v4();
    let text = signex_types::format::SnxSchematic::new(signex_types::schematic::SchematicSheet {
        uuid: restored_uuid,
        ..Default::default()
    })
    .write_string()
    .expect("encode");
    let sha = commit_revision(dir.path(), &path, &text);
    commit_revision(dir.path(), &path, &format!("{text}\n"));
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    app.document_state.tabs.clear();
    schematic_in(&mut app, &path);
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    (dir, path, sha, restored_uuid, app)
}

fn engine_uuid(app: &Signex, path: &Path) -> uuid::Uuid {
    app.document_state.engines[path].document().uuid
}

#[test]
fn a_restore_clicked_while_the_file_is_saving_is_refused_with_a_reason() {
    let _ = crate::diagnostics::init_logging();
    // A realistically long file name: the line must fit the Messages panel
    // with its "[git] restore: " prefix.
    let (_dir, path, sha, mut app) = named_footprint_fixture(&"long-part-name-".repeat(4));
    let before = std::fs::read_to_string(&path).expect("read");
    let job = app
        .document_state
        .document_saves
        .begin(path.clone(), b"saved".to_vec());

    let task = app.handle_history_restore_clicked(&sha);

    assert!(!app.document_state.pending_restores.contains(&path));
    assert_eq!(task.units(), 0, "no restore job was started");
    let needle = format!(
        "[git] restore: {} is still saving, so the restore was refused",
        file_name(&path)
    );
    let entry = crate::diagnostics::recent_entries()
        .into_iter()
        .find(|e| e.message.contains(&needle))
        .expect("the refusal is logged");
    assert!(
        entry.message.len() < 157,
        "Messages-panel budget: {} bytes: {}",
        entry.message.len(),
        entry.message
    );
    assert!(app.document_state.error_notice.is_some());
    assert_eq!(std::fs::read_to_string(&path).expect("read"), before);
    drop(job);
}

/// The committed content of `rel` at HEAD.
fn head_blob(dir: &Path, rel: &str) -> String {
    let repo = git2::Repository::open(dir).expect("repo");
    let tree = repo
        .head()
        .expect("head")
        .peel_to_commit()
        .expect("commit")
        .tree()
        .expect("tree");
    let entry = tree.get_path(Path::new(rel)).expect("entry");
    let blob = repo.find_blob(entry.id()).expect("blob");
    String::from_utf8(blob.content().to_vec()).expect("utf-8")
}

fn head_subject(dir: &Path) -> String {
    let repo = git2::Repository::open(dir).expect("repo");
    let head = repo.head().expect("head").peel_to_commit().expect("commit");
    head.message()
        .expect("utf-8")
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// A restore clicked while Ctrl+S is still writing must not run: its write
/// would land before the "Save X" commit reads the file, and git would
/// record the restored bytes under the save's label. Once the save and its
/// commit are done the restore is allowed, and the commit holds the SAVED
/// bytes.
#[test]
fn a_restore_clicked_mid_save_is_refused_and_the_save_commit_holds_the_saved_bytes() {
    let _ = crate::diagnostics::init_logging();
    let (dir, path, sha, restored, mut app) = schematic_fixture("stamp-route");
    let saved = format!("{}\n", std::fs::read_to_string(&path).expect("read"));
    app.document_state.dirty_paths.insert(path.clone());
    app.document_state.tabs[0].dirty = true;
    let revision = app.document_state.engines[&path].revision();
    let job = app
        .document_state
        .document_saves
        .begin(path.clone(), saved.clone().into_bytes());

    // The first click arms the unsaved-edits confirmation, the second restores.
    assert!(app.handle_history_restore_clicked(&sha).units() == 0);
    let refused_task = app.handle_history_restore_clicked(&sha);
    assert!(
        !app.document_state.pending_restores.contains(&path),
        "no restore claim while the save is writing"
    );
    assert_eq!(refused_task.units(), 0, "no restore job was started");
    assert!(logged(&format!(
        "{} is still saving, so the restore was refused",
        file_name(&path)
    )));

    // The save lands and its commit runs against the real repo.
    let bytes = job.bytes();
    let sequence = job.sequence();
    assert_eq!(job.run(), Ok(SaveOutcome::Written));
    let _ = app.handle_document_saved(DocumentSaveDone {
        path: path.clone(),
        revision,
        net_color_revision: 0,
        save_as_from: None,
        bytes,
        sequence,
        result: Ok(SaveOutcome::Written),
        recovery_hold: None,
    });
    let commits = app.drain_pending_git_commits();
    run_task(&mut app, commits);
    let rel = file_name(&path);
    assert_eq!(head_subject(dir.path()), format!("Save {rel}"));
    assert_eq!(head_blob(dir.path(), &rel), saved);

    assert!(
        app.document_state.error_notice.take().is_some(),
        "the refusal is shown"
    );
    // The save is done, so the restore now goes ahead and lands; a
    // schematic restore leaves the tab dirty and commits nothing.
    let task = app.handle_history_restore_clicked(&sha);
    finish_restore(&mut app, collect_messages(task));
    assert!(app.document_state.error_notice.is_none());
    assert_eq!(engine_uuid(&app, &path), restored, "restore applied");
    assert_eq!(head_subject(dir.path()), format!("Save {rel}"));
    assert_eq!(
        head_blob(dir.path(), &rel),
        saved,
        "HEAD keeps the saved bytes"
    );
    assert_ne!(std::fs::read_to_string(&path).expect("read"), saved);
}

#[test]
fn a_restore_over_edits_marks_the_tab_and_the_dirty_set_together() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, _restored, mut app) = schematic_fixture("over-edits");
    assert!(!app.document_state.tabs[0].dirty, "a clean tab to start");
    let task = app.handle_history_restore_clicked(&sha);
    // A colour override changes what the tab holds without either dirty
    // marker having been told yet.
    app.ui_state.wire_color_overrides.insert(
        uuid::Uuid::new_v4(),
        signex_types::theme::Color::new(9, 0, 0, 255),
    );

    finish_restore(&mut app, collect_messages(task));

    assert!(logged("was edited while its restore"));
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(
        app.document_state.tabs[0].dirty,
        "the tab flag agrees with the dirty set"
    );
}

#[test]
fn a_restore_finishing_on_a_background_schematic_leaves_the_users_tab_state_alone() {
    use crate::app::TextEditState;
    let _ = crate::diagnostics::init_logging();
    let (dir, path, sha, restored, mut app) = schematic_fixture("focus-keep");
    let mine = dir.path().join("mine-focus-keep.snxsch");
    schematic_in(&mut app, &mine);
    let task = app.handle_history_restore_clicked(&sha);
    app.document_state.active_tab = 1;
    app.document_state.active_path = Some(mine.clone());

    app.interaction_state.current_tool = crate::app::Tool::Wire;
    app.interaction_state.active_canvas_mut().move_pick = Some(crate::canvas::MovePick::Object);
    app.interaction_state.editing_text = Some(TextEditState {
        uuid: uuid::Uuid::new_v4(),
        kind: signex_types::schematic::SelectedKind::Label,
        text: "half typed".into(),
        original_text: "x".into(),
        world_x: 0.0,
        world_y: 0.0,
        sheet: None,
        placement: None,
    });
    finish_restore(&mut app, collect_messages(task));

    assert_eq!(engine_uuid(&app, &path), restored, "restore applied");
    assert_eq!(app.document_state.active_tab, 1, "focus stays");
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(mine.as_path())
    );
    assert!(matches!(
        app.interaction_state.current_tool,
        crate::app::Tool::Wire
    ));
    assert!(
        app.interaction_state
            .active_canvas_mut()
            .move_pick
            .is_some()
    );
    assert_eq!(
        app.interaction_state
            .editing_text
            .as_ref()
            .map(|e| e.text.as_str()),
        Some("half typed")
    );
}

fn ghost_item() -> signex_types::schematic::SelectedItem {
    signex_types::schematic::SelectedItem::new(
        uuid::Uuid::new_v4(),
        signex_types::schematic::SelectedKind::Wire,
    )
}

/// Inline text, a pending move and a selection, all on the visible canvas.
fn arm_state_of_the_old_engine(app: &mut Signex) {
    app.interaction_state.editing_text = Some(crate::app::TextEditState {
        uuid: uuid::Uuid::new_v4(),
        kind: signex_types::schematic::SelectedKind::Label,
        text: "half typed".into(),
        original_text: "x".into(),
        world_x: 0.0,
        world_y: 0.0,
        sheet: None,
        placement: None,
    });
    let slot = app.interaction_state.active_canvas_mut();
    slot.move_pick = Some(crate::canvas::MovePick::Object);
    slot.selected.push(ghost_item());
}

fn assert_old_engine_state_gone(app: &mut Signex) {
    assert!(app.interaction_state.editing_text.is_none(), "inline text");
    let slot = app.interaction_state.active_canvas_mut();
    assert!(slot.move_pick.is_none(), "pending move");
    assert!(slot.selected.is_empty(), "selection");
}

#[test]
fn a_restore_replacing_the_active_engine_drops_state_that_pointed_into_it() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, restored, mut app) = schematic_fixture("replace-active");
    let task = app.handle_history_restore_clicked(&sha);
    arm_state_of_the_old_engine(&mut app);
    finish_restore(&mut app, collect_messages(task));
    assert_eq!(engine_uuid(&app, &path), restored, "restore applied");
    assert_old_engine_state_gone(&mut app);
}

#[test]
fn a_clean_reopened_active_tab_that_takes_a_cancelled_restore_drops_that_state_too() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, restored, mut app) = schematic_fixture("replace-reopened");
    let task = app.handle_history_restore_clicked(&sha);
    let _ = app.close_tab_now(0);
    reopen_from_tree(&mut app, &path);
    arm_state_of_the_old_engine(&mut app);
    finish_restore(&mut app, collect_messages(task));
    assert_eq!(engine_uuid(&app, &path), restored, "restore applied");
    assert_old_engine_state_gone(&mut app);
}

#[test]
fn an_undocked_window_on_a_background_restored_schematic_is_redrawn_and_deselected() {
    use crate::schematic_runtime::SchematicRenderCache;
    let _ = crate::diagnostics::init_logging();
    let (dir, path, sha, restored, mut app) = schematic_fixture("undocked-refresh");
    let mine = dir.path().join("mine-undocked-refresh.snxsch");
    schematic_in(&mut app, &mine);
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: path.clone(),
            title: "a".into(),
        },
    );
    let mut slot = crate::canvas::CanvasSlot::new();
    let old = &app.document_state.engines[&path];
    slot.set_render_cache(Some(SchematicRenderCache::from_sheet_at(
        old.document(),
        old.revision(),
    )));
    slot.selected.push(ghost_item());
    app.interaction_state.canvases.insert(window, slot);

    let task = app.handle_history_restore_clicked(&sha);
    app.document_state.active_tab = 1;
    app.document_state.active_path = Some(mine);
    finish_restore(&mut app, collect_messages(task));

    let slot = &app.interaction_state.canvases[&window];
    assert_eq!(
        slot.render_cache.as_ref().expect("cache").snapshot().uuid,
        restored
    );
    assert!(slot.selected.is_empty(), "no ghost selection");
}

fn undocked_slot_on(app: &mut Signex, path: &Path) -> iced::window::Id {
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::state::WindowKind::UndockedTab {
            path: path.to_path_buf(),
            title: "undocked".into(),
        },
    );
    let mut slot = crate::canvas::CanvasSlot::new();
    slot.selected.push(ghost_item());
    app.interaction_state.canvases.insert(window, slot);
    window
}

#[test]
fn a_restore_on_the_active_tab_also_deselects_undocked_windows_on_that_sheet() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, restored, mut app) = schematic_fixture("active-undocked");
    let window = undocked_slot_on(&mut app, &path);
    let task = app.handle_history_restore_clicked(&sha);
    finish_restore(&mut app, collect_messages(task));
    assert_eq!(engine_uuid(&app, &path), restored, "restore applied");
    assert!(app.interaction_state.canvases[&window].selected.is_empty());
}

fn text_edit_on(
    kind: signex_types::schematic::SelectedKind,
    uuid: uuid::Uuid,
) -> crate::app::TextEditState {
    crate::app::TextEditState {
        uuid,
        kind,
        text: "typing".into(),
        original_text: "x".into(),
        world_x: 0.0,
        world_y: 0.0,
        sheet: None,
        placement: None,
    }
}

#[test]
fn inline_text_on_an_object_of_a_background_restored_sheet_is_discarded() {
    use signex_types::schematic::{
        HAlign, Label, LabelType, Point, SchematicSheet, SelectedKind, VAlign,
    };
    let _ = crate::diagnostics::init_logging();
    let (dir, path, sha, _restored, mut app) = schematic_fixture("text-on-background");
    let label_uuid = uuid::Uuid::new_v4();
    let mut sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        ..Default::default()
    };
    sheet.labels.push(Label {
        unknown_columns: Default::default(),
        font_family: None,
        uuid: label_uuid,
        text: "NET".into(),
        position: Point::new(1.0, 1.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: signex_types::schematic::SCHEMATIC_TEXT_MM,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    });
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    let mine = dir.path().join("mine-text-on-background.snxsch");
    schematic_in(&mut app, &mine);
    let task = app.handle_history_restore_clicked(&sha);
    app.document_state.active_tab = 1;
    app.document_state.active_path = Some(mine);

    app.interaction_state.editing_text = Some(text_edit_on(SelectedKind::Label, label_uuid));
    finish_restore(&mut app, collect_messages(task));
    assert!(app.interaction_state.editing_text.is_none());
}

fn arm_autosave_copy(app: &mut Signex, path: &Path) -> PathBuf {
    let dir = tempfile::tempdir().expect("recovery dir").keep();
    app.document_state.autosave.set_dir(Some(dir.clone()));
    let stamp = crate::autosave::DocStamp {
        revision: app.document_state.engines[path].revision(),
        net_color_revision: 0,
    };
    let _ = app.document_state.autosave.begin(path);
    app.document_state.autosave.finish(path, stamp, true);
    crate::autosave::sidecar::write(&dir, path, b"pre-restore edits").expect("sidecar");
    dir
}

fn stamp_of(app: &Signex, path: &Path) -> crate::autosave::DocStamp {
    crate::autosave::DocStamp {
        revision: app.document_state.engines[path].revision(),
        net_color_revision: app.net_color_revision_of(path),
    }
}

/// Delivers the restore's completion and runs the task it returns.
fn finish_restore_running_followups(app: &mut Signex, task: iced::Task<Message>) {
    for message in collect_messages(task) {
        let Message::File(FileMsg::HistoryRestoreFinished {
            request,
            sha,
            edit_stamp,
            result,
        }) = message
        else {
            panic!("expected a restore completion");
        };
        let next = app.handle_history_restore_finished(request, &sha, edit_stamp, result);
        run_task(app, next);
    }
}

#[test]
fn a_restore_that_replaces_the_engine_drops_this_runs_stale_recovery_copy() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, restored, mut app) = schematic_fixture("autosave-stale");
    let recovery_dir = arm_autosave_copy(&mut app, &path);
    let task = app.handle_history_restore_clicked(&sha);
    finish_restore_running_followups(&mut app, task);

    assert_eq!(engine_uuid(&app, &path), restored, "restore applied");
    assert!(
        !crate::autosave::sidecar::sidecar_path(&recovery_dir, &path).exists(),
        "the copy of the discarded edits is gone"
    );
    assert!(
        app.document_state
            .autosave
            .needs_write(&path, stamp_of(&app, &path)),
        "the restored document is written on the next tick"
    );
}

#[test]
fn a_restore_leaves_a_deferred_earlier_run_copy_alone_and_protected() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, restored, mut app) = schematic_fixture("autosave-deferred");
    let dir = tempfile::tempdir().expect("recovery dir").keep();
    app.document_state.autosave.set_dir(Some(dir.clone()));
    crate::autosave::sidecar::write(&dir, &path, b"earlier run").expect("sidecar");
    app.document_state.autosave.defer(&path);
    let task = app.handle_history_restore_clicked(&sha);
    finish_restore_running_followups(&mut app, task);

    assert_eq!(engine_uuid(&app, &path), restored, "restore applied");
    assert_eq!(
        std::fs::read(crate::autosave::sidecar::sidecar_path(&dir, &path)).expect("kept"),
        b"earlier run"
    );
    assert!(
        !app.document_state
            .autosave
            .needs_write(&path, stamp_of(&app, &path)),
        "still no write over the deferred copy"
    );
}
