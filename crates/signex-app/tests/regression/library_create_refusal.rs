//! #1739 - a version-controlled library cannot be created in the root of
//! another repository that has history. The Library Options modal refuses it
//! with the reason, stays open, and nothing is registered or written.

use signex_app::app::{LoadedProject, Message, Signex};
use signex_app::library::messages::LibraryMessage;
use signex_types::project::ProjectData;
use tempfile::TempDir;

/// A project in a git repo with one commit, loaded in a fresh app.
fn git_project() -> (Signex, TempDir, std::path::PathBuf) {
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    let prj = dir.join("p.snxprj");
    std::fs::write(&prj, b"{}").unwrap();
    let repo = git2::Repository::init(&dir).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("p.snxprj")).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::now("t", "t@example.org").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "project", &tree, &[])
        .unwrap();

    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    let data = ProjectData {
        description: None,
        name: "p".into(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: None,
        pcb_file: None,
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: true,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        parameters: Default::default(),
        unknown: Default::default(),
    };
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj.clone(),
        data,
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    (app, tmp, prj)
}

fn create(app: &mut Signex, prj: &std::path::Path, lib: &std::path::Path, git: bool) {
    let send = |app: &mut Signex, m| drop(app.update(Message::Library(m)));
    send(
        app,
        LibraryMessage::CreateLibraryAtPath {
            project_path: prj.to_path_buf(),
            lib_path: lib.to_path_buf(),
        },
    );
    if git {
        send(app, LibraryMessage::LibraryCreateOptionsToggleGit);
    }
    send(app, LibraryMessage::LibraryCreateOptionsConfirm);
}

#[test]
fn the_project_folder_is_refused_with_a_reason_and_nothing_is_written() {
    let (mut app, tmp, prj) = git_project();
    let lib = tmp.path().join("p-lib.snxlib");

    create(&mut app, &prj, &lib, true);

    let modal = app
        .library
        .create_options
        .as_ref()
        .expect("the dialog stays open");
    assert!(
        modal
            .refusal
            .as_deref()
            .is_some_and(|r| r.contains("own history"))
    );
    assert!(app.document_state.projects[0].pending_libraries.is_empty());
    assert!(!lib.exists());
}

#[test]
fn the_new_default_subfolder_is_accepted() {
    let (mut app, tmp, prj) = git_project();
    let lib = signex_app::library::commands::default_new_library_path(tmp.path(), "p");

    create(&mut app, &prj, &lib, true);

    assert!(app.library.create_options.is_none(), "dialog closed");
    assert_eq!(app.document_state.projects[0].pending_libraries.len(), 1);
}

#[test]
fn the_project_folder_without_version_control_is_still_fine() {
    let (mut app, tmp, prj) = git_project();
    create(&mut app, &prj, &tmp.path().join("p-lib.snxlib"), false);
    assert!(app.library.create_options.is_none());
    assert_eq!(app.document_state.projects[0].pending_libraries.len(), 1);
}
