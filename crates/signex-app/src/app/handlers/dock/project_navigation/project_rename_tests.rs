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
pub(super) fn run_task(app: &mut Signex, task: iced::Task<Message>) {
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

pub(super) fn submit_rename(
    app: &mut Signex,
    project_file: &Path,
    new_stem: &str,
) -> iced::Task<Message> {
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
pub(super) struct Fixture {
    pub(super) app: Signex,
    pub(super) folder: PathBuf,
    pub(super) sheet: PathBuf,
}

pub(super) fn fixture(root: &Path, stem: &str, git: bool) -> Fixture {
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

#[test]
fn a_save_started_while_the_folder_moves_writes_nothing_and_keeps_the_edit() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let old_file = fx.folder.join("Foo.snxprj");
    // Dispatched, but the move has not reported back yet.
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    assert!(!fx.app.document_state.project_folder_renames.is_empty());

    // A save in this window is refused rather than recreating `Foo/`.
    let refused = fx.app.begin_schematic_save(fx.sheet.clone(), None);
    assert!(refused.is_err(), "save must be refused during the move");
    assert!(fx.app.document_state.dirty_paths.contains(&fx.sheet));
    assert!(!fx.app.document_state.document_saves.is_any_saving());

    // Opening a document in the moving project is refused too.
    assert!(
        fx.app
            .refuse_while_project_moves(&fx.folder.join("other.snxsch"), "open")
            .is_err()
    );

    // The dialog stays open to report the outcome.
    let _ = fx.app.update(Message::Rename(RenameMsg::Close));
    assert!(fx.app.ui_state.rename_dialog.is_some());

    run_task(&mut fx.app, task);

    let new_dir = root.path().join("Bar");
    let new_sheet = new_dir.join("main.snxsch");
    assert!(!fx.folder.exists(), "no write recreated the old folder");
    assert_eq!(std::fs::read(&new_sheet).expect("sheet"), b"sheet");
    assert!(fx.app.document_state.dirty_paths.contains(&new_sheet));
    // Once the move has landed, the same edit saves under the new path.
    let save = fx.app.begin_schematic_save(new_sheet.clone(), None);
    assert!(save.is_ok());
}

#[test]
fn the_project_file_and_editor_saves_are_refused_while_the_folder_moves() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let old_file = fx.folder.join("Foo.snxprj");
    let symbol = fx.folder.join("Lib.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym1"));
    let mut editor = crate::app::SymbolDocument::new(symbol.clone(), file);
    editor.dirty = true;
    fx.app
        .document_state
        .symbol_editors
        .insert(symbol.clone(), editor);
    fx.app.document_state.dirty_paths.insert(symbol.clone());
    fx.app.document_state.dirty_paths.insert(old_file.clone());
    let project_id = fx.app.document_state.projects[0].id;

    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    assert!(!fx.app.document_state.project_folder_renames.is_empty());

    // The `.snxprj` writer (Ctrl+S, Save All, Close Project).
    assert!(fx.app.persist_project_by_id(project_id).is_err());
    assert_eq!(std::fs::read(&old_file).expect("project file"), b"{}");
    assert!(fx.app.document_state.dirty_paths.contains(&old_file));

    // The symbol / footprint editor writers.
    assert!(fx.app.save_primitive_tab_at(&symbol).is_err());
    assert!(fx.app.save_primitive_tab_blocking(&symbol).is_err());
    assert!(!symbol.exists(), "no editor write reached the old folder");
    assert!(fx.app.document_state.dirty_paths.contains(&symbol));
    assert!(fx.app.document_state.symbol_editors[&symbol].dirty);

    run_task(&mut fx.app, task);
    assert!(!fx.folder.exists(), "no write recreated the old folder");
}

/// Dispatches the rename like `submit_rename`, but runs the disk move by hand
/// so the window between the move and `ProjectFolderRenamed` is observable.
fn move_on_disk_without_reporting(fx: &mut Fixture, new_stem: &str) -> ProjectRenameJob {
    let old_file = fx.folder.join("Foo.snxprj");
    drop(submit_rename(&mut fx.app, &old_file, new_stem));
    assert!(!fx.app.document_state.project_folder_renames.is_empty());
    let job = ProjectRenameJob {
        project_id: fx.app.document_state.projects[0].id,
        old_dir: fx.folder.clone(),
        new_dir: fx.folder.parent().expect("parent").join(new_stem),
        old_stem: "Foo".to_string(),
        new_stem: new_stem.to_string(),
        recent_key: crate::recent_projects::RecentProjects::spelling_of(&old_file),
    };
    rename_project_on_disk(&job).expect("move");
    job
}

#[test]
fn writes_after_the_disk_move_but_before_it_is_reported_do_not_recreate_the_old_folder() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let job = move_on_disk_without_reporting(&mut fx, "Bar");
    assert!(!fx.folder.exists(), "the move has landed");

    // Every writer is refused in this window.
    let refusals = [
        fx.app
            .begin_schematic_save(fx.sheet.clone(), None)
            .err()
            .map(|e| e.to_string()),
        fx.app
            .open_document_path(fx.folder.join("main.snxsch"))
            .err()
            .map(|e| e.to_string()),
        fx.app
            .persist_project_by_id(job.project_id)
            .err()
            .map(|e| e.to_string()),
    ];
    for refusal in refusals {
        let reason = refusal.expect("refused");
        assert!(reason.contains("being renamed"), "{reason}");
    }
    assert!(!fx.folder.exists(), "no write recreated the old folder");

    let _ = fx.app.update(Message::Project(
        crate::app::ProjectMsg::ProjectFolderRenamed {
            job,
            result: Ok(()),
        },
    ));
    assert!(!fx.folder.exists());
    assert!(root.path().join("Bar").join("main.snxsch").exists());
    assert!(fx.app.document_state.project_folder_renames.is_empty());
}

#[test]
fn a_save_as_is_refused_when_only_source_or_target_is_in_the_moving_project() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let _job = move_on_disk_without_reporting(&mut fx, "Bar");
    let outside = root.path().join("elsewhere.snxsch");

    // Source outside, target inside.
    let into = fx
        .app
        .begin_schematic_save(fx.sheet.clone(), Some(outside.clone()))
        .expect_err("target in moving project");
    assert!(into.to_string().contains("being renamed"), "{into}");
    // Source inside, target outside.
    let out_of = fx
        .app
        .begin_schematic_save(outside, Some(fx.sheet.clone()))
        .expect_err("source in moving project");
    assert!(out_of.to_string().contains("being renamed"), "{out_of}");
    assert!(!fx.folder.exists());
}

#[test]
fn a_double_failure_re_keys_the_project_to_the_file_left_in_the_old_folder() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let old_file = fx.folder.join("Foo.snxprj");
    let renamed = fx.folder.join("Bar.snxprj");
    let job = move_on_disk_without_reporting(&mut fx, "Bar");
    // Undo the move, leaving the disk as a failed move + failed restore does:
    // `Foo/Bar.snxprj`.
    std::fs::rename(&job.new_dir, &job.old_dir).expect("undo folder move");
    assert!(renamed.exists() && !old_file.exists());

    let _ = fx.app.update(Message::Project(
        crate::app::ProjectMsg::ProjectFolderRenamed {
            job,
            result: Err("could not be restored".to_string()),
        },
    ));
    assert_eq!(fx.app.document_state.projects[0].path, renamed);
}

#[test]
fn the_busy_check_matches_a_differently_cased_path_under_the_old_folder() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    fx.app.document_state.projects[0].case_fold = crate::app::state::scope::CaseFold::FOLD;
    let upper = PathBuf::from(fx.folder.to_string_lossy().to_uppercase()).join("x.snxsch");
    fx.app.document_state.pending_opens.insert(upper);
    let old_file = fx.folder.join("Foo.snxprj");
    let _ = submit_rename(&mut fx.app, &old_file, "Bar");
    assert!(fx.app.document_state.project_folder_renames.is_empty());
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

// Guards a reviewer found missing on #1240.

pub(super) fn rename_error(fx: &Fixture) -> String {
    fx.app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("dialog stays open")
        .error
        .clone()
        .expect("an error is shown")
}

fn assert_folder_move_refused_for_library(fx: &mut Fixture, what: &str) {
    let old_file = fx.folder.join("Foo.snxprj");
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    run_task(&mut fx.app, task);
    assert!(old_file.is_file(), "{what}: nothing moved");
    assert!(
        rename_error(fx).contains("in use"),
        "{what}: {}",
        rename_error(fx)
    );
    assert!(fx.app.document_state.project_folder_renames.is_empty());
}

#[test]
fn a_library_under_the_folder_refuses_the_folder_move_whatever_holds_it() {
    type Hold = fn(&mut Fixture, PathBuf);
    let holds: [(&str, Hold); 7] = [
        ("open library", |fx, p| {
            fx.app
                .library
                .open_libraries
                .push(crate::library::state::OpenLibrary {
                    root: p,
                    display_name: "p".into(),
                    library_id: uuid::Uuid::new_v4(),
                    tables: Default::default(),
                    cached_components: Vec::new(),
                    cached_symbols: Vec::new(),
                    cached_footprints: Vec::new(),
                    cached_sims: Vec::new(),
                    cached_classes: Vec::new(),
                    observed_stamp: None,
                    dangling: Default::default(),
                    pricing: Default::default(),
                    own_edit_stamp: None,
                    display: Default::default(),
                });
        }),
        ("stock library", |fx, p| {
            fx.app.library.stock_library = Some(p)
        }),
        ("installed", |fx, p| {
            fx.app.library.installed_libraries.push(p)
        }),
        ("global", |fx, p| {
            fx.app.library.global_libraries.push(
                crate::panels::components_panel::global_prefs::GlobalLibraryEntry {
                    path: p,
                    remote: None,
                    auto_pull: None,
                },
            )
        }),
        ("pending mount", |fx, p| {
            fx.app
                .library
                .pending_mounts
                .insert(p, crate::library::mount::MountIntent::Silent);
        }),
        ("browser state", |fx, p| {
            fx.app.library.library_browsers.insert(
                p.clone(),
                crate::library::state::LibraryBrowserState::new(p),
            );
        }),
        ("browser tab", |fx, p| {
            fx.app.document_state.tabs.push(TabInfo {
                title: "p".into(),
                path: p.clone(),
                cached_document: None,
                dirty: false,
                project_id: None,
                kind: TabKind::LibraryBrowser(p),
            });
        }),
    ];
    for (what, hold) in holds {
        let root = tempfile::tempdir().expect("tempdir");
        let mut fx = fixture(root.path(), "Foo", false);
        let path = fx.folder.join("parts").join("p.snxlib");
        hold(&mut fx, path);
        assert_folder_move_refused_for_library(&mut fx, what);
    }
}

#[test]
fn a_library_outside_the_folder_does_not_block_the_folder_move() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    fx.app
        .library
        .installed_libraries
        .push(root.path().join("Elsewhere").join("p.snxlib"));
    let old_file = fx.folder.join("Foo.snxprj");
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    run_task(&mut fx.app, task);
    assert!(root.path().join("Bar").join("Bar.snxprj").is_file());
}

#[test]
fn a_library_spelled_in_another_case_still_blocks_a_case_folding_project() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    fx.app.document_state.projects[0].case_fold = crate::app::state::scope::CaseFold::FOLD;
    let upper = PathBuf::from(fx.folder.to_string_lossy().to_uppercase()).join("p.snxlib");
    fx.app.library.installed_libraries.push(upper);
    assert_folder_move_refused_for_library(&mut fx, "case variant");
}

#[test]
fn an_export_into_a_moving_project_is_refused_and_creates_nothing() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let id = fx.app.document_state.projects[0].id;
    fx.app.document_state.project_folder_renames.insert(id);
    let out = fx.folder.join("outputs").join("netlist.net");

    let task = fx.app.write_export(
        out.clone(),
        b"x".to_vec(),
        "netlist",
        String::new(),
        None,
        Vec::new(),
    );
    assert!(iced_runtime::task::into_stream(task).is_none(), "no write");
    let notice = fx.app.document_state.error_notice.take().expect("notice");
    assert!(notice.detail.contains("being renamed"), "{}", notice.detail);

    let report = fx.folder.join("outputs").join("erc.txt");
    let task = fx.app.handle_erc_report_path_chosen(Ok(report));
    assert!(iced_runtime::task::into_stream(task).is_none(), "no write");
    let notice = fx.app.document_state.error_notice.take().expect("notice");
    assert!(notice.detail.contains("being renamed"), "{}", notice.detail);
    assert!(!fx.folder.join("outputs").exists(), "nothing was created");

    // Once the move is over the same export writes.
    fx.app.document_state.project_folder_renames.clear();
    let task = fx.app.write_export(
        out.clone(),
        b"x".to_vec(),
        "netlist",
        String::new(),
        None,
        Vec::new(),
    );
    run_task(&mut fx.app, task);
    assert!(out.is_file());
}

#[test]
fn a_failed_folder_move_puts_the_project_file_back() {
    let root = tempfile::tempdir().expect("tempdir");
    let fx = fixture(root.path(), "Foo", false);
    let mut bad = job(&fx, root.path(), "Foo", "Bar");
    // The new folder's parent does not exist, so only the folder move fails.
    bad.new_dir = root.path().join("missing").join("Bar");
    let err = rename_project_on_disk(&bad).expect_err("folder move fails");
    assert!(err.contains("could not be renamed"), "{err}");
    assert!(fx.folder.join("Foo.snxprj").is_file(), "put back");
    assert!(!fx.folder.join("Bar.snxprj").exists());
}

#[test]
fn a_folder_rename_is_refused_while_a_git_commit_is_in_flight() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    fx.app
        .document_state
        .inflight_git_commits
        .insert((fx.folder.clone(), PathBuf::from("main.snxsch")));
    let old_file = fx.folder.join("Foo.snxprj");
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    run_task(&mut fx.app, task);
    assert!(old_file.is_file(), "nothing moved");
    assert!(
        rename_error(&fx).contains("version-control"),
        "{}",
        rename_error(&fx)
    );
}

#[test]
fn a_git_commit_for_a_moving_project_is_not_queued() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", true);
    let id = fx.app.document_state.projects[0].id;
    fx.app.document_state.project_folder_renames.insert(id);
    fx.app.commit_project_rename_to_git(
        &fx.folder.join("Foo.snxprj"),
        &fx.folder.join("Bar.snxprj"),
        "m",
    );
    assert!(fx.app.document_state.pending_git_commits.is_empty());
}

#[test]
fn a_double_failure_updates_open_recent_and_queues_the_rename_commit() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", true);
    // The list stores canonical spellings (`/private/var` for a macOS temp
    // dir, no `\?\` prefix on Windows), so compare in that spelling.
    let folder = crate::recent_projects::RecentProjects::spelling_of(&fx.folder);
    let old_file = folder.join("Foo.snxprj");
    let renamed = folder.join("Bar.snxprj");
    let job = move_on_disk_without_reporting(&mut fx, "Bar");
    std::fs::rename(&job.new_dir, &job.old_dir).expect("undo folder move");

    let _ = fx.app.update(Message::Project(
        crate::app::ProjectMsg::ProjectFolderRenamed {
            job,
            result: Err("could not be restored".to_string()),
        },
    ));
    let recent = fx.app.ui_state.recent_projects.entries();
    assert!(
        recent.contains(&renamed) && !recent.contains(&old_file),
        "{recent:?}"
    );
    let ds = &fx.app.document_state;
    let key = (fx.folder.clone(), PathBuf::from("Bar.snxprj"));
    assert!(
        ds.inflight_git_commits.contains(&key)
            || ds.pending_git_commits.iter().any(|c| c.rel_path == key.1),
        "rename commit queued"
    );
}
