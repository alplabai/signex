//! A paused dimension belongs to one sheet (mockup row 157): a document
//! change ends the pause of the sheet it moved, and only that one.

use std::path::{Path, PathBuf};

use signex_types::schematic::{Point, SchDrawing, SchematicSheet};

use super::placement_tests::{app_with_tool, click};
use crate::app::sch_tables::AnnotationEdit;
use crate::app::state::{SheetPlacementPause, WindowKind};
use crate::app::{InputMsg, Message, Signex, TabInfo, TabKind, Tool, ToolMessage, WindowMsg};
use crate::canvas::CanvasEvent;
use crate::menu_bar::MenuMessage;
use crate::panels::{AnnotationPanelMsg, PanelMsg};

const FROM: (f64, f64) = (0.0, 0.0);
const TO: (f64, f64) = (12.7, 0.0);

/// The tool armed on sheet A in main, with `others` open behind it.
fn app_on_sheet_a(others: &[&str]) -> (Signex, Vec<PathBuf>) {
    let mut app = app_with_tool(Tool::LinearDimension);
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let mut paths = vec![PathBuf::from("placement.snxsch")];
    for name in others {
        let path = PathBuf::from(name);
        let engine = signex_engine::Engine::new(SchematicSheet::default()).expect("engine");
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(TabInfo {
            title: (*name).to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: TabKind::Schematic,
        });
        paths.push(path);
    }
    (app, paths)
}

fn show_tab(app: &mut Signex, path: &Path) {
    let index = app.document_state.tabs.iter().position(|t| t.path == path);
    app.document_state.active_tab = index.expect("the tab");
    app.sync_active_tab();
}

fn tab_key(app: &mut Signex) {
    let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
}

fn escape_in(app: &mut Signex, window: Option<iced::window::Id>) {
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window }));
}

fn dimensions_on(app: &Signex, path: &Path) -> usize {
    let sheet = app.document_state.engines[path].document();
    let is_dimension = |d: &&SchDrawing| matches!(d, SchDrawing::Dimension(_));
    sheet.drawings.iter().filter(is_dimension).count()
}

fn undocked_over(app: &mut Signex, path: &Path) -> iced::window::Id {
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: path.to_path_buf(),
            title: "sheet".to_string(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());
    window
}

fn click_in(app: &mut Signex, window: iced::window::Id, (x, y): (f64, f64)) {
    let _ = app.update(Message::CanvasEventInWindow {
        window_id: window,
        event: CanvasEvent::Clicked {
            world_x: x,
            world_y: y,
        },
    });
}

/// Two points down and Tab pressed, in `window`.
fn pause_in(app: &mut Signex, window: iced::window::Id) {
    click_in(app, window, FROM);
    click_in(app, window, TO);
    let _ = app.run_in_window_document(window, |app| {
        app.update(Message::Tool(ToolMessage::PrePlacementTab))
    });
    assert!(app.interaction_state.canvases[&window].placement_paused);
}

fn held_points(app: &Signex) -> usize {
    app.interaction_state.annotation_tool.points.len()
}

#[test]
fn a_pause_carried_to_another_sheet_places_nothing_with_its_points() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    click(&mut app, FROM);
    click(&mut app, TO);
    tab_key(&mut app);
    show_tab(&mut app, &paths[1]);
    escape_in(&mut app, None);
    click(&mut app, (5.08, 5.08));
    assert_eq!(dimensions_on(&app, &paths[1]), 0, "nothing lands on B");
    assert_eq!(dimensions_on(&app, &paths[0]), 0, "nor on A");
}

#[test]
fn a_tab_switch_ends_mains_pause_and_what_esc_would_restore() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    click(&mut app, FROM);
    click(&mut app, TO);
    tab_key(&mut app);
    show_tab(&mut app, &paths[1]);
    let state = &app.interaction_state;
    assert!(!state.active_canvas().placement_paused);
    assert!(state.placement_pause.is_none());
    assert!(state.annotation_tool.paused_with.is_none());
    assert!(state.annotation_tool.points.is_empty());
}

#[test]
fn a_tab_switch_in_main_keeps_the_pause_of_an_undocked_sheet() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch", "c.snxsch"]);
    let window = undocked_over(&mut app, &paths[2]);
    pause_in(&mut app, window);
    show_tab(&mut app, &paths[1]);
    let state = &app.interaction_state;
    assert!(state.canvases[&window].placement_paused, "its pause stays");
    assert!(state.placement_pause.is_some());
    assert!(state.annotation_tool.paused_with.is_some());
    assert_eq!(held_points(&app), 2, "and so do its points");

    // The Esc of that window's Override box, as its dock sends it.
    let escaped = Message::Dock(crate::dock::DockMessage::Panel(PanelMsg::Annotation(
        AnnotationPanelMsg::OverrideEscaped,
    )));
    let scoped = app.scope_dock_message(window, escaped);
    let _ = app.update(scoped);
    assert!(!app.interaction_state.canvases[&window].placement_paused);
    assert_eq!(
        app.interaction_state.annotation_tool.points,
        vec![Point::new(FROM.0, FROM.1), Point::new(TO.0, TO.1)],
        "Esc in the window goes on with its own points"
    );
}

#[test]
fn a_click_in_main_does_not_finish_a_dimension_paused_in_a_window() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    pause_in(&mut app, window);
    click(&mut app, (5.08, 5.08));
    assert_eq!(dimensions_on(&app, &paths[0]), 0, "nothing lands in main");
    assert_eq!(held_points(&app), 2, "the window's points are untouched");
}

#[test]
fn closing_an_undocked_sheet_ends_its_pause_and_its_points() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    pause_in(&mut app, window);
    let _ = app.update(Message::Window(WindowMsg::ReattachTab(window)));
    let state = &app.interaction_state;
    assert!(state.placement_pause.is_none());
    assert!(state.annotation_tool.paused_with.is_none());
    assert!(state.annotation_tool.points.is_empty());
}

#[test]
fn closing_another_undocked_sheet_keeps_the_pause() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch", "c.snxsch"]);
    let paused = undocked_over(&mut app, &paths[1]);
    let other = undocked_over(&mut app, &paths[2]);
    pause_in(&mut app, paused);
    let _ = app.update(Message::Window(WindowMsg::ReattachTab(other)));
    let state = &app.interaction_state;
    assert!(state.canvases[&paused].placement_paused);
    assert!(state.placement_pause.is_some());
    assert_eq!(held_points(&app), 2);
}

#[test]
fn an_undo_in_a_window_keeps_a_pause_on_mains_sheet() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click_in(&mut app, window, FROM);
    click_in(&mut app, window, TO);
    click_in(&mut app, window, (5.08, 5.08));
    assert_eq!(dimensions_on(&app, &paths[1]), 1, "one edit to undo");
    click(&mut app, FROM);
    click(&mut app, TO);
    tab_key(&mut app);
    let _ = app.update(Message::Ui(crate::app::UiMsg::MenuInWindow {
        window,
        msg: MenuMessage::command("undo"),
    }));
    assert_eq!(dimensions_on(&app, &paths[1]), 0, "the undo ran");
    let state = &app.interaction_state;
    assert!(state.active_canvas().placement_paused);
    assert!(state.annotation_tool.paused_with.is_some());
    assert_eq!(held_points(&app), 2);
}

#[test]
fn the_os_closing_an_undocked_sheet_ends_its_pause_and_its_points() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    pause_in(&mut app, window);
    let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(window)));
    let state = &app.interaction_state;
    assert!(state.placement_pause.is_none());
    assert!(state.annotation_tool.paused_with.is_none());
    assert!(state.annotation_tool.points.is_empty());
}

#[test]
fn tab_in_main_leaves_a_pause_with_its_window() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    pause_in(&mut app, window);
    tab_key(&mut app);
    let state = &app.interaction_state;
    assert!(
        !state.active_canvas().placement_paused,
        "main did not pause"
    );
    assert!(state.canvases[&window].placement_paused, "the window did");
    assert!(matches!(
        state.placement_pause,
        Some(SheetPlacementPause::Sheet { window: Some(w), .. }) if w == window
    ));
    assert_eq!(held_points(&app), 2);

    escape_in(&mut app, None);
    click(&mut app, (5.08, 5.08));
    assert_eq!(dimensions_on(&app, &paths[0]), 0, "B's points stay off A");
    assert_eq!(dimensions_on(&app, &paths[1]), 0);
}

#[test]
fn tab_in_a_window_leaves_a_pause_with_main() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click(&mut app, FROM);
    click(&mut app, TO);
    tab_key(&mut app);
    let _ = app.run_in_window_document(window, |app| {
        app.update(Message::Tool(ToolMessage::PrePlacementTab))
    });
    let state = &app.interaction_state;
    assert!(state.active_canvas().placement_paused, "main keeps it");
    assert!(!state.canvases[&window].placement_paused);
    assert!(matches!(
        state.placement_pause,
        Some(SheetPlacementPause::Sheet { window: None, .. })
    ));
    assert_eq!(held_points(&app), 2);
}

#[test]
fn a_pause_with_no_points_refuses_a_click_from_another_window() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    tab_key(&mut app);
    click_in(&mut app, window, (5.08, 5.08));
    assert_eq!(held_points(&app), 0, "B's point stays out of A's buffer");
}

#[test]
fn mains_form_does_not_edit_a_dimension_paused_in_a_window() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    pause_in(&mut app, window);
    let typed = AnnotationPanelMsg::Picked(AnnotationEdit::OverrideText("typed".to_string()));
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::Annotation(typed),
    )));
    let dimension = &app.interaction_state.annotation_tool.dimension;
    assert_eq!(dimension.override_text, None);
}

#[test]
fn space_in_main_does_not_flip_a_dimension_paused_in_a_window() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    pause_in(&mut app, window);
    assert!(app.flip_dimension_side(), "the key is still answered");
    assert!(!app.interaction_state.annotation_tool.dimension.flipped);
}
