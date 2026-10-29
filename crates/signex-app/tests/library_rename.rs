// Standalone binary: reads the process-wide, 200-entry diagnostics ring and installs the logger, which would turn every other test's logging into ring traffic.
//! #739 — renaming a project library's `.snxlib` from the project tree
//! must leave nothing pointing at the old path.
//!
//! Real libraries on disk (`generate_library`), a real project file, and
//! the same `RenameMsg` dispatcher the dialog uses. The rename is one
//! synchronous `update` (file move + re-point), so a test needs nothing
//! beyond the submit.

// Test scaffolding discards the `iced::Task` that `update` returns and the
// already-installed error from `init_logging`; `[lints]` is package-scoped,
// so each integration-test crate root carries its own allow.
#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results/Tasks with `let _ = ...`; test scaffolding, not production"
)]

#[expect(
    dead_code,
    reason = "this target includes the shared support module but uses only part of it"
)]
#[path = "integration/support/mod.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use signex_app::app::{
    FileMsg, LoadedProject, Message, ProjectId, RenameDialogState, RenameMsg, Signex, TabInfo,
    TabKind,
};
use signex_app::diagnostics;
use signex_app::library::messages::{LibraryMessage, PrimitiveEdit};
use signex_app::library::mount::{MountIntent, prepare_mount};
use signex_types::project::{
    LibraryEntry, LibraryEntryKind, ProjectData, parse_project, write_project,
};
use tempfile::TempDir;

use support::{Scale, generate_library};

fn send(app: &mut Signex, msg: Message) {
    let _ = app.update(msg);
}

/// Send `msg`, run the task it returns (an off-thread write) and hand each
/// completion to `update`, the way the iced runtime would.
fn send_and_complete(app: &mut Signex, msg: Message) {
    use futures::StreamExt as _;
    let task = app.update(msg);
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let completions = runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    });
    for message in completions {
        let _ = app.update(message);
    }
}

struct Fixture {
    app: Signex,
    _tmp: TempDir,
    project_id: ProjectId,
    prj: PathBuf,
    alpha: PathBuf,
    beta: PathBuf,
}

/// Built by deserialising only the fields the tests care about, so a field
/// added to `LibraryEntry` later takes its serde default and breaks nothing.
fn entry(rel: &str) -> LibraryEntry {
    serde_json::from_value(serde_json::json!({ "path": rel, "kind": "project_local" }))
        .expect("library entry")
}

/// Two project-local libraries (`alpha`, `beta`) mounted in a loaded project.
fn fixture() -> Fixture {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let alpha = generate_library(&dir, "alpha", &Scale::new("alpha", 2, 2)).expect("alpha");
    let beta = generate_library(&dir, "beta", &Scale::new("beta", 2, 2)).expect("beta");
    let prj = dir.join("proj.snxprj");
    let mut data: ProjectData = serde_json::from_value(serde_json::json!({
        "name": "proj",
        "dir": dir.to_string_lossy(),
        "schematic_root": null,
        "pcb_file": null,
    }))
    .expect("project data");
    data.libraries = vec![entry("alpha/alpha.snxlib"), entry("beta/beta.snxlib")];
    // The user's last save: the project exists on disk.
    write_project(&prj, &data).expect("write prj");

    let (mut app, _task) = Signex::new();
    let project_id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id: project_id,
        path: prj.clone(),
        data,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(project_id);
    app.library
        .open_library(alpha.clone())
        .expect("mount alpha");
    app.library.open_library(beta.clone()).expect("mount beta");
    Fixture {
        app,
        _tmp: tmp,
        project_id,
        prj,
        alpha,
        beta,
    }
}

/// A project that was never saved: loaded in memory, no `.snxprj` on disk.
fn unsaved_fixture() -> Fixture {
    let f = fixture();
    fs::remove_file(&f.prj).expect("remove prj");
    f
}

fn submit_rename(app: &mut Signex, target: &Path, new_name: &str) {
    app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: target.to_path_buf(),
        tree_path: vec![0, 0],
        buffer: new_name.to_string(),
        error: None,
        is_project_rename: false,
    });
    send(app, Message::Rename(RenameMsg::Submit));
}

fn entry_paths(app: &Signex) -> Vec<PathBuf> {
    app.document_state.projects[0]
        .data
        .libraries
        .iter()
        .map(|e| e.path.clone())
        .collect()
}

#[test]
fn rename_repoints_the_entry_marks_it_unsaved_and_keeps_the_library_mounted() {
    let _ = diagnostics::init_logging();
    let mut f = unsaved_fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");
    assert!(
        f.app.ui_state.rename_dialog.is_none(),
        "a project library must be renameable; dialog error: {:?}",
        f.app
            .ui_state
            .rename_dialog
            .as_ref()
            .and_then(|d| d.error.clone())
    );

    // The project entry follows the file; the other entry is untouched.
    assert_eq!(
        entry_paths(&f.app),
        vec![
            PathBuf::from("alpha/alpha2.snxlib"),
            PathBuf::from("beta/beta.snxlib")
        ]
    );
    // A never-saved project has no file to patch: none is created, and the
    // project is marked dirty so Save / Close / quit see it.
    assert!(!f.prj.exists(), "the rename must not save the project");
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert!(
        f.app
            .document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Save the project to keep the new path")),
        "the Messages panel must say the project needs saving"
    );
    // The mounted library moved with it — nothing keyed by the old path.
    assert!(f.app.library.library_at(&renamed).is_some());
    assert!(f.app.library.library_at(&f.alpha).is_none());
    assert!(f.app.library.set.contains_path(&renamed));
    assert!(!f.app.library.set.contains_path(&f.alpha));
    assert!(f.app.library.library_at(&f.beta).is_some());

    // After the explicit save, reopening resolves and mounts both entries.
    send(&mut f.app, Message::File(FileMsg::Save));
    let saved = parse_project(&f.prj).expect("project reparses");
    assert_eq!(
        saved
            .libraries
            .iter()
            .map(|e| e.path.clone())
            .collect::<Vec<_>>(),
        entry_paths(&f.app)
    );
    for e in &saved.libraries {
        let resolved = saved.resolve_library_path(e);
        prepare_mount(&resolved)
            .unwrap_or_else(|err| panic!("{} must mount: {err}", resolved.display()));
    }
}

#[test]
fn library_writes_after_a_rename_land_in_the_renamed_file() {
    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let id = f
        .app
        .library
        .library_at(&renamed)
        .expect("mounted")
        .library_id;
    f.app
        .library
        .set
        .get(id)
        .expect("adapter")
        .create_empty_table("fresh", "add table")
        .expect("table write");

    assert!(
        !f.alpha.exists(),
        "a stale adapter would resurrect the old .snxlib"
    );
    let text = fs::read_to_string(&renamed).expect("renamed file");
    assert!(
        text.contains("fresh"),
        "the new table must be in the renamed file"
    );
}

#[test]
fn an_open_footprint_editor_saves_and_commits_through_the_renamed_library() {
    use signex_app::app::FootprintDocument;
    use signex_library::FootprintFile;

    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    let fpt = fs::read_dir(f.alpha.parent().unwrap().join("footprints"))
        .expect("footprints dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "snxfpt"))
        .expect("a footprint file");
    let file =
        FootprintFile::from_toml_str(&fs::read_to_string(&fpt).expect("read")).expect("parse");
    let mut editor = FootprintDocument::new(fpt.clone(), file);
    // Trunk's saves write nothing for an unchanged footprint (#902), so make
    // a real edit for the save to commit.
    editor.file.footprints[0].name.push_str(" edited");
    editor.mark_dirty();
    f.app
        .document_state
        .footprint_editors
        .insert(fpt.clone(), editor);
    f.app.document_state.tabs.push(TabInfo {
        title: "fp".into(),
        path: fpt.clone(),
        cached_document: None,
        project_id: Some(f.project_id),
        kind: TabKind::FootprintEditor(fpt.clone()),
    });
    f.app.document_state.active_tab = 0;

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert!(
        f.app.document_state.footprint_editors.contains_key(&fpt),
        "the editor must survive the rename"
    );
    assert!(
        f.app.library.set.contains_path(&renamed),
        "the adapter that commits the editor's save must follow the renamed library"
    );
    send_and_complete(
        &mut f.app,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: fpt.clone(),
            msg: PrimitiveEdit::Save,
        }),
    );

    assert!(
        !f.app.document_state.footprint_editors[&fpt].is_dirty(),
        "the save must succeed"
    );
    assert!(
        !f.alpha.exists(),
        "the save must not resurrect the old .snxlib"
    );
    let repo = git2::Repository::open(f.alpha.parent().unwrap()).expect("repo");
    // The version cascade may commit row bumps on top of the save, so look
    // past those for the save itself.
    let mut commit = repo.head().unwrap().peel_to_commit().unwrap();
    while commit.message().unwrap_or("").starts_with("cascade:") {
        commit = commit.parent(0).expect("a commit below the cascade");
    }
    assert!(
        commit.message().unwrap_or("").starts_with("save footprint"),
        "the save must commit through the relocated adapter, got {:?}",
        commit.message()
    );
}

/// A dirty symbol draft at `<library>/symbols/d.snxsym`, behind its tab.
fn open_dirty_draft_in(f: &mut Fixture, library: &Path) -> PathBuf {
    use signex_app::app::SymbolDocument;
    use signex_library::{Symbol, SymbolFile};

    let path = library.join("symbols").join("d.snxsym");
    let mut editor = SymbolDocument::new(path.clone(), SymbolFile::from_symbol(Symbol::empty("D")));
    editor.mark_dirty();
    f.app
        .document_state
        .symbol_editors
        .insert(path.clone(), editor);
    f.app.document_state.tabs.push(TabInfo {
        title: "d".into(),
        path: path.clone(),
        cached_document: None,
        project_id: Some(f.project_id),
        kind: TabKind::SymbolEditor(path.clone()),
    });
    f.app.document_state.active_tab = f.app.document_state.tabs.len() - 1;
    path
}

#[test]
fn renaming_a_library_keeps_a_dirty_draft_inside_it_dirty_at_the_new_path() {
    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    let alpha = f.alpha.clone();
    let old = open_dirty_draft_in(&mut f, &alpha);
    let new = renamed.join("symbols").join("d.snxsym");

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let d = &f.app.document_state;
    assert!(d.symbol_editors.contains_key(&new), "the editor moved");
    assert!(!d.symbol_editors.contains_key(&old));
    assert!(d.is_document_dirty(&new), "the draft is still unsaved");
    assert!(!d.is_document_dirty(&old));
    assert_eq!(d.dirty_document_count(), 1);
    let tab = d.tabs.iter().find(|t| t.path == new).expect("tab moved");
    assert!(
        matches!(&tab.kind, TabKind::SymbolEditor(p) if *p == new),
        "the tab's kind follows the editor, or Save All writes the old folder"
    );
    assert!(d.is_document_dirty(&tab.path), "the tab dot stays on");
}

#[test]
fn a_library_rename_waits_for_a_draft_inside_it_that_is_being_saved() {
    let _ = diagnostics::init_logging();
    let mut f = fixture();
    let alpha = f.alpha.clone();
    let old = open_dirty_draft_in(&mut f, &alpha);
    // A write of the draft is in flight; the guard keeps it so.
    let _in_flight = f.app.document_state.document_saves.begin_work(old.clone());
    assert!(f.app.document_state.document_saves.is_saving(&old));

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert!(f.alpha.exists(), "nothing moved");
    assert!(
        f.app.ui_state.rename_dialog.is_some(),
        "the dialog stays open"
    );
    assert!(
        f.app.document_state.symbol_editors.contains_key(&old),
        "the draft stays where its write will land"
    );
    assert!(f.app.document_state.is_document_dirty(&old));
}

/// The project's volume folds case, so `ALPHA.snxlib` is the library.
fn fold_case(f: &mut Fixture) {
    f.app.document_state.projects[0].case_fold = signex_app::app::CaseFold {
        fold_case: true,
        backslash_is_separator: false,
        normalize_unicode: false,
    };
}

#[test]
fn a_draft_under_a_case_variant_of_the_library_is_rekeyed_too() {
    let mut f = fixture();
    fold_case(&mut f);
    let variant = f.alpha.with_file_name("ALPHA.snxlib");
    let old = open_dirty_draft_in(&mut f, &variant);
    let new = f
        .alpha
        .with_file_name("alpha2.snxlib")
        .join("symbols")
        .join("d.snxsym");

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let d = &f.app.document_state;
    assert!(d.symbol_editors.contains_key(&new), "the editor moved");
    assert!(!d.symbol_editors.contains_key(&old));
    assert!(d.is_document_dirty(&new));
    assert_eq!(d.dirty_document_count(), 1);
}

#[test]
fn a_library_rename_waits_for_a_draft_under_a_case_variant_being_saved() {
    let mut f = fixture();
    fold_case(&mut f);
    let variant = f.alpha.with_file_name("ALPHA.snxlib");
    let old = open_dirty_draft_in(&mut f, &variant);
    let _in_flight = f.app.document_state.document_saves.begin_work(old.clone());

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert!(f.alpha.exists(), "nothing moved");
    assert!(f.app.document_state.symbol_editors.contains_key(&old));
}

#[test]
fn a_recorded_save_failure_follows_its_draft_through_a_library_rename() {
    let mut f = fixture();
    let alpha = f.alpha.clone();
    let old = open_dirty_draft_in(&mut f, &alpha);
    f.app
        .document_state
        .document_saves
        .record_failure(old.clone(), "disk full".into());
    let new = f
        .alpha
        .with_file_name("alpha2.snxlib")
        .join("symbols")
        .join("d.snxsym");

    submit_rename(&mut f.app, &alpha, "alpha2.snxlib");

    let saves = &f.app.document_state.document_saves;
    assert_eq!(saves.failure(&new), Some("disk full"));
    assert!(saves.failure(&old).is_none());
}

#[test]
fn a_failed_file_rename_changes_nothing_and_is_logged() {
    let _ = diagnostics::init_logging();
    let mut f = fixture();
    // The source vanished behind the app's back, so `fs::rename` fails.
    fs::remove_file(&f.alpha).expect("remove source");

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert_eq!(
        entry_paths(&f.app),
        vec![
            PathBuf::from("alpha/alpha.snxlib"),
            PathBuf::from("beta/beta.snxlib")
        ]
    );
    assert!(
        f.app.library.library_at(&f.alpha).is_some(),
        "mount stays on the old path"
    );
    assert!(
        !f.app
            .library
            .set
            .contains_path(&f.alpha.with_file_name("alpha2.snxlib"))
    );
    assert!(!f.app.document_state.is_document_dirty(&f.prj));
    assert!(
        f.app
            .ui_state
            .rename_dialog
            .as_ref()
            .and_then(|d| d.error.as_deref())
            .is_some_and(|e| e.contains("Rename failed")),
        "the dialog stays open with the error"
    );
    assert!(
        f.app
            .document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|d| d.message.contains("could not be renamed")),
        "the failure must reach the Messages panel"
    );
}

#[test]
fn a_destination_that_appeared_is_never_overwritten() {
    let mut f = fixture();
    let taken = f.alpha.with_file_name("taken.snxlib");
    fs::write(&taken, b"precious").expect("write destination");

    submit_rename(&mut f.app, &f.alpha.clone(), "taken.snxlib");

    assert_eq!(fs::read(&taken).unwrap(), b"precious");
    assert!(f.alpha.exists());
    assert!(f.app.library.library_at(&f.alpha).is_some());
    assert_eq!(entry_paths(&f.app)[0], PathBuf::from("alpha/alpha.snxlib"));
}

/// A Project tab save hands the `.snxprj` write to a task that runs only
/// after `update` returns. A library rename then must be refused before the
/// folder moves: it rewrites that file directly, and the late write would
/// put the old path back (#1202).
#[test]
fn a_library_rename_while_the_project_file_is_being_written_is_refused_before_moving() {
    use signex_app::app::{ContextMenuMsg, ProjectTreeAction};
    let mut f = fixture();
    f.app.document_state.mark_document_dirty(&f.prj);
    let root = vec![f.app.document_state.projects[0].id.to_string()];
    send(
        &mut f.app,
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::OpenProjectOptions(root),
        )),
    );
    let _write_in_flight = f.app.update(Message::File(FileMsg::Save));

    submit_rename(&mut f.app, &f.alpha.clone(), "gamma.snxlib");

    let dialog = f
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("the dialog stays open with the reason");
    assert!(
        dialog
            .error
            .as_deref()
            .is_some_and(|e| e.contains("still being saved")),
        "{:?}",
        dialog.error
    );
    assert!(f.alpha.exists());
    assert!(!f.alpha.with_file_name("gamma.snxlib").exists());
    assert!(
        !f.app
            .document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|d| d.message.contains("renamed back")),
        "nothing was moved, so nothing was moved back"
    );
}

#[test]
fn closing_the_project_after_a_library_rename_prompts_for_the_unsaved_path() {
    use signex_app::app::{ContextMenuMsg, ProjectMsg, ProjectTreeAction};

    let mut f = unsaved_fixture();
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    // Close Project resolves the sheet references off-thread (#756); run
    // the task body and send its result back as the completion message.
    let project_id = f.app.document_state.projects[0].id;
    let root = vec![project_id.to_string()];
    send(
        &mut f.app,
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::CloseProject(root),
        )),
    );
    let result = f.app.resolve_sheet_refs_blocking();
    send(
        &mut f.app,
        Message::Project(ProjectMsg::CloseRefsResolved { project_id, result }),
    );

    let modal = f
        .app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("an unsaved library path must prompt on close");
    assert!(modal.dirty_paths.contains(&f.prj));
}

#[test]
fn shared_libraries_are_still_not_renameable_from_the_tree() {
    let mut f = fixture();
    f.app.document_state.projects[0].data.libraries[0].kind = LibraryEntryKind::Shared;
    f.app.document_state.projects[0].data.libraries[0].path = f.alpha.clone();

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let dialog = f
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("dialog stays open");
    assert!(
        dialog.error.is_some(),
        "Shared/Global semantics are untouched (#657)"
    );
}

/// Asserts a refused rename: dialog open with an error, file unmoved, entry
/// and saved `.snxprj` untouched.
fn assert_refused(f: &Fixture, why: &str) {
    let dialog = f
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .unwrap_or_else(|| panic!("dialog must stay open: {why}"));
    assert!(dialog.error.is_some(), "no error shown: {why}");
    assert!(
        f.alpha.exists() && !f.alpha.with_file_name("alpha2.snxlib").exists(),
        "file moved: {why}"
    );
    assert_eq!(entry_paths(&f.app)[0], PathBuf::from("alpha/alpha.snxlib"));
    assert_eq!(
        disk_library_paths(&f.prj)[0],
        PathBuf::from("alpha/alpha.snxlib"),
        "saved project changed: {why}"
    );
}

#[test]
fn a_library_listed_by_another_open_project_is_refused() {
    let mut f = fixture();
    let mut other = f.app.document_state.projects[0].data.clone();
    other.name = "other".to_string();
    let other_id = f.app.document_state.mint_project_id();
    f.app.document_state.projects.push(LoadedProject {
        id: other_id,
        path: f.prj.with_file_name("other.snxprj"),
        data: other,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert_refused(&f, "another open project lists it");
}

#[test]
fn a_globally_registered_library_is_refused() {
    let mut f = fixture();
    f.app.library.installed_libraries.push(f.alpha.clone());

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert_refused(&f, "registered globally");
}

#[test]
fn a_library_still_mounting_is_refused() {
    let mut f = fixture();
    f.app
        .library
        .pending_mounts
        .insert(f.alpha.clone(), MountIntent::Silent);

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert_refused(&f, "still mounting");
}

#[test]
fn an_open_browser_tab_and_its_state_follow_the_rename() {
    use signex_app::library::LibraryBrowserState;

    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    f.app
        .library
        .library_browsers
        .insert(f.alpha.clone(), LibraryBrowserState::new(f.alpha.clone()));
    f.app.document_state.tabs.push(TabInfo {
        title: "alpha".into(),
        path: f.alpha.clone(),
        cached_document: None,
        project_id: Some(f.project_id),
        kind: TabKind::LibraryBrowser(f.alpha.clone()),
    });

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let tab = &f.app.document_state.tabs[0];
    assert_eq!(tab.path, renamed);
    assert_eq!(tab.title, "alpha2");
    assert!(matches!(&tab.kind, TabKind::LibraryBrowser(p) if *p == renamed));
    let browsers = &f.app.library.library_browsers;
    assert!(!browsers.contains_key(&f.alpha));
    assert_eq!(browsers[&renamed].library_path, renamed);
}

fn adapter_write(app: &Signex, lib: &Path, table: &str) {
    let id = app.library.library_at(lib).expect("mounted").library_id;
    app.library
        .set
        .get(id)
        .expect("adapter")
        .create_empty_table(table, "regression write")
        .expect("table write");
}

/// No message can run between the file move and the mount following it, so
/// an edit made right after the submit lands in the renamed file and never
/// recreates the old one.
#[test]
fn an_edit_right_after_the_rename_lands_in_the_renamed_file() {
    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert!(f.app.library.library_at(&f.alpha).is_none());
    adapter_write(&f.app, &renamed, "in_window");

    assert!(!f.alpha.exists(), "old .snxlib resurrected by a write");
    let text = fs::read_to_string(&renamed).unwrap();
    assert!(
        text.contains("in_window"),
        "edit missing from the renamed file"
    );
}

/// The library's own repository records the rename: the old `.snxlib` is no
/// longer tracked and the new name is, so a checkout cannot resurrect it.
#[test]
fn the_library_repository_records_the_rename() {
    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let repo = git2::Repository::open(f.alpha.parent().unwrap()).expect("repo");
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let tree = head.tree().unwrap();
    assert!(
        tree.get_name("alpha.snxlib").is_none(),
        "HEAD still tracks the old name"
    );
    assert!(
        tree.get_name("alpha2.snxlib").is_some(),
        "HEAD does not track the new name"
    );
    assert!(
        head.message()
            .unwrap_or("")
            .starts_with("rename alpha.snxlib")
    );

    adapter_write(&f.app, &renamed, "after");
    let tree = repo.head().unwrap().peel_to_tree().unwrap();
    assert!(tree.get_name("alpha.snxlib").is_none());
}

#[test]
fn the_primitive_picker_target_follows_the_rename() {
    use signex_app::library::state::EditorAddress;
    use signex_app::library::{PrimitivePickerState, PrimitivePickerTarget};
    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    f.app.library.primitive_picker = Some(PrimitivePickerState {
        kind: signex_library::PrimitiveKind::Footprint,
        target: PrimitivePickerTarget::BrowserRow(EditorAddress::new(
            f.alpha.clone(),
            "t".into(),
            signex_library::RowId::new(),
        )),
        filter: String::new(),
        error: None,
        scroll_y: 0.0,
    });
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let p = f.app.library.primitive_picker.as_ref().unwrap();
    let PrimitivePickerTarget::BrowserRow(a) = &p.target else {
        panic!("target kind changed")
    };
    assert_eq!(
        a.library_path, renamed,
        "picker still addresses the old path"
    );
}

#[test]
fn the_library_updates_modal_follows_the_rename() {
    use signex_app::library::updates_dialog::{BumpKind, LibraryUpdateEntry, LibraryUpdatesState};
    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    f.app.library.library_updates = Some(LibraryUpdatesState::new(
        f.prj.clone(),
        vec![LibraryUpdateEntry {
            symbol_uuid: uuid::Uuid::new_v4(),
            ref_des: "R1".into(),
            lib_id: "L:S".into(),
            library_id: uuid::Uuid::new_v4(),
            library_name: "alpha".into(),
            row_id: signex_library::RowId::new(),
            library_path: f.alpha.clone(),
            current_version: "1.0".into(),
            latest_version: "1.1".into(),
            bump_kind: BumpKind::Minor,
            selected: false,
        }],
    ));
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let updates = f.app.library.library_updates.as_ref().unwrap();
    assert_eq!(updates.entries[0].library_path, renamed);
}

/// A pending New Library and other unsaved project edits stay unsaved: the
/// rename writes nothing but the file move and its library's own history.
#[test]
fn rename_does_not_flush_unsaved_project_state() {
    use signex_app::library::commands::PendingLibrarySpec;
    let mut f = fixture();
    let pending = f._tmp.path().join("gamma").join("gamma.snxlib");
    f.app.document_state.projects[0].pending_libraries.insert(
        uuid::Uuid::new_v4(),
        PendingLibrarySpec {
            lib_path: pending.clone(),
            enable_git: false,
            use_lfs: false,
            display_name: "gamma".into(),
        },
    );
    f.app.document_state.mark_document_dirty(&f.prj);
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert!(
        !pending.exists(),
        "unsaved New Library written to disk by a rename"
    );
    assert_eq!(
        disk_library_paths(&f.prj).len(),
        2,
        "the pending library must not reach the saved project"
    );
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert_eq!(f.app.document_state.projects[0].pending_libraries.len(), 1);
}

// ───────────── the saved project follows the rename ─────────────

fn disk_library_paths(prj: &Path) -> Vec<PathBuf> {
    parse_project(prj)
        .expect("project parses")
        .libraries
        .iter()
        .map(|e| e.path.clone())
        .collect()
}

/// After a rename, Discard / Quit / a crash leave only the disk behind. The
/// saved project must already list the renamed library, relative, and mount.
#[test]
fn a_saved_project_lists_the_renamed_library_so_reopening_mounts_it() {
    let mut f = fixture();
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let on_disk = parse_project(&f.prj).unwrap();
    assert_eq!(
        disk_library_paths(&f.prj),
        vec![
            PathBuf::from("alpha/alpha2.snxlib"),
            PathBuf::from("beta/beta.snxlib")
        ]
    );
    let resolved = on_disk.resolve_library_path(&on_disk.libraries[0]);
    assert!(
        prepare_mount(&resolved).is_ok(),
        "reopen after Discard cannot mount the renamed library"
    );
    assert!(
        !f.app.document_state.is_document_dirty(&f.prj),
        "memory and disk agree, so the project is not left dirty"
    );
}

/// The patch touches one entry. Other unsaved edits stay in memory, on disk
/// nothing of them appears, and the project stays dirty for them.
#[test]
fn the_project_patch_does_not_flush_other_unsaved_edits() {
    let mut f = fixture();
    f.app.document_state.projects[0]
        .data
        .variant_definitions
        .push("Unsaved".into());
    f.app.document_state.mark_document_dirty(&f.prj);
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let on_disk = parse_project(&f.prj).unwrap();
    assert!(
        on_disk.variant_definitions.is_empty(),
        "an unsaved edit was written by the rename"
    );
    assert_eq!(
        disk_library_paths(&f.prj)[0],
        PathBuf::from("alpha/alpha2.snxlib")
    );
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert_eq!(
        f.app.document_state.projects[0].data.variant_definitions,
        vec!["Unsaved".to_string()]
    );
}

/// No `.snxprj` on disk yet: nothing to patch, so none is created; the
/// project stays dirty and the Messages panel says it needs saving.
#[test]
fn a_never_saved_project_is_not_created_by_the_rename() {
    let _ = diagnostics::init_logging();
    let mut f = unsaved_fixture();
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert!(!f.prj.exists(), "the rename created the project file");
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert!(
        f.app
            .document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Save the project to keep the new path"))
    );
}

/// The project file cannot be written (read-only folder): the file is moved
/// back and every key stays on the old path.
#[cfg(unix)]
#[test]
fn a_failed_project_patch_rolls_the_rename_back() {
    use std::os::unix::fs::PermissionsExt;
    let _ = diagnostics::init_logging();
    let mut f = fixture();
    let dir = f.prj.parent().unwrap().to_path_buf();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(f.alpha.exists() && !renamed.exists(), "file not moved back");
    assert_eq!(entry_paths(&f.app)[0], PathBuf::from("alpha/alpha.snxlib"));
    assert_eq!(
        disk_library_paths(&f.prj)[0],
        PathBuf::from("alpha/alpha.snxlib")
    );
    assert!(f.app.library.library_at(&f.alpha).is_some());
    assert!(!f.app.library.set.contains_path(&renamed));
    assert!(f.app.document_state.error_notice.is_some());
}

/// With project-scoped git on, the patched `.snxprj` is queued for a
/// follow-up commit named for the rename.
#[test]
fn a_git_enabled_project_queues_a_commit_of_the_patched_project_file() {
    let mut f = fixture();
    f.app.document_state.projects[0].data.enable_git = true;
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let key = (
        f.prj.parent().unwrap().to_path_buf(),
        PathBuf::from("proj.snxprj"),
    );
    assert!(
        f.app.document_state.inflight_git_commits.contains(&key),
        "the patched project file was not queued for commit"
    );
}

#[test]
fn a_project_without_git_queues_no_commit() {
    let mut f = fixture();
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");
    assert!(f.app.document_state.inflight_git_commits.is_empty());
}

/// Duplicate entries for one library all follow, in memory and on disk.
#[test]
fn duplicate_entries_for_the_library_are_all_repointed() {
    let mut f = fixture();
    let dup = entry("alpha/alpha.snxlib");
    f.app.document_state.projects[0].data.libraries.push(dup);
    write_project(&f.prj, &f.app.document_state.projects[0].data).unwrap();
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    let renamed = PathBuf::from("alpha/alpha2.snxlib");
    assert_eq!(entry_paths(&f.app)[0], renamed);
    assert_eq!(entry_paths(&f.app)[2], renamed);
    assert_eq!(disk_library_paths(&f.prj)[0], renamed);
    assert_eq!(disk_library_paths(&f.prj)[2], renamed);
}

/// Disk lists the library as Shared while memory has ProjectLocal: the disk
/// file is left alone, the project is dirty, and the warning says what
/// Discard and Save will do.
#[test]
fn a_shared_disk_entry_gets_an_explicit_warning_and_a_dirty_project() {
    let _ = diagnostics::init_logging();
    let mut f = fixture();
    let mut on_disk = parse_project(&f.prj).unwrap();
    on_disk.libraries[0].kind = LibraryEntryKind::Shared;
    on_disk.libraries[0].path = f.alpha.clone();
    write_project(&f.prj, &on_disk).unwrap();
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert_eq!(disk_library_paths(&f.prj)[0], f.alpha);
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert!(
        f.app
            .document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Discard keeps the old name"))
    );
}

fn lock_branch(dir: &Path) -> PathBuf {
    let repo = git2::Repository::open(dir).unwrap();
    let head = repo.head().unwrap();
    let lock = dir
        .join(".git")
        .join(format!("{}.lock", head.name().unwrap()));
    fs::write(&lock, b"").unwrap();
    lock
}

/// A commit that fails (locked branch ref) leaves HEAD and the index exactly
/// as before, so the next ordinary row write commits only the row.
#[test]
fn a_failed_rename_commit_leaves_the_index_and_head_untouched() {
    let _ = diagnostics::init_logging();
    let mut f = fixture();
    let dir = f.alpha.parent().unwrap().to_path_buf();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    let repo = git2::Repository::open(&dir).unwrap();
    let head_before = repo.head().unwrap().peel_to_commit().unwrap().id();
    let lock = lock_branch(&dir);

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    assert!(f.alpha.exists() && !renamed.exists());
    assert!(f.app.library.library_at(&f.alpha).is_some());
    assert_eq!(
        disk_library_paths(&f.prj)[0],
        PathBuf::from("alpha/alpha.snxlib"),
        "the project patch must be undone with the rename"
    );
    fs::remove_file(lock).unwrap();
    let idx = repo.index().unwrap();
    assert!(idx.get_path(Path::new("alpha2.snxlib"), 0).is_none());
    assert!(idx.get_path(Path::new("alpha.snxlib"), 0).is_some());
    assert_eq!(
        repo.head().unwrap().peel_to_commit().unwrap().id(),
        head_before
    );

    adapter_write(&f.app, &f.alpha, "after_fail");
    let tree = repo.head().unwrap().peel_to_tree().unwrap();
    assert!(tree.get_name("alpha.snxlib").is_some());
    assert!(
        tree.get_name("alpha2.snxlib").is_none(),
        "phantom alpha2.snxlib committed from a stale index"
    );
}

/// A library without a repository renames cleanly.
#[test]
fn a_library_without_a_repository_renames_cleanly() {
    let mut f = fixture();
    fs::remove_dir_all(f.alpha.parent().unwrap().join(".git")).unwrap();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");
    assert!(renamed.exists() && !f.alpha.exists());
    assert!(f.app.library.library_at(&renamed).is_some());
    adapter_write(&f.app, &renamed, "ng");
    assert!(!f.alpha.exists());
}
