//! #1259 / #2630: the Object List's reads, its read generation and the lists
//! of undocked windows.

use signex_types::schematic::{InstancePath, Point, SchematicSheet, Symbol, SymbolInstance};

use super::*;
use crate::app::{TabInfo, TabKind};
use crate::dock::{DockMessage, PanelPosition};
use crate::panels::object_list::model::{ObjectRowsSource, RowName};

fn sheet(n: u128, reference: &str) -> SchematicSheet {
    let mut s = SchematicSheet::default();
    s.symbols.push(Symbol {
        uuid: Uuid::from_u128(n),
        reference: reference.into(),
        value: "10k".into(),
        position: Point::new(1.0, 1.0),
        ..Symbol::default()
    });
    s
}

/// #1259: Project lists every page of the project, the sheet in view
/// first; This sheet lists only the sheet in view.
#[test]
fn the_project_scope_lists_every_sheet_with_the_sheet_in_view_first() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (a, b) = (dir.path().join("a.snxsch"), dir.path().join("b.snxsch"));
    for (path, s) in [(&a, sheet(1, "R1")), (&b, sheet(2, "R2"))] {
        let text = signex_types::format::SnxSchematic::new(s)
            .write_string()
            .expect("encode");
        std::fs::write(path, text).expect("write");
    }
    let open: OpenSheets = [(b.clone(), sheet(2, "R2"))].into_iter().collect();
    let rows = read_project_rows(ProjectRead {
        open,
        pages: vec![a.clone(), b.clone()],
        root: a.clone(),
        fold: CaseFold::host_default(),
        active: Some(b.clone()),
        nets: None,
        looks: SheetLooks::default(),
    });
    let sheets: Vec<&str> = rows.iter().map(|r| r.sheet.as_str()).collect();
    assert_eq!(sheets, ["b", "a"], "the sheet in view first: {rows:?}");

    let look = SheetLook::default();
    let this_sheet = model::sheet_rows(&sheet(2, "R2"), &b, &look, &|_: Uuid| None);
    assert_eq!(this_sheet.len(), 1);
    assert_eq!(this_sheet[0].sheet, "b");
}

/// #2630: each sheet of a Project read draws by its own look; one the
/// looks do not name draws by the fallback.
#[test]
fn a_project_read_names_each_sheets_parts_by_its_look() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (a, b) = (dir.path().join("a.snxsch"), dir.path().join("b.snxsch"));
    let open: OpenSheets = [(a.clone(), sheet(1, "QAC1")), (b.clone(), sheet(2, "QAC2"))]
        .into_iter()
        .collect();
    let signed = SheetLook {
        standard: Some(signex_types::designator::DesignatorStandard::Iec81346_2Full),
        ..SheetLook::default()
    };
    let looks = SheetLooks {
        by_path: [(a.clone(), signed)].into_iter().collect(),
        fallback: SheetLook::default(),
    };
    let rows = read_project_rows(ProjectRead {
        open,
        pages: vec![a.clone(), b.clone()],
        root: a.clone(),
        fold: CaseFold::host_default(),
        active: Some(a.clone()),
        nets: None,
        looks,
    });
    let names: Vec<&RowName> = rows.iter().map(|r| &r.name).collect();
    assert_eq!(
        names,
        [
            &RowName::Text("-QAC1".into()),
            &RowName::Text("QAC2".into())
        ]
    );
}

fn with_engine(app: &mut Signex, path: &Path) {
    let engine = signex_engine::Engine::new(SchematicSheet::default()).expect("engine");
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
}

/// A read that fails keeps its stamp and is marked failed, so a read
/// that fails every time is not started again on every update; it is
/// read again once its source changes.
#[test]
fn a_failed_read_is_tried_again_only_when_its_source_changes() {
    let (mut app, _boot) = Signex::new();
    let a = PathBuf::from("a.snxsch");
    with_engine(&mut app, &a);
    let stamp = app.object_rows_stamp(&a, ObjectScope::ThisSheet);
    let source = &mut app.ui_state.object_list.source;
    source.stamp = Some(stamp);
    source.generation = 4;
    source.reading = true;

    let _ = app.handle_object_list_message(ObjectListMsg::ProjectRowsRead {
        window: None,
        generation: 4,
        rows: Err("the read panicked".to_string()),
    });
    let source = &app.ui_state.object_list.source;
    assert_eq!(source.stamp, Some(stamp), "the stamp is kept");
    assert!(source.failed && !source.reading);

    let _ = app.refresh_object_rows(ListSlot::Main, &a);
    assert_eq!(
        app.ui_state.object_list.source.generation, 4,
        "nothing changed: no read again"
    );

    let b = PathBuf::from("b.snxsch");
    with_engine(&mut app, &b);
    let _ = app.refresh_object_rows(ListSlot::Main, &b);
    let source = &app.ui_state.object_list.source;
    assert_eq!(source.generation, 5, "the source changed: read again");
    assert!(!source.failed);
}

/// #2630: closing the panel forgets the rows but not the read generation:
/// a Project read started before the close is dropped when it lands, and
/// the next read gets a number of its own.
#[test]
fn closing_the_panel_drops_a_read_still_on_its_way() {
    let (mut app, _boot) = Signex::new();
    let _ = app.document_state.dock.close_panel(PanelKind::ObjectList);
    assert!(!app.is_panel_open(PanelKind::ObjectList));
    let source = &mut app.ui_state.object_list.source;
    source.stamp = Some(1);
    source.generation = 4;
    source.reading = true;

    let _ = app.sync_object_list();
    let source = &app.ui_state.object_list.source;
    assert_eq!(source.stamp, None, "the rows are forgotten");
    assert_eq!(source.generation, 5, "the generation moves on");

    let late = model::sheet_rows(
        &sheet(1, "R1"),
        Path::new("a.snxsch"),
        &SheetLook::default(),
        &|_: Uuid| None,
    );
    let _ = app.handle_object_list_message(ObjectListMsg::ProjectRowsRead {
        window: None,
        generation: 4,
        rows: Ok(Arc::new(late)),
    });
    assert!(
        app.ui_state.object_list.source.rows.is_empty(),
        "the read from before the close is dropped"
    );
}

fn tab(path: &Path) -> TabInfo {
    TabInfo {
        title: path.file_stem().unwrap().to_string_lossy().into_owned(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    }
}

struct Undocked {
    app: Signex,
    window: iced::window::Id,
    main_path: PathBuf,
    window_path: PathBuf,
}

/// The main window shows `a.snxsch` (part R1), an undocked window shows
/// `b.snxsch` (part R2); the Object List is open.
fn undocked() -> Undocked {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let (main_path, window_path) = (PathBuf::from("a.snxsch"), PathBuf::from("b.snxsch"));
    for (path, s) in [(&main_path, sheet(1, "R1")), (&window_path, sheet(2, "R2"))] {
        let engine = signex_engine::Engine::new(s).expect("engine");
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(tab(path));
    }
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(main_path.clone());
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: window_path.clone(),
            title: "b".to_string(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    let ctx = app.new_window_panel_ctx(window);
    app.document_state.window_panel_ctx.insert(window, ctx);
    app.document_state
        .dock
        .add_panel(PanelPosition::Right, PanelKind::ObjectList);
    Undocked {
        app,
        window,
        main_path,
        window_path,
    }
}

fn names(view: &ObjectListView) -> Vec<String> {
    view.rows
        .iter()
        .filter_map(|r| match &r.name {
            RowName::Text(name) => Some(name.clone()),
            RowName::Noun(_) => None,
        })
        .collect()
}

fn window_view(f: &Undocked) -> &ObjectListView {
    &f.app.document_state.window_panel_ctx[&f.window].object_list
}

/// #2630: an undocked window's Object List lists the sheet that window
/// shows, beside the main window's list of its own sheet, and a row click
/// there selects on the window's canvas, not on the main window's.
#[test]
fn an_undocked_window_lists_and_selects_on_its_own_sheet() {
    let mut f = undocked();
    let _ = f.app.sync_object_list();
    assert_eq!(names(&f.app.document_state.panel_ctx.object_list), ["R1"]);
    assert_eq!(names(window_view(&f)), ["R2"]);

    let target = window_view(&f).rows[0].target.clone();
    assert_eq!(target.sheet, f.window_path);
    let click = DockMessage::Panel(PanelMsg::ObjectList(ObjectListMsg::RowClicked(
        target.clone(),
    )));
    let _ = f
        .app
        .handle_dock_message(DockMessage::ScopedToWindow(f.window, Box::new(click)));

    assert_eq!(
        f.app.interaction_state.canvases[&f.window].selected,
        vec![target.item]
    );
    assert!(f.app.interaction_state.canvas.selected.is_empty());
    assert_eq!(
        f.app.document_state.active_path.as_deref(),
        Some(f.main_path.as_path()),
        "the main window keeps its sheet"
    );
    assert_eq!(window_view(&f).selected_count, 1);
    assert_eq!(names(&f.app.document_state.panel_ctx.object_list), ["R1"]);
}

/// #2630: a Project read lands in the list it was started for; one for a
/// window that has closed since is dropped.
#[test]
fn a_project_read_lands_in_the_window_it_was_read_for() {
    let mut f = undocked();
    let state = &mut f.app.ui_state.object_list;
    state.windows.insert(
        f.window,
        ObjectRowsSource {
            generation: 3,
            reading: true,
            ..ObjectRowsSource::default()
        },
    );
    let rows = model::sheet_rows(
        &sheet(2, "R2"),
        &f.window_path,
        &SheetLook::default(),
        &|_: Uuid| None,
    );
    let _ = f
        .app
        .handle_object_list_message(ObjectListMsg::ProjectRowsRead {
            window: Some(f.window),
            generation: 3,
            rows: Ok(Arc::new(rows)),
        });
    let state = &f.app.ui_state.object_list;
    assert_eq!(state.windows[&f.window].rows.len(), 1);
    assert!(!state.windows[&f.window].reading);
    assert!(state.source.rows.is_empty(), "not the main window's rows");

    let gone = iced::window::Id::unique();
    let _ = f
        .app
        .handle_object_list_message(ObjectListMsg::ProjectRowsRead {
            window: Some(gone),
            generation: 0,
            rows: Ok(Arc::new(Vec::new())),
        });
    assert!(!f.app.ui_state.object_list.windows.contains_key(&gone));
}

/// #2629 (item 3): a refresh that runs inside an undocked window's swap
/// keeps that window's list, so after it neither list is read again and
/// neither shows the other window's sheet.
#[test]
fn a_refresh_inside_a_windows_swap_leaves_both_lists_settled() {
    let mut f = undocked();
    let window = f.window;
    let _ = f.app.sync_object_list();
    let generations = |app: &Signex| {
        let state = &app.ui_state.object_list;
        (state.source.generation, state.windows[&window].generation)
    };
    let before = generations(&f.app);

    let _ = f
        .app
        .run_in_window_document(window, |app| app.sync_object_list());
    let _ = f.app.sync_object_list();

    assert_eq!(generations(&f.app), before, "no list was read again");
    assert_eq!(names(&f.app.document_state.panel_ctx.object_list), ["R1"]);
    assert_eq!(names(window_view(&f)), ["R2"]);
}

/// #2630: a part of a child sheet placed twice is named by the placement
/// in view (R101 or R201), not by its stored `reference`, and the filter
/// finds that name.
#[test]
fn a_twice_placed_childs_part_reads_as_the_placement_in_view() {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let child = PathBuf::from("child.snxsch");
    let first = InstancePath::from_chain(vec![Uuid::from_u128(100)]);
    let second = InstancePath::from_chain(vec![Uuid::from_u128(200)]);
    let mut s = sheet(1, "R1");
    s.symbols[0].instances = [(&first, "R101"), (&second, "R201")]
        .into_iter()
        .map(|(placement, reference)| SymbolInstance {
            project: "p".into(),
            path: placement.to_string(),
            reference: reference.into(),
            unit: 1,
        })
        .collect();
    let engine = signex_engine::Engine::new(s).expect("engine");
    app.document_state.engines.insert(child.clone(), engine);
    app.document_state.tabs.push(tab(&child));
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(child.clone());
    app.document_state
        .dock
        .add_panel(PanelPosition::Right, PanelKind::ObjectList);
    let listed = |app: &Signex| names(&app.document_state.panel_ctx.object_list);

    app.document_state
        .tab_placements
        .record(&child, Some(second));
    app.ui_state.object_list.filter = "R201".into();
    let _ = app.sync_object_list();
    assert_eq!(listed(&app), ["R201"]);

    // Viewing the other placement reads the rows again.
    app.document_state
        .tab_placements
        .record(&child, Some(first));
    let _ = app.sync_object_list();
    assert!(listed(&app).is_empty(), "R201 is not the placement in view");
    app.ui_state.object_list.filter.clear();
    let _ = app.sync_object_list();
    assert_eq!(listed(&app), ["R101"]);
}

/// #2630: the rows a closed window kept are dropped with it.
#[test]
fn a_closed_windows_rows_are_dropped() {
    let mut f = undocked();
    let _ = f.app.sync_object_list();
    assert!(f.app.ui_state.object_list.windows.contains_key(&f.window));
    f.app.ui_state.windows.remove(&f.window);
    f.app.interaction_state.canvases.remove(&f.window);
    f.app.document_state.window_panel_ctx.remove(&f.window);
    let _ = f.app.sync_object_list();
    assert!(!f.app.ui_state.object_list.windows.contains_key(&f.window));
}
