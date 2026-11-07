//! #1220: Ctrl+Z after a net-class pick reverts the pick, in order with the
//! sheet's own edits.

use signex_types::net_class::NetClass;
use signex_types::schematic::{SelectedItem, SelectedKind, Symbol};

use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::{Signex, TabInfo, TabKind};
use crate::panels::NetClassMsg;

fn app_with_sheet() -> (Signex, uuid::Uuid) {
    let dir = std::env::temp_dir().join(format!("signex-pick-undo-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    app.document_state.projects[0].data.net_classes = vec![NetClass::named("Power")];
    let path = dir.join("a_root.snxsch");
    let symbol = uuid::Uuid::now_v7();
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: symbol,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    open_with(&mut app.document_state, &path, sheet);
    app.document_state.tabs.push(TabInfo {
        title: "a_root.snxsch".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path);
    (app, symbol)
}

fn move_symbol(app: &mut Signex, symbol: uuid::Uuid) {
    let moved = app.apply_engine_command(
        signex_engine::Command::MoveSelection {
            items: vec![SelectedItem::new(symbol, SelectedKind::Symbol)],
            dx: 10.0,
            dy: 0.0,
            keep_wires: false,
        },
        true,
        true,
    );
    assert!(moved, "fixture: the move must change the sheet");
}

fn symbol_x(app: &Signex) -> f64 {
    app.document_state
        .active_engine()
        .unwrap()
        .document()
        .symbols[0]
        .position
        .x
}

fn pick(app: &mut Signex, net: &str, class: Option<&str>) {
    let project = app.document_state.projects[0].id;
    app.handle_net_class_message(&NetClassMsg::AssignNet {
        project,
        net: net.to_string(),
        class: class.map(str::to_string),
    });
}

fn nets(app: &Signex) -> Vec<String> {
    app.document_state.projects[0].data.net_classes[0]
        .nets
        .clone()
}

#[test]
fn undo_reverts_the_pick_and_leaves_the_sheet_alone() {
    let (mut app, symbol) = app_with_sheet();
    move_symbol(&mut app, symbol);
    let moved_x = symbol_x(&app);
    pick(&mut app, "GND", Some("Power"));
    assert_eq!(nets(&app), ["GND"]);

    app.handle_undo_requested();
    assert!(nets(&app).is_empty(), "the pick is reverted");
    assert_eq!(symbol_x(&app), moved_x, "the earlier sheet edit stays");
    assert!(
        !app.document_state
            .is_document_dirty(&app.document_state.projects[0].path),
        "undoing the only project edit lands back on the file's content (#1854)"
    );
    app.handle_redo_requested();
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.projects[0].path)
    );
}

#[test]
fn edit_pick_undo_twice_then_redo_twice_keeps_order() {
    let (mut app, symbol) = app_with_sheet();
    let start = symbol_x(&app);
    move_symbol(&mut app, symbol);
    let moved_x = symbol_x(&app);
    pick(&mut app, "GND", Some("Power"));

    app.handle_undo_requested();
    assert!(nets(&app).is_empty());
    assert_eq!(symbol_x(&app), moved_x);
    app.handle_undo_requested();
    assert_eq!(symbol_x(&app), start, "second undo reverts the sheet edit");

    app.handle_redo_requested();
    assert_eq!(symbol_x(&app), moved_x);
    assert!(nets(&app).is_empty(), "the sheet edit comes back first");
    app.handle_redo_requested();
    assert_eq!(nets(&app), ["GND"], "then the pick");
}

#[test]
fn each_pick_is_one_step_and_a_sheet_edit_after_a_pick_undoes_first() {
    let (mut app, symbol) = app_with_sheet();
    pick(&mut app, "GND", Some("Power"));
    pick(&mut app, "VCC", Some("Power"));
    move_symbol(&mut app, symbol);
    let start = symbol_x(&app) - 10.0;

    app.handle_undo_requested();
    assert_eq!(symbol_x(&app), start);
    assert_eq!(nets(&app), ["GND", "VCC"]);
    app.handle_undo_requested();
    assert_eq!(nets(&app), ["GND"]);
    app.handle_undo_requested();
    assert!(nets(&app).is_empty());
}

#[test]
fn a_sheet_edit_ends_the_pick_redo_branch() {
    let (mut app, symbol) = app_with_sheet();
    pick(&mut app, "GND", Some("Power"));
    app.handle_undo_requested();
    move_symbol(&mut app, symbol);
    app.handle_redo_requested();
    assert!(nets(&app).is_empty(), "the undone pick is not replayed");
}

fn panel(app: &mut Signex, msg: NetClassMsg) {
    app.handle_net_class_message(&msg);
}

fn class_count(app: &Signex) -> usize {
    app.document_state.projects[0].data.net_classes.len()
}

/// Open a second sheet next to the first and make it the active one.
fn open_second_sheet(app: &mut Signex) -> std::path::PathBuf {
    let path = app.document_state.projects[0]
        .path
        .with_file_name("b_sheet.snxsch");
    open_with(
        &mut app.document_state,
        &path,
        signex_types::schematic::SchematicSheet {
            uuid: uuid::Uuid::now_v7(),
            paper_size: "A4".to_string(),
            ..Default::default()
        },
    );
    app.document_state.tabs.push(TabInfo {
        title: "b_sheet.snxsch".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    path
}

fn activate(app: &mut Signex, path: &std::path::Path) {
    app.document_state.active_tab = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab");
    app.document_state.active_path = Some(path.to_path_buf());
}

/// Pick, panel Undo, then a different class edit lands at the same history
/// depth: Ctrl+Z must not revert that other edit.
#[test]
fn a_class_edit_at_the_same_depth_is_not_mistaken_for_the_pick() {
    let (mut app, _symbol) = app_with_sheet();
    pick(&mut app, "GND", Some("Power"));
    panel(&mut app, NetClassMsg::Undo);
    panel(&mut app, NetClassMsg::Add);
    assert_eq!(class_count(&app), 2);

    app.handle_undo_requested();
    assert_eq!(class_count(&app), 2, "the new class stays");
    app.handle_redo_requested();
    assert_eq!(class_count(&app), 2, "redo does not touch it either");
}

/// The same sequence across two sheets: a pick on B takes the depth the
/// undone pick on A had, and Ctrl+Z on A must not revert B's pick.
#[test]
fn another_sheets_pick_at_the_same_depth_is_not_reverted_from_this_sheet() {
    let (mut app, _symbol) = app_with_sheet();
    let a = app.document_state.active_path.clone().unwrap();
    pick(&mut app, "GND", Some("Power"));
    panel(&mut app, NetClassMsg::Undo);
    open_second_sheet(&mut app);
    pick(&mut app, "VCC", Some("Power"));
    activate(&mut app, &a);

    app.handle_undo_requested();
    assert_eq!(nets(&app), ["VCC"], "B's pick is not undone from A");
}

/// At the history cap the depth stops growing; the pick must still be
/// recorded and undoable, and an older pick must not match later states.
#[test]
fn a_pick_made_at_the_history_cap_still_undoes() {
    let (mut app, _symbol) = app_with_sheet();
    for _ in 0..=signex_engine::NET_CLASS_HISTORY_LIMIT {
        panel(&mut app, NetClassMsg::Add);
    }
    pick(&mut app, "GND", Some("Power"));
    assert_eq!(nets(&app), ["GND"]);

    app.handle_undo_requested();
    assert!(nets(&app).is_empty(), "the pick is reverted");
    app.handle_redo_requested();
    assert_eq!(nets(&app), ["GND"]);
}

/// A stale pick is dropped, and a still-valid older pick under it is used
/// instead of being skipped.
#[test]
fn a_stale_pick_is_dropped_and_the_older_valid_one_undone() {
    let (mut app, _symbol) = app_with_sheet();
    pick(&mut app, "GND", Some("Power"));
    pick(&mut app, "VCC", Some("Power"));
    panel(&mut app, NetClassMsg::Undo);
    assert_eq!(nets(&app), ["GND"]);

    app.handle_undo_requested();
    assert!(nets(&app).is_empty(), "the GND pick is undone");
}

#[test]
fn closing_a_sheet_forgets_its_picks() {
    let (mut app, _symbol) = app_with_sheet();
    let path = app.document_state.active_path.clone().unwrap();
    pick(&mut app, "GND", Some("Power"));
    assert!(app.sheet_has_step(&path, true));
    app.document_state.remove_engine(&path);
    assert!(!app.sheet_has_step(&path, true), "the markers left with it");
}

fn sheet_dirty(app: &Signex) -> bool {
    let path = app.document_state.active_path.clone().unwrap();
    app.document_state.is_document_dirty(&path)
}

fn project_dirty(app: &Signex) -> bool {
    app.document_state
        .is_document_dirty(&app.document_state.projects[0].path)
}

/// A pick changes the project, never the sheet: the sheet stays clean, the
/// project reads dirty, and undoing the pick reads the project clean again.
#[test]
fn a_pick_leaves_the_sheet_clean_and_dirties_only_the_project() {
    let (mut app, _symbol) = app_with_sheet();
    let path = app.document_state.active_path.clone().unwrap();
    app.document_state.mark_document_clean(&path);
    app.document_state
        .engines
        .get_mut(&path)
        .unwrap()
        .mark_saved();
    assert!(!sheet_dirty(&app) && !project_dirty(&app));

    pick(&mut app, "GND", Some("Power"));
    assert!(!sheet_dirty(&app), "the sheet file did not change");
    assert!(project_dirty(&app));

    app.handle_undo_requested();
    assert!(
        !sheet_dirty(&app) && !project_dirty(&app),
        "back at both saves"
    );
    app.handle_redo_requested();
    assert!(!sheet_dirty(&app) && project_dirty(&app));
}

/// A sheet edit on top of a pick is dirty; undoing it leaves only the
/// marker above the save point, which reads clean.
#[test]
fn a_marker_above_the_sheets_save_point_does_not_make_it_dirty() {
    let (mut app, symbol) = app_with_sheet();
    let path = app.document_state.active_path.clone().unwrap();
    app.document_state
        .engines
        .get_mut(&path)
        .unwrap()
        .mark_saved();
    app.document_state.mark_document_clean(&path);

    pick(&mut app, "GND", Some("Power"));
    move_symbol(&mut app, symbol);
    assert!(sheet_dirty(&app));
    app.handle_undo_requested();
    assert!(!sheet_dirty(&app), "only the pick lies above the save");
    app.handle_redo_requested();
    assert!(sheet_dirty(&app));
}

/// Picks share the sheet's undo depth: the oldest step falls off first, a
/// pick included.
#[test]
fn picks_share_the_undo_depth_and_fall_off_oldest_first() {
    let (mut app, symbol) = app_with_sheet();
    app.set_undo_depth(2);
    pick(&mut app, "GND", Some("Power"));
    move_symbol(&mut app, symbol);
    move_symbol(&mut app, symbol);

    app.handle_undo_requested();
    app.handle_undo_requested();
    app.handle_undo_requested(); // nothing left on the sheet's timeline

    assert_eq!(
        nets(&app),
        ["GND"],
        "the pick step fell off the two-step history"
    );
}

/// A pick made on one sheet is ordered on that sheet only: another sheet of
/// the project keeps its own timeline.
#[test]
fn a_pick_is_undone_from_the_sheet_it_was_made_on() {
    let (mut app, _symbol) = app_with_sheet();
    let a = app.document_state.active_path.clone().unwrap();
    open_second_sheet(&mut app);
    pick(&mut app, "GND", Some("Power"));
    activate(&mut app, &a);
    app.handle_undo_requested();
    assert_eq!(nets(&app), ["GND"], "sheet A has no pick to undo");
    let b = app.document_state.projects[0]
        .path
        .with_file_name("b_sheet.snxsch");
    activate(&mut app, &b);
    app.handle_undo_requested();
    assert!(nets(&app).is_empty());
}

fn paint(app: &mut Signex, id: uuid::Uuid) {
    use crate::app::handlers::editing_commands::net_color_driver as colours;
    colours::paint(app, &[id], colours::color(1));
}

fn painted(app: &Signex, id: uuid::Uuid) -> bool {
    app.document_state
        .active_engine()
        .unwrap()
        .document()
        .net_colors
        .contains_key(&id)
}

/// A colour, then a pick: Ctrl+Z reverts the newer pick first, the colour
/// second; Ctrl+Y replays them in the opposite order.
#[test]
fn a_colour_then_a_pick_undo_in_true_time_order() {
    let (mut app, _symbol) = app_with_sheet();
    let net = uuid::Uuid::now_v7();
    paint(&mut app, net);
    pick(&mut app, "GND", Some("Power"));

    app.handle_undo_requested();
    assert!(nets(&app).is_empty(), "the newer pick goes first");
    assert!(painted(&app, net));
    app.handle_undo_requested();
    assert!(!painted(&app, net), "then the colour");

    app.handle_redo_requested();
    assert!(painted(&app, net) && nets(&app).is_empty(), "colour first");
    app.handle_redo_requested();
    assert_eq!(nets(&app), ["GND"], "then the pick");
}

/// A pick, then a colour: the newer colour goes first.
#[test]
fn a_pick_then_a_colour_undo_in_true_time_order() {
    let (mut app, _symbol) = app_with_sheet();
    let net = uuid::Uuid::now_v7();
    pick(&mut app, "GND", Some("Power"));
    paint(&mut app, net);

    app.handle_undo_requested();
    assert!(!painted(&app, net), "the newer colour goes first");
    assert_eq!(nets(&app), ["GND"]);
    app.handle_undo_requested();
    assert!(nets(&app).is_empty(), "then the pick");
}

/// Undo is offered only for a step it would move: a marker whose class
/// edit the panel already undid is dead, and a sheet holding only dead
/// markers has nothing to undo.
#[test]
fn a_dead_marker_is_not_a_step_the_menu_offers() {
    let (mut app, symbol) = app_with_sheet();
    let path = app.document_state.active_path.clone().unwrap();
    pick(&mut app, "GND", Some("Power"));
    assert!(app.sheet_has_step(&path, true));

    panel(&mut app, NetClassMsg::Undo);
    assert!(
        !app.sheet_has_step(&path, true),
        "the pick is already undone"
    );
    assert!(!app.undo_active_sheet(), "and Ctrl+Z moves nothing");

    move_symbol(&mut app, symbol);
    assert!(
        app.sheet_has_step(&path, true),
        "an edit above it is a step"
    );
}

/// A restored project copy keeps step ids moving forward: a pick made before
/// the restore must not match a new step, so Ctrl+Z cannot revert an
/// unrelated panel edit.
#[test]
fn a_pick_made_before_a_project_restore_does_not_match_a_later_step() {
    let (mut app, _symbol) = app_with_sheet();
    pick(&mut app, "GND", Some("Power"));
    let path = app.document_state.projects[0].path.clone();
    let data = app.document_state.projects[0].data.clone();
    assert!(app.restore_project_copy(&path, data));

    panel(&mut app, NetClassMsg::Add);
    assert_eq!(class_count(&app), 2);
    app.handle_undo_requested();
    assert_eq!(class_count(&app), 2, "the panel edit is not the pick");
}

/// A LIVE pick on a member sheet is a later step there and blocks a
/// project-wide undo (decision 5); once the class edit is undone the marker
/// is dead, is passed, and the project-wide step goes ahead.
#[test]
fn a_live_pick_on_a_member_sheet_blocks_the_project_wide_undo_until_it_is_dead() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _symbol) = app_with_sheet();
    let a = app.document_state.active_path.clone().unwrap();
    let b = open_second_sheet(&mut app);
    let group = app.mint_undo_group();
    for path in [&a, &b] {
        let wire = signex_types::schematic::Wire {
            uuid: uuid::Uuid::now_v7(),
            ..Default::default()
        };
        let engine = app.document_state.engines.get_mut(path).unwrap();
        engine
            .execute_batch_grouped(
                [signex_engine::Command::PlaceWireSegment { wire }],
                Some(group),
            )
            .unwrap();
    }
    app.set_undo_group_sheets(
        group,
        crate::app::UndoGroupSheets {
            live: vec![a.clone(), b.clone()],
            ..Default::default()
        },
    );
    pick(&mut app, "GND", Some("Power")); // B is active: the marker lands on B
    activate(&mut app, &a);
    let wires = |app: &Signex, path| app.document_state.engines[path].document().wires.len();

    assert!(!app.undo_active_sheet(), "blocked by B's live pick");
    assert_eq!((wires(&app, &a), wires(&app, &b)), (1, 1));
    assert_eq!(nets(&app), ["GND"]);

    panel(&mut app, NetClassMsg::Undo); // the pick's class edit is undone: dead
    assert!(app.undo_active_sheet());
    assert_eq!(
        (wires(&app, &a), wires(&app, &b)),
        (0, 0),
        "the group moved"
    );
}
