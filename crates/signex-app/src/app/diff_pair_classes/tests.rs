//! #1489: the Differential Pair Classes dialog edits a copy; OK applies it
//! as one project step, Cancel drops it.

use std::collections::HashMap;

use signex_types::diff_pair_class::DiffPairClass;
use signex_types::project::ProjectData;

use super::{
    DiffPairClassesMsg, DiffPairClassesState, PairClassField, PairClassProblem, PairMemberRow,
    free_name, new_class,
};
use crate::app::Signex;
use crate::app::state::{LoadedProject, ProjectId};

const PROJECT: ProjectId = ProjectId::test_id(41);

fn class(name: &str) -> DiffPairClass {
    new_class(name.to_string())
}

fn state(names: &[&str], members: &[(&str, &str)]) -> DiffPairClassesState {
    DiffPairClassesState {
        project: PROJECT,
        project_name: "dcdc-buck".to_string(),
        classes: names.iter().map(|n| class(n)).collect(),
        original_names: names.iter().map(|n| Some(n.to_string())).collect(),
        has_closed_sheets: false,
        selected: 0,
        menu: None,
        members: members
            .iter()
            .map(|(class, pair)| PairMemberRow {
                class: class.to_string(),
                pair: pair.to_string(),
                positive: format!("{pair}_P"),
                negative: format!("{pair}_N"),
                sheet: "power_in.snxsch".to_string(),
            })
            .collect(),
        drafts: Vec::new(),
        problem: None,
    }
}

#[test]
fn a_new_name_is_free_and_a_class_with_pairs_cannot_be_deleted() {
    let s = state(&["USB", "USB_2"], &[("USB", "USB_D")]);
    assert_eq!(free_name(&s.classes, "USB"), "USB_3");
    assert_eq!(free_name(&s.classes, "HDMI"), "HDMI");
    assert_eq!(s.pair_count(0), 1);
    assert!(!s.can_delete(0));
    assert!(s.can_delete(1));
    assert!(!s.can_delete(7));
    assert!(class("X").pair_by_suffix, "a new class pairs by suffix");
}

#[test]
fn ok_is_refused_for_a_blank_or_repeated_name() {
    let mut s = state(&["USB", " USB "], &[]);
    assert_eq!(s.problem(), Some(PairClassProblem::DuplicateName));
    s.classes[1].name = "  ".to_string();
    assert_eq!(s.problem(), Some(PairClassProblem::BlankName));
    s.classes[1].name = " HDMI ".to_string();
    assert_eq!(s.problem(), None);
    assert_eq!(s.applied()[1].name, "HDMI");
}

fn app_with_project(classes: Vec<DiffPairClass>) -> Signex {
    let (mut app, _boot) = Signex::new();
    let mut data: ProjectData = serde_json::from_str(
        r#"{"name":"dcdc-buck","dir":"","schematic_root":null,"pcb_file":null}"#,
    )
    .expect("a project");
    data.schematic_design.diff_pair_classes = classes;
    app.document_state.projects.push(LoadedProject {
        id: PROJECT,
        path: std::env::temp_dir().join("dpc-dialog.snxprj"),
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app
}

fn classes_of(app: &Signex) -> Vec<DiffPairClass> {
    app.document_state
        .project_by_id(PROJECT)
        .expect("the project")
        .data
        .schematic_design
        .diff_pair_classes
        .clone()
}

/// Open the dialog on the test project directly: no document is in view.
fn open(app: &mut Signex) {
    let classes = classes_of(app);
    let original_names = classes.iter().map(|c| Some(c.name.clone())).collect();
    app.ui_state.diff_pair_classes = Some(Box::new(DiffPairClassesState {
        classes,
        original_names,
        ..state(&[], &[])
    }));
}

fn dialog(app: &Signex) -> &DiffPairClassesState {
    app.ui_state.diff_pair_classes.as_deref().expect("open")
}

/// Directives name a class, and a rename is not carried to them: a class
/// with pairs keeps its name, so renaming it cannot slip past the guard
/// that keeps it from being deleted.
#[test]
fn a_class_with_pairs_can_be_neither_renamed_nor_then_deleted() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    if let Some(s) = app.ui_state.diff_pair_classes.as_mut() {
        s.members = state(&[], &[("USB", "USB_D")]).members;
    }
    send(&mut app, DiffPairClassesMsg::NameEdited("USB3".into()));
    assert_eq!(dialog(&app).classes[0].name, "USB", "the name is kept");
    assert_eq!(dialog(&app).pair_count(0), 1);
    send(&mut app, DiffPairClassesMsg::ClassDeleted(0));
    assert_eq!(dialog(&app).classes.len(), 1, "and it is not deleted");
    send(&mut app, DiffPairClassesMsg::OkRequested);
    assert_eq!(classes_of(&app), [class("USB")]);
}

/// The pair count follows the name the class had when the dialog opened.
#[test]
fn pairs_are_counted_by_the_original_name() {
    let mut s = state(&["USB", "HDMI"], &[("USB", "USB_D")]);
    s.classes[0].name = "renamed".to_string();
    assert_eq!(s.pair_count(0), 1);
    assert!(!s.can_delete(0));
}

/// Pairs on sheets that are not open are unknown, so no class that was in
/// the project may be renamed or deleted; one added here may.
#[test]
fn closed_sheets_lock_every_existing_class() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    if let Some(s) = app.ui_state.diff_pair_classes.as_mut() {
        s.has_closed_sheets = true;
    }
    assert_eq!(dialog(&app).lock(0), Some(super::ClassLock::SheetsClosed));
    send(&mut app, DiffPairClassesMsg::NameEdited("USB3".into()));
    send(&mut app, DiffPairClassesMsg::ClassDeleted(0));
    assert_eq!(dialog(&app).classes, [class("USB")]);
    send(&mut app, DiffPairClassesMsg::ClassAdded);
    assert_eq!(dialog(&app).classes[1].name, "DIFF");
    assert_eq!(dialog(&app).lock(1), None, "a new class is free");
    send(&mut app, DiffPairClassesMsg::ClassDeleted(1));
    assert_eq!(dialog(&app).classes.len(), 1);
}

fn send(app: &mut Signex, msg: DiffPairClassesMsg) {
    let _ = app.dispatch_diff_pair_classes_message(msg);
}

#[test]
fn ok_applies_every_edit_as_one_undo_step() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    send(
        &mut app,
        DiffPairClassesMsg::FieldEdited(PairClassField::Width, "0.2".into()),
    );
    send(
        &mut app,
        DiffPairClassesMsg::FieldEdited(PairClassField::Gap, "0.15".into()),
    );
    send(&mut app, DiffPairClassesMsg::ClassAdded);
    send(&mut app, DiffPairClassesMsg::NameEdited("HDMI".into()));
    assert_eq!(
        classes_of(&app),
        [class("USB")],
        "nothing applied before OK"
    );

    send(&mut app, DiffPairClassesMsg::OkRequested);
    assert!(app.ui_state.diff_pair_classes.is_none(), "OK closes");
    let applied = classes_of(&app);
    assert_eq!(applied.len(), 2);
    assert_eq!(applied[0].width_mm, Some(0.2));
    assert_eq!(applied[0].gap_mm, Some(0.15));
    assert_eq!(applied[1].name, "HDMI");

    let document = &mut app.document_state;
    let project = document
        .projects
        .iter_mut()
        .find(|p| p.id == PROJECT)
        .unwrap();
    let tab = document.project_tabs.get_mut(&PROJECT).expect("a history");
    assert!(tab.general_history.undo(&mut project.data));
    assert_eq!(
        project.data.schematic_design.diff_pair_classes,
        [class("USB")],
        "one undo step puts every edit back"
    );
    assert!(!tab.general_history.can_undo());
}

#[test]
fn cancel_changes_nothing() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    send(&mut app, DiffPairClassesMsg::ClassDeleted(0));
    send(&mut app, DiffPairClassesMsg::PairBySuffixToggled(false));
    send(&mut app, DiffPairClassesMsg::CancelRequested);
    assert!(app.ui_state.diff_pair_classes.is_none());
    assert_eq!(classes_of(&app), [class("USB")]);
    assert!(
        app.document_state
            .project_tabs
            .get(&PROJECT)
            .is_none_or(|tab| !tab.general_history.can_undo()),
        "no step recorded"
    );
}

#[test]
fn a_typed_value_is_kept_while_typed_and_a_class_with_pairs_stays() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    if let Some(s) = app.ui_state.diff_pair_classes.as_mut() {
        s.members = state(&[], &[("USB", "USB_D")]).members;
    }
    send(
        &mut app,
        DiffPairClassesMsg::FieldEdited(PairClassField::Impedance, "9".into()),
    );
    send(
        &mut app,
        DiffPairClassesMsg::FieldEdited(PairClassField::Impedance, "90.".into()),
    );
    send(&mut app, DiffPairClassesMsg::ClassDeleted(0));
    let s = app.ui_state.diff_pair_classes.as_deref().unwrap();
    assert_eq!(s.draft(PairClassField::Impedance), Some("90."));
    assert_eq!(s.classes[0].impedance_ohm, Some(90.0));
    assert_eq!(s.classes.len(), 1, "a class with pairs is not deleted");
}
