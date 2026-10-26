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
