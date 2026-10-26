//! #1219 / #1221 — Rename Project moves the folder named after the project,
//! re-keys everything under it, and commits the move to version control,
//! against a real directory and a real repository.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git_project::LocalGitProjectAdapter;

use super::project_rename_fs::{folder_target, rename_project_on_disk};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{
    Message, ProjectRenameJob, RenameDialogState, RenameMsg, Signex, TabInfo, TabKind,
};

/// Runs `task` as the iced runtime would and feeds every message back through
/// `update`, following the tasks those return.
fn run_task(app: &mut Signex, task: iced::Task<Message>) {
    use futures::StreamExt;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let messages = runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    });
    for message in messages {
        let next = app.update(message);
        run_task(app, next);
    }
}

fn submit_rename(app: &mut Signex, project_file: &Path, new_stem: &str) -> iced::Task<Message> {
    app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: project_file.to_path_buf(),
        tree_path: vec![0],
        buffer: new_stem.to_string(),
        error: None,
        is_project_rename: true,
    });
    app.update(Message::Rename(RenameMsg::Submit))
}

/// `<root>/<stem>/<stem>.snxprj` plus a sheet, committed in a repo of their
/// own, and an app with that project open, a dirty tab on the sheet and the
/// project in the recent list.
struct Fixture {
    app: Signex,
    folder: PathBuf,
    sheet: PathBuf,
}

fn fixture(root: &Path, stem: &str, git: bool) -> Fixture {
    signex_library::git_identity::test_identity::install();
    let folder = root.join(stem);
    std::fs::create_dir(&folder).expect("project folder");
    let project_file = folder.join(format!("{stem}.snxprj"));
    let sheet = folder.join("main.snxsch");
    std::fs::write(&project_file, b"{}").expect("project file");
    std::fs::write(&sheet, b"sheet").expect("sheet");
    if git {
        let adapter = LocalGitProjectAdapter::open_or_init(folder.clone()).expect("repo");
        for rel in [format!("{stem}.snxprj"), "main.snxsch".to_string()] {
            adapter
                .commit_path(Path::new(&rel), "init")
                .expect("commit");
        }
    }
    let mut app = app_workspace(&folder.to_string_lossy(), &["main.snxsch"]);
    let project = &mut app.document_state.projects[0];
    project.path = project_file.clone();
    project.data.name = stem.to_string();
    project.data.enable_git = git;
    let engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    })
    .expect("engine");
    app.document_state.engines.insert(sheet.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "main".into(),
        path: sheet.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(sheet.clone());
    app.document_state.dirty_paths.insert(sheet.clone());
    // The list is loaded from the per-process prefs file other tests share.
    app.ui_state.recent_projects.clear();
    app.ui_state.recent_projects.record(&project_file);
    Fixture { app, folder, sheet }
}

fn job(fx: &Fixture, root: &Path, old: &str, new: &str) -> ProjectRenameJob {
    ProjectRenameJob {
        project_id: fx.app.document_state.projects[0].id,
        old_dir: root.join(old),
        new_dir: root.join(new),
        old_stem: old.to_string(),
        new_stem: new.to_string(),
        recent_key: PathBuf::new(),
    }
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

fn assert_clean_and_recorded_as_rename(dir: &Path, old_name: &str, new_name: &str) {
    let repo = git2::Repository::open(dir).expect("repo");
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.statuses(Some(&mut opts)).expect("status");
    let dirty: Vec<_> = statuses
        .iter()
        .map(|s| s.path().unwrap_or("?").to_owned())
        .collect();
    assert!(dirty.is_empty(), "git status must be clean, got {dirty:?}");
    let tree = repo.head().expect("head").peel_to_tree().expect("tree");
    assert!(tree.get_name(new_name).is_some(), "{new_name} is committed");
    assert!(
        tree.get_name(old_name).is_none(),
        "{old_name} is gone from HEAD"
    );
}

#[test]
fn the_folder_follows_only_a_name_matching_the_old_stem() {
    let root = Path::new("root");
    assert_eq!(
        folder_target(&root.join("Foo"), "Foo", "Bar"),
        Some(root.join("Bar"))
    );
    assert_eq!(
        folder_target(&root.join("FOO"), "foo", "Bar"),
        Some(root.join("Bar")),
        "ASCII-case-insensitive"
    );
    assert_eq!(folder_target(&root.join("work"), "Foo", "Bar"), None);
}

#[test]
fn the_disk_move_renames_file_and_folder_and_keeps_the_contents() {
    let root = tempfile::tempdir().expect("tempdir");
    let fx = fixture(root.path(), "Foo", false);
    rename_project_on_disk(&job(&fx, root.path(), "Foo", "Bar")).expect("rename");
    assert!(!fx.folder.exists());
    let moved = root.path().join("Bar");
    assert!(moved.join("Bar.snxprj").is_file());
    assert_eq!(
        std::fs::read(moved.join("main.snxsch")).expect("sheet"),
        b"sheet"
    );
}

#[test]
fn the_disk_move_refuses_an_existing_folder_and_changes_nothing() {
    let root = tempfile::tempdir().expect("tempdir");
    let fx = fixture(root.path(), "Foo", false);
    let other = root.path().join("Bar");
    std::fs::create_dir(&other).expect("other");
    std::fs::write(other.join("keep.txt"), b"mine").expect("keep");
    let err = rename_project_on_disk(&job(&fx, root.path(), "Foo", "Bar")).unwrap_err();
    assert!(err.contains("already exists"), "{err}");
    assert!(fx.folder.join("Foo.snxprj").is_file(), "project untouched");
    assert_eq!(
        std::fs::read(other.join("keep.txt")).expect("keep"),
        b"mine"
    );
}

#[test]
fn renaming_a_project_folder_re_keys_documents_recent_projects_and_git() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", true);
    let old_file = fx.folder.join("Foo.snxprj");
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    assert!(
        old_file.is_file(),
        "the move runs on the blocking pool, not inside update"
    );
    run_task(&mut fx.app, task);

    let new_dir = root.path().join("Bar");
    let new_file = new_dir.join("Bar.snxprj");
    let new_sheet = new_dir.join("main.snxsch");
    assert!(new_file.is_file() && !fx.folder.exists());

    let ds = &fx.app.document_state;
    assert_eq!(ds.projects[0].path, new_file);
    assert_eq!(ds.projects[0].data.dir, new_dir.to_string_lossy());
    assert_eq!(ds.projects[0].data.name, "Bar");
    assert!(ds.engines.contains_key(&new_sheet) && !ds.engines.contains_key(&fx.sheet));
    assert_eq!(ds.tabs[0].path, new_sheet);
    assert_eq!(ds.active_path.as_deref(), Some(new_sheet.as_path()));
    assert!(ds.dirty_paths.contains(&new_sheet) && !ds.dirty_paths.contains(&fx.sheet));
    assert!(fx.app.ui_state.rename_dialog.is_none());
    assert!(ds.project_folder_renames.is_empty());
    assert!(
        ds.inflight_git_commits.is_empty(),
        "the rename commit finished"
    );

    let recent = fx.app.ui_state.recent_projects.entries();
    assert_eq!(recent.len(), 1);
    assert!(recent[0].ends_with(Path::new("Bar").join("Bar.snxprj")));

    assert_eq!(head_subject(&new_dir), "Rename project Foo \u{2192} Bar");
    assert_clean_and_recorded_as_rename(&new_dir, "Foo.snxprj", "Bar.snxprj");
}

#[test]
fn a_folder_with_another_name_keeps_it_and_the_rename_is_still_committed() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", true);
    let renamed = root.path().join("work");
    std::fs::rename(&fx.folder, &renamed).expect("move");
    let old_file = renamed.join("Foo.snxprj");
    let project = &mut fx.app.document_state.projects[0];
    project.path = old_file.clone();
    project.data.dir = renamed.to_string_lossy().into_owned();

    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    run_task(&mut fx.app, task);

    assert!(renamed.join("Bar.snxprj").is_file() && !old_file.exists());
    assert_eq!(
        fx.app.document_state.projects[0].path,
        renamed.join("Bar.snxprj")
    );
    assert_eq!(head_subject(&renamed), "Rename project Foo \u{2192} Bar");
    assert_clean_and_recorded_as_rename(&renamed, "Foo.snxprj", "Bar.snxprj");
}

#[test]
fn a_folder_rename_is_refused_while_a_write_into_the_project_is_in_flight() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    fx.app.document_state.pending_opens.insert(fx.sheet.clone());
    let old_file = fx.folder.join("Foo.snxprj");
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    run_task(&mut fx.app, task);

    assert!(old_file.is_file(), "nothing moved");
    let dialog = fx
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("dialog stays open");
    assert!(dialog.error.is_some());
    assert!(fx.app.document_state.project_folder_renames.is_empty());
}

#[test]
fn a_folder_rename_is_refused_when_the_target_folder_exists() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    std::fs::create_dir(root.path().join("Bar")).expect("other");
    let old_file = fx.folder.join("Foo.snxprj");
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    run_task(&mut fx.app, task);

    assert!(old_file.is_file(), "nothing moved");
    assert!(
        fx.app
            .ui_state
            .rename_dialog
            .as_ref()
            .expect("open")
            .error
            .is_some()
    );
}

#[test]
fn a_failed_disk_move_leaves_the_project_as_it_was() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let old_file = fx.folder.join("Foo.snxprj");
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    // The project vanished from under the dialog before the move ran.
    std::fs::remove_file(&old_file).expect("remove");
    run_task(&mut fx.app, task);

    assert_eq!(fx.app.document_state.projects[0].path, old_file);
    assert!(fx.app.document_state.engines.contains_key(&fx.sheet));
    assert!(
        fx.app
            .ui_state
            .rename_dialog
            .as_ref()
            .expect("open")
            .error
            .is_some()
    );
    assert!(fx.app.document_state.project_folder_renames.is_empty());
}
