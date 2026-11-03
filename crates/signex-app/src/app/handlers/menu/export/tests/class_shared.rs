//! One connectivity commit stitches a project once for the net-class styles
//! and the net index, and a child sheet placed twice is named by the net whose
//! style the canvas draws (#1330).

use std::path::PathBuf;

use signex_types::net_class::{NetClass, NetLineStyle};
use signex_types::schematic::{LabelType, SelectedItem, SelectedKind};

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::stitched_netlist::STITCH_RUNS;

fn power_class(net: &str) -> NetClass {
    let mut class = NetClass::named("Power");
    class.nets.push(net.to_string());
    class.style.line = NetLineStyle::Dashed;
    class
}

#[test]
fn one_commit_stitches_the_project_once_for_the_styles_and_the_index() {
    let dir = PathBuf::from("/w/class-shared");
    let parent = sheet_with_net("R_TOP", "VCC", &["child.snxsch"]);
    let child = sheet_with_net("R_CHILD", "LOCAL", &[]);
    let wire = parent.wires[0].uuid;
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch", "child.snxsch"]);
    app.document_state.projects[0].data.net_classes = vec![power_class("VCC")];
    open_with(&mut app.document_state, &dir.join("top.snxsch"), parent);
    open_with(&mut app.document_state, &dir.join("child.snxsch"), child);
    app.document_state.active_path = Some(dir.join("top.snxsch"));
    STITCH_RUNS.with(|runs| runs.set(0));

    // One commit asks for both: the style job is planned, then the index job
    // of the same project at the same inputs, before either has run.
    let styles = app.plan_net_class_refresh().expect("style job planned");
    app.settle_net_index_blocking();
    let resolved = styles.run();
    app.apply_net_class_styles(&Ok(resolved));

    assert_eq!(
        STITCH_RUNS.with(|runs| runs.get()),
        1,
        "styles and index read one stitch"
    );
    assert!(
        app.ui_state.net_class_styles.styles().contains_key(&wire),
        "the styles still resolve from the shared stitch"
    );
    let project = app.document_state.projects[0].id;
    assert!(
        app.document_state
            .net_nav
            .index
            .fresh(project, &key_of(&app))
            .is_some(),
        "the index built from the shared stitch"
    );
}

fn key_of(app: &crate::app::Signex) -> crate::app::state::net_class_styles::ProjectKey {
    crate::app::runtime::net_class_styles::plan_projects(&app.document_state)
        .remove(0)
        .key
}

#[test]
fn a_label_on_a_sheet_placed_twice_names_the_net_the_canvas_styles() {
    let dir = PathBuf::from("/w/class-twice");
    let parent = sheet_with_net("R_TOP", "VCC", &["child.snxsch", "child.snxsch"]);
    let mut child = sheet_with_net("R_CHILD", "LOCAL", &[]);
    child.labels[0].label_type = LabelType::Net;
    let (label, wire) = (child.labels[0].uuid, child.wires[0].uuid);
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch", "child.snxsch"]);
    open_with(&mut app.document_state, &dir.join("top.snxsch"), parent);
    open_with(&mut app.document_state, &dir.join("child.snxsch"), child);
    app.document_state.active_path = Some(dir.join("child.snxsch"));

    // The two placements name the child's net differently; style the second.
    let stitched = crate::app::stitched_netlist::stitched_for_active_project(&app.document_state);
    let names: Vec<String> = stitched
        .netlist
        .expect("stitched")
        .nets
        .iter()
        .filter(|net| net.wires.contains(&wire))
        .map(|net| net.name.clone())
        .collect();
    assert_eq!(names.len(), 2, "one net per placement: {names:?}");
    app.document_state.projects[0].data.net_classes = vec![power_class(&names[1])];
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(label, SelectedKind::Label)];
    app.update_selection_info();
    app.settle_net_class_styles_blocking();
    app.sync_label_class_pick();

    assert_eq!(
        app.ui_state
            .net_class_styles
            .styles()
            .get(&wire)
            .map(|s| s.line),
        Some(NetLineStyle::Dashed),
        "the second placement's net styles the shared wire"
    );
    let pick = app
        .document_state
        .panel_ctx
        .net_classes
        .label
        .as_ref()
        .expect("the label's net resolved");
    assert_eq!(
        pick.net, names[1],
        "the picker names the net whose style is drawn, not the first placement's"
    );
}

#[test]
fn an_answer_for_older_connectivity_is_drawn_until_the_newer_one_lands() {
    use crate::app::{Message, UiMsg};
    use std::sync::Arc;

    let dir = PathBuf::from("/w/class-older");
    let top = dir.join("top.snxsch");
    let parent = sheet_with_net("R_TOP", "VCC", &[]);
    let wire = parent.wires[0].uuid;
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    app.document_state.projects[0].data.net_classes = vec![power_class("VCC")];
    open_with(&mut app.document_state, &top, parent);
    app.document_state.active_path = Some(top.clone());
    let deliver = |app: &mut crate::app::Signex, resolved| {
        let _ = app.update(Message::Ui(UiMsg::NetClassStylesResolved(Arc::new(Ok(
            resolved,
        )))));
    };

    // Two commits in a row, so the first answer arrives after the second was
    // asked for: continuous editing.
    let older = app.plan_net_class_refresh().expect("first commit").run();
    app.document_state
        .engines
        .get_mut(&top)
        .expect("engine")
        .execute(signex_engine::Command::DeleteSelection {
            items: vec![SelectedItem::new(wire, SelectedKind::Wire)],
        })
        .expect("delete wire");
    let newer = app.plan_net_class_refresh().expect("second commit").run();

    deliver(&mut app, older);
    assert!(
        app.ui_state.net_class_styles.styles().contains_key(&wire),
        "styles do not freeze while a newer resolve is outstanding"
    );
    deliver(&mut app, newer);
    assert!(
        !app.ui_state.net_class_styles.styles().contains_key(&wire),
        "the newer answer replaces it"
    );
}
