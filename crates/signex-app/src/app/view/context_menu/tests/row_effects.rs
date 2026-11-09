//! What each context-menu command row DOES (#1727, #1920).
//!
//! Every test builds the real rows, presses one the way a click does (its
//! `on_press` message through `update`) and asserts on the resulting state:
//! the selection, the document, the tool, the ghost, the menu. None of them
//! names the message a row sends, so they keep holding when a row is rewired
//! to another route to the same command.

use super::*;

use crate::active_bar::{ActiveBarAction as A, ActiveBarMsg};
use crate::app::path_ops::settle;
use crate::app::{TabInfo, TabKind};
use signex_types::schematic::{
    Point, SchDrawing, SchematicSheet, SelectedItem, SelectedKind, Symbol, Wire,
};
use uuid::Uuid;

fn symbol(n: u128, reference: &str, x: f64, y: f64) -> Symbol {
    Symbol {
        uuid: Uuid::from_u128(n),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        position: Point::new(x, y),
        ..Default::default()
    }
}

fn line(n: u128) -> SchDrawing {
    SchDrawing::Line {
        uuid: Uuid::from_u128(n),
        start: Point::new(n as f64, 0.0),
        end: Point::new(n as f64, 5.0),
        width: 0.25,
        stroke_color: None,
        locked: false,
    }
}

/// An app showing `sheet` as its only tab, on the Altium keymap.
fn app_with(sheet: SchematicSheet) -> Signex {
    let path = std::path::PathBuf::from("/w/row-effects.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.grid_size_mm = 1.0;
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "row-effects".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

fn sheet_of(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    }
}

fn select(app: &mut Signex, items: Vec<SelectedItem>) {
    app.interaction_state.active_canvas_mut().selected = items;
}

fn select_symbols(app: &mut Signex) {
    let items = doc(app)
        .symbols
        .iter()
        .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol))
        .collect();
    select(app, items);
}

fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

/// `(x, y, rotation, mirror_x, mirror_y)` of every symbol.
fn poses(app: &Signex) -> Vec<(f64, f64, f64, bool, bool)> {
    doc(app)
        .symbols
        .iter()
        .map(|s| {
            (
                s.position.x,
                s.position.y,
                s.rotation,
                s.mirror_x,
                s.mirror_y,
            )
        })
        .collect()
}

fn open_menu(app: &mut Signex) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(300.0, 300.0)));
    assert!(app.interaction_state.context_menu.is_some(), "menu opened");
}

fn assert_menu_closed(app: &Signex, what: &str) {
    assert!(
        app.interaction_state.context_menu.is_none(),
        "{what}: the menu stays open"
    );
    assert!(
        app.interaction_state.context_submenu.is_none(),
        "{what}: the submenu stays open"
    );
}

/// The canvas menu's rows for the app's current selection, as
/// `view_context_menu` builds them.
fn canvas_rows(app: &Signex) -> Vec<DropdownEntry<Message>> {
    let (selection, project_key) = app.canvas_menu_inputs(
        app.ui_state
            .main_window_id
            .expect("the app opens its main window"),
    );
    let sc = CanvasShortcuts::resolve(|id| app.keymap_shortcut_label(id));
    let tokens = theme_tokens(TID);
    canvas_menu_entries(TID, &tokens, selection, None, project_key, &sc)
}

fn hint(app: &Signex) -> impl Fn(&str) -> String + '_ {
    |id| app.keymap_shortcut_label(id)
}

/// The message of the live row that runs command `id`.
fn live_row(rows: &[DropdownEntry<Message>], id: &str) -> Message {
    rows.iter()
        .find_map(|entry| match entry {
            DropdownEntry::Item(item) if item.command_id == Some(id) => item.on_press.clone(),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no live row runs {id}"))
}

fn row_is_live(rows: &[DropdownEntry<Message>], id: &str) -> bool {
    rows.iter().any(|entry| match entry {
        DropdownEntry::Item(item) => item.command_id == Some(id) && item.on_press.is_some(),
        _ => false,
    })
}

/// Press the canvas menu's row for `id`; the menu is open first.
fn press(app: &mut Signex, id: &str) -> iced::Task<Message> {
    open_menu(app);
    let message = live_row(&canvas_rows(app), id);
    app.update(message)
}

// ---- edit rows ----

fn one_symbol() -> Signex {
    let mut app = app_with(sheet_of(vec![symbol(1, "R1", 10.0, 10.0)]));
    select_symbols(&mut app);
    app
}

#[test]
fn copy_row_copies_the_selection_and_leaves_the_sheet_alone() {
    let mut app = one_symbol();
    let _ = press(&mut app, "copy");
    assert!(app.interaction_state.clipboard.is_some(), "nothing copied");
    assert_eq!(doc(&app).symbols.len(), 1);
    assert_eq!(app.interaction_state.active_canvas().selected.len(), 1);
    assert_menu_closed(&app, "copy");
}

#[test]
fn cut_row_copies_then_removes_the_selection() {
    let mut app = one_symbol();
    let _ = press(&mut app, "cut");
    assert!(app.interaction_state.clipboard.is_some(), "nothing copied");
    assert!(doc(&app).symbols.is_empty(), "the symbol was not cut");
    assert_menu_closed(&app, "cut");
}

#[test]
fn delete_row_removes_the_selection_as_one_undo_step() {
    let mut app = one_symbol();
    let _ = press(&mut app, "delete_selection");
    assert!(doc(&app).symbols.is_empty(), "the symbol was not deleted");
    assert!(app.command_context(None).can_undo, "no undo entry");
    assert_menu_closed(&app, "delete");
}

/// Run `task` the way the iced runtime does and feed every message back to
/// the app, answering each clipboard read with `text`.
fn settle_answering_clipboard(app: &mut Signex, task: iced::Task<Message>, text: &str) {
    use futures::StreamExt as _;
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            continue;
        };
        let mut messages = Vec::new();
        runtime.block_on(async {
            while let Some(action) = stream.next().await {
                match action {
                    iced_runtime::Action::Output(next) => messages.push(next),
                    iced_runtime::Action::Clipboard(iced_runtime::clipboard::Action::Read {
                        channel,
                        ..
                    }) => assert!(
                        channel
                            .send(Ok(iced::clipboard::Content::Text(text.to_owned())))
                            .is_ok()
                    ),
                    _ => {}
                }
            }
        });
        pending.extend(messages.into_iter().map(|m| app.update(m)));
    }
}

/// An app that copied its symbol through the Copy row; the text is what the
/// system clipboard then holds.
fn app_that_copied() -> (Signex, String) {
    let mut app = one_symbol();
    let _ = press(&mut app, "copy");
    let text = app
        .interaction_state
        .clipboard_text
        .clone()
        .expect("the copy reached the clipboard text");
    (app, text)
}

#[test]
fn paste_row_puts_the_copy_on_the_cursor() {
    let (mut app, text) = app_that_copied();
    let task = press(&mut app, "paste");
    assert_menu_closed(&app, "paste");
    settle_answering_clipboard(&mut app, task, &text);
    let session = app.interaction_state.placement.as_ref();
    assert!(
        session.is_some_and(|s| s.kind.is_paste()),
        "no paste on the cursor"
    );
    assert_eq!(doc(&app).symbols.len(), 1, "a paste is placed by a click");
}

#[test]
fn smart_paste_row_places_the_copy_at_once() {
    let (mut app, text) = app_that_copied();
    let task = press(&mut app, "smart_paste");
    assert_menu_closed(&app, "smart paste");
    settle_answering_clipboard(&mut app, task, &text);
    assert_eq!(doc(&app).symbols.len(), 2, "the copy was not placed");
    assert!(app.interaction_state.placement.is_none());
}

/// Copy and Cut have nothing to act on without a selection, so the rows are
/// greyed, as the Edit menu greys them and as the keys do nothing there.
#[test]
fn cut_and_copy_rows_are_greyed_with_nothing_selected() {
    let mut app = app_with(sheet_of(vec![symbol(1, "R1", 10.0, 10.0)]));
    select(&mut app, Vec::new());
    let rows = canvas_rows(&app);
    assert!(!row_is_live(&rows, "copy"));
    assert!(!row_is_live(&rows, "cut"));
    assert!(row_is_live(&rows, "paste"));
}

#[test]
fn rotate_row_turns_the_selection_like_the_edit_command() {
    let mut app = one_symbol();
    let before = poses(&app);
    let _ = press(&mut app, "rotate_counterclockwise");
    let mut direct = one_symbol();
    let _ = direct.update(Message::Edit(EditMsg::RotateSelectedCounterclockwise));
    assert_ne!(poses(&app), before, "the symbol did not rotate");
    assert_eq!(poses(&app), poses(&direct));
    assert_menu_closed(&app, "rotate");
}

#[test]
fn mirror_rows_flip_the_axes_the_keys_flip() {
    // The X key is a left-right flip, which the engine calls MirrorSelectedY.
    for (id, message) in [
        ("mirror_x", Message::Edit(EditMsg::MirrorSelectedY)),
        ("mirror_y", Message::Edit(EditMsg::MirrorSelectedX)),
    ] {
        let mut app = one_symbol();
        let before = poses(&app);
        let _ = press(&mut app, id);
        let mut direct = one_symbol();
        let _ = direct.update(message);
        assert_ne!(poses(&app), before, "{id}: the symbol did not flip");
        assert_eq!(poses(&app), poses(&direct), "{id}: flipped the other axis");
        assert_menu_closed(&app, id);
    }
}

// ---- Find Similar ----

#[test]
fn find_similar_row_opens_at_the_point_the_menu_was_opened_over() {
    let over = Wire {
        uuid: Uuid::from_u128(77),
        start: Point::new(0.0, 20.0),
        end: Point::new(10.0, 20.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    };
    let far = Wire {
        uuid: Uuid::from_u128(78),
        start: Point::new(100.0, 20.0),
        end: Point::new(110.0, 20.0),
        ..over.clone()
    };
    let mut sheet = sheet_of(Vec::new());
    sheet.wires = vec![over, far];
    let mut app = app_with(sheet.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    app.ui_state.cursor_document = app.document_state.active_path.clone();
    app.ui_state.cursor_x = 5.0;
    app.ui_state.cursor_y = 20.0;
    open_menu(&mut app);
    // The cursor moved off to the menu row, away from the wire.
    app.ui_state.cursor_x = 500.0;
    let message = live_row(&canvas_rows(&app), "find_similar_objects");
    let _ = app.update(message);

    assert_menu_closed(&app, "find similar");
    assert!(app.ui_state.find_similar.is_some(), "dialog not opened");
    let _ = app.update(Message::Selection(
        crate::app::selection_request::SelectionRequest::FindSimilar(
            crate::find_similar::FindSimilarMsg::ApplyPressed,
        ),
    ));
    let selected: Vec<Uuid> = app
        .interaction_state
        .active_canvas()
        .selected
        .iter()
        .map(|item| item.uuid)
        .collect();
    assert!(selected.contains(&Uuid::from_u128(77)));
}

// ---- Arrange ----

fn drawing_order(app: &Signex) -> Vec<u128> {
    doc(app)
        .drawings
        .iter()
        .map(|d| d.uuid().as_u128())
        .collect()
}

#[test]
fn arrange_rows_reorder_the_selected_drawing() {
    let cases: [(&str, u128, [u128; 4]); 4] = [
        ("bring_drawings_to_front", 2, [1, 3, 4, 2]),
        ("send_drawings_to_back", 3, [3, 1, 2, 4]),
        ("bring_drawings_forward", 3, [1, 2, 4, 3]),
        ("send_drawings_backward", 3, [1, 3, 2, 4]),
    ];
    for (id, picked, expected) in cases {
        let mut sheet = sheet_of(Vec::new());
        sheet.drawings = (1..=4).map(line).collect();
        let mut app = app_with(sheet);
        select(
            &mut app,
            vec![SelectedItem::new(
                Uuid::from_u128(picked),
                SelectedKind::Drawing,
            )],
        );
        open_menu(&mut app);
        let rows = super::super::submenu::arrange_entries(&hint(&app));
        let _ = app.update(live_row(&rows, id));
        assert_eq!(drawing_order(&app), expected, "{id}");
        assert_menu_closed(&app, id);
    }
}

// ---- placed-part rows ----

fn logged(needles: &[&str]) -> bool {
    crate::diagnostics::recent_entries()
        .iter()
        .any(|e| needles.iter().all(|n| e.message.contains(n)))
}

#[test]
fn placed_part_rows_reach_the_placed_part_flow() {
    let _ = crate::diagnostics::init_logging();
    // A part with no library behind it: each action says how to fix that,
    // naming the part, which proves the row reached the flow for it.
    let cases = [
        (
            "open_datasheet",
            "ZQ1",
            vec!["Place ZQ1 from a library part"],
        ),
        (
            "edit_selected_symbol_in_symbol_editor",
            "ZQ2",
            vec!["Place ZQ2 from a library part to edit its symbol"],
        ),
        ("change_component", "ZQ3", vec!["ZQ3"]),
    ];
    for (id, reference, needles) in cases {
        let mut app = app_with(sheet_of(vec![symbol(1, reference, 10.0, 10.0)]));
        select_symbols(&mut app);
        let _ = press(&mut app, id);
        assert!(logged(&needles), "{id}: nothing logged");
        assert_menu_closed(&app, id);
    }
}

// ---- Active Bar rows ----

/// Tool, ghosts and pending port: what arming a tool leaves behind.
fn armed(app: &Signex) -> String {
    let canvas = app.interaction_state.active_canvas();
    format!(
        "{:?}|{:?}|{:?}|{:?}|{}",
        app.interaction_state.current_tool,
        canvas.ghost_label.as_ref().map(|l| l.text.clone()),
        canvas.ghost_text.as_ref().map(|t| t.text.clone()),
        app.interaction_state.pending_port,
        app.library.picker.is_some(),
    )
}

fn armed_by_active_bar(action: A) -> String {
    let mut app = one_symbol();
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(action)));
    armed(&app)
}

fn place_rows(app: &Signex) -> Vec<DropdownEntry<Message>> {
    super::super::submenu::place_entries(TID, &hint(app))
}

#[test]
fn place_rows_arm_what_the_active_bar_arms() {
    let cases = [
        ("draw_wire", A::DrawWire),
        ("draw_bus", A::DrawBus),
        ("place_bus_entry", A::PlaceBusEntry),
        ("place_net_label", A::PlaceNetLabel),
        ("place_junction", A::PlaceJunction),
        ("place_port", A::PlacePort),
        ("place_off_sheet_connector", A::PlaceOffSheetConnector),
        ("place_no_erc", A::PlaceNoERC),
        ("place_sheet_symbol", A::PlaceSheetSymbol),
        ("place_sheet_entry", A::PlaceSheetEntry),
        ("place_component", A::PlaceComponent),
        ("place_text_string", A::PlaceTextString),
        ("place_text_frame", A::PlaceTextFrame),
        ("place_note", A::PlaceNote),
    ];
    for (id, action) in cases {
        let mut app = one_symbol();
        open_menu(&mut app);
        let _ = app.update(live_row(&place_rows(&app), id));
        assert_eq!(armed(&app), armed_by_active_bar(action), "{id}");
        assert_menu_closed(&app, id);
    }
}

#[test]
fn place_rows_arm_the_expected_tools() {
    use crate::app::Tool;
    for (id, tool) in [
        ("draw_wire", Tool::Wire),
        ("draw_bus", Tool::Bus),
        ("place_junction", Tool::Junction),
        ("place_net_label", Tool::Label),
        ("place_no_erc", Tool::NoErcDirective),
        ("place_text_string", Tool::Text),
    ] {
        let mut app = one_symbol();
        open_menu(&mut app);
        let _ = app.update(live_row(&place_rows(&app), id));
        assert_eq!(app.interaction_state.current_tool, tool, "{id}");
    }
}

#[test]
fn net_label_row_arms_the_net_ghost_and_ends_a_pending_port() {
    let mut app = one_symbol();
    open_menu(&mut app);
    // A right-click with a port armed would cancel it, so arm it after.
    app.interaction_state.pending_port = Some((
        signex_types::schematic::LabelType::Global,
        "PORT".to_string(),
    ));
    let _ = app.update(live_row(&place_rows(&app), "place_net_label"));
    assert_eq!(app.interaction_state.current_tool, crate::app::Tool::Label);
    assert!(app.interaction_state.pending_port.is_none());
    let ghost = app.interaction_state.active_canvas().ghost_label.clone();
    assert_eq!(ghost.map(|g| g.text).as_deref(), Some("NET"));
}

#[test]
fn power_port_rows_arm_what_the_active_bar_arms() {
    let cases = [
        ("GND Power Port", A::PlacePowerGND),
        ("VCC Power Port", A::PlacePowerVCC),
        ("+5 Power Port", A::PlacePowerPlus5),
        ("+12 Power Port", A::PlacePowerPlus12),
    ];
    for (label, action) in cases {
        let mut app = one_symbol();
        open_menu(&mut app);
        let message = place_rows(&app)
            .iter()
            .find_map(|entry| match entry {
                DropdownEntry::Item(item) if item.label == label => item.on_press.clone(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no live row {label}"));
        let _ = app.update(message);
        assert_eq!(armed(&app), armed_by_active_bar(action), "{label}");
        assert_menu_closed(&app, label);
    }
}

/// Three off-grid symbols, all selected, for the Align rows.
fn three_symbols() -> Signex {
    let mut app = app_with(sheet_of(vec![
        symbol(1, "R1", 0.3, 0.4),
        symbol(2, "R2", 7.7, 13.1),
        symbol(3, "R3", 20.3, 4.9),
    ]));
    select_symbols(&mut app);
    app
}

#[test]
fn align_rows_move_the_selection_as_the_active_bar_does() {
    let cases = [
        ("align_left", A::AlignLeft),
        ("align_right", A::AlignRight),
        ("align_top", A::AlignTop),
        ("align_bottom", A::AlignBottom),
        ("align_horizontal_centers", A::AlignHorizontalCenters),
        ("align_vertical_centers", A::AlignVerticalCenters),
        ("distribute_horizontally", A::DistributeHorizontally),
        ("distribute_vertically", A::DistributeVertically),
        ("align_to_grid", A::AlignToGrid),
    ];
    for (id, action) in cases {
        let mut app = three_symbols();
        let before = poses(&app);
        open_menu(&mut app);
        let rows = super::super::submenu::align_entries(TID, 3, &hint(&app));
        let _ = app.update(live_row(&rows, id));
        let mut direct = three_symbols();
        let _ = direct.update(Message::ActiveBar(ActiveBarMsg::Action(action)));
        assert_ne!(poses(&app), before, "{id}: nothing moved");
        assert_eq!(poses(&app), poses(&direct), "{id}");
        assert_menu_closed(&app, id);
    }
}

// ---- Project Options and the project tree's Save ----

fn loaded_project(app: &mut Signex, file: &str, owns_sheet: bool) -> usize {
    use crate::app::state::LoadedProject;
    use signex_types::project::{ProjectData, SheetEntry};
    let sheets: Vec<SheetEntry> = owns_sheet
        .then(|| SheetEntry {
            name: "s".to_string(),
            filename: "row-effects.snxsch".to_string(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        })
        .into_iter()
        .collect();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: std::path::PathBuf::from(format!("/w/{file}")),
        data: ProjectData {
            document_id: None,
            description: None,
            name: file.to_string(),
            dir: "/w".to_string(),
            schematic_root: owns_sheet.then(|| "row-effects.snxsch".to_string()),
            pcb_file: None,
            sheets,
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.projects.len() - 1
}

#[test]
fn canvas_project_options_row_opens_the_active_projects_tab() {
    let mut app = one_symbol();
    let idx = loaded_project(&mut app, "P.snxprj", true);
    let id = app.document_state.projects[idx].id;
    app.document_state.active_project = Some(id);
    app.document_state.tabs[0].project_id = Some(id);
    let _ = press(&mut app, "open_project_options");
    assert!(
        app.document_state
            .tabs
            .iter()
            .any(|tab| tab.kind.as_project() == Some(id)),
        "no Project tab"
    );
    assert_menu_closed(&app, "project options");
}

/// The project tree's root menu rows for the project at `idx`.
fn tree_root_rows(
    app: &mut Signex,
    idx: usize,
    has_schematic: bool,
) -> Vec<DropdownEntry<Message>> {
    use signex_widgets::tree_view::TreeNode;
    let id = app.document_state.projects[idx].id.to_string();
    let tree: Vec<TreeNode> = app
        .document_state
        .projects
        .iter()
        .map(|p| {
            TreeNode::branch(
                p.id.to_string(),
                signex_widgets::tree_view::TreeIcon::Folder,
                Vec::new(),
            )
        })
        .collect();
    app.document_state.panel_ctx.project_tree = tree;
    app.document_state.panel_ctx.has_schematic = has_schematic;
    let ctx = crate::app::ProjectTreeContextMenuState {
        x: 0.0,
        y: 0.0,
        key: Some(vec![id]),
        window: None,
    };
    app.project_tree_entries(&ctx)
        .expect("the project row has a menu")
}

#[test]
fn tree_save_row_writes_the_active_schematic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("s.snxsch");
    let mut app = one_symbol();
    let old = app.document_state.active_path.clone().expect("path");
    let engine = app.document_state.engines.remove(&old).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs[0].path = path.clone();
    app.document_state.active_path = Some(path.clone());
    let idx = loaded_project(&mut app, "P.snxprj", true);
    app.document_state.active_project = Some(app.document_state.projects[idx].id);
    app.document_state.mark_document_dirty(&path);

    let rows = tree_root_rows(&mut app, idx, true);
    assert!(row_is_live(&rows, "save_document"));
    app.interaction_state.project_tree_context_menu =
        Some(crate::app::ProjectTreeContextMenuState {
            x: 0.0,
            y: 0.0,
            key: None,
            window: None,
        });
    settle(&mut app, live_row(&rows, "save_document"));

    assert!(path.exists(), "the schematic was not written");
    assert!(
        app.interaction_state.project_tree_context_menu.is_none(),
        "the tree menu stays open after Save"
    );
}

/// Save writes the ACTIVE document and project, so with no schematic and a
/// clean active project the row has nothing to do, whatever the clicked
/// project holds: it is greyed, like File ▸ Save and Ctrl+S are.
#[test]
fn tree_save_row_is_greyed_when_save_would_write_nothing() {
    let mut app = one_symbol();
    let active = loaded_project(&mut app, "A.snxprj", false);
    let other = loaded_project(&mut app, "B.snxprj", false);
    app.document_state.active_project = Some(app.document_state.projects[active].id);
    let other_path = app.document_state.projects[other].path.clone();
    app.document_state.mark_document_dirty(&other_path);
    // No schematic tab is active: the clicked project is the only dirty one.
    app.document_state.tabs.clear();
    app.document_state.active_path = None;

    let rows = tree_root_rows(&mut app, other, false);

    assert!(!row_is_live(&rows, "save_document"));
}

#[test]
fn tree_project_options_row_opens_the_clicked_projects_tab() {
    let mut app = one_symbol();
    let active = loaded_project(&mut app, "A.snxprj", false);
    let other = loaded_project(&mut app, "B.snxprj", false);
    app.document_state.active_project = Some(app.document_state.projects[active].id);
    let other_id = app.document_state.projects[other].id;

    let rows = tree_root_rows(&mut app, other, false);
    let _ = app.update(live_row(&rows, "open_project_options"));

    let opened: Vec<_> = app
        .document_state
        .tabs
        .iter()
        .filter_map(|tab| tab.kind.as_project())
        .collect();
    assert_eq!(opened, [other_id], "the clicked project's tab");
}

// ---- rows that already sent the bridge's own message ----

fn sheet_symbol(n: u128, filename: &str) -> signex_types::schematic::ChildSheet {
    signex_types::schematic::ChildSheet {
        uuid: Uuid::from_u128(n),
        name: "Child".to_string(),
        filename: filename.to_string(),
        position: Point::new(100.0, 100.0),
        size: (30.0, 30.0),
        stroke_width: 0.12,
        fill: signex_types::schematic::FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

/// A parent sheet on disk in a temp dir with one selected sheet symbol that
/// points at `child.snxsch`, written only when `child` is given.
fn app_with_sheet_symbol(
    child: Option<SchematicSheet>,
) -> (Signex, tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let parent_path = tmp.path().join("parent.snxsch");
    let child_path = tmp.path().join("child.snxsch");
    if let Some(child) = child {
        let text = signex_types::format::SnxSchematic::new(child)
            .write_string()
            .expect("serialise");
        std::fs::write(&child_path, text).expect("write child");
    }
    let mut parent = sheet_of(Vec::new());
    parent.child_sheets = vec![sheet_symbol(5, "child.snxsch")];
    let text = signex_types::format::SnxSchematic::new(parent.clone())
        .write_string()
        .expect("serialise");
    std::fs::write(&parent_path, text).expect("write parent");
    let mut app = app_with(parent.clone());
    let old = app.document_state.active_path.clone().expect("path");
    app.document_state.engines.remove(&old);
    let engine =
        signex_engine::Engine::new_with_path(parent, Some(parent_path.clone())).expect("engine");
    app.document_state
        .engines
        .insert(parent_path.clone(), engine);
    app.document_state.tabs[0].path = parent_path.clone();
    app.document_state.active_path = Some(parent_path);
    select(
        &mut app,
        vec![SelectedItem::new(
            Uuid::from_u128(5),
            SelectedKind::ChildSheet,
        )],
    );
    (app, tmp, child_path)
}

#[test]
fn sync_sheet_pins_row_reconciles_the_symbols_pins() {
    let mut child = sheet_of(Vec::new());
    child.labels = vec![signex_types::schematic::Label {
        uuid: Uuid::from_u128(9),
        justify: signex_types::schematic::HAlign::Left,
        justify_v: signex_types::schematic::VAlign::Bottom,
        ..signex_types::schematic::Label::new(
            "SDA",
            Point::new(10.0, 10.0),
            signex_types::schematic::LabelType::Hierarchical,
        )
    }];
    let (mut app, _tmp, _child) = app_with_sheet_symbol(Some(child));
    let _ = press(&mut app, "sync_sheet_pins");
    let pins: Vec<String> = doc(&app).child_sheets[0]
        .pins
        .iter()
        .map(|p| p.name.clone())
        .collect();
    assert_eq!(pins, ["SDA"]);
    assert_menu_closed(&app, "sync sheet pins");
}

#[test]
fn open_and_create_child_sheet_rows_start_the_descent_through_the_symbol() {
    for id in ["open_child_sheet", "create_sheet_from_sheet_symbol"] {
        let (mut app, _tmp, child_path) = app_with_sheet_symbol(Some(sheet_of(Vec::new())));
        let _ = press(&mut app, id);
        assert!(
            app.ui_state.pending_sheet_opens.contains_key(&child_path),
            "{id}: no descent started"
        );
        assert_menu_closed(&app, id);
    }
}

#[test]
fn find_row_opens_the_find_dialog() {
    let mut app = one_symbol();
    let _ = press(&mut app, "find");
    assert!(app.ui_state.find_replace.open);
    assert_menu_closed(&app, "find");
}

#[test]
fn preferences_row_opens_preferences() {
    let mut app = one_symbol();
    let _ = press(&mut app, "open_preferences");
    assert!(app.ui_state.preferences_open);
    assert_menu_closed(&app, "preferences");
}

#[test]
fn properties_row_shows_the_properties_panel() {
    let mut app = one_symbol();
    let properties = crate::panels::PanelKind::Properties;
    app.document_state.dock.close_panel(properties);
    assert!(!app.document_state.dock.is_frontmost(properties));
    let _ = press(&mut app, "edit_selected_object_properties");
    assert!(app.document_state.dock.is_frontmost(properties));
    assert_menu_closed(&app, "properties");
}

/// Opened near the bottom edge, the project root's whole card stays inside
/// the window: the clamp uses the height of the rows it really has (#2129).
#[test]
fn the_tree_root_menu_opened_near_the_bottom_edge_stays_inside_the_window() {
    let mut app = one_symbol();
    let idx = loaded_project(&mut app, "P.snxprj", true);
    app.ui_state.window_size = (1000.0, 700.0);
    let rows = tree_root_rows(&mut app, idx, true);
    let height = super::super::placement::entries_height(&rows);
    assert!(
        height > super::super::placement::menu_height(6, false),
        "the root menu is taller than the old six-row guess"
    );
    let id = app.document_state.projects[idx].id.to_string();
    for anchor_y in [600.0_f32, 690.0] {
        let ctx = crate::app::ProjectTreeContextMenuState {
            x: 20.0,
            y: anchor_y,
            key: Some(vec![id.clone()]),
            window: None,
        };
        let (_card, _width, origin) =
            app.view_project_tree_context_menu(iced::window::Id::unique(), &ctx);
        assert!(origin.1 >= 0.0, "opens above the window at {anchor_y}");
        assert!(
            origin.1 + height <= 700.0,
            "the card runs past the bottom edge at {anchor_y}: top {} + {height}",
            origin.1
        );
    }
}
