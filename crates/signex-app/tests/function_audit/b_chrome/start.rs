//! m1 Start screen, m2 project hint, m3 row menu.

use std::path::PathBuf;

use signex_app::app::{DocumentSetupMsg, Message, NewProjectMsg, ProjectMsg, Signex};
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::recent_projects::RecentProjects;
use signex_app::start_screen::{
    self, Action, HintFacts, ProjectScope, StartPage, StartScreenMsg, Surface, project_listing,
    project_sections,
};
use tempfile::TempDir;

use super::support::{app, esc, sample_project};
use crate::settle_support::settle;

fn send(app: &mut Signex, msg: StartScreenMsg) {
    let _ = app.update(start_screen::message(msg));
}

fn command(app: &mut Signex, id: &str) {
    let id = AppCommandId::new(id).expect("a catalog command");
    settle(app, Message::Menu(MenuMessage::Command(id)));
}

struct Rows {
    app: Signex,
    paths: Vec<PathBuf>,
    _dir: TempDir,
}

/// An app whose recent list holds `names`, files that exist, newest first.
fn rows(names: &[&str]) -> Rows {
    let dir = TempDir::new().expect("tempdir");
    let files: Vec<PathBuf> = names
        .iter()
        .map(|name| {
            let path = dir.path().join(name);
            std::fs::write(&path, b"").expect("create the project file");
            path
        })
        .collect();
    let mut app = app();
    app.ui_state.recent_projects = RecentProjects::from_paths(files);
    let paths = app.ui_state.recent_projects.entries().to_vec();
    Rows {
        app,
        paths,
        _dir: dir,
    }
}

/// What the Projects page lists for the filter and switch now set.
fn listed(app: &Signex) -> Vec<PathBuf> {
    let state = &app.ui_state.start_screen;
    project_listing(
        &app.ui_state.i18n,
        app.ui_state.recent_projects.entries(),
        &app.ui_state.pinned_projects,
        &state.project_filter,
        state.project_scope,
    )
    .into_iter()
    .map(|row| row.path)
    .collect()
}

// ---- m1 Start screen ------------------------------------------------------

#[test]
fn m1_the_navigation_rows_change_the_page_and_home_comes_back() {
    let mut app = app();
    assert_eq!(app.ui_state.start_screen.page, StartPage::Home);
    send(&mut app, StartScreenMsg::PageSelected(StartPage::Projects));
    assert_eq!(app.ui_state.start_screen.page, StartPage::Projects);
    send(&mut app, StartScreenMsg::PageSelected(StartPage::Libraries));
    assert_eq!(app.ui_state.start_screen.page, StartPage::Libraries);
    send(&mut app, StartScreenMsg::PageSelected(StartPage::Home));
    assert_eq!(app.ui_state.start_screen.page, StartPage::Home);
}

#[test]
fn m1_home_lists_five_recent_projects_and_offers_show_all() {
    let rows = rows(&[
        "a.snxprj", "b.snxprj", "c.snxprj", "d.snxprj", "e.snxprj", "f.snxprj", "g.snxprj",
    ]);
    let sections = project_sections(
        rows.app.ui_state.recent_projects.entries(),
        &rows.app.ui_state.pinned_projects,
    );
    assert_eq!(sections.recent.len(), 5);
    assert_eq!(sections.recent_total, 7);
    assert!(sections.has_more_recent(), "no Show all link");
    assert_eq!(listed(&rows.app).len(), 7, "the Projects page has no cap");
}

#[test]
fn m1_the_projects_filter_ignores_case_and_clear_filter_lists_everything_again() {
    let mut rows = rows(&["qzalpha.snxprj", "qzbeta.snxprj"]);
    send(
        &mut rows.app,
        StartScreenMsg::PageSelected(StartPage::Projects),
    );
    send(
        &mut rows.app,
        StartScreenMsg::ProjectFilterChanged("QZBETA".to_string()),
    );
    assert_eq!(listed(&rows.app), [rows.paths[1].clone()]);

    send(&mut rows.app, StartScreenMsg::FilterCleared);
    assert!(rows.app.ui_state.start_screen.project_filter.is_empty());
    assert_eq!(listed(&rows.app).len(), 2);
}

#[test]
fn m1_a_filter_that_matches_nothing_lists_no_project() {
    let mut rows = rows(&["qzalpha.snxprj"]);
    send(
        &mut rows.app,
        StartScreenMsg::PageSelected(StartPage::Projects),
    );
    send(
        &mut rows.app,
        StartScreenMsg::ProjectFilterChanged("no-such-project-zzz".to_string()),
    );
    assert!(listed(&rows.app).is_empty());
    assert_eq!(
        rows.app.ui_state.recent_projects.entries().len(),
        1,
        "a filter changes the view, not the list"
    );
}

#[test]
fn m1_the_libraries_filter_is_cleared_on_its_own_page_only() {
    let mut app = app();
    send(
        &mut app,
        StartScreenMsg::ProjectFilterChanged("kept".to_string()),
    );
    send(&mut app, StartScreenMsg::PageSelected(StartPage::Libraries));
    send(
        &mut app,
        StartScreenMsg::LibraryFilterChanged("passives".to_string()),
    );
    send(&mut app, StartScreenMsg::FilterCleared);
    assert!(app.ui_state.start_screen.library_filter.is_empty());
    assert_eq!(app.ui_state.start_screen.project_filter, "kept");
}

#[test]
fn m1_the_pinned_and_missing_switch_narrow_the_projects_page() {
    let mut rows = rows(&["a.snxprj", "b.snxprj", "c.snxprj"]);
    let (pinned, gone) = (rows.paths[0].clone(), rows.paths[1].clone());
    send(&mut rows.app, StartScreenMsg::ProjectPinned(pinned.clone()));
    send(&mut rows.app, StartScreenMsg::ProjectPinned(gone.clone()));
    send(
        &mut rows.app,
        StartScreenMsg::PinsChecked(Ok(vec![gone.clone()])),
    );

    send(
        &mut rows.app,
        StartScreenMsg::ProjectScopePicked(ProjectScope::Pinned),
    );
    assert_eq!(listed(&rows.app), [pinned, gone.clone()]);
    send(
        &mut rows.app,
        StartScreenMsg::ProjectScopePicked(ProjectScope::Missing),
    );
    assert_eq!(listed(&rows.app), [gone]);
    send(
        &mut rows.app,
        StartScreenMsg::ProjectScopePicked(ProjectScope::All),
    );
    assert_eq!(listed(&rows.app).len(), 3);
}

#[test]
fn m1_pin_moves_a_row_to_pinned_and_unpin_moves_it_back() {
    let mut rows = rows(&["a.snxprj", "b.snxprj"]);
    let path = rows.paths[1].clone();
    send(&mut rows.app, StartScreenMsg::ProjectPinned(path.clone()));
    {
        let sections = project_sections(
            rows.app.ui_state.recent_projects.entries(),
            &rows.app.ui_state.pinned_projects,
        );
        assert_eq!(sections.pinned, [path.clone()]);
        assert!(!sections.recent.contains(&&path), "listed twice");
    }

    send(&mut rows.app, StartScreenMsg::ProjectUnpinned(path.clone()));
    let sections = project_sections(
        rows.app.ui_state.recent_projects.entries(),
        &rows.app.ui_state.pinned_projects,
    );
    assert!(sections.pinned.is_empty());
    assert!(sections.recent.contains(&&path));
}

#[test]
fn m1_a_recent_row_whose_file_is_gone_is_dropped_with_a_notice_and_opens_nothing() {
    let mut app = app();
    let gone = std::env::temp_dir().join("signex-audit-gone-recent.snxprj");
    app.ui_state.recent_projects = RecentProjects::from_paths([gone.clone()]);
    let entry = app.ui_state.recent_projects.entries()[0].clone();

    let _ = app.update(start_screen::recent_message(&entry));

    assert!(app.ui_state.recent_projects.entries().is_empty());
    assert!(app.document_state.error_notice.is_some(), "no notice");
    assert!(app.document_state.projects.is_empty());
    assert!(app.document_state.tabs.is_empty());
}

#[test]
fn m1_a_pin_whose_file_is_gone_stays_listed_as_missing_and_remove_drops_it() {
    let mut app = app();
    let gone = std::env::temp_dir().join("signex-audit-gone-pin.snxprj");
    send(&mut app, StartScreenMsg::ProjectPinned(gone.clone()));

    let _ = app.update(start_screen::message(StartScreenMsg::PinnedProjectClicked(
        gone.clone(),
    )));

    assert!(app.ui_state.pinned_projects.contains(&gone), "pin dropped");
    assert!(app.ui_state.pinned_projects.is_missing(&gone));
    assert!(app.document_state.projects.is_empty());

    send(&mut app, StartScreenMsg::ProjectUnpinned(gone.clone()));
    assert!(!app.ui_state.pinned_projects.contains(&gone));
    assert!(!app.ui_state.pinned_projects.is_missing(&gone));
}

#[test]
fn m1_new_project_opens_its_setup_and_cancel_creates_nothing() {
    let mut app = app();
    let _ = app.update(Action::NewProject.message());
    assert!(app.ui_state.new_project.is_some(), "no setup dialog");

    let _ = app.update(Message::Project(ProjectMsg::Setup(
        NewProjectMsg::CancelRequested,
    )));

    assert!(app.ui_state.new_project.is_none());
    assert!(app.document_state.projects.is_empty());
    assert!(app.document_state.tabs.is_empty());
}

#[test]
fn m1_open_sample_project_loads_the_bundled_project() {
    let mut app = app();
    settle(&mut app, Action::OpenSampleProject.message());
    assert!(
        !app.document_state.projects.is_empty(),
        "the sample project did not load: {:?}",
        app.document_state
            .error_notice
            .as_ref()
            .map(|notice| notice.detail_text(&app.ui_state.i18n))
    );
}

#[test]
fn m1_the_keyboard_shortcuts_row_opens_the_sheet() {
    let mut app = app();
    let _ = app.update(Action::KeyboardShortcuts.message());
    assert!(app.ui_state.keyboard_shortcuts_open);
}

#[test]
fn m1_opening_a_project_sends_the_screen_back_home_unfiltered() {
    let sample = sample_project();
    let mut fresh = app();
    send(
        &mut fresh,
        StartScreenMsg::PageSelected(StartPage::Projects),
    );
    send(
        &mut fresh,
        StartScreenMsg::ProjectFilterChanged("x".to_string()),
    );
    settle(
        &mut fresh,
        Message::File(signex_app::app::FileMsg::Opened(Some(
            sample.project.clone(),
        ))),
    );
    assert!(fresh.ui_state.start_screen.is_home());
}

// ---- m2 Project hint ------------------------------------------------------

#[test]
fn m2_a_project_with_no_document_picked_shows_the_hint() {
    let sample = sample_project();
    assert!(sample.app.document_state.tabs.is_empty());
    assert_eq!(
        Surface::select(sample.app.document_state.active_project.is_some()),
        Surface::ProjectHint
    );
    assert!(HintFacts::of(sample.app.document_state.projects.first()).has_documents);
}

#[test]
fn m2_a_project_with_no_sheet_and_no_board_gets_the_empty_project_wording() {
    let mut sample = sample_project();
    let project = sample
        .app
        .document_state
        .projects
        .first_mut()
        .expect("a project");
    project.data.sheets.clear();
    project.data.schematic_root = None;
    project.data.pcb_file = None;
    assert!(!HintFacts::of(sample.app.document_state.projects.first()).has_documents);
}

#[test]
fn m2_new_sheet_opens_its_setup_and_cancel_adds_nothing_to_the_project() {
    let mut sample = sample_project();
    let sheets = sample.app.document_state.projects[0].data.sheets.len();
    let on_disk = std::fs::read(&sample.project).expect("read the project");

    command(&mut sample.app, "new_schematic_sheet");
    assert!(sample.app.ui_state.document_setup.is_some(), "no setup");

    let _ = sample
        .app
        .update(Message::from(DocumentSetupMsg::CancelRequested));

    assert!(sample.app.ui_state.document_setup.is_none());
    assert_eq!(
        sample.app.document_state.projects[0].data.sheets.len(),
        sheets
    );
    assert_eq!(std::fs::read(&sample.project).expect("read"), on_disk);
}

#[test]
fn m2_new_pcb_opens_its_setup_and_cancel_adds_no_board() {
    let mut sample = sample_project();
    let board = sample.app.document_state.projects[0].data.pcb_file.clone();

    command(&mut sample.app, "new_pcb_document");
    assert!(sample.app.ui_state.document_setup.is_some(), "no setup");

    let _ = sample
        .app
        .update(Message::from(DocumentSetupMsg::CancelRequested));

    assert!(sample.app.ui_state.document_setup.is_none());
    assert_eq!(sample.app.document_state.projects[0].data.pcb_file, board);
}

#[test]
fn m2_project_options_opens_the_project_page_and_writes_nothing() {
    let mut sample = sample_project();
    let on_disk = std::fs::read(&sample.project).expect("read the project");

    command(&mut sample.app, "open_project_options");

    assert!(
        sample
            .app
            .document_state
            .tabs
            .iter()
            .any(|tab| tab.kind.is_project_page()),
        "no project page tab"
    );
    assert_eq!(std::fs::read(&sample.project).expect("read"), on_disk);
}

// ---- m3 Row menu ----------------------------------------------------------

#[test]
fn m3_right_click_opens_the_menu_of_that_row_and_esc_changes_nothing() {
    let mut rows = rows(&["a.snxprj", "b.snxprj"]);
    let path = rows.paths[1].clone();
    send(&mut rows.app, StartScreenMsg::RowRightClicked(path.clone()));
    assert_eq!(
        rows.app
            .interaction_state
            .start_row_menu
            .as_ref()
            .map(|menu| menu.path.clone()),
        Some(path)
    );

    esc(&mut rows.app);

    assert!(rows.app.interaction_state.start_row_menu.is_none());
    assert!(rows.app.ui_state.pinned_projects.is_empty());
    assert_eq!(rows.app.ui_state.recent_projects.entries().len(), 2);
}

#[test]
fn m3_pin_and_unpin_from_the_menu_close_it_and_change_the_list() {
    let mut rows = rows(&["a.snxprj"]);
    let path = rows.paths[0].clone();
    send(&mut rows.app, StartScreenMsg::RowRightClicked(path.clone()));
    send(&mut rows.app, StartScreenMsg::ProjectPinned(path.clone()));
    assert!(rows.app.interaction_state.start_row_menu.is_none());
    assert!(rows.app.ui_state.pinned_projects.contains(&path));

    send(&mut rows.app, StartScreenMsg::RowRightClicked(path.clone()));
    send(&mut rows.app, StartScreenMsg::ProjectUnpinned(path.clone()));
    assert!(rows.app.interaction_state.start_row_menu.is_none());
    assert!(!rows.app.ui_state.pinned_projects.contains(&path));
}

#[test]
fn m3_open_from_the_menu_closes_it_and_opens_the_project() {
    let sample = sample_project();
    let mut app = app();
    app.ui_state.recent_projects = RecentProjects::from_paths([sample.project.clone()]);
    let entry = app.ui_state.recent_projects.entries()[0].clone();
    send(&mut app, StartScreenMsg::RowRightClicked(entry.clone()));

    settle(
        &mut app,
        start_screen::message(StartScreenMsg::RecentProjectClicked(entry)),
    );

    assert!(app.interaction_state.start_row_menu.is_none());
    assert_eq!(app.document_state.projects.len(), 1);
}
