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

fn write_sheet(path: &std::path::Path, sheet: signex_types::schematic::SchematicSheet) {
    std::fs::write(
        path,
        signex_types::format::SnxSchematic::new(sheet)
            .write_string()
            .unwrap(),
    )
    .unwrap();
}

#[test]
fn a_net_only_on_the_second_page_of_a_flat_project_opens_that_page() {
    use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
    let dir = tempfile::tempdir().unwrap();
    let (page1, page2) = (
        dir.path().join("page1.snxsch"),
        dir.path().join("page2.snxsch"),
    );
    write_sheet(&page1, sheet_with_net("R1", "ONLY_ON_PAGE_ONE", &[]));
    write_sheet(&page2, sheet_with_net("R2", "ONLY_ON_PAGE_TWO", &[]));
    let mut app = app_workspace(
        &dir.path().to_string_lossy(),
        &["page1.snxsch", "page2.snxsch"],
    );
    assert!(app.open_or_focus_sheet_path(page1.clone()));

    search(&mut app, "ONLY_ON_PAGE_TWO");
    let net = app
        .ui_state
        .command_palette
        .results
        .iter()
        .position(|e| e.source == CommandSource::Net)
        .unwrap_or_else(|| panic!("a net row: {:?}", app.ui_state.command_palette.results));
    select_row(&mut app, net);

    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(page2.as_path())
    );
}

#[test]
fn the_index_of_another_project_is_not_shown() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _root, _child) = placed_twice_on_disk(&dir);
    search(&mut app, "R101");
    assert_eq!(rows(&app, "R101").len(), 1);

    // The active project now has another root: the cached index is of the
    // old one, so its rows are not offered.
    app.document_state.projects[0].data.schematic_root = Some("elsewhere.snxsch".into());
    app.refresh_command_palette_results();

    assert!(rows(&app, "R101").is_empty());
}

#[test]
fn no_root_clears_the_index() {
    let (mut app, _task) = Signex::new();
    app.ui_state.command_palette.index = Some(std::sync::Arc::new(Default::default()));

    let _ = app.start_palette_index();

    assert!(app.ui_state.command_palette.index.is_none());
}

#[test]
fn an_unchanged_stamp_takes_no_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _root, _child) = placed_twice_on_disk(&dir);
    search(&mut app, "R");
    let taken = app.ui_state.command_palette.index_snapshots_taken;
    assert!(taken >= 1);

    let _ = app.start_palette_index();
    send(&mut app, CommandPaletteMsg::QueryChanged("R1".into()));
    assert_eq!(app.ui_state.command_palette.index_snapshots_taken, taken);

    // A document change is a new stamp: now it snapshots.
    app.document_state.note_documents_changed();
    let _ = app.start_palette_index();
    assert_eq!(
        app.ui_state.command_palette.index_snapshots_taken,
        taken + 1
    );
}

#[test]
fn reveal_focuses_where_the_symbol_is_now_not_where_it_was_indexed() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, root, _child) = placed_twice_on_disk(&dir);
    search(&mut app, "R_TOP");
    let row = rows(&app, "R_TOP")[0];
    let symbol = app.document_state.engines[&root].document().symbols[0].uuid;
    app.document_state
        .engines
        .get_mut(&root)
        .unwrap()
        .execute(signex_engine::Command::MoveSymbolAbsolute {
            symbol_id: symbol,
            x: 123.0,
            y: 45.0,
        })
        .unwrap();

    select_row(&mut app, row);

    let fit = app
        .interaction_state
        .canvas
        .pending_fit
        .get()
        .expect("zoomed");
    assert!((fit.x + fit.width / 2.0 - 123.0).abs() < 0.01, "{fit:?}");
    assert!((fit.y + fit.height / 2.0 - 45.0).abs() < 0.01, "{fit:?}");
}

#[test]
fn a_sheet_row_lands_on_the_right_placement_of_a_child_placed_twice() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _root, child) = placed_twice_on_disk(&dir);
    search(&mut app, "child");
    let catalog = crate::app::command_palette::build_catalog(&app);
    let row = catalog
        .into_iter()
        .find(|e| e.source == CommandSource::Sheet && e.label == "B")
        .expect("a Sheet row for placement B");
    app.ui_state.command_palette.results = vec![row];
    app.ui_state.command_palette.total_matches = 1;

    select_row(&mut app, 0);

    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(child.as_path())
    );
    assert_eq!(
        app.document_state.tab_placements.get(&child),
        Some(&chain(PLACEMENT_B))
    );
}

#[test]
fn revealing_a_dirty_parked_sheet_reattaches_its_engine_not_the_disk() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _root, child) = placed_twice_on_disk(&dir);
    assert!(app.open_or_focus_sheet_path(child.clone()));
    let label = app.document_state.engines[&child].document().labels[0].uuid;
    assert!(app.apply_engine_command(
        signex_engine::Command::UpdateText {
            target: signex_engine::TextTarget::Label(label),
            value: "DIRTY_UNSAVED".into(),
        },
        true,
        true,
    ));
    let idx = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == child)
        .unwrap();
    let _ = app.close_tab_now(idx);
    assert!(
        app.document_state.engines.contains_key(&child),
        "parked, dirty"
    );
    assert!(!app.document_state.tabs.iter().any(|t| t.path == child));

    search(&mut app, "R101");
    let row = rows(&app, "R101")[0];
    select_row(&mut app, row);

    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(child.as_path())
    );
    assert_eq!(
        app.document_state.engines[&child].document().labels[0].text,
        "DIRTY_UNSAVED",
        "the unsaved edit survived: the engine was reattached"
    );
}

#[test]
fn an_older_index_is_dropped_and_a_failed_build_is_logged() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _root, _child) = placed_twice_on_disk(&dir);
    search(&mut app, "R101");
    let generation = app.ui_state.command_palette.index_generation;
    let stamp = app.ui_state.command_palette.index.as_ref().unwrap().stamp;

    let other = crate::app::palette_index::ProjectSearchIndex {
        stamp: stamp.wrapping_add(1),
        ..Default::default()
    };
    send(
        &mut app,
        CommandPaletteMsg::IndexBuilt {
            generation: generation - 1,
            result: std::sync::Arc::new(Ok(std::sync::Arc::new(other))),
        },
    );
    assert_eq!(
        app.ui_state.command_palette.index.as_ref().unwrap().stamp,
        stamp
    );

    send(
        &mut app,
        CommandPaletteMsg::IndexBuilt {
            generation,
            result: std::sync::Arc::new(Err("index worker boom".into())),
        },
    );
    assert!(logged("index worker boom"));
    assert_eq!(
        app.ui_state.command_palette.index.as_ref().unwrap().stamp,
        stamp
    );
}
