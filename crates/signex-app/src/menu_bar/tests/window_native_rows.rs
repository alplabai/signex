//! The same rows pressed from the system menu bar and from an undocked
//! window's own bar.

use super::*;
use crate::menu_bar::native::build;

/// Press the native menu row `path` the way the macOS bar does: the action
/// map of the bar as it stands, then the activation handler.
fn native_click(app: &mut Signex, path: &str) -> Task<Message> {
    let model = menu_bar_model(&menu_context(app));
    app.native_menu.actions = build(&model, false).actions;
    app.update(Message::Window(
        crate::app::WindowMsg::NativeMenuItemActivated(path.to_string()),
    ))
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
    let (mut app, text) = app_that_copied();
    let _ = text;
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

/// Rows that act on the main window's document do the same from an
/// undocked window's bar.
#[test]
fn undocked_window_rows_act_on_the_main_document_like_the_main_bar() {
    for path in [
        "Edit/Copy",
        "Edit/Select All",
        "Edit/Find",
        "View/Zoom In",
        "View/Toggle Grid",
        "View/AutoFocus (dim unselected)",
        "Place/Wire",
        "Place/Net Label",
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
