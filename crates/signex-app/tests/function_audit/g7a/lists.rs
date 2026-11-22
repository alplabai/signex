//! m57 Object List and m58 Net Inspector.

use super::*;

fn object_list(app: &mut Signex, msg: ObjectListMsg) {
    drive(app, panel(PanelMsg::ObjectList(msg)));
}

fn list_view(app: &Signex) -> &ObjectListView {
    &app.document_state.panel_ctx.object_list
}

fn listed_sample() -> (Signex, Fixture) {
    let (mut app, fx) = sample();
    open_panel(&mut app, PanelKind::ObjectList);
    assert_eq!(list_view(&app).status, ObjectListStatus::Ready);
    (app, fx)
}

fn inspector(app: &mut Signex, msg: NetInspectorMsg) {
    drive(app, panel(PanelMsg::NetInspector(msg)));
}

fn inspected_sample() -> (Signex, Fixture) {
    let (mut app, fx) = sample();
    open_panel(&mut app, PanelKind::NetInspector);
    assert_eq!(
        app.document_state.panel_ctx.net_inspector.status,
        NetListStatus::Ready
    );
    (app, fx)
}

#[test]
fn m57_the_list_holds_the_sheets_parts_wires_and_labels() {
    let (app, _fx) = listed_sample();
    let (_, wires, _) = doc_counts(&app);
    let view = list_view(&app);
    assert!(view.counts.of(ObjectKind::Part) > 0);
    assert_eq!(view.counts.of(ObjectKind::Wire), wires);
    let r1 = symbol_id(&app, "R1").expect("R1");
    assert!(view.rows.iter().any(|row| row.target.item.uuid == r1));
}

#[test]
fn m57_the_filter_narrows_to_the_matching_row_and_clearing_restores_the_list() {
    let (mut app, _fx) = listed_sample();
    let all = list_view(&app).rows.len();
    let r1 = symbol_id(&app, "R1").expect("R1");

    object_list(&mut app, ObjectListMsg::FilterChanged("470r".into()));
    let view = list_view(&app);
    assert_eq!(view.rows.len(), 1, "only R1 has the value 470R");
    assert_eq!(view.rows[0].target.item.uuid, r1);

    object_list(&mut app, ObjectListMsg::FilterChanged(String::new()));
    assert_eq!(list_view(&app).rows.len(), all);
}

#[test]
fn m57_a_filter_nothing_matches_lists_no_rows_and_selects_nothing() {
    let (mut app, _fx) = listed_sample();
    object_list(
        &mut app,
        ObjectListMsg::FilterChanged("nothing is called this".into()),
    );
    assert!(list_view(&app).rows.is_empty());
    assert!(app.interaction_state.active_canvas().selected.is_empty());
    assert!(is_clean(&app));
}

#[test]
fn m57_a_kind_chip_keeps_that_kind_and_all_clears_it() {
    let (mut app, _fx) = listed_sample();
    let all = list_view(&app).rows.len();
    object_list(
        &mut app,
        ObjectListMsg::KindChipToggled(KindChip::Kind(ObjectKind::Wire)),
    );
    let view = list_view(&app);
    assert!(!view.rows.is_empty());
    assert!(view.rows.iter().all(|row| row.kind == ObjectKind::Wire));

    object_list(&mut app, ObjectListMsg::KindChipToggled(KindChip::All));
    assert_eq!(list_view(&app).rows.len(), all);
}

#[test]
fn m57_more_shows_and_hides_the_other_kinds() {
    let (mut app, _fx) = listed_sample();
    assert!(!list_view(&app).more_open);
    object_list(&mut app, ObjectListMsg::MoreToggled);
    assert!(list_view(&app).more_open);
    object_list(&mut app, ObjectListMsg::MoreToggled);
    assert!(!list_view(&app).more_open);
}

#[test]
fn m57_a_row_click_selects_on_the_canvas_and_the_row_shows_it() {
    let (mut app, _fx) = listed_sample();
    let r1 = symbol_id(&app, "R1").expect("R1");
    let target = list_view(&app)
        .rows
        .iter()
        .find(|row| row.target.item.uuid == r1)
        .expect("R1's row")
        .target
        .clone();

    object_list(&mut app, ObjectListMsg::RowClicked(target.clone()));

    assert_eq!(
        app.interaction_state.active_canvas().selected,
        vec![target.item]
    );
    assert_eq!(list_view(&app).selected_count, 1);
    assert!(is_clean(&app), "the list only selects");
}

#[test]
fn m57_a_double_click_selects_the_object_too() {
    let (mut app, _fx) = listed_sample();
    let target = list_view(&app).rows[0].target.clone();
    object_list(&mut app, ObjectListMsg::RowDoubleClicked(target.clone()));
    assert_eq!(
        app.interaction_state.active_canvas().selected,
        vec![target.item]
    );
    assert!(is_clean(&app));
}

#[test]
fn m57_the_project_scope_lists_the_project_and_this_sheet_comes_back() {
    let (mut app, _fx) = listed_sample();
    let r1 = symbol_id(&app, "R1").expect("R1");
    object_list(&mut app, ObjectListMsg::ScopePicked(ObjectScope::Project));
    let view = list_view(&app);
    assert_eq!(view.scope, ObjectScope::Project);
    assert_eq!(view.status, ObjectListStatus::Ready);
    assert!(view.rows.iter().any(|row| row.target.item.uuid == r1));

    object_list(&mut app, ObjectListMsg::ScopePicked(ObjectScope::ThisSheet));
    assert_eq!(list_view(&app).scope, ObjectScope::ThisSheet);
}

#[test]
fn m57_a_late_project_read_from_an_older_request_is_dropped() {
    let (mut app, _fx) = listed_sample();
    let rows = list_view(&app).rows.len();
    assert!(rows > 0);
    object_list(
        &mut app,
        ObjectListMsg::ProjectRowsRead {
            generation: u64::MAX,
            rows: Ok(Arc::new(Vec::new())),
        },
    );
    assert_eq!(list_view(&app).rows.len(), rows);
}

#[test]
fn m58_a_net_expands_to_its_members_and_folds_again() {
    let (mut app, _fx) = inspected_sample();
    inspector(&mut app, NetInspectorMsg::NetToggled("+3V3".into()));
    let net = |app: &Signex| {
        app.document_state
            .panel_ctx
            .net_inspector
            .nets
            .iter()
            .find(|n| n.name == "+3V3")
            .expect("the +3V3 net")
            .clone()
    };
    let open = net(&app);
    assert!(open.expanded);
    assert!(!open.members.is_empty(), "an expanded net lists members");

    inspector(&mut app, NetInspectorMsg::NetToggled("+3V3".into()));
    let folded = net(&app);
    assert!(!folded.expanded);
    assert!(folded.members.is_empty());
    assert!(is_clean(&app));
}

#[test]
fn m58_the_filter_narrows_the_nets_and_clear_filter_restores_them() {
    let (mut app, _fx) = inspected_sample();
    let total = app.document_state.panel_ctx.net_inspector.total;
    assert!(total > 1);

    inspector(&mut app, NetInspectorMsg::FilterChanged("3v3".into()));
    let view = &app.document_state.panel_ctx.net_inspector;
    assert!(view.nets.iter().any(|n| n.name == "+3V3"));
    assert!(view.nets.len() < total, "the filter narrows the list");
    assert_eq!(view.total, total, "the footer still counts every net");

    inspector(&mut app, NetInspectorMsg::FilterCleared);
    let view = &app.document_state.panel_ctx.net_inspector;
    assert_eq!(view.filter, "");
    assert_eq!(view.nets.len(), total);
}

#[test]
fn m58_a_filter_nothing_matches_shows_no_nets_and_keeps_the_total() {
    let (mut app, _fx) = inspected_sample();
    let total = app.document_state.panel_ctx.net_inspector.total;
    inspector(&mut app, NetInspectorMsg::FilterChanged("VREF".into()));
    let view = &app.document_state.panel_ctx.net_inspector;
    assert!(view.nets.is_empty());
    assert_eq!(view.total, total);
    assert_eq!(view.status, NetListStatus::Ready);
}

#[test]
fn m58_by_component_lists_parts_and_an_expanded_part_accounts_for_every_pin() {
    let (mut app, _fx) = inspected_sample();
    inspector(
        &mut app,
        NetInspectorMsg::TabSelected(NetInspectorTab::ByComponent),
    );
    let view = &app.document_state.panel_ctx.net_inspector;
    assert_eq!(view.tab, NetInspectorTab::ByComponent);
    assert_eq!(view.components.len(), view.component_total);
    let key = view
        .components
        .iter()
        .find(|c| c.designator == "R1")
        .expect("R1 is listed")
        .key
        .clone();

    inspector(&mut app, NetInspectorMsg::ComponentToggled(key.clone()));

    let r1 = app
        .document_state
        .panel_ctx
        .net_inspector
        .components
        .iter()
        .find(|c| c.key == key)
        .expect("R1 is still listed");
    assert!(r1.expanded);
    assert!(r1.pins > 0);
    assert_eq!(r1.pin_rows.len() + r1.more, r1.pins);
}

#[test]
fn m58_clicking_a_member_highlights_its_net_and_edits_nothing() {
    let (mut app, _fx) = inspected_sample();
    let before = doc_counts(&app);
    inspector(&mut app, NetInspectorMsg::NetToggled("+3V3".into()));
    let anchor = app
        .document_state
        .panel_ctx
        .net_inspector
        .nets
        .iter()
        .find(|n| n.name == "+3V3")
        .and_then(|n| n.members.first())
        .expect("a member of +3V3")
        .anchor
        .clone();

    inspector(
        &mut app,
        NetInspectorMsg::MemberClicked {
            net: "+3V3".into(),
            anchor,
        },
    );

    let highlight = app.document_state.net_nav.highlight.as_ref();
    assert_eq!(highlight.map(|h| h.name.as_str()), Some("+3V3"));
    assert_eq!(doc_counts(&app), before);
    assert!(is_clean(&app));
}

#[test]
fn m58_with_no_project_it_says_so() {
    let mut app = fixture::fresh(ThemeId::DEFAULT);
    open_panel(&mut app, PanelKind::NetInspector);
    assert_eq!(
        app.document_state.panel_ctx.net_inspector.status,
        NetListStatus::NoProject
    );
}
