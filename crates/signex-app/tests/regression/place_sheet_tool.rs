//! #358 — the Place Sheet tool, driven through the real message path
//! (`SelectTool`, `CanvasEvent::Clicked`, Esc, Undo).

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{
    EditMsg, LoadedProject, Message, MoveSelectionMsg, Signex, TabInfo, TabKind, Tool,
};
use signex_app::canvas::CanvasEvent;
use signex_app::toolbar::ToolMessage;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{SchematicSheet, SelectedKind};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

pub(crate) fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        version: 0,
        generator: String::new(),
        generator_version: String::new(),
        paper_size: "A4".to_string(),
        root_sheet_page: "1".to_string(),
        symbols: Vec::new(),
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: HashMap::new(),
        lib_symbols: HashMap::new(),
        net_colors: HashMap::new(),
        unknown: Default::default(),
        font_family: None,
    }
}

/// An app with one active loose schematic and the Place Sheet tool armed.
pub(crate) fn app_with_sheet_tool(path: PathBuf) -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = false;
    let engine = signex_engine::Engine::new(empty_sheet()).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "root".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Sheet)));
    app
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

pub(crate) fn drag(app: &mut Signex, from: (f64, f64), to: (f64, f64)) {
    click(app, from.0, from.1);
    click(app, to.0, to.1);
}

pub(crate) fn sheets(app: &Signex) -> Vec<signex_types::schematic::ChildSheet> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .child_sheets
        .clone()
}

#[test]
fn drag_places_a_sheet_with_the_dragged_rect_and_generated_names() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("root.snxsch");
    let mut app = app_with_sheet_tool(path.clone());

    // Opposite corner first: the rect must still be normalised.
    drag(&mut app, (60.0, 50.0), (20.0, 10.0));

    let placed = sheets(&app);
    assert_eq!(placed.len(), 1);
    let s = &placed[0];
    assert_eq!((s.position.x, s.position.y), (20.0, 10.0));
    assert_eq!(s.size, (40.0, 40.0));
    assert_eq!(s.name, "sheet1");
    assert_eq!(s.filename, "sheet1.snxsch");
    assert!(s.pins.is_empty());
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(
        !tmp.path().join("sheet1.snxsch").exists(),
        "placing must not create the child file"
    );
}

#[test]
fn tiny_or_zero_area_drag_is_rejected_and_the_tool_stays_armed() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("root.snxsch");
    let mut app = app_with_sheet_tool(path.clone());

    // First click arms the tool; the tiny second click is refused and
    // disarms the gesture without leaving the tool.
    click(&mut app, 10.0, 10.0);
    assert!(
        app.interaction_state.shape_anchor.is_some(),
        "the first click must arm the tool"
    );
    click(&mut app, 10.0, 10.0);
    assert!(app.interaction_state.shape_anchor.is_none());
    drag(&mut app, (10.0, 10.0), (11.0, 40.0));

    assert!(sheets(&app).is_empty());
    assert!(!app.document_state.dirty_paths.contains(&path));
    assert_eq!(app.interaction_state.current_tool, Tool::Sheet);
    assert!(app.interaction_state.shape_anchor.is_none());

    // Still armed: a valid drag now places exactly one sheet.
    drag(&mut app, (10.0, 10.0), (40.0, 40.0));
    assert_eq!(sheets(&app).len(), 1);
    assert!(app.document_state.dirty_paths.contains(&path));
}

#[test]
fn escape_mid_drag_cancels_and_returns_to_select() {
    let tmp = TempDir::new().expect("tempdir");
    let mut app = app_with_sheet_tool(tmp.path().join("root.snxsch"));

    click(&mut app, 10.0, 10.0);
    assert!(app.interaction_state.shape_anchor.is_some());
    let _ = app.update(Message::EscapePressed { window: None });

    assert_eq!(app.interaction_state.current_tool, Tool::Select);
    assert!(app.interaction_state.shape_anchor.is_none());
    click(&mut app, 50.0, 50.0);
    assert!(sheets(&app).is_empty(), "no partial sheet after Esc");
}

#[test]
fn cancel_drawing_mid_drag_also_cancels() {
    let tmp = TempDir::new().expect("tempdir");
    let mut app = app_with_sheet_tool(tmp.path().join("root.snxsch"));

    click(&mut app, 10.0, 10.0);
    let _ = app.update(Message::Tool(ToolMessage::CancelDrawing));

    assert_eq!(app.interaction_state.current_tool, Tool::Select);
    assert!(app.interaction_state.shape_anchor.is_none());
}

#[test]
fn undo_removes_the_placed_sheet() {
    let tmp = TempDir::new().expect("tempdir");
    let mut app = app_with_sheet_tool(tmp.path().join("root.snxsch"));

    drag(&mut app, (10.0, 10.0), (40.0, 40.0));
    assert_eq!(sheets(&app).len(), 1);
    let _ = app.update(Message::Edit(EditMsg::Undo));

    assert!(sheets(&app).is_empty());
}

#[test]
fn defaults_are_unique_in_the_parent_and_the_project() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("root.snxsch");
    let mut app = app_with_sheet_tool(path.clone());

    // The project already lists sheet1.snxsch, so the first default is sheet2.
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: tmp.path().join("P.snxprj"),
        data: ProjectData {
            name: "P".to_string(),
            dir: tmp.path().to_string_lossy().into_owned(),
            schematic_root: Some("root.snxsch".to_string()),
            pcb_file: None,
            sheets: ["root.snxsch", "sheet1.snxsch"]
                .iter()
                .map(|f| SheetEntry {
                    name: (*f).to_string(),
                    filename: (*f).to_string(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                })
                .collect(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            rules: None,
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
    });
    app.document_state.tabs[0].project_id = Some(id);

    drag(&mut app, (0.0, 0.0), (20.0, 20.0));
    drag(&mut app, (30.0, 0.0), (50.0, 20.0));

    let names: Vec<String> = sheets(&app).into_iter().map(|s| s.filename).collect();
    assert_eq!(names, ["sheet2.snxsch", "sheet3.snxsch"]);
}

#[test]
fn defaults_are_unique_across_parents_of_one_project() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path().join("root.snxsch");
    let other = tmp.path().join("other.snxsch");
    let mut app = app_with_sheet_tool(root.clone());

    // A second open parent in the same project.
    app.document_state.engines.insert(
        other.clone(),
        signex_engine::Engine::new(empty_sheet()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "other".to_string(),
        path: other.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: tmp.path().join("P.snxprj"),
        data: ProjectData {
            name: "P".to_string(),
            dir: tmp.path().to_string_lossy().into_owned(),
            schematic_root: Some("root.snxsch".to_string()),
            pcb_file: None,
            sheets: ["root.snxsch", "other.snxsch"]
                .iter()
                .map(|f| SheetEntry {
                    name: (*f).to_string(),
                    filename: (*f).to_string(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                })
                .collect(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            rules: None,
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
    });
    app.document_state.tabs[0].project_id = Some(id);
    app.document_state.tabs[1].project_id = Some(id);

    drag(&mut app, (0.0, 0.0), (20.0, 20.0));
    assert_eq!(sheets(&app)[0].filename, "sheet1.snxsch");

    // Switch to the other parent: sheet1 is taken by a sibling parent.
    app.document_state.active_tab = 1;
    app.document_state.active_path = Some(other);
    drag(&mut app, (0.0, 0.0), (20.0, 20.0));
    assert_eq!(sheets(&app)[0].filename, "sheet2.snxsch");
}

/// Switch to the Sheet tool the way the user does: through `SelectTool`.
fn pick_sheet_tool(app: &mut Signex) {
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Sheet)));
}

#[test]
fn a_leftover_line_anchor_does_not_turn_the_first_sheet_click_into_a_commit() {
    let tmp = TempDir::new().expect("tempdir");
    let mut app = app_with_sheet_tool(tmp.path().join("root.snxsch"));
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Line)));
    // The Line tool re-arms at its last endpoint.
    click(&mut app, 0.0, 0.0);
    click(&mut app, 20.0, 0.0);
    pick_sheet_tool(&mut app);

    click(&mut app, 60.0, 40.0);
    assert!(
        sheets(&app).is_empty(),
        "the first Sheet click must only arm a corner"
    );
    click(&mut app, 100.0, 80.0);
    let placed = sheets(&app);
    assert_eq!(placed.len(), 1);
    assert_eq!((placed[0].position.x, placed[0].position.y), (60.0, 40.0));
}

#[test]
fn a_half_placed_sheet_corner_does_not_follow_the_user_to_another_tab() {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path().join("root.snxsch");
    let other = tmp.path().join("other.snxsch");
    let mut app = app_with_sheet_tool(root);
    app.document_state.engines.insert(
        other.clone(),
        signex_engine::Engine::new(empty_sheet()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "other".to_string(),
        path: other,
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    let window_id = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window_id);

    click(&mut app, 10.0, 10.0);
    let _ = app.update(Message::Tab {
        window_id,
        msg: signex_app::tab_bar::TabMessage::Select(1),
    });
    assert_eq!(app.document_state.active_tab, 1);

    click(&mut app, 60.0, 60.0);
    assert!(
        sheets(&app).is_empty(),
        "a corner armed in tab A must not complete in tab B"
    );
}

#[test]
fn the_active_bar_sheet_symbol_entry_arms_the_sheet_tool() {
    let tmp = TempDir::new().expect("tempdir");
    let mut app = app_with_sheet_tool(tmp.path().join("root.snxsch"));
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));

    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(
        ActiveBarAction::PlaceSheetSymbol,
    )));
    assert_eq!(app.interaction_state.current_tool, Tool::Sheet);
}

#[test]
fn a_placed_sheet_is_selectable_by_its_edge_and_movable() {
    let tmp = TempDir::new().expect("tempdir");
    let mut app = app_with_sheet_tool(tmp.path().join("root.snxsch"));
    drag(&mut app, (10.0, 10.0), (50.0, 40.0));
    let placed = sheets(&app);
    assert_eq!(placed.len(), 1);

    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
    click(&mut app, 10.0, 25.0); // on the left edge
    let selected = app.interaction_state.active_canvas().selected.clone();
    assert!(
        selected
            .iter()
            .any(|i| i.kind == SelectedKind::ChildSheet && i.uuid == placed[0].uuid),
        "a click on the sheet edge must select the ChildSheet, got {selected:?}"
    );

    app.ui_state.move_selection.dx = "5".to_string();
    app.ui_state.move_selection.dy = "0".to_string();
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::Apply));
    assert_eq!(sheets(&app)[0].position.x, 15.0);
}
