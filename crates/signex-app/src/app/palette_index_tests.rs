//! "Search everywhere" through the command palette (#1268).

use crate::app::command_palette::{CommandAction, CommandSource};
use crate::app::handlers::menu::export::tests::current_placement::{
    PLACEMENT_A, PLACEMENT_B, placed_twice_on_disk,
};
use crate::app::{CommandPaletteMsg, Message, Signex};
use signex_types::schematic::InstancePath;

fn chain(n: u128) -> InstancePath {
    InstancePath::from_chain(vec![uuid::Uuid::from_u128(n)])
}

fn send(app: &mut Signex, msg: CommandPaletteMsg) {
    let _ = app.update(Message::CommandPalette(msg));
}

/// The palette open on `query`, with the project index built.
fn search(app: &mut Signex, query: &str) {
    send(app, CommandPaletteMsg::QueryChanged(query.into()));
    app.build_palette_index_blocking();
}

fn rows(app: &Signex, label: &str) -> Vec<usize> {
    app.ui_state
        .command_palette
        .results
        .iter()
        .enumerate()
        .filter(|(_, e)| e.label == label)
        .map(|(i, _)| i)
        .collect()
}

fn select_row(app: &mut Signex, index: usize) {
    send(app, CommandPaletteMsg::Select(index));
}

fn logged(fragment: &str) -> bool {
    crate::diagnostics::recent_entries()
        .iter()
        .any(|entry| entry.message.contains(fragment))
}

#[test]
fn palette_designator_on_another_sheet_opens_and_focuses() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root, child) = placed_twice_on_disk(&dir);
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(root.as_path())
    );
    assert!(!app.document_state.engines.contains_key(&child));

    search(&mut app, "R101");
    let hit = rows(&app, "R101");
    assert_eq!(hit.len(), 1, "{:?}", app.ui_state.command_palette.results);
    select_row(&mut app, hit[0]);

    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(child.as_path())
    );
    assert!(
        app.interaction_state.canvas.pending_fit.get().is_some(),
        "zoomed"
    );
    assert_eq!(app.interaction_state.active_canvas().selected.len(), 1);
}

#[test]
fn palette_lists_a_hierarchical_child_missing_from_the_page_list() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _root, child) = placed_twice_on_disk(&dir);
    assert!(
        !crate::app::project_sheets::declared_pages(&app.document_state).contains(&child),
        "the child is not a declared page"
    );

    search(&mut app, "child");

    let sheets: Vec<_> = app
        .ui_state
        .command_palette
        .results
        .iter()
        .filter(|e| e.source == CommandSource::Sheet)
        .collect();
    assert!(
        !sheets.is_empty(),
        "{:?}",
        app.ui_state.command_palette.results
    );
    assert!(sheets.iter().all(|e| matches!(
        &e.action,
        CommandAction::Reveal { path, .. } if path == &child
    )));
}

#[test]
fn a_net_row_jumps_to_a_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root, child) = placed_twice_on_disk(&dir);
    app.refresh_project_netlist();
    search(&mut app, "CHILD_NET");

    let net = app
        .ui_state
        .command_palette
        .results
        .iter()
        .position(|e| e.source == CommandSource::Net)
        .unwrap_or_else(|| panic!("a net row: {:?}", app.ui_state.command_palette.results));
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(root.as_path())
    );
    select_row(&mut app, net);

    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(child.as_path())
    );
    assert!(app.interaction_state.canvas.pending_fit.get().is_some());
}

#[test]
fn a_stale_row_after_a_sheet_is_renamed_opens_nothing_and_does_not_crash() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root, child) = placed_twice_on_disk(&dir);
    search(&mut app, "R101");
    let hit = rows(&app, "R101")[0];
    let tabs_before = app.document_state.tabs.len();
    std::fs::rename(&child, dir.path().join("renamed.snxsch")).unwrap();

    select_row(&mut app, hit);

    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(root.as_path())
    );
    assert_eq!(
        app.document_state.tabs.len(),
        tabs_before,
        "no other file opened"
    );
    assert!(logged("Search: could not open"));
}

#[test]
fn a_row_whose_symbol_is_gone_opens_the_sheet_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _root, child) = placed_twice_on_disk(&dir);
    search(&mut app, "R101");
    let hit = rows(&app, "R101")[0];
    let mut sheet =
        signex_types::format::SnxSchematic::parse(&std::fs::read_to_string(&child).unwrap())
            .unwrap()
            .sheet;
    sheet.symbols.clear();
    std::fs::write(
        &child,
        signex_types::format::SnxSchematic::new(sheet)
            .write_string()
            .unwrap(),
    )
    .unwrap();

    select_row(&mut app, hit);

    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(child.as_path())
    );
    assert!(logged("Search: that item is no longer on"));
}

#[test]
fn rows_for_a_twice_placed_sheet_focus_the_right_placement() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _root, child) = placed_twice_on_disk(&dir);

    search(&mut app, "R101");
    let row = rows(&app, "R101")[0];
    select_row(&mut app, row);
    assert_eq!(
        app.document_state.tab_placements.get(&child),
        Some(&chain(PLACEMENT_B))
    );

    search(&mut app, "R1");
    let first = app
        .ui_state
        .command_palette
        .results
        .iter()
        .position(|e| {
            e.label == "R1"
                && matches!(&e.action, CommandAction::Reveal { placement: Some(p), .. } if *p == chain(PLACEMENT_A))
        })
        .expect("the placement A row");
    select_row(&mut app, first);
    assert_eq!(
        app.document_state.tab_placements.get(&child),
        Some(&chain(PLACEMENT_A))
    );
}
