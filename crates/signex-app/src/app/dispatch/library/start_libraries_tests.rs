//! The Libraries page's actions on a real library on disk: the version
//! control read (#2591).

use std::path::{Path, PathBuf};

use iced::Task;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::library_vcs::LibraryVcsState;
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use uuid::Uuid;

use crate::app::{Message, Signex};
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
