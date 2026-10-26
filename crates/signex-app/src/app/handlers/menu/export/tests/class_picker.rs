//! The net label's Net Class picker works on the net the project netlist puts
//! the label on and edits the table of the label's own project; and a late or
//! failed background resolution delivered through `update` is handled.

use std::path::PathBuf;
use std::sync::Arc;

use signex_types::net_class::{NetClass, NetLineStyle};
use signex_types::schematic::{LabelType, SelectedItem, SelectedKind};
use uuid::Uuid;

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::{Message, ProjectId, ResolveFailed, Signex, UiMsg};
use crate::dock::DockMessage;
use crate::panels::{NetClassMsg, PanelMsg};

fn power_class() -> NetClass {
    let mut class = NetClass::named("Power");
    class.style.line = NetLineStyle::Dashed;
    class
}

/// A parent with a child sheet, and the child open and active with a local
/// label `LOCAL` on a wire, selected. Returns the app, the label's uuid and
/// the child wire's uuid.
fn child_label_selected(dir: &str) -> (Signex, Uuid, Uuid) {
    let parent = sheet_with_net("R_TOP", "VCC", &["child.snxsch"]);
    let mut child = sheet_with_net("R_CHILD", "LOCAL", &[]);
    child.labels[0].label_type = LabelType::Net;
    let (label, wire) = (child.labels[0].uuid, child.wires[0].uuid);
    let mut app = app_workspace(dir, &["top.snxsch", "child.snxsch"]);
    app.document_state.projects[0].data.net_classes = vec![power_class()];
    let dir = PathBuf::from(dir);
    open_with(&mut app.document_state, &dir.join("top.snxsch"), parent);
    open_with(&mut app.document_state, &dir.join("child.snxsch"), child);
    app.document_state.active_path = Some(dir.join("child.snxsch"));
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(label, SelectedKind::Label)];
    app.update_selection_info();
    app.settle_net_class_styles_blocking();
    app.sync_label_class_pick();
    (app, label, wire)
}

fn assign(app: &mut Signex, project: ProjectId, net: &str, class: &str) {
    let msg = NetClassMsg::AssignNet {
        project,
        net: net.to_string(),
        class: Some(class.to_string()),
    };
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::NetClass(msg))));
    app.settle_net_class_styles_blocking();
}

#[test]
fn the_picker_names_the_net_the_netlist_gives_a_child_sheets_local_label() {
    let (app, ..) = child_label_selected("/w/pick-net");

    let pick = app
        .document_state
        .panel_ctx
        .net_classes
        .label
        .as_ref()
        .expect("the label's net resolved");

    assert_eq!(pick.net, "/LOCAL", "not the label text `LOCAL`");
    assert_eq!(Some(pick.project), app.document_state.active_project);
}

#[test]
fn assigning_from_a_child_label_stores_the_net_name_and_styles_the_wire() {
    let (mut app, _, wire) = child_label_selected("/w/pick-assign");
    let project = app.document_state.active_project.expect("project");

    assign(&mut app, project, "/LOCAL", "Power");

    assert_eq!(
        app.document_state.projects[0].data.net_classes[0].nets,
        ["/LOCAL"]
    );
    assert_eq!(
        app.ui_state
            .net_class_styles
            .styles()
            .get(&wire)
            .map(|s| s.line),
        Some(NetLineStyle::Dashed),
        "the child wire is styled, which the label text `LOCAL` never did"
    );
}

#[test]
fn a_sheet_of_no_project_gets_no_picker() {
    let (mut app, ..) = child_label_selected("/w/pick-loose");
    // The tab is a loose document now: no project lists or reaches it.
    let loose = PathBuf::from("/elsewhere/scratch.snxsch");
    let engine = app
        .document_state
        .engines
        .remove(&PathBuf::from("/w/pick-loose").join("child.snxsch"))
        .expect("engine");
    app.document_state.engines.insert(loose.clone(), engine);
    app.document_state.active_path = Some(loose);

    assert!(app.selected_label_class_pick().is_none());
}

#[test]
fn an_assignment_edits_the_labels_project_not_the_active_one() {
    let (mut app, ..) = child_label_selected("/w/pick-target");
    let active = app.document_state.active_project.expect("project");
    let mut other = app.document_state.projects[0].clone();
    other.id = app.document_state.mint_project_id();
    other.path = PathBuf::from("/w/other/Other.snxprj");
    other.data.net_classes = vec![NetClass::named("Mine")];
    let other_id = other.id;
    let other_path = other.path.clone();
    app.document_state.projects.push(other);
    app.document_state.dirty_paths.clear();

    assign(&mut app, other_id, "SIG", "Mine");

    let table = |id: ProjectId| {
        app.document_state
            .project_by_id(id)
            .expect("project")
            .data
            .net_classes
            .clone()
    };
    assert_eq!(table(other_id)[0].nets, ["SIG"]);
    assert!(
        table(active)[0].nets.is_empty(),
        "the active project is untouched"
    );
    assert!(app.document_state.dirty_paths.contains(&other_path));
    assert!(
        !app.document_state
            .dirty_paths
            .contains(&app.document_state.projects[0].path),
        "and not dirtied"
    );
    assert_eq!(
        app.document_state.active_project,
        Some(active),
        "the active project did not change"
    );
}

#[test]
fn a_child_that_will_not_parse_is_reported_not_dropped_silently() {
    let dir = std::env::temp_dir().join(format!("signex-class-unreadable-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    std::fs::write(dir.join("child.snxsch"), "this is not a schematic").expect("write child");
    let parent = sheet_with_net("R_TOP", "VCC", &["child.snxsch"]);
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    app.document_state.projects[0].data.net_classes = vec![power_class()];
    let top = dir.join("top.snxsch");
    open_with(&mut app.document_state, &top, parent);
    app.document_state.active_path = Some(top);

    let job = app.plan_net_class_refresh().expect("planned");
    let resolved = job.run();
    std::fs::remove_dir_all(&dir).ok();

    let warnings = &resolved.projects[0].warnings;
    assert!(
        warnings.iter().any(|w| w.contains("child.snxsch")),
        "the Messages panel must name the unreadable sheet: {warnings:?}"
    );
    let line = &warnings[0];
    assert!(
        line.find("could not be read") < line.find("child.snxsch"),
        "reason first, path last, so the 157-byte cut keeps the reason: {line}"
    );
}

#[test]
fn late_and_failed_resolutions_delivered_through_update_leave_the_canvas_alone() {
    let dir = "/w/pick-update";
    let (mut app, _, wire) = child_label_selected(dir);
    let project = app.document_state.active_project.expect("project");
    assign(&mut app, project, "/LOCAL", "Power");
    let drawn = |app: &Signex| app.ui_state.net_class_styles.styles().get(&wire).copied();
    let settled = drawn(&app);
    assert!(settled.is_some());

    // Two class-table edits in a row: the first answer is already stale when
    // it arrives.
    let mut dotted = power_class();
    dotted.nets = vec!["/LOCAL".to_string()];
    dotted.style.line = NetLineStyle::Dotted;
    app.document_state.projects[0].data.net_classes = vec![dotted.clone()];
    let stale = app.plan_net_class_refresh().expect("first plan").run();
    dotted.style.line = NetLineStyle::Solid;
    dotted.style.wire_width_mm = Some(0.6);
    app.document_state.projects[0].data.net_classes = vec![dotted];
    let current = app.plan_net_class_refresh().expect("second plan").run();

    let deliver = |app: &mut Signex, result: Result<_, ResolveFailed>| {
        let _ = app.update(Message::Ui(UiMsg::NetClassStylesResolved(Arc::new(result))));
    };
    deliver(&mut app, Ok(stale));
    assert_eq!(drawn(&app), settled, "the stale answer is dropped");
    // Another project's task failed: nothing drawn changes, and this
    // project's request in flight is not cancelled by it.
    let unrelated = app.document_state.mint_project_id();
    deliver(
        &mut app,
        Err(ResolveFailed {
            projects: vec![unrelated],
            reason: "the net-class style task failed".to_string(),
        }),
    );
    assert_eq!(drawn(&app), settled, "a failed task keeps what is drawn");
    deliver(&mut app, Ok(current));
    assert_eq!(
        drawn(&app).and_then(|s| s.wire_width_mm),
        Some(0.6),
        "the answer for the current table lands"
    );
}
