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
    };
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj_path.clone(),
        data,
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
    assert!(app.ui_state.rename_dialog.is_none(), "modal closed on success");
}

#[test]
fn f6_project_rename_refuses_to_overwrite_existing_target() {
    let (mut app, tmp, old_prj) = fixture_project_with_companions("Alpha");

    // Pre-existing target file blocks the rename.
    let collision = tmp.path().join("Beta.snxprj");
    fs::write(&collision, b"existing content").unwrap();

    arm_project_rename(&mut app, &old_prj, "Beta");
    let _ = app.update(Message::RenameSubmit);

    assert!(old_prj.exists(), "old .snxprj must NOT be removed on collision");
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

    assert!(!target.exists(), "DeleteFile must remove the file from disk");
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

    let state = app
        .ui_state
        .project_options
        .as_ref()
        .expect("modal opened");
    assert_eq!(state.project_idx, 0);
    assert_eq!(state.name, "Oscar");
    assert_eq!(
        state.directory,
        tmp.path().to_string_lossy().to_string()
    );
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
    assert!(prj_path.exists(), "no rename happened — original still there");
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
    let canonical = tmp.path().join("canonical").join("signex").join("prefs.json");
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
    let canonical = tmp.path().join("canonical").join("signex").join("prefs.json");
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
    let canonical = tmp.path().join("canonical").join("signex").join("prefs.json");
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
    };

    signex_types::project::write_project(&path, &data).expect("write");
    let loaded = signex_types::project::parse_project(&path).expect("parse");

    assert_eq!(loaded.name, "Hotel");
    assert_eq!(loaded.schematic_root.as_deref(), Some("Hotel.snxsch"));
    assert_eq!(loaded.pcb_file.as_deref(), Some("Hotel.snxpcb"));
    assert_eq!(loaded.variant_definitions.len(), 2);
    assert_eq!(loaded.active_variant.as_deref(), Some("Production"));
}
