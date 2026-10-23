//! #739 — renaming a project library's `.snxlib` from the project tree
//! must leave nothing pointing at the old path.
//!
//! Real libraries on disk (`generate_library`), a real project file, and
//! the same `RenameMsg` dispatcher the dialog uses. The `fs::rename` runs
//! in a `Task` in the app, which a test cannot drive, so each test plays
//! the task's part (`finish_rename`) and feeds the completion message.

// Test scaffolding discards the `iced::Task` that `update` returns and the
// already-installed error from `init_logging`; `[lints]` is package-scoped,
// so each integration-test crate root carries its own allow.
#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results/Tasks with `let _ = ...`; test scaffolding, not production"
)]

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use signex_app::app::{
    LoadedProject, Message, ProjectId, RenameDialogState, RenameMsg, Signex, TabInfo, TabKind,
};
use signex_app::diagnostics;
use signex_app::library::messages::{LibraryMessage, PrimitiveEdit};
use signex_app::library::mount::prepare_mount;
use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData, parse_project};
use tempfile::TempDir;

use support::{Scale, generate_library};

fn send(app: &mut Signex, msg: Message) {
    let _ = app.update(msg);
}

struct Fixture {
    app: Signex,
    _tmp: TempDir,
    project_id: ProjectId,
    prj: PathBuf,
    alpha: PathBuf,
    beta: PathBuf,
}

fn entry(rel: &str) -> LibraryEntry {
    LibraryEntry {
        path: PathBuf::from(rel),
        kind: LibraryEntryKind::ProjectLocal,
        library_id: None,
    }
}

/// Two project-local libraries (`alpha`, `beta`) mounted in a loaded project.
fn fixture() -> Fixture {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let alpha = generate_library(&dir, "alpha", &Scale::new("alpha", 2, 2)).expect("alpha");
    let beta = generate_library(&dir, "beta", &Scale::new("beta", 2, 2)).expect("beta");
    let prj = dir.join("proj.snxprj");
    fs::write(&prj, b"{}").expect("write prj");

    let (mut app, _task) = Signex::new();
    let project_id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id: project_id,
        path: prj.clone(),
        data: ProjectData {
            name: "proj".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: vec![entry("alpha/alpha.snxlib"), entry("beta/beta.snxlib")],
            enable_git: false,
        },
        pending_libraries: Default::default(),
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

fn submit_rename(app: &mut Signex, target: &Path, new_name: &str) {
    app.ui_state.rename_dialog = Some(RenameDialogState {
        target_path: target.to_path_buf(),
        tree_path: vec![0, 0],
        buffer: new_name.to_string(),
        error: None,
        is_project_rename: false,
    });
    send(app, Message::Rename(RenameMsg::Submit));
}

/// Play the `Task`'s part: rename on disk, then deliver the completion.
fn finish_rename(f: &mut Fixture, new: &Path) {
    let old = f.alpha.clone();
    fs::rename(&old, new).expect("disk rename");
    send(
        &mut f.app,
        Message::Rename(RenameMsg::LibraryFileRenamed {
            project: f.project_id,
            old_path: old,
            new_path: new.to_path_buf(),
            result: Ok(()),
        }),
    );
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
fn rename_repoints_the_entry_persists_it_and_keeps_the_library_mounted() {
    let mut f = fixture();
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
    finish_rename(&mut f, &renamed);

    // The project entry follows the file; the other entry is untouched.
    assert_eq!(
        entry_paths(&f.app),
        vec![
            PathBuf::from("alpha/alpha2.snxlib"),
            PathBuf::from("beta/beta.snxlib")
        ]
    );
    // Persisted through the atomic save path.
    let on_disk = parse_project(&f.prj).expect("project reparses");
    assert_eq!(
        on_disk
            .libraries
            .iter()
            .map(|e| e.path.clone())
            .collect::<Vec<_>>(),
        entry_paths(&f.app)
    );
    // The mounted library moved with it — nothing keyed by the old path.
    assert!(f.app.library.library_at(&renamed).is_some());
    assert!(f.app.library.library_at(&f.alpha).is_none());
    assert!(f.app.library.set.contains_path(&renamed));
    assert!(!f.app.library.set.contains_path(&f.alpha));
    assert!(f.app.library.library_at(&f.beta).is_some());

    // Reopening the project: both entries resolve and mount.
    for e in &on_disk.libraries {
        let resolved = on_disk.resolve_library_path(e);
        prepare_mount(&resolved)
            .unwrap_or_else(|err| panic!("{} must mount: {err}", resolved.display()));
    }
}

#[test]
fn library_writes_after_a_rename_land_in_the_renamed_file() {
    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");
    finish_rename(&mut f, &renamed);

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
    use signex_app::app::FootprintEditorState;
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
    let mut editor = FootprintEditorState::new(fpt.clone(), file);
    editor.dirty = true;
    f.app
        .document_state
        .footprint_editors
        .insert(fpt.clone(), editor);
    f.app.document_state.tabs.push(TabInfo {
        title: "fp".into(),
        path: fpt.clone(),
        cached_document: None,
        dirty: true,
        project_id: Some(f.project_id),
        kind: TabKind::FootprintEditor(fpt.clone()),
    });
    f.app.document_state.active_tab = 0;

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");
    finish_rename(&mut f, &renamed);

    assert!(
        f.app.document_state.footprint_editors.contains_key(&fpt),
        "the editor must survive the rename"
    );
    assert!(
        f.app.library.set.contains_path(&renamed),
        "the adapter that commits the editor's save must follow the renamed library"
    );
    send(
        &mut f.app,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: fpt.clone(),
            msg: PrimitiveEdit::Save,
        }),
    );

    assert!(
        !f.app.document_state.footprint_editors[&fpt].dirty,
        "the save must succeed"
    );
    assert!(
        !f.alpha.exists(),
        "the save must not resurrect the old .snxlib"
    );
    let repo = git2::Repository::open(f.alpha.parent().unwrap()).expect("repo");
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert!(
        head.message().unwrap_or("").starts_with("save footprint"),
        "the save must commit through the relocated adapter, got {:?}",
        head.message()
    );
}

#[test]
fn a_failed_rename_changes_nothing_and_is_logged() {
    let _ = diagnostics::init_logging();
    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");

    send(
        &mut f.app,
        Message::Rename(RenameMsg::LibraryFileRenamed {
            project: f.project_id,
            old_path: f.alpha.clone(),
            new_path: renamed.clone(),
            result: Err("disk on fire 739".to_string()),
        }),
    );

    assert_eq!(
        entry_paths(&f.app),
        vec![
            PathBuf::from("alpha/alpha.snxlib"),
            PathBuf::from("beta/beta.snxlib")
        ]
    );
    assert!(f.app.library.library_at(&f.alpha).is_some());
    assert!(
        f.app
            .document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|d| d.message.contains("disk on fire 739")),
        "the failure must reach the Messages panel"
    );
    assert!(f.app.document_state.error_notice.is_some());
}

#[test]
fn a_failed_project_save_reverts_the_entry_and_the_mount() {
    let _ = diagnostics::init_logging();
    let mut f = fixture();
    let renamed = f.alpha.with_file_name("alpha2.snxlib");
    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");
    // The .snxprj cannot be written: a directory sits where the file should be.
    let blocked = f._tmp.path().join("blocked.snxprj");
    fs::create_dir(&blocked).expect("blocker dir");
    f.app.document_state.projects[0].path = blocked;

    finish_rename(&mut f, &renamed);

    assert_eq!(
        entry_paths(&f.app)[0],
        PathBuf::from("alpha/alpha.snxlib"),
        "the project must not be left pointing at a path it could not persist"
    );
    assert!(f.app.library.library_at(&f.alpha).is_some());
    assert!(!f.app.library.set.contains_path(&renamed));
    assert!(f.app.document_state.error_notice.is_some());
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
        dirty: false,
        project_id: Some(f.project_id),
        kind: TabKind::LibraryBrowser(f.alpha.clone()),
    });

    submit_rename(&mut f.app, &f.alpha.clone(), "alpha2.snxlib");
    finish_rename(&mut f, &renamed);

    let tab = &f.app.document_state.tabs[0];
    assert_eq!(tab.path, renamed);
    assert_eq!(tab.title, "alpha2");
    assert!(matches!(&tab.kind, TabKind::LibraryBrowser(p) if *p == renamed));
    let browsers = &f.app.library.library_browsers;
    assert!(!browsers.contains_key(&f.alpha));
    assert_eq!(browsers[&renamed].library_path, renamed);
}
