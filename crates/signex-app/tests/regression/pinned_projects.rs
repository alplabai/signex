//! #2305 — Start screen Pinned Projects driven through `Signex::update`:
//! Pin and Unpin move a project between the Start screen's sections and
//! save the list to `prefs.json` (redirected per process by
//! `test-prefs-redirect`); a pinned project missing on disk is shown as
//! missing and Remove drops it; the row menu closes on Open and on Esc; a
//! pin follows a rename and a pin missing at start-up is marked missing.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use signex_app::app::{InputMsg, Message, ProjectMsg, ProjectTabMsg, RenameMsg, Signex};
use signex_app::pinned_projects::PinnedProjects;
use signex_app::recent_projects::RecentProjects;
use signex_app::start_screen::{self, StartScreenMsg};

/// Every test here reads and writes the one `pinned_projects` key.
static PINNED_PREFS: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    PINNED_PREFS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Send `msg` and run the Tasks it returns, so the background prefs write
/// has landed before the test reads the file.
fn send(app: &mut Signex, msg: StartScreenMsg) {
    let task = app.update(start_screen::message(msg));
    crate::drive::drive(app, task);
}

/// A fresh app with no pins and `names` as its recent projects (files that
/// exist), newest first, as the recent list spells them.
fn app_with_recent(dir: &std::path::Path, names: &[&str]) -> (Signex, Vec<PathBuf>) {
    let (mut app, _startup) = Signex::new();
    let paths: Vec<PathBuf> = names
        .iter()
        .map(|name| {
            let path = dir.join(name);
            std::fs::write(&path, b"").expect("create project");
            path
        })
        .collect();
    app.ui_state.recent_projects = RecentProjects::from_paths(paths);
    app.ui_state.pinned_projects = PinnedProjects::default();
    let spelled = app.ui_state.recent_projects.entries().to_vec();
    (app, spelled)
}

fn sections(app: &Signex) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let s = start_screen::project_sections(
        app.ui_state.recent_projects.entries(),
        &app.ui_state.pinned_projects,
    );
    (s.pinned.to_vec(), s.recent.into_iter().cloned().collect())
}

#[test]
fn pin_and_unpin_move_a_project_between_the_sections_and_are_saved() {
    let _lock = lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, recent) = app_with_recent(dir.path(), &["a.snxprj", "b.snxprj", "c.snxprj"]);
    let (a, b, c) = (recent[0].clone(), recent[1].clone(), recent[2].clone());

    send(&mut app, StartScreenMsg::ProjectPinned(c.clone()));
    send(&mut app, StartScreenMsg::ProjectPinned(a.clone()));
    // A second pin of the same project changes nothing.
    send(&mut app, StartScreenMsg::ProjectPinned(c.clone()));

    assert_eq!(
        app.ui_state.pinned_projects.entries(),
        [c.clone(), a.clone()]
    );
    assert_eq!(
        sections(&app),
        (vec![c.clone(), a.clone()], vec![b.clone()])
    );
    assert_eq!(
        signex_app::fonts::read_pinned_projects(),
        [c.clone(), a.clone()],
        "pinning saves the list in the user's order"
    );
    let (restarted, _startup) = Signex::new();
    assert_eq!(
        restarted.ui_state.pinned_projects.entries(),
        [c.clone(), a.clone()],
        "a restart reads the pins back"
    );

    send(&mut app, StartScreenMsg::ProjectUnpinned(c.clone()));

    assert_eq!(
        app.ui_state.pinned_projects.entries(),
        std::slice::from_ref(&a)
    );
    assert_eq!(
        sections(&app),
        (vec![a.clone()], vec![b.clone(), c.clone()])
    );
    assert_eq!(
        signex_app::fonts::read_pinned_projects(),
        std::slice::from_ref(&a)
    );

    send(&mut app, StartScreenMsg::ProjectUnpinned(a.clone()));
    assert!(
        sections(&app).0.is_empty(),
        "nothing pinned: no Pinned section"
    );
    assert!(signex_app::fonts::read_pinned_projects().is_empty());
}

#[test]
fn the_row_menu_opens_on_right_click_and_closes_on_a_pick() {
    let _lock = lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, recent) = app_with_recent(dir.path(), &["a.snxprj"]);
    let a = recent[0].clone();

    send(&mut app, StartScreenMsg::RowRightClicked(a.clone()));
    assert_eq!(
        app.interaction_state
            .start_row_menu
            .as_ref()
            .map(|m| &m.path),
        Some(&a)
    );
    send(&mut app, StartScreenMsg::RowMenuDismissed);
    assert!(app.interaction_state.start_row_menu.is_none());

    send(&mut app, StartScreenMsg::RowRightClicked(a.clone()));
    send(&mut app, StartScreenMsg::ProjectPinned(a.clone()));
    assert!(app.interaction_state.start_row_menu.is_none());
    assert_eq!(
        app.ui_state.pinned_projects.entries(),
        std::slice::from_ref(&a)
    );

    send(&mut app, StartScreenMsg::ProjectUnpinned(a));
    assert!(signex_app::fonts::read_pinned_projects().is_empty());
}

#[test]
fn a_pinned_project_opens_and_a_missing_one_is_marked_then_removed() {
    let _lock = lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, recent) = app_with_recent(dir.path(), &["here.snxprj", "gone.snxprj"]);
    let (here, gone) = (recent[0].clone(), recent[1].clone());
    send(&mut app, StartScreenMsg::ProjectPinned(here.clone()));
    send(&mut app, StartScreenMsg::ProjectPinned(gone.clone()));
    std::fs::remove_file(&gone).expect("delete project");

    let _ = app.update(start_screen::message(StartScreenMsg::PinnedProjectClicked(
        here.clone(),
    )));
    assert!(
        app.document_state.projects.iter().any(|p| p.path == here),
        "a pinned row opens its project"
    );

    let opened = app.document_state.projects.len();
    send(&mut app, StartScreenMsg::PinnedProjectClicked(gone.clone()));
    assert_eq!(app.document_state.projects.len(), opened, "nothing opens");
    assert!(app.ui_state.pinned_projects.is_missing(&gone));
    assert!(
        app.ui_state.pinned_projects.contains(&gone),
        "a missing pin stays until the user removes it"
    );

    send(&mut app, StartScreenMsg::ProjectUnpinned(gone.clone()));
    assert_eq!(
        app.ui_state.pinned_projects.entries(),
        std::slice::from_ref(&here)
    );
    assert_eq!(
        signex_app::fonts::read_pinned_projects(),
        std::slice::from_ref(&here)
    );

    send(&mut app, StartScreenMsg::ProjectUnpinned(here));
}

/// Review of #2305: Open in a recent row's menu closes the menu, whether
/// the project opens or has vanished.
#[test]
fn open_from_a_recent_row_menu_closes_the_menu() {
    let _lock = lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, recent) = app_with_recent(dir.path(), &["here.snxprj", "gone.snxprj"]);
    let (here, gone) = (recent[0].clone(), recent[1].clone());
    std::fs::remove_file(&gone).expect("delete project");

    send(&mut app, StartScreenMsg::RowRightClicked(here.clone()));
    let _ = app.update(start_screen::message(StartScreenMsg::RecentProjectClicked(
        here.clone(),
    )));
    assert!(app.interaction_state.start_row_menu.is_none());
    assert!(
        app.document_state.projects.iter().any(|p| p.path == here),
        "Open opens the project"
    );

    send(&mut app, StartScreenMsg::RowRightClicked(gone.clone()));
    send(&mut app, StartScreenMsg::RecentProjectClicked(gone.clone()));
    assert!(app.interaction_state.start_row_menu.is_none());
    assert!(
        !app.ui_state.recent_projects.entries().contains(&gone),
        "a vanished entry is dropped as File > Open Recent drops it"
    );
}

#[test]
fn esc_closes_the_row_menu() {
    let _lock = lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, recent) = app_with_recent(dir.path(), &["a.snxprj"]);

    send(&mut app, StartScreenMsg::RowRightClicked(recent[0].clone()));
    assert!(app.interaction_state.start_row_menu.is_some());
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    assert!(app.interaction_state.start_row_menu.is_none());
}

#[test]
fn a_pin_follows_a_project_rename() {
    let _lock = lock();
    let mut f = crate::project_tab_support::fixture();
    f.app.ui_state.pinned_projects = PinnedProjects::default();
    let spelled = RecentProjects::spelling_of(&f.prj);
    send(&mut f.app, StartScreenMsg::ProjectPinned(spelled));

    crate::project_tab_support::open_tab(&mut f.app);
    let project_id = crate::project_tab_support::project_id(&f.app);
    let _ = f.app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::RenameRequested { project_id },
    )));
    let _ = f.app.update(Message::Rename(RenameMsg::BufferChanged(
        "Oscillator".into(),
    )));
    crate::settle_support::settle(&mut f.app, Message::Rename(RenameMsg::Submit));

    let renamed = f.prj.with_file_name("Oscillator.snxprj");
    assert!(
        renamed.exists() && !f.prj.exists(),
        "the rename went through"
    );
    let pins = f.app.ui_state.pinned_projects.entries().to_vec();
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].file_name().unwrap(), "Oscillator.snxprj");
    assert!(!f.app.ui_state.pinned_projects.is_missing(&pins[0]));
    assert_eq!(signex_app::fonts::read_pinned_projects(), pins, "saved");

    send(&mut f.app, StartScreenMsg::ProjectUnpinned(pins[0].clone()));
}

#[test]
fn a_pin_missing_at_start_up_shows_as_missing() {
    let _lock = lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let gone = dir.path().join("gone.snxprj");
    signex_app::fonts::write_pinned_projects(std::slice::from_ref(&gone));

    let (mut app, _startup) = Signex::new();

    let pins = app.ui_state.pinned_projects.entries().to_vec();
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].file_name().unwrap(), "gone.snxprj");
    assert!(app.ui_state.pinned_projects.is_missing(&pins[0]));
    let sections = start_screen::project_sections(
        app.ui_state.recent_projects.entries(),
        &app.ui_state.pinned_projects,
    );
    assert_eq!(sections.pinned, pins.as_slice());

    send(&mut app, StartScreenMsg::ProjectUnpinned(pins[0].clone()));
    assert!(signex_app::fonts::read_pinned_projects().is_empty());
}
