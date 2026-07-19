//! Project/document lifecycle: modals, git pipeline, exit guard, open-gating.

use signex_app::app::{
    ContextMenuMsg, FileMsg, LoadedProject, Message, ProjectCloseChoice, ProjectMsg,
    ProjectTreeAction, RemoveChoice, RemoveDialogState, RemoveMsg, RenameDialogState, RenameMsg,
    Signex, WindowMsg,
};
use signex_types::project::{ProjectData, SheetEntry};

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// The general "exercise dispatchers without the iced runtime" methodology
// this whole regression suite follows lives in the parent module doc at
// `tests/regression.rs`.

// ─────────────────────────────────────────────────────────────────
// Smoke — `Signex::new()` constructs cleanly without iced runtime
// ─────────────────────────────────────────────────────────────────

#[test]
fn signex_new_constructs_with_default_state() {
    let (app, _initial_task) = Signex::new();
    // Empty workspace — nothing loaded.
    assert!(app.document_state.projects.is_empty());
    assert_eq!(app.document_state.active_project, None);
    assert!(app.document_state.tabs.is_empty());
    assert!(app.document_state.dirty_paths.is_empty());
    // Modals all closed.
    assert!(app.ui_state.rename_dialog.is_none());
    assert!(app.ui_state.remove_dialog.is_none());
    assert!(app.ui_state.project_close_confirm.is_none());
    assert!(app.ui_state.project_options.is_none());
}

// ─────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────

/// Project skeleton: writes `<stem>.snxprj` + companion
/// `<stem>.snxsch` + `<stem>.snxpcb` into a fresh tempdir and
/// returns a populated `Signex` with the project loaded.
fn fixture_project_with_companions(stem: &str) -> (Signex, TempDir, PathBuf) {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();

    let prj_path = dir.join(format!("{stem}.snxprj"));
    let sch_path = dir.join(format!("{stem}.snxsch"));
    let pcb_path = dir.join(format!("{stem}.snxpcb"));

    fs::write(&prj_path, b"{}").expect("write .snxprj");
    fs::write(&sch_path, b"schematic-bytes").expect("write .snxsch");
    fs::write(&pcb_path, b"pcb-bytes").expect("write .snxpcb");

    let (mut app, _initial_task) = Signex::new();

    let id = app.document_state.mint_project_id();
    let data = ProjectData {
        name: stem.to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some(format!("{stem}.snxsch")),
        pcb_file: Some(format!("{stem}.snxpcb")),
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
    };
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj_path.clone(),
        data,
        pending_libraries: std::collections::HashMap::new(),
    });
    app.document_state.active_project = Some(id);

    (app, tmp, prj_path)
}

/// Open the rename modal targeting a project root.
fn arm_project_rename(app: &mut Signex, target: &Path, new_stem: &str) {
    app.ui_state.rename_dialog = Some(RenameDialogState {
        target_path: target.to_path_buf(),
        tree_path: vec![0],
        buffer: new_stem.to_string(),
        error: None,
        is_project_rename: true,
    });
}

/// Open the remove modal for a tree leaf.
fn arm_remove_dialog(app: &mut Signex, target: &Path) {
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: target.to_path_buf(),
        tree_path: vec![0, 0],
        display_name: target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string(),
    });
}

// ─────────────────────────────────────────────────────────────────
// F6 — Project rename scopes to .snxprj only
// ─────────────────────────────────────────────────────────────────

#[test]
fn f6_project_rename_does_not_touch_companion_snxsch_snxpcb() {
    let (mut app, tmp, old_prj) = fixture_project_with_companions("OldProj");
    let dir = tmp.path();
    let companion_sch = dir.join("OldProj.snxsch");
    let companion_pcb = dir.join("OldProj.snxpcb");

    arm_project_rename(&mut app, &old_prj, "NewProj");
    let _ = app.update(Message::Rename(RenameMsg::Submit));

    let new_prj = dir.join("NewProj.snxprj");
    assert!(!old_prj.exists(), "old .snxprj must be removed");
    assert!(new_prj.exists(), "new .snxprj must exist");

    // The whole point of F6: companion sheets / boards stay put.
    assert!(
        companion_sch.exists(),
        "companion .snxsch must NOT be renamed (F6)"
    );
    assert!(
        companion_pcb.exists(),
        "companion .snxpcb must NOT be renamed (F6)"
    );

    // No same-stem .snxsch/.snxpcb should appear under the new stem either.
    assert!(
        !dir.join("NewProj.snxsch").exists(),
        "must not produce a NewProj.snxsch shadow"
    );
    assert!(
        !dir.join("NewProj.snxpcb").exists(),
        "must not produce a NewProj.snxpcb shadow"
    );

    // In-memory state migrated to the new path.
    let project = &app.document_state.projects[0];
    assert_eq!(project.path, new_prj);
    assert_eq!(project.data.name, "NewProj");
    assert!(
        app.ui_state.rename_dialog.is_none(),
        "modal closed on success"
    );
}

#[test]
fn f6_project_rename_refuses_to_overwrite_existing_target() {
    let (mut app, tmp, old_prj) = fixture_project_with_companions("Alpha");

    // Pre-existing target file blocks the rename.
    let collision = tmp.path().join("Beta.snxprj");
    fs::write(&collision, b"existing content").unwrap();

    arm_project_rename(&mut app, &old_prj, "Beta");
    let _ = app.update(Message::Rename(RenameMsg::Submit));

    assert!(
        old_prj.exists(),
        "old .snxprj must NOT be removed on collision"
    );
    assert!(
        collision.exists(),
        "pre-existing target file must remain untouched"
    );
    let dlg = app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("modal should remain open with an error");
    assert!(dlg.error.is_some(), "modal carries an error message");
}

#[test]
fn f6_project_rename_rejects_path_separators_in_buffer() {
    let (mut app, _tmp, old_prj) = fixture_project_with_companions("Gamma");

    arm_project_rename(&mut app, &old_prj, "../Escape");
    let _ = app.update(Message::Rename(RenameMsg::Submit));

    assert!(old_prj.exists(), ".snxprj must remain untouched");
    let dlg = app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("modal stays open after validation error");
    assert!(dlg.error.is_some(), "modal carries an error message");
}

#[test]
fn f6_project_rename_with_unchanged_stem_is_a_silent_noop() {
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Delta");

    arm_project_rename(&mut app, &prj_path, "Delta");
    let _ = app.update(Message::Rename(RenameMsg::Submit));

    assert!(prj_path.exists(), "file remains at original path");
    assert!(
        app.ui_state.rename_dialog.is_none(),
        "modal closes on noop submit"
    );
}

// ─────────────────────────────────────────────────────────────────
// §3.6 — Remove from Project: Delete vs Exclude file effect
// ─────────────────────────────────────────────────────────────────

#[test]
fn remove_with_delete_choice_unlinks_the_file() {
    let (mut app, tmp, _prj) = fixture_project_with_companions("Echo");
    let target = tmp.path().join("Echo.snxsch");
    assert!(target.exists());

    arm_remove_dialog(&mut app, &target);
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));

    assert!(
        !target.exists(),
        "DeleteFile must remove the file from disk"
    );
}

#[test]
fn remove_with_exclude_choice_keeps_the_file_on_disk() {
    let (mut app, tmp, _prj) = fixture_project_with_companions("Foxtrot");
    let target = tmp.path().join("Foxtrot.snxsch");
    assert!(target.exists());

    arm_remove_dialog(&mut app, &target);
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::ExcludeFromProject,
    )));

    assert!(
        target.exists(),
        "ExcludeFromProject must NOT delete the file on disk"
    );
}

// ─────────────────────────────────────────────────────────────────
// F10 — Project dirty bit clears + panel_ctx refreshes after save
// ─────────────────────────────────────────────────────────────────

#[test]
fn f10_save_clears_dirty_paths_and_refreshes_panel_ctx() {
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Golf");

    // Mark the project dirty (matches what `Add Existing` / project
    // metadata mutations would do).
    app.document_state.dirty_paths.insert(prj_path.clone());
    assert!(app.document_state.dirty_paths.contains(&prj_path));

    let _ = app.update(Message::File(FileMsg::Save));

    // Title strip: dirty_paths empty → "(N unsaved)" suffix gone.
    assert!(
        !app.document_state.dirty_paths.contains(&prj_path),
        "save must remove the project from dirty_paths"
    );

    // Tree row dirty dot: panel_ctx.projects[0].is_dirty must read false
    // after save. Pre-F10 fix, refresh_panel_ctx wasn't called inside
    // save_active_document so the cached snapshot kept is_dirty=true.
    let projects = &app.document_state.panel_ctx.projects;
    assert!(
        !projects.is_empty(),
        "panel_ctx.projects must be populated after save (proves refresh ran)"
    );
    assert!(
        !projects[0].is_dirty,
        "panel_ctx.projects[0].is_dirty must clear after save (F10)"
    );
}

#[test]
fn f10_save_persists_snxprj_as_valid_json() {
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Hospital");

    // Mutate the in-memory ProjectData — change the variant list,
    // for example — then mark dirty and save.
    {
        let proj = &mut app.document_state.projects[0];
        proj.data.variant_definitions = vec!["Production".into(), "Prototype".into()];
        proj.data.active_variant = Some("Production".into());
    }
    app.document_state.dirty_paths.insert(prj_path.clone());

    let _ = app.update(Message::File(FileMsg::Save));

    // Re-parse the file from disk — assert the mutations landed.
    let reloaded = signex_types::project::parse_project(&prj_path).expect("parse");
    assert_eq!(reloaded.name, "Hospital");
    assert_eq!(reloaded.variant_definitions.len(), 2);
    assert_eq!(reloaded.active_variant.as_deref(), Some("Production"));
}

// ─────────────────────────────────────────────────────────────────
// §3.3 — Add Existing dedup: same file twice is silently skipped
// ─────────────────────────────────────────────────────────────────

#[test]
fn add_existing_same_file_twice_is_silently_skipped() {
    let (mut app, tmp, _prj) = fixture_project_with_companions("India");
    let dir = tmp.path();

    // The fixture wrote India.snxsch but didn't register it on the
    // project's sheets list. Pick it up via Add Existing.
    let new_sheet = dir.join("India.snxsch");
    assert!(new_sheet.exists());

    // First add — succeeds, sheet appears.
    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilePicked {
        project_idx: 0,
        paths: Some(vec![new_sheet.clone()]),
    }));
    assert_eq!(
        app.document_state.projects[0].data.sheets.len(),
        1,
        "first add registers the sheet"
    );

    // Second add — silent no-op, list size unchanged.
    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilePicked {
        project_idx: 0,
        paths: Some(vec![new_sheet.clone()]),
    }));
    assert_eq!(
        app.document_state.projects[0].data.sheets.len(),
        1,
        "second add of same file must NOT duplicate the entry (§3.3)"
    );
}

#[test]
fn add_existing_with_external_path_copies_into_project_dir() {
    let (mut app, tmp, _prj) = fixture_project_with_companions("Juliet");
    let project_dir = tmp.path();

    // External tempdir — separate from the project directory.
    let external = TempDir::new().unwrap();
    let external_sheet = external.path().join("ExternalSheet.snxsch");
    fs::write(&external_sheet, b"external sheet bytes").unwrap();

    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilePicked {
        project_idx: 0,
        paths: Some(vec![external_sheet.clone()]),
    }));

    let copied = project_dir.join("ExternalSheet.snxsch");
    assert!(
        copied.exists(),
        "external sheet must be copied into the project dir"
    );
    assert!(
        external_sheet.exists(),
        "external source file must remain in place (copy, not move)"
    );
    assert_eq!(
        app.document_state.projects[0].data.sheets.len(),
        1,
        "copied sheet registers as a project entry"
    );
}

// ─────────────────────────────────────────────────────────────────
// §3.4 — Project rename migrates engine-map / dirty / active state
// ─────────────────────────────────────────────────────────────────

#[test]
fn project_rename_migrates_dirty_paths_to_new_path() {
    let (mut app, tmp, old_prj) = fixture_project_with_companions("Kilo");

    // Project dirty before rename.
    app.document_state.dirty_paths.insert(old_prj.clone());

    arm_project_rename(&mut app, &old_prj, "Lima");
    let _ = app.update(Message::Rename(RenameMsg::Submit));

    let new_prj = tmp.path().join("Lima.snxprj");
    assert!(
        !app.document_state.dirty_paths.contains(&old_prj),
        "old path no longer in dirty_paths"
    );
    assert!(
        app.document_state.dirty_paths.contains(&new_prj),
        "new path migrated into dirty_paths"
    );
}

// ─────────────────────────────────────────────────────────────────
// §3.2 — Add New ▸ Schematic (post-Save-As-dialog dispatch)
// ─────────────────────────────────────────────────────────────────

#[test]
fn add_new_schematic_writes_blank_snxsch_marks_project_dirty_no_tab_open() {
    let (mut app, tmp, prj_path) = fixture_project_with_companions("Mike");
    let new_sheet = tmp.path().join("FreshSheet.snxsch");
    assert!(!new_sheet.exists());

    let _ = app.update(Message::Project(ProjectMsg::AddNewSchematicPicked {
        project_idx: 0,
        path: Some(new_sheet.clone()),
    }));

    assert!(new_sheet.exists(), "new .snxsch must land on disk");
    assert_eq!(
        app.document_state.projects[0].data.sheets.len(),
        1,
        "new sheet appears in the project's sheets list"
    );
    assert!(
        app.document_state.dirty_paths.contains(&prj_path),
        "project marked dirty so user knows to Save"
    );

    // §3.2 important UX: NO tab opens automatically. The user clicks
    // the tree entry to open it. Pre-fix the Add New flow would have
    // opened a tab as a side effect.
    assert!(
        app.document_state.tabs.is_empty(),
        "Add New ▸ Schematic must NOT auto-open a tab (§3.2)"
    );
}

#[test]
fn add_new_schematic_cancelled_picker_is_a_clean_noop() {
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("November");

    let _ = app.update(Message::Project(ProjectMsg::AddNewSchematicPicked {
        project_idx: 0,
        path: None, // user cancelled the Save-As dialog
    }));

    assert!(
        app.document_state.projects[0].data.sheets.is_empty(),
        "cancellation makes no project mutations"
    );
    assert!(
        !app.document_state.dirty_paths.contains(&prj_path),
        "cancellation does not flip the dirty bit"
    );
    assert!(app.document_state.tabs.is_empty());
}

// ─────────────────────────────────────────────────────────────────
// §3.5 — Project Options modal (open + close lifecycle)
// ─────────────────────────────────────────────────────────────────

#[test]
fn project_options_modal_opens_with_metadata_then_closes() {
    let (mut app, tmp, _prj) = fixture_project_with_companions("Oscar");
    // Add some sheets and libraries so library_count is meaningful.
    {
        let proj = &mut app.document_state.projects[0];
        proj.data.sheets.push(SheetEntry {
            name: "Power".into(),
            filename: "Power.snxsch".into(),
            symbols_count: 3,
            wires_count: 5,
            labels_count: 2,
        });
    }

    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::OpenProjectOptions(vec![0]),
    )));

    let state = app.ui_state.project_options.as_ref().expect("modal opened");
    assert_eq!(state.project_idx, 0);
    assert_eq!(state.name, "Oscar");
    assert_eq!(state.directory, tmp.path().to_string_lossy().to_string());
    assert_eq!(state.schematic_root.as_deref(), Some("Oscar.snxsch"));
    assert_eq!(state.pcb_file.as_deref(), Some("Oscar.snxpcb"));
    assert_eq!(state.library_count, 0);

    // Close-X / Esc both fire CloseProjectOptions.
    let _ = app.update(Message::Project(ProjectMsg::CloseOptions));
    assert!(
        app.ui_state.project_options.is_none(),
        "Project Options modal closed after CloseProjectOptions"
    );
}

// ─────────────────────────────────────────────────────────────────
// Modal lifecycle (rename / remove): buffer edit + close-without-submit
// ─────────────────────────────────────────────────────────────────

#[test]
fn rename_buffer_changed_updates_modal_buffer() {
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Papa");
    arm_project_rename(&mut app, &prj_path, "");

    let _ = app.update(Message::Rename(RenameMsg::BufferChanged(
        "PartialName".into(),
    )));
    let dlg = app.ui_state.rename_dialog.as_ref().unwrap();
    assert_eq!(dlg.buffer, "PartialName");

    let _ = app.update(Message::Rename(RenameMsg::BufferChanged(
        "LongerName".into(),
    )));
    let dlg = app.ui_state.rename_dialog.as_ref().unwrap();
    assert_eq!(dlg.buffer, "LongerName");
}

#[test]
fn close_rename_dialog_dismisses_modal_without_filesystem_changes() {
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Quebec");
    arm_project_rename(&mut app, &prj_path, "WouldBeRenamed");

    let _ = app.update(Message::Rename(RenameMsg::Close));

    assert!(app.ui_state.rename_dialog.is_none(), "modal closed");
    assert!(
        prj_path.exists(),
        "no rename happened — original still there"
    );
    assert!(
        !prj_path.with_file_name("WouldBeRenamed.snxprj").exists(),
        "no new file created"
    );
}

#[test]
fn close_remove_dialog_dismisses_modal_without_filesystem_changes() {
    let (mut app, tmp, _prj) = fixture_project_with_companions("Romeo");
    let target = tmp.path().join("Romeo.snxsch");
    assert!(target.exists());

    arm_remove_dialog(&mut app, &target);
    let _ = app.update(Message::Remove(RemoveMsg::Close));

    assert!(app.ui_state.remove_dialog.is_none(), "modal closed");
    assert!(target.exists(), "no removal happened — file still there");
}

// ─────────────────────────────────────────────────────────────────
// F13 — New Library: register-pending only, materialise on save
// ─────────────────────────────────────────────────────────────────

#[test]
fn f13_register_pending_library_does_not_touch_disk() {
    let tmp = TempDir::new().unwrap();
    let lib_path = tmp.path().join("MyLib.snxlib");

    let (library_id, spec) = signex_app::library::commands::register_pending_library(
        lib_path.clone(),
        false, // enable_git
        false, // use_lfs
    )
    .expect("register pending");

    // F13 load-bearing assertion: nothing on disk yet.
    assert!(!lib_path.exists(), "register must NOT touch disk");
    assert_eq!(spec.lib_path, lib_path);
    assert_eq!(spec.display_name, "MyLib");
    assert!(!spec.enable_git);
    assert!(!spec.use_lfs);
    // A real Uuid v7 was minted (timestamp-prefixed).
    assert_ne!(library_id, uuid::Uuid::nil());
}

#[test]
fn f13_register_pending_rejects_existing_path() {
    let tmp = TempDir::new().unwrap();
    let lib_path = tmp.path().join("Existing.snxlib");
    fs::create_dir_all(&lib_path).unwrap();

    let result =
        signex_app::library::commands::register_pending_library(lib_path.clone(), false, false);
    assert!(result.is_err(), "must reject paths that already exist");
}

#[test]
fn f13_register_pending_rejects_non_snxlib_extension() {
    let tmp = TempDir::new().unwrap();
    let bad_path = tmp.path().join("BadExt.snxsch");

    let result = signex_app::library::commands::register_pending_library(bad_path, false, false);
    assert!(
        result.is_err(),
        "must reject paths whose extension isn't .snxlib"
    );
}

// ─────────────────────────────────────────────────────────────────
// §8.2 — `.snxprj` round-trip (engine-level — adds to the
// `signex-types::project` tests by exercising the multi-project
// `LoadedProject` shape that signex-app actually uses)
// ─────────────────────────────────────────────────────────────────

#[test]
fn loaded_project_data_round_trips_via_write_then_parse() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("Hotel.snxprj");
    let data = ProjectData {
        name: "Hotel".into(),
        dir: tmp.path().to_string_lossy().into_owned(),
        schematic_root: Some("Hotel.snxsch".into()),
        pcb_file: Some("Hotel.snxpcb".into()),
        sheets: Vec::new(),
        variant_definitions: vec!["Production".into(), "Prototype".into()],
        active_variant: Some("Production".into()),
        libraries: Vec::new(),
        enable_git: false,
    };

    signex_types::project::write_project(&path, &data).expect("write");
    let loaded = signex_types::project::parse_project(&path).expect("parse");

    assert_eq!(loaded.name, "Hotel");
    assert_eq!(loaded.schematic_root.as_deref(), Some("Hotel.snxsch"));
    assert_eq!(loaded.pcb_file.as_deref(), Some("Hotel.snxpcb"));
    assert_eq!(loaded.variant_definitions.len(), 2);
    assert_eq!(loaded.active_variant.as_deref(), Some("Production"));
}

// ─────────────────────────────────────────────────────────────────
// v0.23 — async git commit pipeline
// ─────────────────────────────────────────────────────────────────

#[test]
fn project_git_commit_done_clears_inflight_entry() {
    // The async pipeline tracks (project_root, rel_path) pairs in
    // `inflight_git_commits` while the commit runs in the
    // background. `Message::Project(ProjectMsg::GitCommitDone)` must
    // remove the pair regardless of success/failure so the "Saving…" pill
    // clears.
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Foxtrot");
    let project_root = prj_path.parent().unwrap().to_path_buf();
    let rel_path = PathBuf::from("Foxtrot.snxsch");
    let key = (project_root.clone(), rel_path.clone());
    app.document_state.inflight_git_commits.insert(key.clone());
    assert!(app.document_state.inflight_git_commits.contains(&key));

    // Success path.
    let _ = app.update(Message::Project(ProjectMsg::GitCommitDone {
        project_root: project_root.clone(),
        rel_path: rel_path.clone(),
        result: Ok("deadbeef".to_string()),
    }));
    assert!(
        !app.document_state.inflight_git_commits.contains(&key),
        "inflight entry must be cleared on success"
    );

    // Failure path also clears (data is on disk regardless of git).
    app.document_state.inflight_git_commits.insert(key.clone());
    let _ = app.update(Message::Project(ProjectMsg::GitCommitDone {
        project_root: project_root.clone(),
        rel_path: rel_path.clone(),
        result: Err("commit_path: …".to_string()),
    }));
    assert!(
        !app.document_state.inflight_git_commits.contains(&key),
        "inflight entry must be cleared on failure too"
    );
}

#[test]
fn commit_save_to_project_git_skips_when_enable_git_off() {
    // The save-handler should silently skip the queue + inflight
    // mutation when the owning project hasn't opted into git.
    // Otherwise every save would burn an entry the dispatcher
    // would later have to clear.
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Golf");
    let project_root = prj_path.parent().unwrap().to_path_buf();
    let sch_path = project_root.join("Golf.snxsch");
    assert!(!app.document_state.projects[0].data.enable_git);

    app.commit_save_to_project_git(&sch_path, "Save Golf.snxsch");
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(app.document_state.inflight_git_commits.is_empty());
}

#[test]
fn commit_save_to_project_git_enqueues_when_enable_git_on() {
    // With enable_git on, save handler pushes a PendingGitCommit
    // and adds (project_root, rel_path) to the inflight set so the
    // status bar's "Saving…" pill renders immediately.
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Hotel2");
    app.document_state.projects[0].data.enable_git = true;
    let project_root = prj_path.parent().unwrap().to_path_buf();
    let sch_path = project_root.join("Hotel2.snxsch");

    app.commit_save_to_project_git(&sch_path, "Save Hotel2.snxsch");
    assert_eq!(app.document_state.pending_git_commits.len(), 1);
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);

    // Idempotent: a second enqueue for the same (root, rel) while
    // the first is still inflight is a silent no-op.
    app.commit_save_to_project_git(&sch_path, "Save Hotel2.snxsch (retry)");
    assert_eq!(
        app.document_state.pending_git_commits.len(),
        1,
        "duplicate enqueue must be ignored while inflight"
    );
}

// ─────────────────────────────────────────────────────────────────
// App-exit unsaved-changes guard (issue #95)
//
// Chrome ✕, File ▸ Exit and OS close (Alt+F4) all funnel through
// Message::Window(WindowMsg::CloseMainWindow) / Message::Window(WindowMsg::WindowCloseRequested). With
// unsaved edits present the app must open the confirm modal instead
// of exiting; without them it may close. Save All must never lose a
// file it cannot save — it keeps the app open and reports it.
// ─────────────────────────────────────────────────────────────────

#[test]
fn app_exit_with_no_dirty_paths_does_not_open_confirm_modal() {
    let (mut app, _t) = Signex::new();
    assert!(app.document_state.dirty_paths.is_empty());

    // Clean workspace: exit request must not raise the guard modal.
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    assert!(
        app.ui_state.app_quit_confirm.is_none(),
        "a clean workspace must exit without the unsaved-changes modal"
    );
}

#[test]
fn app_exit_with_dirty_paths_opens_confirm_modal_instead_of_exiting() {
    let (mut app, _t) = Signex::new();
    let dirty = PathBuf::from("/tmp/does-not-matter/board.snxsch");
    app.document_state.dirty_paths.insert(dirty.clone());

    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));

    let modal = app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("dirty workspace must open the app-quit confirm modal");
    assert!(
        modal.dirty_paths.contains(&dirty),
        "the modal must list the dirty file the user is about to lose"
    );
    // Data-loss guard: the dirty entry must still be present — nothing
    // was discarded by merely asking to exit.
    assert!(app.document_state.dirty_paths.contains(&dirty));
}

#[test]
fn app_exit_confirm_cancel_dismisses_modal_and_keeps_dirty_state() {
    let (mut app, _t) = Signex::new();
    let dirty = PathBuf::from("/tmp/does-not-matter/board.snxsch");
    app.document_state.dirty_paths.insert(dirty.clone());
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    assert!(app.ui_state.app_quit_confirm.is_some());

    let _ = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::Cancel,
    )));
    assert!(
        app.ui_state.app_quit_confirm.is_none(),
        "Cancel must dismiss the modal"
    );
    assert!(
        app.document_state.dirty_paths.contains(&dirty),
        "Cancel must not touch the dirty state"
    );
}

#[test]
fn app_exit_confirm_discard_all_clears_modal() {
    let (mut app, _t) = Signex::new();
    app.document_state
        .dirty_paths
        .insert(PathBuf::from("/tmp/does-not-matter/board.snxsch"));
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    assert!(app.ui_state.app_quit_confirm.is_some());

    // Discard All resolves the modal (and returns the exit task).
    let _ = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    assert!(
        app.ui_state.app_quit_confirm.is_none(),
        "Discard All must resolve the modal"
    );
}

#[test]
fn app_exit_save_all_never_loses_an_unsaveable_file() {
    // A dirty path with no live engine (e.g. a .snxprj or primitive
    // draft) cannot be saved through the engine path. Save All must
    // NOT exit and lose it — it keeps the app open and surfaces it.
    let (mut app, _t) = Signex::new();
    let dirty = PathBuf::from("/tmp/does-not-matter/proj.snxprj");
    app.document_state.dirty_paths.insert(dirty.clone());
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    assert!(app.ui_state.app_quit_confirm.is_some());

    let _ = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::SaveAll,
    )));

    // Modal resolved, but the unsaveable file is reported and retained.
    assert!(app.ui_state.app_quit_confirm.is_none());
    assert!(
        app.document_state.export_error.is_some(),
        "Save All must report files it could not save"
    );
    assert!(
        app.document_state.dirty_paths.contains(&dirty),
        "an unsaveable dirty file must be kept, never silently dropped"
    );
}

// ─────────────────────────────────────────────────────────────────
// New Project must not truncate an existing .snxprj (issue #104)
// ─────────────────────────────────────────────────────────────────

#[test]
fn new_project_over_existing_non_empty_snxprj_is_refused() {
    // File ▸ New Project pointing at an existing, non-empty .snxprj used
    // to write an empty marker over it, destroying the project. The
    // create guard must leave the existing file byte-for-byte intact.
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Board");
    let before = std::fs::read(&prj_path).expect("read .snxprj");
    assert!(!before.is_empty(), "fixture .snxprj should be non-empty");

    let _ = app.update(Message::File(FileMsg::NewProject(Some(prj_path.clone()))));

    let after = std::fs::read(&prj_path).expect("read .snxprj after");
    assert_eq!(
        after, before,
        "New Project must not overwrite an existing .snxprj"
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.13.0 — footprint editor gated off for release
//
// The footprint / sketch editor is feature-incomplete and is hidden
// behind `signex_app::feature_flags::FOOTPRINT_EDITOR_ENABLED` for the
// v0.13.0 "Symbol & Library" release. These tests pin the gate at the
// two behavioural funnels every footprint-editor entry routes through:
//
//   * OPEN  — `Message::File(FileMsg::Opened)` → `open_document_path` →
//     `handle_open_primitive` ("snxfpt" arm). A valid `.snxfpt` must
//     NOT push an editable FootprintEditor tab while the flag is off.
//   * The symbol path (`.snxsym`) is the positive control — it MUST
//     still open, proving the gate is footprint-specific, not a
//     blanket primitive-open break.
//
// When the footprint editor is ready, flip the flag to `true` and
// these tests flip with it (the open-blocked assertion is guarded on
// the flag so it documents intent rather than hard-coding "off").
// ─────────────────────────────────────────────────────────────────

/// Write a valid single-footprint `.snxfpt` envelope to `path`.
/// Uses the real `FootprintFile::to_toml_string` so the file parses
/// cleanly — the test then proves the *gate* blocks the open, not a
/// parse failure (which would be a false green).
fn write_valid_snxfpt(path: &Path, name: &str) {
    use signex_library::{Footprint, FootprintFile};
    let file = FootprintFile::from_footprint(Footprint::empty(name));
    let toml = file.to_toml_string().expect("serialise .snxfpt envelope");
    fs::write(path, toml).expect("write .snxfpt");
}

/// Write a valid single-symbol `.snxsym` envelope to `path`.
fn write_valid_snxsym(path: &Path, name: &str) {
    use signex_library::{Symbol, SymbolFile};
    let file = SymbolFile::from_symbol(Symbol::empty(name));
    let toml = file.to_toml_string().expect("serialise .snxsym envelope");
    fs::write(path, toml).expect("write .snxsym");
}

#[test]
fn opening_snxfpt_does_not_create_editable_tab_when_gated() {
    use signex_app::app::TabKind;
    let tmp = TempDir::new().expect("tempdir");
    let fpt = tmp.path().join("gated.snxfpt");
    write_valid_snxfpt(&fpt, "GATED");
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::File(FileMsg::Opened(Some(fpt.clone()))));
    let opened_footprint_tab = app
        .document_state
        .tabs
        .iter()
        .any(|t| matches!(t.kind, TabKind::FootprintEditor(_)));
    if signex_app::feature_flags::FOOTPRINT_EDITOR_ENABLED {
        assert!(
            opened_footprint_tab,
            "flag is ON — a valid .snxfpt should open a FootprintEditor tab"
        );
    } else {
        assert!(
            !opened_footprint_tab,
            "flag is OFF — opening a .snxfpt must not create a FootprintEditor tab"
        );
        assert!(
            !app.document_state.footprint_editors.contains_key(&fpt),
            "flag is OFF — no FootprintEditorState should be registered for the path"
        );
    }
}

#[test]
fn opening_snxsym_still_creates_editable_tab() {
    use signex_app::app::TabKind;
    let tmp = TempDir::new().expect("tempdir");
    let sym = tmp.path().join("control.snxsym");
    write_valid_snxsym(&sym, "CONTROL");
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::File(FileMsg::Opened(Some(sym.clone()))));
    // Positive control: the symbol editor is the headline feature of
    // v0.13.0 and must open regardless of the footprint gate.
    assert!(
        app.document_state
            .tabs
            .iter()
            .any(|t| matches!(t.kind, TabKind::SymbolEditor(_))),
        "a valid .snxsym must open a SymbolEditor tab (footprint gate must not affect symbols)"
    );
    assert!(
        app.document_state.symbol_editors.contains_key(&sym),
        "a SymbolEditorState should be registered for the opened .snxsym path"
    );
}
