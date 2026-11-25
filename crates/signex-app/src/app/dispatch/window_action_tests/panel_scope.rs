//! A panel in an undocked window edits that window's document, whatever its
//! kind: with two documents of a kind open, the other stays clean. And what
//! undocking and docking back do to the main window's tab.

use std::path::{Path, PathBuf};

use super::*;
use crate::app::{FootprintDocument, SymbolDocument};
use crate::panels::PanelMsg;

/// A window over the tab at `path`, seated as `UndockedTabOpened` leaves it.
fn window_over(app: &mut Signex, path: &Path) -> iced::window::Id {
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: path.to_path_buf(),
            title: "doc".to_string(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    window
}

fn tab_of(path: &Path, kind: TabKind) -> TabInfo {
    TabInfo {
        title: "doc".into(),
        path: path.to_path_buf(),
        project_id: None,
        kind,
    }
}

fn app_with_main() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app
}

fn send_from(app: &mut Signex, window: iced::window::Id, message: PanelMsg) {
    let scoped = app.scope_dock_message(window, dock(message));
    assert!(is_scoped(&scoped, window), "not scoped to the window");
    let _ = app.update(scoped);
}

#[test]
fn a_footprint_panel_edit_changes_only_the_windows_footprint() {
    let mut app = app_with_main();
    let (a, b) = (PathBuf::from("a.snxfpt"), PathBuf::from("b.snxfpt"));
    for path in [&a, &b] {
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
        app.document_state
            .footprint_editors
            .insert(path.clone(), FootprintDocument::new(path.clone(), file));
        app.document_state
            .tabs
            .push(tab_of(path, TabKind::FootprintEditor(path.clone())));
    }
    app.document_state.active_tab = app.document_state.tabs.len() - 2;
    let window = window_over(&mut app, &b);

    send_from(
        &mut app,
        window,
        PanelMsg::FpEditorSetFootprintDescription("edited".into()),
    );

    let editors = &app.document_state.footprint_editors;
    assert_eq!(editors[&b].primitive().description, "edited");
    assert_eq!(editors[&a].primitive().description, "");
    assert_eq!(editors[&a].history.undo_len(), 0, "A has no undo step");
    assert!(!app.document_state.is_document_dirty(&a), "A stays clean");
    assert!(app.document_state.is_document_dirty(&b));
}

#[test]
fn a_symbol_panel_edit_changes_only_the_windows_symbol() {
    let mut app = app_with_main();
    let (a, b) = (PathBuf::from("a.snxsym"), PathBuf::from("b.snxsym"));
    for path in [&a, &b] {
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym"));
        app.document_state
            .symbol_editors
            .insert(path.clone(), SymbolDocument::new(path.clone(), file));
        app.document_state
            .tabs
            .push(tab_of(path, TabKind::SymbolEditor(path.clone())));
    }
    app.document_state.active_tab = app.document_state.tabs.len() - 2;
    let window = window_over(&mut app, &b);

    send_from(
        &mut app,
        window,
        PanelMsg::SymEditorSetSymbolDescription("edited".into()),
    );

    let editors = &app.document_state.symbol_editors;
    assert_eq!(editors[&b].file.symbols[0].description, "edited");
    assert_eq!(editors[&a].file.symbols[0].description, "");
    assert_eq!(editors[&a].history.undo_len(), 0, "A has no undo step");
    assert!(!app.document_state.is_document_dirty(&a), "A stays clean");
    assert!(app.document_state.is_document_dirty(&b));
}

/// Sheets: a window's edit leaves the main sheet at its revision.
#[test]
fn a_sheet_panel_edit_changes_only_the_windows_sheet() {
    let mut f = fixture();
    let before = f.app.document_state.engines[&f.main_path].revision();
    send_from(&mut f.app, f.window, PanelMsg::SetPaperSize("A2".into()));
    let engines = &f.app.document_state.engines;
    assert_eq!(engines[&f.window_path].document().paper_size, "A2");
    assert_ne!(engines[&f.main_path].document().paper_size, "A2");
    assert_eq!(engines[&f.main_path].revision(), before);
}

/// The app-wide messages stay on the main window.
#[test]
fn app_wide_panel_messages_are_not_scoped() {
    let f = fixture();
    for message in [
        PanelMsg::ToggleGrid,
        PanelMsg::SetGridSize(1.27),
        PanelMsg::Noop,
    ] {
        let sent = f.app.scope_dock_message(f.window, dock(message.clone()));
        assert!(!is_scoped(&sent, f.window), "{message:?}");
    }
}

/// Two sheets and a third, each a tab, with the first in view.
fn three_sheets() -> (Signex, [PathBuf; 3]) {
    let mut app = app_with_main();
    let paths = ["1.snxsch", "2.snxsch", "3.snxsch"].map(PathBuf::from);
    for path in &paths {
        let engine =
            signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(tab(path));
    }
    let first = app.document_state.tabs.len() - 3;
    app.document_state.active_tab = first;
    app.document_state.active_path = Some(paths[0].clone());
    (app, paths)
}

fn undock(app: &mut Signex, path: &Path) -> iced::window::Id {
    let index = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("the tab");
    let before: Vec<iced::window::Id> = app.ui_state.windows.keys().copied().collect();
    let _ = app.update(Message::Window(WindowMsg::UndockTab(index)));
    let window = *app
        .ui_state
        .windows
        .keys()
        .find(|id| !before.contains(id))
        .expect("a window");
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: path.to_path_buf(),
        id: window,
    }));
    window
}

#[test]
fn undocking_another_tab_leaves_main_where_it_was() {
    let (mut app, paths) = three_sheets();
    let _ = undock(&mut app, &paths[2]);
    assert_eq!(app.active_tab_path().as_ref(), Some(&paths[0]));
}

#[test]
fn a_redocked_tab_becomes_the_active_tab() {
    let (mut app, paths) = three_sheets();
    let window = undock(&mut app, &paths[2]);
    let _ = app.update(Message::Window(WindowMsg::ReattachTab(window)));
    assert_eq!(app.active_tab_path().as_ref(), Some(&paths[2]));
}

/// A wire being drawn on the sheet in view does not carry over to the sheet
/// main moves to.
#[test]
fn undocking_the_sheet_in_view_ends_a_wire_in_progress() {
    let (mut app, paths) = three_sheets();
    start_main_wire(&mut app);
    let _ = undock(&mut app, &paths[0]);
    assert_ne!(app.active_tab_path().as_ref(), Some(&paths[0]));
    assert!(!app.interaction_state.wire_drawing);
    assert!(app.interaction_state.wire_points.is_empty());
}

fn start_main_wire(app: &mut Signex) {
    let state = &mut app.interaction_state;
    state.wire_drawing = true;
    state.canvas.drawing_mode = true;
    state.wire_points.push(Point::new(1.0, 1.0));
}

/// Docking a tab back moves main off its sheet: main's wire must not go on
/// and commit into the sheet that comes back.
#[test]
fn a_redocked_tab_ends_the_wire_main_was_drawing() {
    let (mut app, paths) = three_sheets();
    let window = undock(&mut app, &paths[2]);
    start_main_wire(&mut app);
    let _ = app.update(Message::Window(WindowMsg::ReattachTab(window)));
    assert_eq!(app.active_tab_path().as_ref(), Some(&paths[2]));
    assert!(!app.interaction_state.wire_drawing);
    assert!(!app.interaction_state.canvas.drawing_mode);
}

/// A wire drawn in a window ends with that window, not on main's sheet.
#[test]
fn closing_a_window_ends_the_wire_drawn_in_it() {
    let (mut app, paths) = three_sheets();
    let window = undock(&mut app, &paths[2]);
    let state = &mut app.interaction_state;
    state.wire_drawing = true;
    state.wire_window = Some(window);
    state.wire_points.push(Point::new(1.0, 1.0));
    state.canvases.get_mut(&window).unwrap().drawing_mode = true;
    let _ = app.update(Message::Window(WindowMsg::ReattachTab(window)));
    assert!(!app.interaction_state.wire_drawing);
    assert!(app.interaction_state.wire_points.is_empty());
}

/// A window whose `drawing_mode` is left over from an ended wire does not
/// own the wire main is drawing now.
#[test]
fn a_stale_window_drawing_flag_does_not_end_mains_wire() {
    let (mut app, paths) = three_sheets();
    let window = undock(&mut app, &paths[2]);
    start_main_wire(&mut app);
    let state = &mut app.interaction_state;
    state.canvases.get_mut(&window).unwrap().drawing_mode = true;
    app.end_wire_of_window(window);
    assert!(app.interaction_state.wire_drawing);
    assert_eq!(app.interaction_state.wire_points.len(), 1);
}

/// Undocking main's tab leaves a wire another window is drawing alone.
#[test]
fn undocking_main_tab_keeps_a_wire_of_another_window() {
    let (mut app, paths) = three_sheets();
    let other = undock(&mut app, &paths[1]);
    let state = &mut app.interaction_state;
    state.wire_drawing = true;
    state.wire_window = Some(other);
    state.wire_points.push(Point::new(1.0, 1.0));
    let slot = state.canvases.get_mut(&other).unwrap();
    slot.drawing_mode = true;
    slot.wire_preview = vec![Point::new(1.0, 1.0)];
    let _ = undock(&mut app, &paths[0]);
    let state = &app.interaction_state;
    assert!(state.wire_drawing);
    assert_eq!(state.wire_points.len(), 1);
    assert!(state.canvases[&other].drawing_mode);
    assert_eq!(state.canvases[&other].wire_preview.len(), 1);
}

/// A wire started in a window does not go on from a click on main's sheet.
#[test]
fn a_click_in_main_does_not_continue_a_wire_of_another_window() {
    use crate::canvas::CanvasEvent::Clicked;
    let (mut app, paths) = three_sheets();
    let window = undock(&mut app, &paths[1]);
    app.interaction_state.current_tool = crate::app::Tool::Wire;
    let _ = app.update(Message::CanvasEventInWindow {
        window_id: window,
        event: Clicked {
            world_x: 10.16,
            world_y: 10.16,
        },
    });
    assert_eq!(app.interaction_state.wire_window, Some(window));
    let third = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == paths[2])
        .expect("the tab");
    app.document_state.active_tab = third;
    app.sync_active_tab();
    assert!(
        app.interaction_state.wire_drawing,
        "the window keeps its wire"
    );
    let _ = app.update(Message::CanvasEvent(Clicked {
        world_x: 50.8,
        world_y: 50.8,
    }));
    let state = &app.interaction_state;
    assert_eq!(state.wire_window, None, "the click started main's own wire");
    assert_eq!(state.wire_points.len(), 1);
    assert!(state.canvases[&window].wire_preview.is_empty());
    assert!(!state.canvases[&window].drawing_mode);
    let sheet = app.document_state.engines[&paths[2]].document();
    assert!(
        sheet.wires.is_empty(),
        "nothing committed into main's sheet"
    );
}

/// A window's click takes over from a wire in main: main stops drawing it.
#[test]
fn a_click_in_a_window_ends_the_wire_main_was_drawing() {
    use crate::canvas::CanvasEvent::Clicked;
    let (mut app, paths) = three_sheets();
    let window = undock(&mut app, &paths[1]);
    app.interaction_state.current_tool = crate::app::Tool::Wire;
    let _ = app.update(Message::CanvasEvent(Clicked {
        world_x: 10.16,
        world_y: 10.16,
    }));
    assert!(app.interaction_state.canvas.drawing_mode);
    let _ = app.update(Message::CanvasEventInWindow {
        window_id: window,
        event: Clicked {
            world_x: 50.8,
            world_y: 50.8,
        },
    });
    let state = &app.interaction_state;
    assert_eq!(state.wire_window, Some(window));
    assert!(state.canvas.wire_preview.is_empty());
    assert!(!state.canvas.drawing_mode);
    assert!(state.canvases[&window].drawing_mode);
    assert!(
        app.document_state.engines[&paths[0]]
            .document()
            .wires
            .is_empty()
    );
}

/// Two footprints, A in main and B in a window.
fn two_footprints() -> (Signex, PathBuf, PathBuf, iced::window::Id) {
    let mut app = app_with_main();
    let (a, b) = (PathBuf::from("a.snxfpt"), PathBuf::from("b.snxfpt"));
    for path in [&a, &b] {
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
        app.document_state
            .footprint_editors
            .insert(path.clone(), FootprintDocument::new(path.clone(), file));
        app.document_state
            .tabs
            .push(tab_of(path, TabKind::FootprintEditor(path.clone())));
    }
    app.document_state.active_tab = app.document_state.tabs.len() - 2;
    let window = window_over(&mut app, &b);
    (app, a, b, window)
}

fn grid_step(app: &Signex, path: &Path) -> f64 {
    app.document_state.footprint_editors[path]
        .state
        .snap_options
        .grid_step_mm
}

#[test]
fn grid_properties_opened_in_a_window_applies_to_its_footprint() {
    use crate::app::GridPropertiesMsg as G;
    let (mut app, a, b, window) = two_footprints();
    let before = grid_step(&app, &a);
    send_from(&mut app, window, PanelMsg::FpEditorGridManagerProperties);
    let _ = app.update(Message::GridProperties(G::SetStep("2.5".into())));
    let _ = app.update(Message::GridProperties(G::Apply));
    assert_eq!(grid_step(&app, &b), 2.5);
    assert_eq!(grid_step(&app, &a), before, "main's footprint is untouched");
}

#[test]
fn a_custom_filter_opened_in_a_window_applies_to_its_footprint() {
    use crate::app::SelectionFilterMsg as S;
    use crate::library::editor::footprint::SelectionFilterKind;
    let (mut app, a, b, window) = two_footprints();
    send_from(
        &mut app,
        window,
        PanelMsg::FpEditorOpenSelectionFilterCustom,
    );
    let toggle = S::ToggleCustomKind(SelectionFilterKind::Pads);
    let _ = app.update(Message::SelectionFilter(toggle));
    let _ = app.update(Message::SelectionFilter(S::ApplyCustom));
    let editors = &app.document_state.footprint_editors;
    assert!(!editors[&b].state.selection_filter.pads);
    assert!(editors[&a].state.selection_filter.pads, "main's stays");
}

/// Two footprints, B in a window, with main on a sheet.
fn footprint_window_over_a_sheet() -> (Signex, PathBuf, PathBuf, iced::window::Id) {
    let (mut app, a, b, window) = two_footprints();
    let sheet = PathBuf::from("main.snxsch");
    let engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    app.document_state.engines.insert(sheet.clone(), engine);
    app.document_state.tabs.push(tab(&sheet));
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(sheet);
    (app, a, b, window)
}

fn run_in(app: &mut Signex, id: &str, window: iced::window::Id) {
    let command = crate::keymap::AppCommandId::new(id).expect("a catalog id");
    let args = crate::app::command::CommandArgs::none();
    let _ = app.dispatch_command(&command, args, Some(window));
}

#[test]
fn the_grid_properties_command_in_a_window_targets_its_footprint() {
    use crate::app::GridPropertiesMsg as G;
    let (mut app, a, b, window) = footprint_window_over_a_sheet();
    let before = grid_step(&app, &a);
    run_in(&mut app, "open_grid_properties", window);
    let dialog = app.ui_state.grid_properties.as_ref().expect("it opened");
    assert_eq!(dialog.target, b);
    let _ = app.update(Message::GridProperties(G::SetStep("2.5".into())));
    let _ = app.update(Message::GridProperties(G::Apply));
    assert_eq!(grid_step(&app, &b), 2.5);
    assert_eq!(grid_step(&app, &a), before);
}

/// The picker's rows arrive with no window: they act on the footprint it
/// opened on, and its last row opens Grid Properties for that footprint.
#[test]
fn the_grid_picker_in_a_window_acts_on_its_footprint() {
    use crate::app::{GridPropertiesMsg as G, UiMsg};
    let (mut app, a, b, window) = footprint_window_over_a_sheet();
    let before = grid_step(&app, &a);
    run_in(&mut app, "open_grid_picker", window);
    let picker = app.interaction_state.grid_picker.as_ref().expect("open");
    assert_eq!((&picker.target, picker.owner), (&b, Some(window)));
    let _ = app.update(Message::Ui(UiMsg::GridPickerSelect(1.27)));
    assert_eq!(grid_step(&app, &b), 1.27);
    assert_eq!(grid_step(&app, &a), before);

    run_in(&mut app, "open_grid_picker", window);
    let _ = app.update(Message::GridProperties(G::OpenFromPicker));
    let dialog = app.ui_state.grid_properties.as_ref().expect("it opened");
    assert_eq!(dialog.target, b);
}

/// Grid Properties from main, with a window's picker open: main's footprint.
#[test]
fn grid_properties_from_main_ignores_another_windows_picker() {
    use crate::app::GridPropertiesMsg as G;
    let (mut app, a, b, window) = two_footprints();
    run_in(&mut app, "open_grid_picker", window);
    let picker = app.interaction_state.grid_picker.as_ref().expect("open");
    assert_eq!(picker.target, b);
    let _ = app.update(Message::GridProperties(G::Open));
    let dialog = app.ui_state.grid_properties.as_ref().expect("it opened");
    assert_eq!(dialog.target, a);
}

/// An undocked symbol window enables symbol commands by its own tab.
#[test]
fn a_symbol_window_is_a_symbol_editor_with_main_on_a_sheet() {
    let mut app = app_with_main();
    let (sheet, symbol) = (PathBuf::from("m.snxsch"), PathBuf::from("s.snxsym"));
    let engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    app.document_state.engines.insert(sheet.clone(), engine);
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym"));
    app.document_state
        .symbol_editors
        .insert(symbol.clone(), SymbolDocument::new(symbol.clone(), file));
    app.document_state
        .tabs
        .push(tab_of(&symbol, TabKind::SymbolEditor(symbol.clone())));
    app.document_state.tabs.push(tab(&sheet));
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(sheet);
    let window = window_over(&mut app, &symbol);
    assert!(app.command_context(Some(window)).has_symbol_editor);
    let main = app.ui_state.main_window_id;
    assert!(!app.command_context(main).has_symbol_editor);
}

/// A board report asked for in a window reads that window's board.
#[test]
fn a_board_report_in_a_window_reads_its_board() {
    use crate::app::BoardOutputDialog;
    let mut app = app_with_main();
    let (a, b) = (PathBuf::from("a.snxpcb"), PathBuf::from("b.snxpcb"));
    for path in [&a, &b] {
        let data = serde_json::json!({"uuid": uuid::Uuid::now_v7()});
        let board = serde_json::from_value(data).unwrap();
        app.open_pcb_tab(path.clone(), "board".into(), board);
    }
    let first = app.document_state.tabs.len() - 2;
    app.document_state.active_tab = first;
    let window = undock(&mut app, &b);
    assert_eq!(app.active_tab_path().as_ref(), Some(&a));
    run_in(&mut app, "board_report_bom", window);
    let dialog = app.ui_state.board_output.dialog.as_deref();
    let Some(BoardOutputDialog::Reports(report)) = dialog else {
        panic!("Board Reports did not open");
    };
    assert_eq!(report.board, b);
    // An SVG command exports the open dialog's board, not main's.
    assert_eq!(
        app.assembly_svg_source().map(|(board, _)| board),
        Some(b.clone())
    );
    let _ = app.update(Message::from(crate::app::BoardOutputMsg::Closed));
    run_in(&mut app, "export_board_pdf", window);
    assert!(matches!(
        app.ui_state.board_output.dialog.as_deref(),
        Some(BoardOutputDialog::Print(_))
    ));
    // And a CSV command with Board Print open does too.
    assert_eq!(app.board_csv_source().map(|(board, _)| board), Some(b));
}

/// The dialog's document closed first: Apply writes nothing, anywhere.
#[test]
fn grid_properties_apply_does_nothing_once_its_footprint_is_closed() {
    use crate::app::GridPropertiesMsg as G;
    let (mut app, a, b, window) = two_footprints();
    let before = grid_step(&app, &a);
    send_from(&mut app, window, PanelMsg::FpEditorGridManagerProperties);
    app.document_state.footprint_editors.remove(&b);
    let _ = app.update(Message::GridProperties(G::SetStep("2.5".into())));
    let _ = app.update(Message::GridProperties(G::Apply));
    assert_eq!(grid_step(&app, &a), before);
    assert!(app.ui_state.grid_properties.is_none());
}

/// A DRC rule link in a window: its board names the project, and the Rules
/// tab opens on the main window once the swap is over.
#[test]
fn a_drc_rule_link_in_a_window_opens_the_rules_tab_on_main() {
    use crate::panels::{BaseRule, DrcPanelMsg};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_string_lossy().into_owned();
    let mut app = crate::app::handlers::menu::export::tests::app_workspace(&root, &["t.snxsch"]);
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let board = dir.path().join("board.snxpcb");
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let data = serde_json::json!({"uuid": uuid::Uuid::now_v7()});
    app.open_pcb_tab(
        board.clone(),
        "board".into(),
        serde_json::from_value(data).unwrap(),
    );
    let id = app.document_state.projects[0].id;
    let window = undock(&mut app, &board);

    let link = PanelMsg::Drc(DrcPanelMsg::BaseRuleOpened(BaseRule::TrackWidth));
    let sent = app.scope_dock_message(window, dock(link));
    assert!(is_scoped(&sent, window));
    let task = app.update(sent);
    assert_eq!(app.rules_tab_in_window(true), None, "not inside the swap");
    crate::app::path_ops::settle_task(&mut app, task);
    assert_eq!(app.rules_tab_in_window(true), Some(id));
}

/// Mockup row 157: with the sheet undocked and Properties in the shared
/// dock, the Override box is in the sheet window's own tree, so the keys
/// that window receives reach it.
#[test]
fn tab_on_an_undocked_sheet_puts_the_keys_in_that_windows_override_box() {
    use crate::app::{Tool, ToolMessage};
    use iced::advanced::shell::{Bus, Waker};
    use iced::advanced::widget::operation::focusable;
    use iced::keyboard::{self, Key};
    use iced_runtime::UserInterface;
    use iced_runtime::user_interface::Cache;

    let mut f = fixture();
    let window = f.window;
    let _ = f.app.run_in_window_document(window, |app| {
        app.update(Message::Tool(ToolMessage::SelectTool(
            Tool::LinearDimension,
        )))
    });
    let (outcome, _task) = super::surfaces::command(&mut f.app, "placement_properties", window);
    drop(outcome);

    let mut renderer = crate::layout_probe::headless_renderer();
    let mut published = Bus::new();
    {
        let view = f.app.view(window);
        let mut ui = UserInterface::build(view, SIZE, Cache::default(), &mut renderer);
        let id = iced::widget::Id::new(crate::panels::DIMENSION_OVERRIDE_INPUT);
        ui.operate(&renderer, &mut focusable::focus::<()>(id));
        let key = Key::Character("j".into());
        let event = iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyJ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: Some("j".into()),
            repeat: false,
        });
        let _ = ui.update(
            &iced::window::Headless,
            &Waker::noop(),
            std::slice::from_ref(&event),
            iced::mouse::Cursor::Unavailable,
            &mut renderer,
            &mut published,
        );
    }
    let typed: Vec<Message> = published.into_iter().collect();
    assert!(typed.iter().all(|m| is_scoped(m, window)), "{typed:?}");
    feed(&mut f.app, typed);
    let dimension = &f.app.interaction_state.annotation_tool.dimension;
    assert_eq!(dimension.override_text.as_deref(), Some("j"));
}

/// Esc in `window`'s focused Override box: whether a widget captured it, and
/// what the widgets published.
fn escape_in_override_box(app: &Signex, window: iced::window::Id) -> (bool, Vec<Message>) {
    use iced::advanced::shell::{Bus, Waker};
    use iced::advanced::widget::operation::focusable;
    use iced::keyboard::{self, Key, key::Named};
    use iced_runtime::UserInterface;
    use iced_runtime::user_interface::Cache;

    let mut renderer = crate::layout_probe::headless_renderer();
    let mut published = Bus::new();
    let captured = {
        let view = app.view(window);
        let mut ui = UserInterface::build(view, SIZE, Cache::default(), &mut renderer);
        let id = iced::widget::Id::new(crate::panels::DIMENSION_OVERRIDE_INPUT);
        ui.operate(&renderer, &mut focusable::focus::<()>(id));
        let key = Key::Named(Named::Escape);
        let event = iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        let (_, statuses) = ui.update(
            &iced::window::Headless,
            &Waker::noop(),
            std::slice::from_ref(&event),
            iced::mouse::Cursor::Unavailable,
            &mut renderer,
            &mut published,
        );
        statuses[0] == iced::event::Status::Captured
    };
    (captured, published.into_iter().collect())
}

/// Mockup row 157: one Esc in that window's Override box resumes the sheet
/// the window shows, and the tool stays armed.
#[test]
fn one_esc_in_an_undocked_sheets_override_box_resumes_that_sheet() {
    use crate::app::{Tool, ToolMessage};

    let mut f = fixture();
    let window = f.window;
    let _ = f.app.run_in_window_document(window, |app| {
        app.update(Message::Tool(ToolMessage::SelectTool(
            Tool::LinearDimension,
        )))
    });
    let (outcome, _task) = super::surfaces::command(&mut f.app, "placement_properties", window);
    drop(outcome);
    assert!(f.app.interaction_state.canvases[&window].placement_paused);

    let (captured, escaped) = escape_in_override_box(&f.app, window);
    assert!(captured, "the box answers this Esc, nothing else does");
    assert!(escaped.iter().all(|m| is_scoped(m, window)), "{escaped:?}");
    feed(&mut f.app, escaped);
    assert!(!f.app.interaction_state.canvases[&window].placement_paused);
    assert_eq!(f.app.interaction_state.current_tool, Tool::LinearDimension);
}
