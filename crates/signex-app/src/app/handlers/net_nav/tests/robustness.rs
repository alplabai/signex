//! Review round: a parked command survives a hover tick, a highlight never
//! moves to an unrelated net, placements are per instance, and index warnings
//! reach Messages once.

use std::time::{Duration, Instant};

use signex_types::schematic::{InstancePath, SelectedItem, SelectedKind};

use super::fixture::*;
use crate::app::state::net_nav::hover::DWELL;
use crate::app::{EditMsg, Message, NetNavMsg};

fn delete_selected(p: &mut Project) {
    let _task = p.app.update(Message::Edit(EditMsg::DeleteSelected));
}

#[test]
fn a_hover_tick_during_the_build_does_not_drop_a_parked_jump() {
    let mut p = project();
    // A real edit makes the cached index stale.
    p.focus(&p.mid.clone());
    p.select(23, SelectedKind::Wire);
    delete_selected(&mut p);
    p.focus(&p.root.clone());
    p.select(ROOT_VCC, SelectedKind::Label);
    p.app.observe_net_hover(
        Some(SelectedItem::new(id(ROOT_VCC), SelectedKind::Label)),
        (0.0, 0.0),
    );
    p.app
        .document_state
        .net_nav
        .hover
        .candidate
        .as_mut()
        .expect("candidate")
        .since = Instant::now()
        .checked_sub(DWELL + Duration::from_millis(50))
        .expect("clock");

    p.send(NetNavMsg::PartnerJumpRequested);
    p.send(NetNavMsg::HoverDwellTicked);
    p.app.settle_net_index_blocking();

    assert_eq!(p.active(), Some(p.mid.clone()), "the jump still ran");
}

#[test]
fn deleting_the_anchor_never_moves_the_highlight_to_an_unrelated_net() {
    let mut p = project();
    p.select(R5_WIRE, SelectedKind::Wire);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    let name = p
        .app
        .document_state
        .net_nav
        .highlight
        .as_ref()
        .map(|h| h.name.clone())
        .expect("highlight");
    assert!(name.starts_with("N$"), "{name} is auto-named");

    // R5's net vanishes; R4's net is renumbered into its auto-name.
    p.select(R5_WIRE, SelectedKind::Wire);
    delete_selected(&mut p);
    p.app.settle_net_index_blocking();

    assert!(p.app.document_state.net_nav.highlight.is_none());
    assert!(p.bright().is_none());
    assert!(logged(&["no longer exists"]));
}

#[test]
fn a_successful_resolve_moves_the_anchor_to_a_member_that_still_exists() {
    let mut p = project();
    p.select(ROOT_VCC_WIRE, SelectedKind::Wire);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    p.select(ROOT_VCC_WIRE, SelectedKind::Wire);
    delete_selected(&mut p);
    p.app.settle_net_index_blocking();

    let h = p
        .app
        .document_state
        .net_nav
        .highlight
        .as_ref()
        .expect("VCC survives");
    assert_eq!(h.name, "VCC");
    assert_ne!(
        h.anchor.1,
        id(ROOT_VCC_WIRE),
        "the deleted wire is no anchor"
    );
}

#[test]
fn hover_and_partners_follow_the_placement_the_tab_was_entered_through() {
    let (mut p, child) = placed_twice();
    let via_b = InstancePath::from_chain(vec![id(TWICE_B)]);
    p.app.open_sheet_with_placement(child.clone(), Some(via_b));
    assert_eq!(p.active(), Some(child));

    p.app.observe_net_hover(
        Some(SelectedItem::new(id(TWICE_LABEL), SelectedKind::Label)),
        (0.0, 70.0),
    );
    p.app
        .document_state
        .net_nav
        .hover
        .candidate
        .as_mut()
        .expect("candidate")
        .since = Instant::now()
        .checked_sub(DWELL + Duration::from_millis(50))
        .expect("clock");
    p.send_settled(NetNavMsg::HoverDwellTicked);
    let card = p
        .app
        .document_state
        .net_nav
        .hover
        .card
        .clone()
        .expect("card");
    assert!(
        card.name.starts_with("B/"),
        "instance B's net, got {}",
        card.name
    );

    p.select(TWICE_LABEL, SelectedKind::Label);
    p.send_settled(NetNavMsg::PartnerJumpRequested);
    assert_eq!(p.active(), Some(p.root.clone()));
    assert_eq!(
        p.selected().first().map(|i| i.uuid),
        Some(id(TWICE_ENTRY_B))
    );
}

#[test]
fn an_unreadable_page_is_reported_once_however_often_the_index_rebuilds() {
    let mut p = project_with_broken_page();
    p.select(ROOT_VCC, SelectedKind::Label);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    // An edit forces a second build.
    p.select(R5_WIRE, SelectedKind::Wire);
    delete_selected(&mut p);
    p.app.settle_net_index_blocking();

    assert_eq!(crate::diagnostics::logged_count(BROKEN), 1);
}

#[test]
fn a_page_that_breaks_after_a_clean_class_run_is_reported_once() {
    use signex_types::net_class::NetClass;
    use signex_types::project::SheetEntry;
    let mut p = project();
    let dir = p.root.parent().expect("dir").to_path_buf();
    // A valid listed page and a class table: the class job runs clean.
    let page = dir.join(LATE_PAGE);
    let valid = crate::app::handlers::menu::export::tests::sheet_with_net("R7", "X", &[]);
    let text = signex_types::format::SnxSchematic::new(valid)
        .write_string()
        .expect("encode");
    std::fs::write(&page, text).expect("write page");
    let pid = p.app.document_state.projects[0].id;
    let project = p
        .app
        .document_state
        .project_by_id_mut(pid)
        .expect("project");
    project.data.net_classes = vec![NetClass {
        nets: vec!["VCC".to_string()],
        ..NetClass::named("Power")
    }];
    project.data.sheets.push(SheetEntry {
        name: LATE_PAGE.to_string(),
        filename: LATE_PAGE.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    });
    p.app.settle_net_class_styles_blocking();
    assert_eq!(
        crate::diagnostics::logged_count(LATE_PAGE),
        0,
        "clean so far"
    );

    // The page breaks on disk; only the index build sees it.
    std::fs::write(&page, "not a schematic").expect("break page");
    p.select(ROOT_VCC, SelectedKind::Label);
    p.send_settled(NetNavMsg::RelatedNetHighlightRequested);
    assert_eq!(crate::diagnostics::logged_count(LATE_PAGE), 1);

    // The class job meeting the same page later adds nothing.
    p.app.settle_net_class_styles_blocking();
    assert_eq!(crate::diagnostics::logged_count(LATE_PAGE), 1);
}

#[test]
fn a_command_parked_in_a_closed_project_never_runs_in_another() {
    let mut p = project();
    p.select(ROOT_VCC, SelectedKind::Label);
    p.send(NetNavMsg::PartnerJumpRequested);

    // Project A closes while its index builds; project B opens and is focused.
    let a = p.app.document_state.projects[0].id;
    p.app.document_state.projects.retain(|x| x.id != a);
    let (b_sheet, b_label) = p.add_second_project();
    p.select_uuid(b_label, SelectedKind::Label);
    p.send(NetNavMsg::HoverDwellTicked);
    p.app.settle_net_index_blocking();

    assert_eq!(p.active(), Some(b_sheet));
    assert!(p.app.document_state.net_nav.pending.is_none());
    assert!(
        !logged(&["has no partner", ONLY_B]),
        "A's jump did not run against B's label"
    );
}

#[test]
fn a_stale_answer_starts_the_newest_build_and_the_parked_command_still_runs() {
    use crate::app::runtime::net_class_styles::plan_projects;
    use crate::app::state::net_nav::{IndexOutcome, ProjectNetData};
    let mut p = project();
    p.select(ROOT_VCC, SelectedKind::Label);
    p.send(NetNavMsg::PartnerJumpRequested);
    let planned = plan_projects(&p.app.document_state).remove(0);
    let (project_id, old_key) = (planned.project, planned.key);

    // An edit moves the key while that build runs.
    p.focus(&p.mid.clone());
    p.select(23, SelectedKind::Wire);
    delete_selected(&mut p);
    p.focus(&p.root.clone());
    p.select(ROOT_VCC, SelectedKind::Label);
    let _ = p.app.net_index_lookup(project_id);

    let stale = IndexOutcome {
        project: project_id,
        key: old_key,
        result: Ok(std::sync::Arc::new(ProjectNetData {
            index: signex_net::NetIndex::default(),
            paths: Default::default(),
            base_dir: None,
            fold: Default::default(),
            classes: Vec::new(),
        })),
        warnings: Vec::new(),
    };
    let follow_up = p.app.apply_net_index(&stale);
    assert!(follow_up.units() > 0, "the newest key's build starts now");
    assert_eq!(
        p.active(),
        Some(p.root.clone()),
        "not run on the stale answer"
    );

    p.app.settle_net_index_blocking();
    assert_eq!(p.active(), Some(p.mid.clone()), "the parked jump ran");
}
