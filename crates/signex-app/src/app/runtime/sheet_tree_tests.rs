//! The Projects tree nests child sheets under their parents (#1618).

use std::path::{Path, PathBuf};

use signex_widgets::tree_view::TreeNode;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
use crate::app::hierarchy_fixture::hierarchical_app;
use crate::app::state::project_fs::probe;

/// Run the probe the way its Task would, then deliver the answer.
pub(crate) fn settle(app: &mut Signex, force: bool) {
    app.refresh_panel_ctx();
    let (generation, request) = app
        .document_state
        .project_fs
        .begin_probe(force)
        .expect("a probe should start");
    let landed = app.handle_project_fs_probed(generation, Ok(Box::new(probe(&request))));
    assert_eq!(landed.units(), 0, "the probe answer starts no follow-up");
}

/// `(label, nested labels)` for each Source Documents row.
fn shape(app: &Signex) -> Vec<(String, Vec<String>)> {
    fn labels(nodes: &[TreeNode]) -> Vec<String> {
        nodes.iter().map(|n| n.key.clone()).collect()
    }
    source_documents(app)
        .iter()
        .map(|n| (n.key.clone(), labels(&n.children)))
        .collect()
}

fn source_documents(app: &Signex) -> &[TreeNode] {
    &app.document_state.panel_ctx.project_tree[0].children[0].children
}

pub(crate) fn write_sheet(dir: &Path, name: &str, children: &[&str]) {
    let sheet = sheet_with_net("R1", "N", children);
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode");
    std::fs::write(dir.join(name), text).expect("write sheet");
}

#[test]
fn children_nest_under_their_parent_and_loose_pages_stay_on_top() {
    let mut app = hierarchical_app();
    settle(&mut app, false);

    let tree = source_documents(&app);
    let docs: Vec<&str> = tree.iter().map(|n| n.key.as_str()).collect();
    assert_eq!(docs, ["top.snxsch", "other.snxsch"], "mid and leaf nest");
    assert_eq!(tree[0].children[0].key, "mid.snxsch");
    assert_eq!(tree[0].children[0].children[0].key, "leaf.snxsch");
    assert!(
        tree[1].children.is_empty(),
        "the loose page has no children"
    );

    let listed: Vec<&str> = app.document_state.projects[0]
        .data
        .sheets
        .iter()
        .map(|s| s.filename.as_str())
        .collect();
    assert_eq!(
        listed,
        ["top.snxsch", "other.snxsch", "mid.snxsch", "leaf.snxsch"],
        "the `.snxprj` list is not narrowed"
    );
}

#[test]
fn closed_sheets_are_read_from_disk_and_open_ones_from_memory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &["mid.snxsch"]);
    write_sheet(tmp.path(), "mid.snxsch", &[]);
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch", "mid.snxsch"]);
    settle(&mut app, false);
    assert_eq!(
        shape(&app),
        [("top.snxsch".to_string(), vec!["mid.snxsch".to_string()])]
    );

    // An unsaved edit that removes the symbol un-nests the row, though the
    // file on disk still has it.
    open_with(
        &mut app.document_state,
        &tmp.path().join("top.snxsch"),
        sheet_with_net("R1", "N", &[]),
    );
    settle(&mut app, false);
    assert_eq!(
        shape(&app),
        [
            ("top.snxsch".to_string(), vec![]),
            ("mid.snxsch".to_string(), vec![])
        ]
    );
}

#[test]
fn a_poll_tick_does_not_walk_the_sheets_again() {
    let mut app = hierarchical_app();
    settle(&mut app, false);
    app.refresh_panel_ctx();
    let (_, request) = app
        .document_state
        .project_fs
        .begin_probe(true)
        .expect("the poll tick probes");
    assert!(request.hierarchy.jobs.is_empty(), "nothing to re-walk");
    assert!(!request.exists.is_empty(), "the stat checks still run");
}

#[test]
fn a_sheet_placed_twice_or_looping_back_appears_once() {
    let mut app = app_workspace("/w/tree-twice", &["top.snxsch"]);
    let path = |n: &str| PathBuf::from("/w/tree-twice").join(n);
    open_with(
        &mut app.document_state,
        &path("top.snxsch"),
        sheet_with_net("R_TOP", "N1", &["mid.snxsch", "mid.snxsch"]),
    );
    open_with(
        &mut app.document_state,
        &path("mid.snxsch"),
        // Loops back to the root.
        sheet_with_net("R_MID", "N2", &["top.snxsch"]),
    );
    settle(&mut app, false);

    assert_eq!(
        shape(&app),
        [("top.snxsch".to_string(), vec!["mid.snxsch".to_string()])]
    );
    assert!(source_documents(&app)[0].children[0].children.is_empty());
}

#[test]
fn a_symbol_without_a_file_is_a_missing_row_under_its_parent() {
    let mut app = app_workspace("/w/tree-missing", &["top.snxsch"]);
    open_with(
        &mut app.document_state,
        &PathBuf::from("/w/tree-missing").join("top.snxsch"),
        sheet_with_net("R_TOP", "N", &["sub/ghost.snxsch"]),
    );
    settle(&mut app, false);

    let top = &source_documents(&app)[0];
    assert_eq!(top.children[0].label, "sub/ghost.snxsch  (missing)");
    assert_eq!(top.children[0].key, "sub/ghost.snxsch");
}

#[test]
fn a_missing_row_follows_the_file_being_created_and_stops_offering_create() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &["later.snxsch"]);
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch"]);
    settle(&mut app, false);
    settle(&mut app, true);
    assert_eq!(
        source_documents(&app)[0].children[0].label,
        "later.snxsch  (missing)"
    );

    write_sheet(tmp.path(), "later.snxsch", &[]);
    settle(&mut app, true);

    let top = &source_documents(&app)[0];
    assert_eq!(top.children[0].label, "later.snxsch", "no longer missing");
    let info = &app.document_state.panel_ctx.projects[0].sheets[0].children[0];
    assert!(info.is_child_reference && !info.missing);
}

#[test]
fn an_unreadable_child_is_marked() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &["broken.snxsch"]);
    std::fs::write(tmp.path().join("broken.snxsch"), "not a sheet").expect("write");
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch"]);
    settle(&mut app, false);

    let child = &source_documents(&app)[0].children[0];
    assert_eq!(child.label, "broken.snxsch  (unreadable)");
}

#[test]
fn a_project_without_a_declared_root_nests_under_its_first_page() {
    let mut app = hierarchical_app();
    app.document_state.projects[0].data.schematic_root = None;
    settle(&mut app, false);

    assert_eq!(
        shape(&app),
        [
            ("top.snxsch".to_string(), vec!["mid.snxsch".to_string()]),
            ("other.snxsch".to_string(), vec![])
        ]
    );
}
