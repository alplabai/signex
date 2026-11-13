//! The same rows pressed from the system menu bar and from an undocked
//! window's own bar.

use super::*;
use crate::menu_bar::native::{build, probe::row_message};

/// Press the native menu row `path` the way the macOS bar does: the message
/// the row of the bar, as it stands, sends.
fn native_click(app: &mut Signex, path: &str) -> Task<Message> {
    let menu = build(
        &menu_bar_model(&menu_context(app)),
        &app.ui_state.i18n,
        false,
    );
    let message = row_message(&menu, path).unwrap_or_else(|| panic!("{path} is not live"));
    app.update(message)
}

/// Every live row of the app's bar, as a path.
fn live_paths(app: &Signex) -> Vec<String> {
    fn walk(menu: &Menu, parent: &str, out: &mut Vec<String>) {
        for entry in &menu.entries {
            match entry {
                MenuEntry::Item(item) if item.is_enabled() => {
                    out.push(format!("{parent}/{}", item.label));
                }
                MenuEntry::Submenu(sub) => walk(sub, &format!("{parent}/{}", sub.title), out),
                _ => {}
            }
        }
    }
    let model = menu_bar_model(&menu_context(app));
    let mut out = Vec::new();
    for menu in &model.menus {
        walk(menu, &menu.title, &mut out);
    }
    out
}

fn rich() -> Signex {
    let (mut app, _) = app_that_copied();
    // One undo step, so Undo is live too.
    let _ = app.update(Message::Edit(crate::app::EditMsg::Duplicate));
    app
}

fn states() -> Vec<Fixture> {
    vec![
        ("schematic", rich),
        ("symbol editor", symbol_editor_app),
        ("footprint editor", footprint_editor_app),
        ("no document", bare_app),
    ]
}

/// A system-menu click on a row has the effect the in-window click has.
#[test]
fn every_live_row_has_the_same_effect_from_the_native_bar() {
    let mut pressed = 0;
    for (state, build_app) in states() {
        for path in live_paths(&build_app()) {
            // Exit lives in the app menu on macOS, and a recent project's
            // native id is its file path.
            if path == "File/Exit" || path.starts_with("File/Open Recent/") {
                continue;
            }
            let mut in_window = build_app();
            let window_task = click(&mut in_window, &path);
            let mut native = build_app();
            let native_task = native_click(&mut native, &path);
            assert_eq!(
                probe(&native),
                probe(&in_window),
                "{path} in {state}: native differs from the window row"
            );
            assert_eq!(
                has_work(&native_task),
                has_work(&window_task),
                "{path} in {state}: native task differs"
            );
            pressed += 1;
        }
    }
    assert!(pressed > 60, "only {pressed} rows pressed");
}

/// An undocked tab window: its own window id, canvas and tab, with the main
/// window's tab placed after it.
pub(super) fn with_undocked_window(mut app: Signex) -> (Signex, iced::window::Id) {
    let path = std::path::PathBuf::from("/w/undocked.snxsch");
    let sheet = sheet_of(vec![symbol(5, "R5", 50.0, 50.0)]);
    let engine = signex_engine::Engine::new(sheet.clone()).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    let main_tab = app.document_state.active_tab;
    app.document_state.tabs.insert(
        0,
        TabInfo {
            title: "undocked".into(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: TabKind::Schematic,
        },
    );
    app.document_state.active_tab = main_tab + 1;
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(
        window,
        crate::app::WindowKind::UndockedTab {
            path,
            title: "undocked".into(),
        },
    );
    let slot = crate::canvas::CanvasSlot::new();
    slot.set_viewport(iced::Rectangle::new(
        iced::Point::ORIGIN,
        iced::Size::new(800.0, 600.0),
    ));
    app.interaction_state.canvases.insert(window, slot);
    app.interaction_state
        .canvases
        .get_mut(&window)
        .expect("canvas")
        .set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    (app, window)
}

/// The message the bar of `window` sends for `msg`, as the window's view
/// routes it.
fn window_message(window: iced::window::Id, msg: crate::menu_bar::MenuMessage) -> Message {
    if msg.window_command().is_some() {
        Message::Ui(crate::app::UiMsg::MenuInWindow { window, msg })
    } else {
        Message::Menu(msg)
    }
}

/// Press the live row `path` of `window`'s own bar.
pub(super) fn click_in_window(
    app: &mut Signex,
    window: iced::window::Id,
    path: &str,
) -> Task<Message> {
    let msg = with_row_in(app, Some(window), path, |item| {
        item.dispatches()
            .unwrap_or_else(|| panic!("{path} is not live"))
            .clone()
    });
    app.update(window_message(window, msg))
}

/// App-wide rows do the same from an undocked window's bar as from the main
/// bar: the app state they touch is the same, whichever window is clicked.
#[test]
fn undocked_window_app_wide_rows_act_like_the_main_bar() {
    for path in [
        "Edit/Find",
        "View/Toggle Grid",
        "View/AutoFocus (dim unselected)",
        "Place/Wire",
        "Place/Junction",
        "Tools/Preferences...",
        "Help/Keyboard Shortcuts",
    ] {
        let (mut from_window, window) = with_undocked_window(one_symbol_selected());
        let _ = click_in_window(&mut from_window, window, path);
        let (mut from_main, _) = with_undocked_window(one_symbol_selected());
        let _ = click(&mut from_main, path);
        assert_eq!(probe(&from_window), probe(&from_main), "{path}");
    }
}

/// Select every symbol on the sheet `window` shows.
pub(super) fn select_window_symbols(app: &mut Signex, window: iced::window::Id) {
    let Some(crate::app::WindowKind::UndockedTab { path, .. }) = app.ui_state.windows.get(&window)
    else {
        panic!("not an undocked tab window");
    };
    let items = app.document_state.engines[path]
        .document()
        .symbols
        .iter()
        .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol))
        .collect();
    app.interaction_state
        .canvases
        .get_mut(&window)
        .expect("canvas")
        .selected = items;
}

/// #1922: View ▸ Fit All is enabled by the undocked window's own canvas and
/// now fits that canvas, as its shortcut does; it used to fit the main
/// window's canvas.
#[test]
fn undocked_fit_all_fits_its_own_canvas_not_the_main_one() {
    let (mut app, window) = with_undocked_window(one_symbol_selected());
    let sheet = sheet_of(vec![symbol(5, "R5", 50.0, 50.0)]);
    let cache = || {
        Some(crate::schematic_runtime::SchematicRenderCache::from_sheet(
            &sheet,
        ))
    };
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(cache());
    app.interaction_state
        .canvases
        .get_mut(&window)
        .expect("canvas")
        .set_render_cache(cache());
    assert!(row_is_live_in(&app, window, "View/Fit All"));
    let _ = click_in_window(&mut app, window, "View/Fit All");
    assert!(
        app.interaction_state.canvases[&window]
            .pending_fit
            .get()
            .is_some(),
        "the undocked canvas was not fitted"
    );
    assert!(
        app.interaction_state
            .active_canvas()
            .pending_fit
            .get()
            .is_none(),
        "the main canvas was fitted"
    );
}

fn row_is_live_in(app: &Signex, window: iced::window::Id, path: &str) -> bool {
    with_row_in(app, Some(window), path, |item| item.is_enabled())
}

/// An app whose main sheet holds R1 (selected in the main canvas) and whose
/// undocked window shows a different sheet holding R5.
fn main_with_r1_window_with_r5() -> (Signex, iced::window::Id) {
    with_undocked_window(one_symbol_selected())
}

/// Copy from the window's bar carries the window's R5 (at 50, 50), not the
/// main sheet's R1 (at 10, 10), and leaves the main selection alone.
#[test]
fn copy_from_a_windows_bar_carries_the_windows_symbol() {
    let (mut app, window) = main_with_r1_window_with_r5();
    select_window_symbols(&mut app, window);
    let main_selection = app.interaction_state.canvas.selected.clone();
    let _ = click_in_window(&mut app, window, "Edit/Copy");

    let copied = app.interaction_state.clipboard.as_ref().expect("copied");
    assert!(copied.origin.x > 40.0, "carried R1: {:?}", copied.origin);
    assert_eq!(app.interaction_state.canvas.selected, main_selection);
}

/// Select All from the window's bar selects the window sheet's symbol in the
/// window's canvas and leaves the main selection alone.
#[test]
fn select_all_from_a_windows_bar_selects_in_the_window() {
    let (mut app, window) = main_with_r1_window_with_r5();
    let main_selection = app.interaction_state.canvas.selected.clone();
    let _ = click_in_window(&mut app, window, "Edit/Select All");

    let in_window = app
        .interaction_state
        .canvas_for_window(window)
        .selected
        .clone();
    let r5 = app.document_state.engines[std::path::Path::new("/w/undocked.snxsch")]
        .document()
        .symbols[0]
        .uuid;
    assert_eq!(in_window, vec![SelectedItem::new(r5, SelectedKind::Symbol)]);
    assert_eq!(app.interaction_state.canvas.selected, main_selection);
}

/// Zoom In from the window's bar zooms the window's canvas, not the main one.
#[test]
fn zoom_in_from_a_windows_bar_zooms_the_windows_canvas() {
    let (mut app, window) = main_with_r1_window_with_r5();
    let main_before = app.interaction_state.canvas.zoom_percent();
    let window_before = app
        .interaction_state
        .canvas_for_window(window)
        .zoom_percent();
    let _ = click_in_window(&mut app, window, "View/Zoom In");

    assert!(
        app.interaction_state
            .canvas_for_window(window)
            .zoom_percent()
            > window_before
    );
    assert_eq!(app.interaction_state.canvas.zoom_percent(), main_before);
}

/// Net Label from the window's bar puts the label ghost on the window's
/// canvas.
#[test]
fn net_label_from_a_windows_bar_ghosts_on_the_windows_canvas() {
    let (mut app, window) = main_with_r1_window_with_r5();
    let _ = click_in_window(&mut app, window, "Place/Net Label");

    assert!(
        app.interaction_state
            .canvas_for_window(window)
            .ghost_label
            .is_some()
    );
    assert!(app.interaction_state.canvas.ghost_label.is_none());
}
