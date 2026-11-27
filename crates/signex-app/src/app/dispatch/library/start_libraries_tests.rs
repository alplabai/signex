//! The Libraries page's actions on a real library on disk: the version
//! control read (#2591) and Remove from Signex (#2592).

use std::path::{Path, PathBuf};

use iced::Task;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::library_vcs::LibraryVcsState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use uuid::Uuid;

use crate::app::state::{NoticeAction, NoticeSeverity};
use crate::app::{Message, Signex};
use crate::library::messages::{CloseLibraryChoice, LibraryMessage};
use crate::start_screen::{StartPage, StartScreenMsg, message};

/// Run `task` the way the runtime does, feeding every message back.
fn settle(app: &mut Signex, task: Task<Message>) {
    for next in crate::persist::run_task::outputs(task) {
        let more = app.update(next);
        settle(app, more);
    }
}

fn send(app: &mut Signex, msg: StartScreenMsg) {
    let task = app.update(message(msg));
    settle(app, task);
}

/// A real library without version control, mounted and listed as open.
fn mounted_library(app: &mut Signex, dir: &Path, name: &str) -> PathBuf {
    let file = dir.join(name).join(format!("{name}.snxlib"));
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: name.into(),
            description: None,
            ..Default::default()
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let options = LibraryInitOptions {
        enable_git: false,
        use_lfs: false,
    };
    let adapter = LocalGitAdapter::init(&file, manifest, options).expect("init");
    app.library.set.mount(Box::new(adapter)).expect("mount");
    app.library
        .open_libraries
        .push(crate::library::state::OpenLibrary {
            root: file.clone(),
            display_name: name.into(),
            library_id,
            tables: std::collections::HashMap::new(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
            pricing: Default::default(),
            provenance: Default::default(),
        });
    file
}

/// Opening the Libraries page reads each library's version control on a
/// worker; "off" then opens the library's Enable Version Control, and a
/// finished run reads again while the page shows.
#[test]
fn the_page_reads_version_control_off_the_ui_thread_and_off_opens_enable() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _startup) = Signex::new();
    let file = mounted_library(&mut app, dir.path(), "Parts");
    let folder = file.parent().unwrap().to_path_buf();
    assert!(app.ui_state.start_screen.library_vcs.is_empty());

    let read = app.update(message(StartScreenMsg::PageSelected(StartPage::Libraries)));
    assert!(
        app.ui_state.start_screen.library_vcs.is_empty(),
        "nothing is read inside update"
    );
    settle(&mut app, read);
    assert_eq!(
        app.ui_state.start_screen.library_vcs.get(&folder),
        Some(&LibraryVcsState::Off)
    );

    send(
        &mut app,
        StartScreenMsg::LibraryVcsEnableClicked(file.clone()),
    );
    let dialog = app
        .ui_state
        .enable_version_control
        .as_ref()
        .expect("Enable Version Control opens");
    assert_eq!(dialog.project_dir, folder);
    assert!(matches!(
        dialog.scope,
        crate::app::VersionControlScope::Library
    ));

    // After a run the page reads again; off the page nothing is read.
    assert_eq!(app.refresh_start_library_vcs().units(), 1);
    app.ui_state.start_screen.page = StartPage::Home;
    assert_eq!(app.refresh_start_library_vcs().units(), 0);
}

/// An older read that lands late is dropped; "off" on a folder the last
/// read found under git opens nothing.
#[test]
fn a_late_read_is_dropped_and_a_stale_off_opens_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _startup) = Signex::new();
    let file = mounted_library(&mut app, dir.path(), "Parts");
    let folder = file.parent().unwrap().to_path_buf();
    let state = &mut app.ui_state.start_screen;
    state.library_vcs_generation = 4;
    state
        .library_vcs
        .insert(folder.clone(), LibraryVcsState::Git);

    send(
        &mut app,
        StartScreenMsg::LibraryVcsRead {
            generation: 3,
            result: Ok(vec![(folder.clone(), LibraryVcsState::Off)]),
        },
    );
    assert_eq!(
        app.ui_state.start_screen.library_vcs.get(&folder),
        Some(&LibraryVcsState::Git)
    );

    send(&mut app, StartScreenMsg::LibraryVcsEnableClicked(file));
    assert!(app.ui_state.enable_version_control.is_none());
}

/// Decided 2026-11-27: Remove from Signex asks first, saying the files stay;
/// accepted, the library closes and leaves the registry, its files on disk.
#[test]
fn remove_from_signex_asks_then_unregisters_and_keeps_the_files() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _startup) = Signex::new();
    let file = mounted_library(&mut app, dir.path(), "Parts");
    app.library.installed_libraries.push(file.clone());

    send(&mut app, StartScreenMsg::LibraryMenuOpened(file.clone()));
    assert!(app.interaction_state.start_row_menu.is_some());
    send(
        &mut app,
        StartScreenMsg::LibraryRemoveRequested(file.clone()),
    );
    assert!(app.interaction_state.start_row_menu.is_none());
    let notice = app
        .document_view
        .error_notice
        .as_ref()
        .expect("the confirm card");
    assert_eq!(
        notice.action,
        Some(NoticeAction::RemoveLibraryFromSignex { path: file.clone() })
    );
    assert_eq!(notice.severity(), NoticeSeverity::Warn);
    let detail = notice.detail_text(&app.ui_state.i18n);
    assert!(detail.contains("Its files stay on disk"), "{detail}");
    assert!(
        app.library.library_at(&file).is_some(),
        "nothing removed yet"
    );

    send(
        &mut app,
        StartScreenMsg::LibraryRemoveConfirmed(file.clone()),
    );
    assert!(app.document_view.error_notice.is_none());
    assert!(app.library.library_at(&file).is_none(), "unmounted");
    assert!(!app.library.installed_libraries.contains(&file));
    assert_eq!(app.ui_state.start_screen.removing_library, None);
    assert!(file.exists(), "the library file stays on disk");
}

/// Decided 2026-11-27: a removal waiting on Close Library's unsaved-edits
/// prompt is dropped when that prompt is cancelled; a later plain close then
/// leaves the library registered.
#[test]
fn cancelling_the_close_prompt_removes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _startup) = Signex::new();
    let file = mounted_library(&mut app, dir.path(), "Parts");
    app.library.installed_libraries.push(file.clone());
    app.ui_state.start_screen.removing_library = Some(file.clone());
    app.library.close_library_confirm = Some(crate::library::state::CloseLibraryConfirmState {
        library_path: file.clone(),
        library_name: "Parts".into(),
        dirty_editors: Vec::new(),
        dirty_paths: Vec::new(),
    });

    let task = app.update(Message::Library(LibraryMessage::CloseLibraryConfirm(
        CloseLibraryChoice::Cancel,
    )));
    settle(&mut app, task);
    assert_eq!(app.ui_state.start_screen.removing_library, None);
    assert!(app.library.library_at(&file).is_some());

    let task = app.update(Message::Library(LibraryMessage::CloseLibrary(file.clone())));
    settle(&mut app, task);
    assert!(app.library.library_at(&file).is_none());
    assert!(
        app.library.installed_libraries.contains(&file),
        "a plain close keeps it registered"
    );
}

/// Only a mounted Global or Installed library is offered for removal.
#[test]
fn a_library_that_is_not_mounted_is_not_asked_about() {
    let (mut app, _startup) = Signex::new();
    let gone = PathBuf::from("gone").join("gone.snxlib");
    send(
        &mut app,
        StartScreenMsg::LibraryRemoveRequested(gone.clone()),
    );
    assert!(app.document_view.error_notice.is_none());
    send(&mut app, StartScreenMsg::LibraryRemoveConfirmed(gone));
    assert_eq!(app.ui_state.start_screen.removing_library, None);
}

/// A Global library leaves `global_libraries.toml` on a worker once it is
/// unmounted; the saved list replaces the in-memory one, and a failed write
/// keeps it and says so.
#[test]
fn a_global_library_leaves_the_saved_list_on_a_worker() {
    use crate::panels::components_panel::global_prefs::GlobalLibraryEntry;
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _startup) = Signex::new();
    let file = mounted_library(&mut app, dir.path(), "Parts");
    let entry = GlobalLibraryEntry {
        path: file.clone(),
        remote: None,
        auto_pull: None,
    };
    app.library.global_libraries = vec![entry.clone()];
    app.ui_state.start_screen.removing_library = Some(file.clone());

    app.library.close_library(&file);
    let unmounted = std::mem::take(&mut app.library.unmounted);
    let write = app.unregister_removed_library(&unmounted);
    assert_eq!(write.units(), 1, "the prefs write is a Task");
    assert_eq!(app.ui_state.start_screen.removing_library, None);

    drop(
        app.update(message(StartScreenMsg::GlobalLibraryUnregistered(Err(
            "disk full".into(),
        )))),
    );
    assert_eq!(app.library.global_libraries, vec![entry]);
    assert!(
        app.document_view.error_notice.is_some(),
        "the failure is told"
    );

    drop(
        app.update(message(StartScreenMsg::GlobalLibraryUnregistered(Ok(
            Vec::new(),
        )))),
    );
    assert!(app.library.global_libraries.is_empty());
}
