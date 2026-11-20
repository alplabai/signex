//! #2523 — page tabs are titled "<project> — <page>" (decisions XF-14):
//! opening the Rules or the Project tab of project `dcdc-buck` names both.

use signex_app::app::{Message, ProjectMsg, RulesTabMsg, TabKind};

use crate::project_tab_support::{fixture, open_tab, project_id};

/// The shared fixture's project, named `dcdc-buck`.
fn dcdc_buck() -> crate::project_tab_support::Fixture {
    let mut f = fixture();
    f.app.document_state.projects[0].data.name = "dcdc-buck".to_string();
    f
}

#[test]
fn the_rules_tab_of_dcdc_buck_is_titled_dcdc_buck_rules() {
    let mut f = dcdc_buck();
    let project = project_id(&f.app);
    let _ = f.app.update(Message::Project(ProjectMsg::Rules(
        RulesTabMsg::OpenRequested {
            project: Some(project),
        },
    )));
    let ds = &f.app.document_state;
    let tab = &ds.tabs[ds.active_tab];
    assert!(matches!(tab.kind, TabKind::Rules(id) if id == project));
    assert_eq!(tab.title, "dcdc-buck — Rules");
    assert_eq!(tab.path, f.prj, "the page saves into the .snxprj");
}

#[test]
fn the_project_tab_of_dcdc_buck_is_titled_dcdc_buck_project() {
    let mut f = dcdc_buck();
    open_tab(&mut f.app);
    let ds = &f.app.document_state;
    let tab = &ds.tabs[ds.active_tab];
    assert!(tab.kind.as_project().is_some());
    assert_eq!(tab.title, "dcdc-buck — Project");
}
