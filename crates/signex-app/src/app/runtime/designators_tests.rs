//! #1151 — a child sheet opened through one of its placements shows, and
//! edits, that placement's designators on the canvas, as its export does.

use uuid::Uuid;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::current_placement::{
    PLACEMENT_A, PLACEMENT_B, current_page, descend_by_canvas, placed_twice_on_disk,
};
use crate::panels::PanelMsg;
use signex_types::schematic::{InstancePath, SelectedItem, SelectedKind};

fn child_symbol(app: &Signex) -> Uuid {
    app.active_render_snapshot().expect("child open").symbols[0].uuid
}

fn shown(app: &Signex) -> Option<String> {
    let id = child_symbol(app);
    app.interaction_state
        .active_canvas()
        .designators
        .get(&id)
        .cloned()
}

fn select_symbol(app: &mut Signex) {
    let id = child_symbol(app);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Symbol)];
    app.update_selection_info();
}

fn panel_designator(app: &Signex) -> String {
    app.document_state
        .panel_ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == "Reference")
        .map(|(_, v)| v.clone())
        .expect("a symbol is selected")
}

#[test]
fn the_canvas_and_the_panel_show_the_designators_of_the_placement_descended_through() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);

    descend_by_canvas(&mut app, PLACEMENT_B);
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(child.as_path())
    );
    assert_eq!(shown(&app).as_deref(), Some("R101"));
    select_symbol(&mut app);
    assert_eq!(panel_designator(&app), "R101");
    assert_eq!(
        current_page(&app, &child).0,
        "R101",
        "screen matches the page"
    );
}

#[test]
fn a_placement_whose_designator_is_the_base_one_draws_it_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, _child) = placed_twice_on_disk(&dir);

    descend_by_canvas(&mut app, PLACEMENT_A);
    assert_eq!(shown(&app), None, "R1 is the symbol's own reference");
}

#[test]
fn editing_the_designator_there_changes_that_placement_only_and_undoes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    let id = child_symbol(&app);

    let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::EditSymbolDesignator(id, "R205".into()),
    )));

    assert_eq!(shown(&app).as_deref(), Some("R205"));
    assert_eq!(current_page(&app, &child).0, "R205", "the export follows");
    let symbol = app.active_render_snapshot().unwrap().symbols[0].clone();
    assert_eq!(symbol.reference, "R1", "the base designator is untouched");
    let a = InstancePath::from_chain(vec![Uuid::from_u128(PLACEMENT_A)]);
    assert_eq!(symbol.designator_in(&a), "R1", "placement A keeps its own");

    let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));
    assert_eq!(shown(&app).as_deref(), Some("R101"));
}

#[test]
fn find_and_replace_matches_and_rewrites_the_designator_this_tab_shows() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);

    use crate::find_replace::FindReplaceMsg;
    let send = |app: &mut Signex, msg: FindReplaceMsg| {
        let _ = app.update(crate::app::Message::FindReplaceMsg(msg));
    };
    send(&mut app, FindReplaceMsg::QueryChanged("R101".into()));
    assert_eq!(app.ui_state.find_replace.matches.len(), 1);
    send(&mut app, FindReplaceMsg::ReplacementChanged("R300".into()));
    send(&mut app, FindReplaceMsg::ReplaceAll);

    assert_eq!(shown(&app).as_deref(), Some("R300"));
    assert_eq!(current_page(&app, &child).0, "R300");
}

// ---------------------------------------------------------------------------
// #1200 — a tab with no recorded placement, and a recorded one that died
// ---------------------------------------------------------------------------

fn write_sheet(path: &std::path::Path, sheet: signex_types::schematic::SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write sheet");
}

/// `top` places `child` once, as A; the child's symbol is R1 with an entry
/// giving placement A the designator R7. Only `top` is open.
fn placed_once_with_an_entry(
    dir: &tempfile::TempDir,
) -> (Signex, std::path::PathBuf, std::path::PathBuf) {
    use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
    let _ = crate::diagnostics::init_logging();
    let (top, child) = (
        dir.path().join("top.snxsch"),
        dir.path().join("child.snxsch"),
    );
    let mut top_sheet = sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]);
    top_sheet.child_sheets[0].name = "A".into();
    top_sheet.child_sheets[0].uuid = Uuid::from_u128(PLACEMENT_A);
    let mut child_sheet = sheet_with_net("R1", "CHILD_NET", &[]);
    child_sheet.labels[0].label_type = signex_types::schematic::LabelType::Net;
    child_sheet.symbols[0].instances = vec![signex_types::schematic::SymbolInstance {
        project: "p".into(),
        path: InstancePath::from_chain(vec![Uuid::from_u128(PLACEMENT_A)]).to_string(),
        reference: "R7".into(),
        unit: 1,
    }];
    write_sheet(&top, top_sheet);
    write_sheet(&child, child_sheet);
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    app.open_or_focus_sheet_path(top.clone());
    (app, top, child)
}

#[test]
fn a_child_opened_without_a_placement_shows_what_its_page_prints() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_once_with_an_entry(&dir);

    // Opened from the project, not by descending: no recorded placement.
    app.open_or_focus_sheet_path(child.clone());
    app.refresh_panel_ctx();
    app.settle_navigator();
    assert_eq!(app.document_state.tab_placements.get(&child), None);
    assert_eq!(shown(&app).as_deref(), Some("R7"));
    assert_eq!(current_page(&app, &child).0, "R7");
}

/// On a sheet placed once, a designator edit writes `reference` and drops
/// the one placement's entry, as one undo step: a simple design keeps a
/// simple file.
#[test]
fn a_designator_edit_on_a_singly_placed_child_writes_the_reference() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_once_with_an_entry(&dir);
    app.open_or_focus_sheet_path(child.clone());
    app.refresh_panel_ctx();
    app.settle_navigator();
    let id = child_symbol(&app);

    let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::EditSymbolDesignator(id, "R9".into()),
    )));
    let symbol = app.active_render_snapshot().unwrap().symbols[0].clone();
    assert_eq!(symbol.reference, "R9");
    assert!(
        symbol.instances.is_empty(),
        "the placement's entry is dropped"
    );
    assert_eq!(shown(&app), None, "R9 is the reference");
    assert_eq!(current_page(&app, &child).0, "R9");

    let _ = app.update(crate::app::Message::Edit(crate::app::EditMsg::Undo));
    let symbol = app.active_render_snapshot().unwrap().symbols[0].clone();
    assert_eq!(symbol.reference, "R1");
    assert_eq!(symbol.instances.len(), 1, "one undo step restores both");
    assert_eq!(shown(&app).as_deref(), Some("R7"));
}

/// A recorded placement whose sheet symbol was deleted is no longer shown
/// or edited; the sheet's remaining placement stands in, as in the export.
#[test]
fn a_recorded_placement_the_walk_no_longer_reaches_is_not_shown() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, top, child) = placed_twice_on_disk(&dir);
    descend_by_canvas(&mut app, PLACEMENT_B);
    assert_eq!(shown(&app).as_deref(), Some("R101"));

    // Delete sheet symbol B from `top` behind the child tab's back.
    let engine = app.document_state.engines.get_mut(&top).expect("top open");
    let mut document = engine.document().clone();
    document
        .child_sheets
        .retain(|cs| cs.uuid != Uuid::from_u128(PLACEMENT_B));
    engine
        .execute(signex_engine::Command::ReplaceDocument { document })
        .expect("replace");
    app.refresh_panel_ctx();
    app.settle_navigator();

    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(child.as_path())
    );
    assert_eq!(
        shown(&app),
        None,
        "placement A's designator is the reference R1"
    );

    // An edit now goes where a placed-once sheet's edit goes: `reference`.
    let id = child_symbol(&app);
    let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::EditSymbolDesignator(id, "R5".into()),
    )));
    let symbol = app.active_render_snapshot().unwrap().symbols[0].clone();
    let b = InstancePath::from_chain(vec![Uuid::from_u128(PLACEMENT_B)]);
    assert_eq!(symbol.reference, "R5");
    assert_eq!(
        symbol.designator_in(&b),
        "R101",
        "no entry written for dead B"
    );
}

/// Pages X and Y are listed in the project but no sheet symbol of the root
/// reaches them, and X places Y. Y's placements are then the root chain (it
/// is a loose page) and `/<symbol>`; the root chain has no instance entries,
/// so an edit shown at the root must still write `reference`, not be dropped.
#[test]
fn a_designator_edit_on_a_loose_page_that_another_loose_page_places_writes_the_reference() {
    use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (root, x, y) = (
        dir.path().join("root.snxsch"),
        dir.path().join("x.snxsch"),
        dir.path().join("y.snxsch"),
    );
    write_sheet(&root, sheet_with_net("R_ROOT", "ROOT_NET", &[]));
    write_sheet(&x, sheet_with_net("R_X", "X_NET", &["y.snxsch"]));
    write_sheet(&y, sheet_with_net("R1", "Y_NET", &[]));
    let mut app = app_workspace(
        &dir.path().to_string_lossy(),
        &["root.snxsch", "x.snxsch", "y.snxsch"],
    );
    app.open_or_focus_sheet_path(root);
    app.open_or_focus_sheet_path(y.clone());
    app.refresh_panel_ctx();
    app.settle_navigator();
    let placements = app
        .document_state
        .panel_ctx
        .navigator
        .hierarchy
        .as_ref()
        .expect("hierarchy")
        .placements_of(&y);
    assert!(placements.len() > 1, "root chain plus X's symbol");
    let id = child_symbol(&app);

    let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::EditSymbolDesignator(id, "R9".into()),
    )));
    let symbol = app.active_render_snapshot().unwrap().symbols[0].clone();
    assert_eq!(symbol.reference, "R9", "the edit is not dropped");
}

/// Review of #1207: `top` places `child` as A (entry R7) and `sub` as S;
/// `sub` places `child` as B (no entry, so B reads `reference` R1); and
/// `sub.snxsch` does not parse. The walk then sees only A, but it is
/// incomplete, so an edit through A must write A's entry, not `reference`,
/// or B would silently change too.
#[test]
fn an_incomplete_walk_never_edits_the_reference() {
    use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (top, child, sub) = (
        dir.path().join("top.snxsch"),
        dir.path().join("child.snxsch"),
        dir.path().join("sub.snxsch"),
    );
    let mut top_sheet = sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch", "sub.snxsch"]);
    top_sheet.child_sheets[0].uuid = Uuid::from_u128(PLACEMENT_A);
    top_sheet.child_sheets[1].uuid = Uuid::from_u128(3);
    let mut child_sheet = sheet_with_net("R1", "CHILD_NET", &[]);
    let a = InstancePath::from_chain(vec![Uuid::from_u128(PLACEMENT_A)]);
    child_sheet.symbols[0].instances = vec![signex_types::schematic::SymbolInstance {
        project: "p".into(),
        path: a.to_string(),
        reference: "R7".into(),
        unit: 1,
    }];
    write_sheet(&top, top_sheet);
    write_sheet(&child, child_sheet);
    std::fs::write(&sub, "this is not a schematic").unwrap();
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    app.open_or_focus_sheet_path(top);
    descend_by_canvas(&mut app, PLACEMENT_A);
    assert_eq!(shown(&app).as_deref(), Some("R7"));
    assert!(
        !app.document_state
            .panel_ctx
            .navigator
            .hierarchy
            .as_ref()
            .unwrap()
            .is_complete()
    );

    let id = child_symbol(&app);
    let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::EditSymbolDesignator(id, "R9".into()),
    )));
    let symbol = app.active_render_snapshot().unwrap().symbols[0].clone();
    assert_eq!(symbol.reference, "R1", "B, unseen, keeps reading R1");
    assert_eq!(symbol.designator_in(&a), "R9");
}

/// A placed-twice child opened without a recorded placement shows the
/// smallest placement (A, whose designator is the reference R1), not B.
#[test]
fn a_child_opened_directly_shows_its_smallest_placement() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_twice_on_disk(&dir);
    app.open_or_focus_sheet_path(child.clone());
    app.refresh_panel_ctx();
    app.settle_navigator();
    assert_eq!(app.document_state.tab_placements.get(&child), None);
    assert_eq!(shown(&app), None, "A's R1, not B's R101");
    assert_eq!(
        app.shown_placement(&child),
        Some(InstancePath::from_chain(vec![Uuid::from_u128(PLACEMENT_A)]))
    );
}

/// #1224 - a child placed once here but also placed by another open project
/// (under its own parent page, so a different placement) is shared: an edit
/// writes this placement's entry, never `reference`, which the other project
/// reads for its own placement.
#[test]
fn a_designator_edit_on_a_sheet_another_project_shares_writes_the_entry() {
    use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _top, child) = placed_once_with_an_entry(&dir);
    let mut top_b = sheet_with_net("R_TOP_B", "TOP_B_NET", &["child.snxsch"]);
    top_b.child_sheets[0].uuid = Uuid::from_u128(PLACEMENT_B);
    let top_b_path = dir.path().join("top_b.snxsch");
    write_sheet(&top_b_path, top_b);
    let mut other = app_workspace(&dir.path().to_string_lossy(), &["top_b.snxsch"])
        .document_state
        .projects
        .remove(0);
    other.id = app.document_state.mint_project_id();
    other.path = dir.path().join("ProjectB.snxprj");
    app.document_state.projects.push(other);
    app.open_or_focus_sheet_path(top_b_path);
    app.open_or_focus_sheet_path(child.clone());
    app.refresh_panel_ctx();
    app.settle_navigator();
    let id = child_symbol(&app);

    let _ = app.update(crate::app::Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::EditSymbolDesignator(id, "R9".into()),
    )));
    let symbol = app.active_render_snapshot().unwrap().symbols[0].clone();
    let b = InstancePath::from_chain(vec![Uuid::from_u128(PLACEMENT_B)]);
    assert_eq!(symbol.reference, "R1", "the base designator is untouched");
    assert_eq!(symbol.designator_in(&b), "R1", "the other project keeps R1");
    assert_eq!(shown(&app).as_deref(), Some("R9"));
}
