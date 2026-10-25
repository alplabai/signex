//! #859 — a cancelled History restore keeps its claim until its worker
//! reports, so saves of the file stay refused while the worker can still
//! write it. These tests cancel while the worker is still pending: the task
//! is held unrun until after the "meanwhile" actions. Every fixture has its
//! own file stem, so the lines asserted in the (global) log are its own.

use std::path::{Path, PathBuf};

use super::open_read_tests::open_from_tree;
use super::restore_commit_tests::{
    collect_messages, commit_revision, edit_description, editor_name, file_name, finish_restore,
    footprint_fixture, logged, marker_editor, open_tab, run_task,
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
fn a_save_completing_mid_restore_is_not_an_edit_so_the_restore_still_lands() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, restored, mut app) = schematic_fixture("stamp-route");
    app.document_state.dirty_paths.insert(path.clone());
    app.document_state.tabs[0].dirty = true;
    // The first click arms the unsaved-edits confirmation, the second restores.
    assert!(app.handle_history_restore_clicked(&sha).units() == 0);
    let task = app.handle_history_restore_clicked(&sha);
    let revision = app.document_state.engines[&path].revision();

    // Ctrl+S's write finishes while the restore's worker is pending.
    let _ = app.update(Message::File(FileMsg::DocumentSaved(DocumentSaveDone {
        path: path.clone(),
        revision,
        net_color_revision: 0,
        save_as_from: None,
        result: Ok(SaveOutcome::Written),
    })));
    assert!(
        !app.document_state.tabs[0].dirty,
        "the save cleaned the tab"
    );

    finish_restore(&mut app, collect_messages(task));
    assert_eq!(engine_uuid(&app, &path), restored, "restore applied");
    assert!(app.document_state.error_notice.is_none());
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
