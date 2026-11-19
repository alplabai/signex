//! Page tab titles and the owner-path dirty rule (#2523).

use std::collections::HashSet;
use std::path::PathBuf;

use super::*;
use crate::app::TabInfo;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::tab_bar::tab_is_dirty as is_dirty;

fn tab(kind: TabKind, path: &str) -> TabInfo {
    TabInfo {
        title: String::new(),
        path: PathBuf::from(path),
        cached_document: None,
        project_id: None,
        kind,
    }
}

fn workspace(name: &str) -> (Signex, ProjectId) {
    let mut app = app_workspace("/work/dcdc", &["top.snxsch"]);
    app.document_state.projects[0].data.name = name.to_owned();
    let id = app.document_state.projects[0].id;
    (app, id)
}

#[test]
fn a_page_title_is_the_project_then_the_page() {
    let en = signex_i18n::I18n::en_us();
    assert_eq!(
        page_title(&en, "dcdc-buck", ids::RULES_TAB_TITLE),
        "dcdc-buck — Rules"
    );
    assert_eq!(
        page_title(&en, "sensor-node", ids::CHROME_PAGE_TAB_PROJECT),
        "sensor-node — Project"
    );
}

#[test]
fn project_and_rules_tabs_are_titled_by_their_project() {
    let (app, id) = workspace("dcdc-buck");
    assert_eq!(
        app.page_tab_title(&TabKind::Rules(id)).as_deref(),
        Some("dcdc-buck — Rules")
    );
    assert_eq!(
        app.page_tab_title(&TabKind::Project(id)).as_deref(),
        Some("dcdc-buck — Project")
    );
}

#[test]
fn drawing_and_library_tabs_have_no_page_title() {
    let (app, _) = workspace("dcdc-buck");
    for kind in [
        TabKind::Schematic,
        TabKind::Pcb,
        TabKind::SymbolEditor(PathBuf::from("a.snxsym")),
        TabKind::FootprintEditor(PathBuf::from("a.snxfpt")),
        TabKind::LibraryBrowser(PathBuf::from("a.snxlib")),
    ] {
        assert_eq!(app.page_tab_title(&kind), None, "{kind:?}");
        assert_eq!(page_project(&kind), None, "{kind:?}");
    }
}

#[test]
fn page_kinds_are_exactly_the_project_pages() {
    let (_, id) = workspace("p");
    for kind in [
        TabKind::Project(id),
        TabKind::Rules(id),
        TabKind::Schematic,
        TabKind::Pcb,
        TabKind::LibraryBrowser(PathBuf::new()),
    ] {
        assert_eq!(
            page_name(&kind).is_some(),
            kind.is_project_page(),
            "{kind:?}"
        );
    }
}

#[test]
fn a_page_tab_of_a_closed_project_has_no_title() {
    let (mut app, id) = workspace("dcdc-buck");
    app.document_state.projects.clear();
    assert_eq!(app.page_tab_title(&TabKind::Rules(id)), None);
}

#[test]
fn a_blank_project_name_falls_back_to_the_file_stem() {
    let (app, id) = workspace("  ");
    assert_eq!(
        app.page_tab_title(&TabKind::Rules(id)).as_deref(),
        Some("ProjectA — Rules")
    );
}

#[test]
fn a_page_tab_is_dirty_exactly_when_its_owner_file_is() {
    let (_, id) = workspace("dcdc-buck");
    let rules = tab(TabKind::Rules(id), "/work/dcdc/dcdc-buck.snxprj");
    let project = tab(TabKind::Project(id), "/work/dcdc/dcdc-buck.snxprj");
    let board = tab(TabKind::Pcb, "/work/dcdc/dcdc-buck.snxpcb");

    let prj_dirty: HashSet<PathBuf> = [PathBuf::from("/work/dcdc/dcdc-buck.snxprj")].into();
    assert!(is_dirty(&rules, &prj_dirty));
    assert!(is_dirty(&project, &prj_dirty));
    assert!(!is_dirty(&board, &prj_dirty), "the board did not change");

    let board_dirty: HashSet<PathBuf> = [PathBuf::from("/work/dcdc/dcdc-buck.snxpcb")].into();
    assert!(
        !is_dirty(&rules, &board_dirty),
        "an edit to another file leaves the page clean"
    );
    assert!(is_dirty(&board, &board_dirty));

    let clean = HashSet::new();
    assert!(!is_dirty(&rules, &clean));
    assert!(!is_dirty(&project, &clean));
}

#[test]
fn refreshing_retitles_page_tabs_after_a_rename_and_leaves_others() {
    let (mut app, id) = workspace("Oscar");
    app.document_state.tabs.clear();
    app.document_state
        .tabs
        .push(tab(TabKind::Rules(id), "/work/dcdc/Oscar.snxprj"));
    app.document_state
        .tabs
        .push(tab(TabKind::Project(id), "/work/dcdc/Oscar.snxprj"));
    let mut sheet = tab(TabKind::Schematic, "/work/dcdc/top.snxsch");
    sheet.title = "top".into();
    app.document_state.tabs.push(sheet);
    app.document_state.projects[0].data.name = "Motor".into();

    app.refresh_page_tab_titles();

    let titles: Vec<&str> = app
        .document_state
        .tabs
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(titles, ["Motor — Rules", "Motor — Project", "top"]);
}
