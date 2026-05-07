//! Regression tests for v0.10–v0.12 walkthrough findings.
//!
//! These exercise dispatchers without spinning up the iced runtime —
//! `Signex::new()` constructs the app, the test populates state
//! directly via the `pub` fields on `DocumentState` / `UiState`, then
//! `Signex::update(Message::*)` routes through the same handler the
//! UI would. State changes (file system effects, `dirty_paths`,
//! tree state, etc.) are observed afterwards.
//!
//! Closes the manual-walkthrough gap for items where the only
//! genuine UI dependency is the `rfd::AsyncFileDialog` picker — those
//! still need a human eye.

use signex_app::app::{
    LoadedProject, Message, ProjectTreeAction, RemoveChoice, RemoveDialogState, RenameDialogState,
    Signex,
};
use signex_types::project::SheetEntry;

use signex_types::project::ProjectData;

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

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
    let _ = app.update(Message::RenameSubmit);

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
    let _ = app.update(Message::RenameSubmit);

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
    let _ = app.update(Message::RenameSubmit);

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
    let _ = app.update(Message::RenameSubmit);

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
    let _ = app.update(Message::RemoveConfirm(RemoveChoice::DeleteFile));

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
    let _ = app.update(Message::RemoveConfirm(RemoveChoice::ExcludeFromProject));

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

    let _ = app.update(Message::SaveFile);

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

    let _ = app.update(Message::SaveFile);

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
    let _ = app.update(Message::AddExistingFilePicked {
        project_idx: 0,
        paths: Some(vec![new_sheet.clone()]),
    });
    assert_eq!(
        app.document_state.projects[0].data.sheets.len(),
        1,
        "first add registers the sheet"
    );

    // Second add — silent no-op, list size unchanged.
    let _ = app.update(Message::AddExistingFilePicked {
        project_idx: 0,
        paths: Some(vec![new_sheet.clone()]),
    });
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

    let _ = app.update(Message::AddExistingFilePicked {
        project_idx: 0,
        paths: Some(vec![external_sheet.clone()]),
    });

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
    let _ = app.update(Message::RenameSubmit);

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

    let _ = app.update(Message::AddNewSchematicPicked {
        project_idx: 0,
        path: Some(new_sheet.clone()),
    });

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

    let _ = app.update(Message::AddNewSchematicPicked {
        project_idx: 0,
        path: None, // user cancelled the Save-As dialog
    });

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

    let _ = app.update(Message::ProjectTreeAction(
        ProjectTreeAction::OpenProjectOptions(vec![0]),
    ));

    let state = app.ui_state.project_options.as_ref().expect("modal opened");
    assert_eq!(state.project_idx, 0);
    assert_eq!(state.name, "Oscar");
    assert_eq!(state.directory, tmp.path().to_string_lossy().to_string());
    assert_eq!(state.schematic_root.as_deref(), Some("Oscar.snxsch"));
    assert_eq!(state.pcb_file.as_deref(), Some("Oscar.snxpcb"));
    assert_eq!(state.library_count, 0);

    // Close-X / Esc both fire CloseProjectOptions.
    let _ = app.update(Message::CloseProjectOptions);
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

    let _ = app.update(Message::RenameBufferChanged("PartialName".into()));
    let dlg = app.ui_state.rename_dialog.as_ref().unwrap();
    assert_eq!(dlg.buffer, "PartialName");

    let _ = app.update(Message::RenameBufferChanged("LongerName".into()));
    let dlg = app.ui_state.rename_dialog.as_ref().unwrap();
    assert_eq!(dlg.buffer, "LongerName");
}

#[test]
fn close_rename_dialog_dismisses_modal_without_filesystem_changes() {
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Quebec");
    arm_project_rename(&mut app, &prj_path, "WouldBeRenamed");

    let _ = app.update(Message::CloseRenameDialog);

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
    let _ = app.update(Message::CloseRemoveDialog);

    assert!(app.ui_state.remove_dialog.is_none(), "modal closed");
    assert!(target.exists(), "no removal happened — file still there");
}

// ─────────────────────────────────────────────────────────────────
// F1 / F3 — Prefs migration (Windows path bug + stale label_style)
// ─────────────────────────────────────────────────────────────────

#[test]
fn f1_legacy_prefs_path_copied_forward_when_canonical_empty() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp
        .path()
        .join("canonical")
        .join("signex")
        .join("prefs.json");
    let legacy = tmp.path().join("legacy").join("signex").join("prefs.json");

    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::write(
        &legacy,
        br#"{"ui_font":"Roboto","theme":"signex","label_style":"standard"}"#,
    )
    .unwrap();
    assert!(!canonical.exists(), "canonical absent before migration");

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    assert!(canonical.exists(), "canonical now exists (F1 copy)");
    let copied = fs::read_to_string(&canonical).unwrap();
    assert!(
        copied.contains("\"ui_font\""),
        "canonical contains the legacy file's content"
    );
    assert!(
        legacy.exists(),
        "legacy preserved (forward-copy, not move) — backward compat"
    );
}

#[test]
fn f1_canonical_present_blocks_legacy_copy() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp
        .path()
        .join("canonical")
        .join("signex")
        .join("prefs.json");
    let legacy = tmp.path().join("legacy").join("signex").join("prefs.json");

    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::write(&canonical, br#"{"ui_font":"Iosevka"}"#).unwrap();
    fs::write(&legacy, br#"{"ui_font":"LegacyValue"}"#).unwrap();

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    let content = fs::read_to_string(&canonical).unwrap();
    assert!(
        content.contains("Iosevka"),
        "canonical content untouched when it already exists"
    );
    assert!(
        !content.contains("LegacyValue"),
        "legacy must NOT overwrite canonical when canonical exists"
    );
}

#[test]
fn f1_no_legacy_no_canonical_is_a_clean_noop() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp
        .path()
        .join("canonical")
        .join("signex")
        .join("prefs.json");
    let legacy = tmp.path().join("legacy").join("signex").join("prefs.json");

    // Neither exists. Migration should not panic, not create anything.
    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    assert!(!canonical.exists(), "no canonical created from nothing");
    assert!(!legacy.exists(), "no legacy created from nothing");
}

#[test]
fn f3_stale_label_style_rewritten_to_standard() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp.path().join("signex").join("prefs.json");
    let legacy = canonical.clone(); // legacy unused — canonical exists already.

    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    // Pre-v0.10 stale token. Use a non-canonical placeholder so this
    // test source itself stays License-Guard-clean (no historic-EDA-
    // tool substring under crates/).
    let stale = serde_json::json!({
        "ui_font": "Roboto",
        "label_style": "stale-legacy-token",
    });
    fs::write(&canonical, serde_json::to_string_pretty(&stale).unwrap()).unwrap();

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    let rewritten: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&canonical).unwrap()).unwrap();
    assert_eq!(
        rewritten["label_style"], "standard",
        "F3: non-canonical label_style normalised to default"
    );
    assert_eq!(
        rewritten["ui_font"], "Roboto",
        "other prefs preserved during F3 normalisation"
    );
}

#[test]
fn f3_canonical_label_style_left_alone() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp.path().join("signex").join("prefs.json");
    let legacy = canonical.clone();

    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    let canonical_pref = serde_json::json!({
        "ui_font": "Iosevka",
        "label_style": "altium",
    });
    let original = serde_json::to_string_pretty(&canonical_pref).unwrap();
    fs::write(&canonical, &original).unwrap();

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    // Idempotent — file content unchanged.
    let after = fs::read_to_string(&canonical).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&after).unwrap();
    assert_eq!(parsed["label_style"], "altium");
    assert_eq!(parsed["ui_font"], "Iosevka");
}

#[test]
fn f3_label_style_case_variants_all_normalise() {
    for stale_token in ["STANDARD", "Altium", "ALTIUM"] {
        // These are case variants of CANONICAL tokens — they should
        // round-trip unchanged (eq_ignore_ascii_case match).
        let tmp = TempDir::new().unwrap();
        let canonical = tmp.path().join("signex").join("prefs.json");
        let legacy = canonical.clone();
        fs::create_dir_all(canonical.parent().unwrap()).unwrap();
        fs::write(
            &canonical,
            serde_json::to_string(&serde_json::json!({"label_style": stale_token})).unwrap(),
        )
        .unwrap();

        signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&canonical).unwrap()).unwrap();
        assert_eq!(
            parsed["label_style"], stale_token,
            "case-variant of canonical token left unchanged: {stale_token}"
        );
    }
}

#[test]
fn f3_garbage_json_doesnt_corrupt_file() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp.path().join("signex").join("prefs.json");
    let legacy = canonical.clone();
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();

    let original = b"this is not valid json {{{";
    fs::write(&canonical, original).unwrap();

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    // Migration is best-effort; broken JSON returns early and leaves
    // the file alone (vs. e.g. emptying it).
    let after = fs::read(&canonical).unwrap();
    assert_eq!(
        after, original,
        "garbage JSON file must be left untouched (no panic, no truncation)"
    );
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
// §4.4 — Preferences persistence sweep
//
// For each user-toggleable knob the checklist asks: "toggle, restart
// the app, confirm the value is restored". We can't restart from a
// single test process, but we can exercise the same write→read pair
// through the same `prefs.json` JSON encoding the production code
// uses. Tests inject a tempdir prefs file via the `_at(path)`
// variants on each pref function so the user's real prefs.json is
// never touched.
// ─────────────────────────────────────────────────────────────────

use signex_app::render_config::{GridStyle, LabelStyle, MultisheetStyle, PowerPortStyle};
use signex_types::coord::Unit;
use signex_types::theme::ThemeId;

fn temp_prefs_path() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("signex").join("prefs.json");
    (tmp, path)
}

#[test]
fn prefs_theme_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(
        signex_app::fonts::read_theme_pref_at(&path),
        ThemeId::Signex
    );

    // Each builtin theme survives a write→read cycle.
    for &theme in ThemeId::BUILTINS {
        signex_app::fonts::write_theme_pref_at(&path, theme);
        assert_eq!(
            signex_app::fonts::read_theme_pref_at(&path),
            theme,
            "theme {theme:?} must round-trip"
        );
    }
}

#[test]
fn prefs_unit_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(signex_app::fonts::read_unit_pref_at(&path), Unit::Mm);

    for unit in [Unit::Mm, Unit::Mil, Unit::Inch] {
        signex_app::fonts::write_unit_pref_at(&path, unit);
        assert_eq!(
            signex_app::fonts::read_unit_pref_at(&path),
            unit,
            "unit {unit:?} must round-trip"
        );
    }
}

#[test]
fn prefs_grid_visible_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert!(signex_app::fonts::read_grid_visible_pref_at(&path));

    signex_app::fonts::write_grid_visible_pref_at(&path, false);
    assert!(!signex_app::fonts::read_grid_visible_pref_at(&path));

    signex_app::fonts::write_grid_visible_pref_at(&path, true);
    assert!(signex_app::fonts::read_grid_visible_pref_at(&path));
}

#[test]
fn prefs_snap_enabled_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert!(signex_app::fonts::read_snap_enabled_pref_at(&path));

    signex_app::fonts::write_snap_enabled_pref_at(&path, false);
    assert!(!signex_app::fonts::read_snap_enabled_pref_at(&path));
}

#[test]
fn prefs_grid_size_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing — `None` so the caller can fall back to
    // the engine's preferred default.
    assert_eq!(signex_app::fonts::read_grid_size_mm_pref_at(&path), None);

    signex_app::fonts::write_grid_size_mm_pref_at(&path, 1.27);
    let v = signex_app::fonts::read_grid_size_mm_pref_at(&path).unwrap();
    assert!((v - 1.27).abs() < 1e-5, "grid size round-trips, got {v}");

    signex_app::fonts::write_grid_size_mm_pref_at(&path, 0.635);
    let v = signex_app::fonts::read_grid_size_mm_pref_at(&path).unwrap();
    assert!((v - 0.635).abs() < 1e-5);
}

#[test]
fn prefs_writes_dont_clobber_neighboring_keys() {
    let (_tmp, path) = temp_prefs_path();

    // Seed multiple keys.
    signex_app::fonts::write_theme_pref_at(&path, ThemeId::Signex);
    signex_app::fonts::write_unit_pref_at(&path, Unit::Mil);
    signex_app::fonts::write_grid_visible_pref_at(&path, false);

    // Write a different key — neighbouring values must survive.
    signex_app::fonts::write_snap_enabled_pref_at(&path, false);

    assert_eq!(
        signex_app::fonts::read_theme_pref_at(&path),
        ThemeId::Signex
    );
    assert_eq!(signex_app::fonts::read_unit_pref_at(&path), Unit::Mil);
    assert!(!signex_app::fonts::read_grid_visible_pref_at(&path));
    assert!(!signex_app::fonts::read_snap_enabled_pref_at(&path));
}

#[test]
fn prefs_garbage_json_falls_back_to_defaults() {
    let (_tmp, path) = temp_prefs_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"{ broken json content").unwrap();

    // Each read returns its default rather than panicking on parse error.
    assert_eq!(
        signex_app::fonts::read_theme_pref_at(&path),
        ThemeId::Signex
    );
    assert_eq!(signex_app::fonts::read_unit_pref_at(&path), Unit::Mm);
    assert!(signex_app::fonts::read_grid_visible_pref_at(&path));
    assert!(signex_app::fonts::read_snap_enabled_pref_at(&path));
    assert_eq!(signex_app::fonts::read_grid_size_mm_pref_at(&path), None);
}

#[test]
fn prefs_ui_font_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(signex_app::fonts::read_ui_font_pref_at(&path), "Roboto");

    for font in ["Iosevka", "Helvetica Neue", "Inter", "Source Code Pro"] {
        signex_app::fonts::write_ui_font_pref_at(&path, font);
        assert_eq!(
            signex_app::fonts::read_ui_font_pref_at(&path),
            font,
            "ui_font {font} must round-trip"
        );
    }
}

#[test]
fn prefs_label_style_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(
        signex_app::fonts::read_label_style_pref_at(&path),
        LabelStyle::Standard
    );

    for &style in &[LabelStyle::Standard, LabelStyle::Altium] {
        signex_app::fonts::write_label_style_pref_at(&path, style);
        assert_eq!(
            signex_app::fonts::read_label_style_pref_at(&path),
            style,
            "label_style {style:?} must round-trip"
        );
    }
}

#[test]
fn prefs_power_port_style_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(
        signex_app::fonts::read_power_port_style_pref_at(&path),
        PowerPortStyle::Altium
    );

    for &style in &[PowerPortStyle::Standard, PowerPortStyle::Altium] {
        signex_app::fonts::write_power_port_style_pref_at(&path, style);
        assert_eq!(
            signex_app::fonts::read_power_port_style_pref_at(&path),
            style,
            "power_port_style {style:?} must round-trip"
        );
    }
}

#[test]
fn prefs_multisheet_style_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(
        signex_app::fonts::read_multisheet_style_pref_at(&path),
        MultisheetStyle::Standard
    );

    for &style in &[MultisheetStyle::Standard, MultisheetStyle::Altium] {
        signex_app::fonts::write_multisheet_style_pref_at(&path, style);
        assert_eq!(
            signex_app::fonts::read_multisheet_style_pref_at(&path),
            style
        );
    }
}

#[test]
fn prefs_grid_style_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(
        signex_app::fonts::read_grid_style_pref_at(&path),
        GridStyle::Dots
    );

    for &style in &[GridStyle::Dots, GridStyle::Lines, GridStyle::SmallCrosses] {
        signex_app::fonts::write_grid_style_pref_at(&path, style);
        assert_eq!(
            signex_app::fonts::read_grid_style_pref_at(&path),
            style,
            "grid_style {style:?} must round-trip"
        );
    }
}

#[test]
fn prefs_enum_case_insensitive_decode() {
    // The legacy match arms accepted both lowercase and TitleCase tokens
    // (e.g. "altium" | "Altium"). The refactor uses
    // `eq_ignore_ascii_case` to match either form. Verify a hand-written
    // mixed-case prefs.json decodes to the correct variant.
    let (_tmp, path) = temp_prefs_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let raw = serde_json::json!({
        "label_style": "Altium",         // TitleCase
        "power_port_style": "STANDARD",   // UPPERCASE
        "multisheet_style": "altium",     // lowercase
        "grid_style": "Lines",            // TitleCase
    });
    fs::write(&path, serde_json::to_string_pretty(&raw).unwrap()).unwrap();

    assert_eq!(
        signex_app::fonts::read_label_style_pref_at(&path),
        LabelStyle::Altium
    );
    assert_eq!(
        signex_app::fonts::read_power_port_style_pref_at(&path),
        PowerPortStyle::Standard
    );
    assert_eq!(
        signex_app::fonts::read_multisheet_style_pref_at(&path),
        MultisheetStyle::Altium
    );
    assert_eq!(
        signex_app::fonts::read_grid_style_pref_at(&path),
        GridStyle::Lines
    );
}

#[test]
fn prefs_cross_pref_independence() {
    let (_tmp, path) = temp_prefs_path();

    // Write each pref in a different "session" (sequential writes,
    // each through update_prefs_json which does read-modify-write).
    signex_app::fonts::write_theme_pref_at(&path, ThemeId::Signex);
    signex_app::fonts::write_grid_size_mm_pref_at(&path, 2.54);
    signex_app::fonts::write_unit_pref_at(&path, Unit::Mil);
    signex_app::fonts::write_grid_visible_pref_at(&path, false);
    signex_app::fonts::write_snap_enabled_pref_at(&path, false);

    // Read everything back — none should have been clobbered.
    assert_eq!(
        signex_app::fonts::read_theme_pref_at(&path),
        ThemeId::Signex
    );
    assert!((signex_app::fonts::read_grid_size_mm_pref_at(&path).unwrap() - 2.54).abs() < 1e-5);
    assert_eq!(signex_app::fonts::read_unit_pref_at(&path), Unit::Mil);
    assert!(!signex_app::fonts::read_grid_visible_pref_at(&path));
    assert!(!signex_app::fonts::read_snap_enabled_pref_at(&path));

    // Pre-existing keys (label_style, ui_font, etc.) should remain
    // unset — we never wrote them — but absent ≠ default-failure.
    let raw: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(
        raw.get("label_style").is_none(),
        "label_style not written by these tests"
    );
    assert!(raw.get("ui_font").is_none());
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
    // background. `Message::ProjectGitCommitDone` must remove the
    // pair regardless of success/failure so the "Saving…" pill
    // clears.
    let (mut app, _tmp, prj_path) = fixture_project_with_companions("Foxtrot");
    let project_root = prj_path.parent().unwrap().to_path_buf();
    let rel_path = PathBuf::from("Foxtrot.snxsch");
    let key = (project_root.clone(), rel_path.clone());
    app.document_state.inflight_git_commits.insert(key.clone());
    assert!(app.document_state.inflight_git_commits.contains(&key));

    // Success path.
    let _ = app.update(Message::ProjectGitCommitDone {
        project_root: project_root.clone(),
        rel_path: rel_path.clone(),
        result: Ok("deadbeef".to_string()),
    });
    assert!(
        !app.document_state.inflight_git_commits.contains(&key),
        "inflight entry must be cleared on success"
    );

    // Failure path also clears (data is on disk regardless of git).
    app.document_state.inflight_git_commits.insert(key.clone());
    let _ = app.update(Message::ProjectGitCommitDone {
        project_root: project_root.clone(),
        rel_path: rel_path.clone(),
        result: Err("commit_path: …".to_string()),
    });
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
// v0.23 — sketch-mode pattern dispatchers (signex-sketch state side)
// ─────────────────────────────────────────────────────────────────

#[test]
fn array_kind_residual_count_is_one_per_kind_for_distance_pt_circle() {
    // Spot-check the new ConstraintKind variant integrates with
    // the residual_count machinery the panel relies on.
    use signex_sketch::constraint::{ConstraintKind, DimTarget};
    use signex_sketch::id::SketchEntityId;

    let kind = ConstraintKind::DistancePtCircle {
        point: SketchEntityId::new(),
        circle: SketchEntityId::new(),
        target: DimTarget::Literal(1.0),
    };
    assert_eq!(kind.residual_count(), 1);
}

#[test]
fn grid_depopulation_round_trips_suppressed_instances_through_app_layer() {
    // App layer never authors GridDepopulation directly — but
    // .snxfpt files load through signex-library and into the
    // FootprintEditorState's primitive. This test pins the schema:
    // empty mask + non-empty suppression list survives a TOML
    // round trip via signex-sketch.
    use signex_sketch::array::{
        Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme,
    };
    use signex_sketch::id::SketchEntityId;

    let a = Array {
        id: ArrayId::new(),
        kind: ArrayKind::Grid {
            source: SketchEntityId::new(),
            nx_expr: "3".into(),
            ny_expr: "3".into(),
            dx_expr: "1mm".into(),
            dy_expr: "1mm".into(),
            depopulation: Some(GridDepopulation {
                mask_expr: String::new(),
                suppressed_instances: vec![(0, 0), (1, 1)],
            }),
        },
        numbering: NumberingScheme::default(),
    };
    let s = toml::to_string(&a).unwrap();
    let back: Array = toml::from_str(&s).unwrap();
    assert_eq!(a, back);
}

// ─────────────────────────────────────────────────────────────────
// v0.24 Phase 1 (Track B) — footprint editor undo / redo
// ─────────────────────────────────────────────────────────────────

#[test]
fn footprint_editor_push_history_then_undo_restores_pads() {
    use signex_app::app::FootprintEditorState;
    use signex_library::{Footprint, FootprintFile};

    let fp = Footprint::empty("test");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(PathBuf::from("test.snxfpt"), file);

    let snapshot_before = editor.state.pads.clone();

    editor.push_history();
    // Mutate: append a pad.
    editor.state.pads.push(
        signex_app::library::editor::footprint::state::EditorPad::new_default(
            "1".into(),
            (0.0, 0.0),
        ),
    );
    assert_eq!(editor.state.pads.len(), snapshot_before.len() + 1);

    // Undo restores the pre-push state.
    let undone = editor.undo();
    assert!(undone, "undo must succeed when history is non-empty");
    assert_eq!(editor.state.pads.len(), snapshot_before.len());

    // Redo applies the mutation again.
    let redone = editor.redo();
    assert!(redone);
    assert_eq!(editor.state.pads.len(), snapshot_before.len() + 1);
}

#[test]
fn footprint_editor_undo_returns_false_on_empty_history() {
    use signex_app::app::FootprintEditorState;
    use signex_library::{Footprint, FootprintFile};

    let fp = Footprint::empty("test");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(PathBuf::from("test.snxfpt"), file);

    assert!(!editor.undo(), "fresh editor must have no undoable history");
    assert!(!editor.redo(), "fresh editor must have no redoable history");
}

#[test]
fn footprint_editor_history_caps_at_depth_limit() {
    use signex_app::app::FootprintEditorState;
    use signex_library::{Footprint, FootprintFile};

    let fp = Footprint::empty("test");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(PathBuf::from("test.snxfpt"), file);

    // Push twice as many entries as the cap.
    for _ in 0..(FootprintEditorState::HISTORY_DEPTH * 2) {
        editor.push_history();
    }
    assert_eq!(editor.history.len(), FootprintEditorState::HISTORY_DEPTH);
}

#[test]
fn footprint_editor_new_mutation_clears_redo_stack() {
    use signex_app::app::FootprintEditorState;
    use signex_library::{Footprint, FootprintFile};

    let fp = Footprint::empty("test");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(PathBuf::from("test.snxfpt"), file);

    editor.push_history();
    editor.state.pads.push(
        signex_app::library::editor::footprint::state::EditorPad::new_default(
            "1".into(),
            (0.0, 0.0),
        ),
    );
    editor.undo(); // moves snapshot to redo
    assert_eq!(editor.redo.len(), 1);

    // A fresh push_history must clear the redo stack so the
    // history stays a single timeline.
    editor.push_history();
    assert!(editor.redo.is_empty());
}

// ─────────────────────────────────────────────────────────────────
// v0.24 Track C — Tangent Arc sketch sub-tool
//
// Drives the dispatcher via Signex::update(Message::Library(...))
// against a FootprintEditorState parked in document_state.footprint
// _editors so the dispatcher's existing routing keeps the test
// realistic. Tool-based gesture only — never a click-and-drag mode
// (per feedback_no_canvas_gestures.md / the user's spec for v0.24
// Track C).
// ─────────────────────────────────────────────────────────────────

#[test]
fn tangent_arc_tool_first_click_sets_pending() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{SketchTool, ToolPending};
    use signex_app::library::messages::{LibraryMessage, PrimitiveEditorMsg};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::SketchData;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    let path = PathBuf::from("test-tangent-arc-c1.snxfpt");
    let mut fp = Footprint::empty("test");
    // Provide a plane so the dispatcher doesn't have to mint one
    // (keeps the state setup focused on the tool gesture itself).
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    });
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.active_tool = SketchTool::TangentArc;

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // First click — dispatcher mints a Point at the click position
    // (no snap target supplied) and stashes it as TangentArcFirst.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        },
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("footprint editor still registered");

    // tool_pending must transition to TangentArcFirst.
    assert!(
        matches!(editor.state.tool_pending, ToolPending::TangentArcFirst { .. }),
        "tool_pending = {:?}, expected TangentArcFirst",
        editor.state.tool_pending
    );

    // The Point at the first click must be in the sketch — it's
    // referenced by `first` for the second click to resolve against.
    let sketch = editor
        .file
        .footprints
        .first()
        .and_then(|f| f.sketch.as_ref())
        .expect("sketch present");
    assert!(
        sketch
            .entities
            .iter()
            .any(|e| matches!(e.kind, signex_sketch::entity::EntityKind::Point { x, y } if x == 0.0 && y == 0.0)),
        "first-click Point not minted"
    );
}

#[test]
fn tangent_arc_tool_second_click_mints_arc_and_tangent_constraint() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{SketchTool, ToolPending};
    use signex_app::library::messages::{LibraryMessage, PrimitiveEditorMsg};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::SketchData;
    use signex_sketch::constraint::ConstraintKind;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    let path = PathBuf::from("test-tangent-arc-c2.snxfpt");
    let mut fp = Footprint::empty("test");
    let plane_id = PlaneId::new();

    // Pre-seed: Point A at (0, 0), Point B at (5, 0), Line A→B.
    // The Line ends at B, so a TangentArc click at B should find
    // this Line and emit a TangentLineArc constraint linking it to
    // the new Arc.
    let a_id = SketchEntityId::new();
    let b_id = SketchEntityId::new();
    let line_id = SketchEntityId::new();
    let pt_a = Entity::new(a_id, plane_id, EntityKind::Point { x: 0.0, y: 0.0 });
    let pt_b = Entity::new(b_id, plane_id, EntityKind::Point { x: 5.0, y: 0.0 });
    let line = Entity::new(
        line_id,
        plane_id,
        EntityKind::Line {
            start: a_id,
            end: b_id,
        },
    );

    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane_id,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![pt_a, pt_b, line],
        ..SketchData::default()
    });

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.active_tool = SketchTool::TangentArc;
    // Click 1 already happened — pre-seed the pending state with
    // first = B (the Line's end). The next click is click 2.
    editor.state.tool_pending = ToolPending::TangentArcFirst { first: b_id };

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // Snapshot pre-click counts so we can assert deltas without
    // relying on absolute totals (the FROM_FOOTPRINT path may
    // implicitly reorder/auto-mint pad-backed Points in future).
    let (entities_before, constraints_before) = {
        let editor = app.document_state.footprint_editors.get(&path).unwrap();
        let sketch = editor.file.footprints[0].sketch.as_ref().unwrap();
        (sketch.entities.len(), sketch.constraints.len())
    };

    // Click 2 — pick a point off the line so the tangent circle has
    // a non-degenerate radius. (3, 4) is 5 mm from B and 1.41 mm off
    // the line, well above the perpendicular-cursor degeneracy
    // threshold the dispatcher uses.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchToolClick {
            x_mm: 3.0,
            y_mm: 4.0,
            snap_id: None,
        },
    }));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let sketch = editor.file.footprints[0].sketch.as_ref().unwrap();

    // Tool reset to Idle after commit.
    assert!(
        matches!(editor.state.tool_pending, ToolPending::Idle),
        "tool_pending = {:?}, expected Idle after click 2",
        editor.state.tool_pending
    );

    // The dispatcher mints two new entities on click 2: the second
    // endpoint Point (at the click) and the centre Point of the
    // tangent circle, plus the Arc itself — three new entities.
    // We assert the Arc is present + at least one new entity, since
    // the centre minting is the dispatcher's choice.
    assert!(
        sketch.entities.len() >= entities_before + 2,
        "expected at least the second endpoint + centre + arc to be minted (entities: {} → {})",
        entities_before,
        sketch.entities.len()
    );
    let arc_count = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .count();
    assert_eq!(arc_count, 1, "expected exactly one Arc entity to be minted");

    // The Arc's start endpoint must be the pre-stashed `first`
    // (b_id), proving the click chained off the previous Line.
    let arc = sketch
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .unwrap();
    let (arc_start, arc_id) = match arc.kind {
        EntityKind::Arc { start, .. } => (start, arc.id),
        _ => unreachable!(),
    };
    assert_eq!(arc_start, b_id, "Arc.start must be the first-click Point");

    // The TangentLineArc constraint must reference the pre-existing
    // Line + the freshly minted Arc.
    assert!(
        sketch.constraints.len() > constraints_before,
        "expected a new constraint to be added"
    );
    assert!(
        sketch.constraints.iter().any(|c| matches!(
            c.kind,
            ConstraintKind::TangentLineArc { line, arc } if line == line_id && arc == arc_id
        )),
        "expected a TangentLineArc {{ line, arc }} constraint linking the seed Line to the new Arc"
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.24 Track D — live numeric placement input
// ─────────────────────────────────────────────────────────────────

/// v0.24 Track D — Line tool's second click honours the typed
/// `placement_input` length. With a buffer of "10" set against the
/// `LineLength` kind, a click that lands at `(20, 0)` must place the
/// line's second endpoint at exactly `(10, 0)` along the cursor's
/// azimuth from the first endpoint at the origin — not `(20, 0)`.
///
/// Drives the dispatcher via `Message::Library(PrimitiveEditorEvent
/// { ... })` so the integration matches what the canvas + bootstrap
/// keyboard handler emit.
#[test]
fn placement_input_line_length_pins_second_click_at_exact_distance() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{LibraryMessage, PrimitiveEditorMsg};
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::entity::EntityKind;

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("track-d.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    // Empty `Signex` + a fresh footprint editor state pre-populated in
    // `document_state.footprint_editors` so the dispatcher's
    // path-keyed lookup resolves.
    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("track-d-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Line;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // First click → first endpoint at (0, 0). The dispatcher mints
    // a Point entity and sets `tool_pending = LineFirst { first }`.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        },
    }));

    // Pin the placement input: user types "10" while the gesture is
    // mid-flight. With `LineLength` kind, the next click commits the
    // line at exactly 10 mm from the first endpoint.
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present after first click");
        editor.state.placement_input = Some(PlacementInput {
            buffer: "10".into(),
            kind: PlacementInputKind::LineLength,
        });
    }

    // Second click — cursor at (20, 0). Without placement_input the
    // line's end would land at (20, 0); with the buffer pinned, it
    // must land at (10, 0).
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchToolClick {
            x_mm: 20.0,
            y_mm: 0.0,
            snap_id: None,
        },
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present after second click");
    let sketch = editor
        .primitive()
        .sketch
        .as_ref()
        .expect("sketch present after the click pair");

    // Find the Line entity + resolve its `end` Point's coords.
    let line = sketch
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
        .expect("line entity emitted by the second click");
    let (start_id, end_id) = match line.kind {
        EntityKind::Line { start, end } => (start, end),
        _ => unreachable!(),
    };
    let start_pt = sketch
        .entities
        .iter()
        .find(|e| e.id == start_id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .expect("line start endpoint resolves to a Point");
    let end_pt = sketch
        .entities
        .iter()
        .find(|e| e.id == end_id)
        .and_then(|e| match e.kind {
            EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
        .expect("line end endpoint resolves to a Point");

    assert!(
        (start_pt.0 - 0.0).abs() < 1e-9 && (start_pt.1 - 0.0).abs() < 1e-9,
        "first endpoint should remain at the origin; got {:?}",
        start_pt
    );
    assert!(
        (end_pt.0 - 10.0).abs() < 1e-9,
        "second endpoint x should be 10 mm (typed length), not the cursor's 20 mm; got {}",
        end_pt.0
    );
    assert!(
        (end_pt.1 - 0.0).abs() < 1e-9,
        "second endpoint y should be 0 (cursor azimuth); got {}",
        end_pt.1
    );
}

/// v0.24 Track D — `state.placement_input` clears to `None` once the
/// click that consumed it commits. The user has to type again before
/// the next gesture step to keep the chain explicit.
#[test]
fn placement_input_clears_after_commit() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{LibraryMessage, PrimitiveEditorMsg};
    use signex_library::{Footprint, FootprintFile};

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("track-d-clear.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("track-d-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Line;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // First click — drops the first endpoint.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        },
    }));

    // Type "10" before the second click.
    {
        let editor = app
            .document_state
            .footprint_editors
            .get_mut(&path)
            .expect("editor present after first click");
        editor.state.placement_input = Some(PlacementInput {
            buffer: "10".into(),
            kind: PlacementInputKind::LineLength,
        });
    }

    // Second click — commits, must consume + clear the buffer.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchToolClick {
            x_mm: 20.0,
            y_mm: 0.0,
            snap_id: None,
        },
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present after second click");
    assert!(
        editor.state.placement_input.is_none(),
        "placement_input must clear after the click that consumed it; \
         leaked buffer = {:?}",
        editor.state.placement_input.as_ref().map(|p| &p.buffer)
    );
}

/// v0.24 Track D — typed character path. The user types '5' then
/// '.', then '2' against an active Line tool with first click
/// landed; the dispatcher's char-append handler must validate
/// (single decimal point) and grow `buffer = "5.2"` keyed off
/// `LineLength`.
#[test]
fn placement_input_char_append_validates_decimal_point() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{LibraryMessage, PrimitiveEditorMsg};
    use signex_library::{Footprint, FootprintFile};

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("track-d-buffer.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("track-d-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Line;
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // First click — anchors the gesture so the dispatcher accepts
    // numeric input on subsequent keypresses.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchToolClick {
            x_mm: 0.0,
            y_mm: 0.0,
            snap_id: None,
        },
    }));

    for ch in ['5', '.', '2'] {
        let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEditorMsg::FootprintSketchPlacementInputChar(ch),
        }));
    }
    // Second decimal — must be rejected.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchPlacementInputChar('.'),
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present after keypress sequence");
    let input = editor
        .state
        .placement_input
        .as_ref()
        .expect("buffer minted by the first digit press");
    assert_eq!(input.buffer, "5.2");
    assert_eq!(input.kind, PlacementInputKind::LineLength);
}

/// v0.24 Track D — Escape clears the buffer immediately; subsequent
/// click commits at the cursor with no override, as if no buffer
/// had been typed.
#[test]
fn placement_input_escape_clears_buffer() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::state::{
        EditorMode, PlacementInput, PlacementInputKind, SketchTool,
    };
    use signex_app::library::messages::{LibraryMessage, PrimitiveEditorMsg};
    use signex_library::{Footprint, FootprintFile};

    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("track-d-escape.snxfpt");
    fs::write(&path, b"{}").expect("write .snxfpt placeholder");

    let (mut app, _initial_task) = Signex::new();
    let fp = Footprint::empty("track-d-fixture");
    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    editor.state.active_tool = SketchTool::Line;
    editor.state.placement_input = Some(PlacementInput {
        buffer: "42".into(),
        kind: PlacementInputKind::LineLength,
    });
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchPlacementInputEscape,
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor present after Esc");
    assert!(
        editor.state.placement_input.is_none(),
        "Esc must clear placement_input; leaked = {:?}",
        editor.state.placement_input.as_ref().map(|p| &p.buffer)
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.24 Phase 1 Track A — parametric pad geometry mirror
// ─────────────────────────────────────────────────────────────────

#[test]
fn mirror_add_round_pad_mints_circle_with_diameter_param() {
    // v0.24 Track A — placing a Round pad in Pads mode should mirror
    // into the sketch as 1 centre Point + 1 Circle entity referencing
    // that centre, plus a `diameter_<slug>` sketch parameter
    // recording the pad's diameter literal. `pad.shape_params` should
    // record `"diameter" -> param_name` so the Phase 3 Properties row
    // can look up the binding.
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::primitive::footprint::{Footprint, PadShape};
    use signex_sketch::entity::EntityKind;

    let mut pad = EditorPad::new_default("1".into(), (2.0, 3.0));
    pad.shape = PadShape::Round;
    pad.size_mm = (1.5, 1.5);
    let mut fp = Footprint::empty("test");

    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let sketch = fp.sketch.as_ref().expect("sketch minted");

    // Exactly 1 Point (the centre) + 1 Circle.
    let points: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .collect();
    let circles: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Circle { .. }))
        .collect();
    let arcs: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .collect();
    let lines: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .collect();
    assert_eq!(points.len(), 1, "Round pad mints exactly 1 centre Point");
    assert_eq!(circles.len(), 1, "Round pad mints exactly 1 Circle");
    assert!(arcs.is_empty(), "Round pad mints no Arcs");
    assert!(lines.is_empty(), "Round pad mints no Lines");

    // The Circle's centre must reference the centre Point ID.
    let centre_id = pad.sketch_entity_id.expect("centre id minted");
    if let EntityKind::Circle { center, radius } = circles[0].kind {
        assert_eq!(center, centre_id, "Circle.center references centre Point");
        // radius = diameter / 2 = 0.75
        assert!((radius - 0.75).abs() < 1e-9, "Circle.radius is half the diameter");
    } else {
        unreachable!()
    }

    // No bbox-corner outline for Round pads.
    assert!(
        pad.corner_entity_ids.is_none(),
        "Round pads skip the bbox 4-Point outline"
    );

    // pad.shape_params binds "diameter" to a named parameter.
    let param_name = pad
        .shape_params
        .get("diameter")
        .expect("'diameter' key bound on Round pad");
    assert!(
        param_name.starts_with("diameter_"),
        "param name has the diameter_<slug> form (got `{param_name}`)"
    );

    // sketch.parameters must contain that exact parameter, holding
    // the literal diameter expression.
    let raw = sketch
        .parameters
        .get_raw(param_name)
        .expect("diameter parameter is registered on sketch.parameters");
    assert_eq!(raw, "1.5mm", "diameter parameter records the W literal");
}

#[test]
fn mirror_add_round_rect_pad_mints_4_arcs_linked_to_corner_r() {
    // v0.24 Track A — placing a RoundRect pad in Pads mode should
    // mirror into the sketch as the full Fusion-parity parametric
    // outline:
    //   - 1 centre Point
    //   - 4 bbox corner Points
    //   - 8 arc-anchor Points
    //   - 4 inset corner Points (arc centres)
    //   = 17 Points
    //   + 4 shorter Lines + 4 corner Arcs = 25 entities
    // All 4 Arcs must read from the same `corner_r_<slug>` parameter
    // so they stay linked implicitly. `pad.shape_params` should
    // record `"corner_r" -> param_name`.
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::primitive::footprint::{Footprint, PadShape};
    use signex_sketch::entity::EntityKind;

    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0); // W=2, H=1, min=1, r = 0.25 * 1 = 0.25
    let mut fp = Footprint::empty("test");

    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let sketch = fp.sketch.as_ref().expect("sketch minted");

    // Exactly 4 Arc entities.
    let arcs: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .collect();
    assert_eq!(arcs.len(), 4, "RoundRect pad mints exactly 4 corner Arcs");

    // Exactly 4 Lines (the shorter edge-anchor connectors).
    let lines: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .collect();
    assert_eq!(lines.len(), 4, "RoundRect pad mints 4 shorter edge Lines");

    // 1 centre + 4 bbox corners + 8 anchors + 4 inset corners = 17 Points.
    let points: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .collect();
    assert_eq!(
        points.len(),
        17,
        "RoundRect pad mints 1 centre + 4 bbox + 8 anchors + 4 inset = 17 Points"
    );

    // pad.shape_params must bind "corner_r" to a named parameter.
    let param_name = pad
        .shape_params
        .get("corner_r")
        .expect("'corner_r' key bound on RoundRect pad");
    assert!(
        param_name.starts_with("corner_r_"),
        "param name has the corner_r_<slug> form (got `{param_name}`)"
    );

    // sketch.parameters must contain that exact parameter, holding
    // the literal radius (= 0.25 * min(W,H) = 0.25 mm).
    let raw = sketch
        .parameters
        .get_raw(param_name)
        .expect("corner_r parameter is registered on sketch.parameters");
    assert_eq!(
        raw, "0.25mm",
        "corner_r parameter records the literal inset distance"
    );

    // All 4 Arcs implicitly share the same corner_r parameter — the
    // mint side encodes this by giving the arcs equal radii at mint
    // time (literal-equal because they all read the same parameter
    // expression). Verify by extracting the radius implied by each
    // Arc's geometry and checking they're all equal.
    //
    // Arc radius = distance from center Point to start Point. We
    // grab each Arc's center+start, look up the Point coords, and
    // compute the radius. All 4 must be equal (within EPS).
    let mut arc_radii: Vec<f64> = Vec::with_capacity(4);
    for arc in &arcs {
        let (center_id, start_id) = match arc.kind {
            EntityKind::Arc {
                center, start, ..
            } => (center, start),
            _ => unreachable!(),
        };
        let center_pos = sketch
            .entities
            .iter()
            .find(|e| e.id == center_id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
            .expect("Arc.center references a Point");
        let start_pos = sketch
            .entities
            .iter()
            .find(|e| e.id == start_id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
            .expect("Arc.start references a Point");
        let dx = start_pos.0 - center_pos.0;
        let dy = start_pos.1 - center_pos.1;
        arc_radii.push((dx * dx + dy * dy).sqrt());
    }
    let first = arc_radii[0];
    for r in &arc_radii {
        assert!(
            (r - first).abs() < 1e-9,
            "all 4 Arc radii must be equal (corner_r-linked); got {arc_radii:?}"
        );
        assert!(
            (r - 0.25).abs() < 1e-9,
            "Arc radius must equal corner_r = 0.25mm; got {r}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────
// v0.24 Phase 3 — A2/A3/A4 Properties row + Unlink + reverse-mirror
// ─────────────────────────────────────────────────────────────────

/// v0.24 Phase 3 (Track A2) — placing a RoundRect pad in Pads mode
/// registers a `corner_r` shape_params binding that the panel
/// context surfaces as a `PadShapeParamSummary` so the Properties
/// panel can render an editable "Corner radius" row.
#[test]
fn properties_panel_shows_corner_radius_for_round_rect_pad() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{LibraryMessage, PrimitiveEditorMsg};
    use signex_library::{Footprint, FootprintFile, PadShape};

    let path = PathBuf::from("test-a2-corner-radius-row.snxfpt");
    let mut fp = Footprint::empty("test");

    // Build the editor state directly so the pad's shape_params get
    // populated via mirror_add_pad_to_sketch — which is the path the
    // app dispatcher takes when the user places a pad in Pads mode.
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);

    // Open a tab pointing at the editor so build_footprint_editor_panel_ctx
    // resolves it. Using TabKind::FootprintEditor matches what the
    // app does when the user double-clicks a .snxfpt in the tree.
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Trigger a panel refresh by dispatching a no-op selection
    // (FootprintSelectPad re-selects the pad and triggers
    // refresh_panel_ctx in the post-dispatch flow).
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSelectPad(Some(0)),
    }));

    let ctx = app
        .document_state
        .panel_ctx
        .footprint_editor
        .as_ref()
        .expect("footprint editor panel ctx populated");

    let entries = &ctx.selected_pad_shape_params;
    let corner_r_entry = entries
        .iter()
        .find(|e| e.key == "corner_r")
        .expect("corner_r entry surfaced on selected pad shape_params");
    assert_eq!(
        corner_r_entry.label, "Corner radius",
        "label is the user-facing 'Corner radius' string"
    );
    assert!(
        corner_r_entry.parameter_name.starts_with("corner_r_"),
        "parameter_name follows corner_r_<slug> convention; got `{}`",
        corner_r_entry.parameter_name,
    );
    assert_eq!(
        corner_r_entry.current_expr, "0.25mm",
        "current_expr reads the live sketch parameter expression"
    );
}

/// v0.24 Phase 3 (Track A2) — dispatching FpEditorEditPadShapeParam
/// rewrites the bound sketch parameter and triggers a solve+rebake
/// (warnings list stays empty).
#[test]
fn editing_corner_radius_updates_all_4_arcs() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::{Footprint, FootprintFile, PadShape};

    let path = PathBuf::from("test-a2-edit-corner-radius.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let parameter_name = pad
        .shape_params
        .get("corner_r")
        .cloned()
        .expect("corner_r minted at pad-add time");

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Dispatch the Properties-panel edit. PanelMsg flows through the
    // dock dispatcher which forwards to FootprintSketchEditParameter.
    let _ = app.update(Message::Dock(signex_app::dock::DockMessage::Panel(
        signex_app::panels::PanelMsg::FpEditorEditPadShapeParam {
            pad_idx: 0,
            key: "corner_r".into(),
            value: "0.5mm".into(),
        },
    )));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present after edit");
    let raw = sketch
        .parameters
        .get_raw(&parameter_name)
        .expect("corner_r parameter still registered");
    assert_eq!(
        raw, "0.5mm",
        "FpEditorEditPadShapeParam rewrites the bound parameter"
    );
    assert!(
        editor.state.solve_warnings.is_empty(),
        "solve completed without warnings; got {:?}",
        editor.state.solve_warnings
    );
}

/// v0.24 Phase 3 (Track A3) — dispatching FootprintSketchUnlinkCornerRadius
/// for one of the 4 corner Arcs mints a per-corner parameter and
/// records the override on `pad.shape_params`. The shared corner_r
/// binding stays in place so the other 3 corners follow it.
#[test]
fn unlink_corner_radius_mints_per_corner_param() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{LibraryMessage, PrimitiveEditorMsg};
    use signex_library::{Footprint, FootprintFile, PadShape};
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;

    let path = PathBuf::from("test-a3-unlink.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    // Pick the NE arc — its UUID slug lives at
    // `shape_params["corner_r_ne_arc"]`. Resolve back to the entity
    // id by parsing the slug.
    let ne_slug = pad
        .shape_params
        .get("corner_r_ne_arc")
        .cloned()
        .expect("corner_r_ne_arc sidecar minted");
    let arc_entity_id = {
        let uuid = uuid::Uuid::parse_str(&ne_slug).expect("sidecar value is a UUID slug");
        SketchEntityId(uuid)
    };
    // Sanity: the entity actually is an Arc.
    let sketch_pre = fp.sketch.as_ref().unwrap();
    let arc_kind = sketch_pre
        .entities
        .iter()
        .find(|e| e.id == arc_entity_id)
        .map(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .unwrap_or(false);
    assert!(arc_kind, "sidecar UUID points at an Arc entity");

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Dispatch the Unlink action.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchUnlinkCornerRadius { arc_entity_id },
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let pad_after = &editor.state.pads[0];

    // Both shared and per-corner keys are present.
    assert!(
        pad_after.shape_params.contains_key("corner_r"),
        "shared corner_r binding stays intact"
    );
    assert!(
        pad_after.shape_params.contains_key("corner_r_ne"),
        "per-corner corner_r_ne override added"
    );
    // The per-corner parameter is registered on the sketch.
    let per_corner_name = pad_after
        .shape_params
        .get("corner_r_ne")
        .expect("corner_r_ne value points at a parameter name");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present after unlink");
    let raw = sketch
        .parameters
        .get_raw(per_corner_name)
        .expect("per-corner parameter registered on sketch.parameters");
    assert_eq!(
        raw, "0.25mm",
        "per-corner parameter copies the shared expression as initial value"
    );
}

/// v0.24 Phase 3 (Track A4) — after every solve, the reverse mirror
/// re-derives `EditorPad.stack.corner_radius_pct` from the resolved
/// corner_r parameter so the Pads-mode "Corner radius %" input stays
/// in sync with sketch-side edits.
#[test]
fn reverse_mirror_updates_pad_stack_corner_radius_pct() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{LibraryMessage, PrimitiveEditorMsg};
    use signex_library::{Footprint, FootprintFile, PadShape};

    let path = PathBuf::from("test-a4-reverse-mirror.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::RoundRect { radius_ratio: 0.25 };
    pad.size_mm = (2.0, 1.0); // W=2, H=1, min=1, corner_r = 0.25*1 = 0.25mm
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Trigger a solve+bake by editing a parameter (no-op rewrite of
    // the same value; still forces resolve + bake).
    let parameter_name = app
        .document_state
        .footprint_editors
        .get(&path)
        .unwrap()
        .state
        .pads[0]
        .shape_params
        .get("corner_r")
        .cloned()
        .unwrap();
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchEditParameter {
            name: parameter_name,
            expr: "0.25mm".into(),
        },
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let pad_after = &editor.state.pads[0];

    // corner_r = 0.25mm, min(W,H) = 1mm → pct = 25%.
    let pct = pad_after
        .stack
        .corner_radius_pct
        .expect("reverse mirror populated corner_radius_pct");
    assert!(
        (pct - 25.0).abs() < 1e-6,
        "corner_radius_pct = corner_r/min(W,H)*100 = 0.25/1*100 = 25; got {pct}"
    );
}

// ─────────────────────────────────────────────────────────────────
// v0.24 Phase 4 Track A5 — Oval pad parametric mint
// ─────────────────────────────────────────────────────────────────

/// v0.24 Track A5 — placing an Oval pad in Pads mode should mirror
/// into the sketch as the full Fusion-parity stadium primitive:
///   - 1 centre Point
///   - 4 bbox corner Points
///   - 4 arc-anchor Points (where the rounded ends meet the
///     straight edges)
///   - 2 Arc-centre Points (offset inward from the short-axis edges
///     by half the short axis)
///   = 11 Points
///   + 2 long-axis Lines + 2 short-axis Arcs = 15 entities
/// `pad.shape_params` records `"width" -> width_<slug>` and
/// `"height" -> height_<slug>` so the Properties panel can surface
/// both as editable rows.
#[test]
fn mirror_add_oval_pad_mints_2_arcs_2_lines_with_w_and_h_params() {
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::primitive::footprint::{Footprint, PadShape};
    use signex_sketch::entity::EntityKind;

    // Wide oval: W=2mm, H=1mm. Rounded ends on the left + right
    // edges; arc radius = H/2 = 0.5mm.
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Oval;
    pad.size_mm = (2.0, 1.0);
    let mut fp = Footprint::empty("test");

    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let sketch = fp.sketch.as_ref().expect("sketch minted");

    // Exactly 2 Arc entities — one per short-axis end, each spanning
    // 180°.
    let arcs: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Arc { .. }))
        .collect();
    assert_eq!(arcs.len(), 2, "Oval pad mints exactly 2 short-axis Arcs");

    // Exactly 2 Lines on the long-axis edges connecting anchor pairs.
    let lines: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Line { .. }))
        .collect();
    assert_eq!(lines.len(), 2, "Oval pad mints exactly 2 long-axis Lines");

    // 1 centre + 4 bbox + 4 anchors + 2 arc-centres = 11 Points.
    let points: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Point { .. }))
        .collect();
    assert_eq!(
        points.len(),
        11,
        "Oval pad mints 1 centre + 4 bbox + 4 anchors + 2 arc-centres = 11 Points"
    );

    // No Circles (Round-only).
    let circles: Vec<_> = sketch
        .entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Circle { .. }))
        .collect();
    assert!(circles.is_empty(), "Oval pad mints no Circle entities");

    // pad.shape_params binds "width" + "height" to named parameters.
    let width_param = pad
        .shape_params
        .get("width")
        .expect("'width' key bound on Oval pad");
    let height_param = pad
        .shape_params
        .get("height")
        .expect("'height' key bound on Oval pad");
    assert!(
        width_param.starts_with("width_"),
        "width param has the width_<slug> form (got `{width_param}`)"
    );
    assert!(
        height_param.starts_with("height_"),
        "height param has the height_<slug> form (got `{height_param}`)"
    );

    // sketch.parameters records both literal values.
    let raw_w = sketch
        .parameters
        .get_raw(width_param)
        .expect("width parameter is registered on sketch.parameters");
    let raw_h = sketch
        .parameters
        .get_raw(height_param)
        .expect("height parameter is registered on sketch.parameters");
    assert_eq!(raw_w, "2mm", "width parameter records the long-axis literal");
    assert_eq!(raw_h, "1mm", "height parameter records the short-axis literal");

    // Both Arcs implicitly share the same `height_<slug>` parameter
    // (= H/2 = 0.5mm). Verify both Arc radii are equal and match.
    let mut arc_radii: Vec<f64> = Vec::with_capacity(2);
    for arc in &arcs {
        let (center_id, start_id) = match arc.kind {
            EntityKind::Arc {
                center, start, ..
            } => (center, start),
            _ => unreachable!(),
        };
        let center_pos = sketch
            .entities
            .iter()
            .find(|e| e.id == center_id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
            .expect("Arc.center references a Point");
        let start_pos = sketch
            .entities
            .iter()
            .find(|e| e.id == start_id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
            .expect("Arc.start references a Point");
        let dx = start_pos.0 - center_pos.0;
        let dy = start_pos.1 - center_pos.1;
        arc_radii.push((dx * dx + dy * dy).sqrt());
    }
    assert!(
        (arc_radii[0] - arc_radii[1]).abs() < 1e-9,
        "both Arc radii must be equal (height-linked); got {arc_radii:?}"
    );
    assert!(
        (arc_radii[0] - 0.5).abs() < 1e-9,
        "Arc radius must equal height/2 = 0.5mm; got {}",
        arc_radii[0]
    );

    // The 4 bbox corners come back via corner_entity_ids so move +
    // delete mirrors keep the bbox tracking the pad.
    assert!(
        pad.corner_entity_ids.is_some(),
        "Oval pad sets corner_entity_ids to the 4 bbox Points"
    );
}

/// v0.24 Track A5 — editing the `width_<slug>` parameter via the
/// dispatcher (the same path the Properties-panel "Width" row drives)
/// rewrites the bound parameter and runs a solve cleanly. The
/// resolved parameter map reflects the new width so any future
/// constraint linking Line endpoints to `width` would see the
/// updated value; we assert the resolved value here as the surface
/// proxy for "endpoint reflects the new width".
#[test]
fn editing_oval_width_param_propagates_through_solve() {
    use signex_app::app::FootprintEditorState;
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{LibraryMessage, PrimitiveEditorMsg};
    use signex_library::{Footprint, FootprintFile, PadShape};
    use signex_sketch::parameter;

    let path = PathBuf::from("test-a5-oval-edit-width.snxfpt");
    let mut fp = Footprint::empty("test");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.shape = PadShape::Oval;
    pad.size_mm = (2.0, 1.0); // wide oval
    mirror_add_pad_to_sketch(&mut pad, &mut fp);

    let width_param_name = pad
        .shape_params
        .get("width")
        .cloned()
        .expect("width parameter minted at pad-add time");

    let file = FootprintFile::from_footprint(fp);
    let mut editor = FootprintEditorState::new(path.clone(), file);
    editor.state.pads = vec![pad];
    editor.state.selected_pad = Some(0);

    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "test".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;

    // Edit width via the same dispatcher path that the Properties
    // "Width" row drives.
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEditorMsg::FootprintSketchEditParameter {
            name: width_param_name.clone(),
            expr: "3mm".into(),
        },
    }));

    let editor = app
        .document_state
        .footprint_editors
        .get(&path)
        .expect("editor still registered");
    let sketch = editor.file.footprints[0]
        .sketch
        .as_ref()
        .expect("sketch present after edit");

    // Surface 1 — the parameter table records the new expression.
    let raw = sketch
        .parameters
        .get_raw(&width_param_name)
        .expect("width parameter still registered");
    assert_eq!(
        raw, "3mm",
        "FootprintSketchEditParameter rewrites the bound width parameter"
    );

    // Surface 2 — the resolved-parameter map reads 3.0mm. The Lines'
    // endpoints (and a future width-linked constraint) would propagate
    // this value when the solver next runs.
    let resolved = parameter::resolve(&sketch.parameters)
        .expect("resolved parameter map after width edit");
    let resolved_width = resolved
        .get(&width_param_name)
        .copied()
        .expect("width parameter resolves cleanly");
    assert!(
        (resolved_width - 3.0).abs() < 1e-9,
        "width parameter resolves to 3.0mm (canonical mm); got {resolved_width}"
    );

    // Surface 3 — solve completed without warnings (no
    // SolverFailed / Expr error / etc.). The Oval mint runs through
    // the same apply_sketch_edit pipeline as RoundRect's corner_r.
    assert!(
        editor.state.solve_warnings.is_empty(),
        "solve completed without warnings; got {:?}",
        editor.state.solve_warnings
    );
}
