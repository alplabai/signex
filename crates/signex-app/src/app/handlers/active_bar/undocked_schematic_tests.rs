//! #1910: the Active Bar, its dropdown and the canvas context menu painted
//! in an undocked schematic window act on the sheet that window shows, and
//! paint in that window only.

use std::path::{Path, PathBuf};

use super::super::super::*;
use crate::active_bar::{ActiveBarAction, ActiveBarMenu, ActiveBarMsg};
use crate::app::handlers::menu::export::tests as fx;
use crate::app::state::WindowKind;
use signex_types::schematic::{SelectedItem, SelectedKind};

struct Fixture {
    app: Signex,
    main: iced::window::Id,
    window: iced::window::Id,
    a: PathBuf,
    b: PathBuf,
}

fn tab(path: &Path) -> TabInfo {
    TabInfo {
        title: "t".to_string(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    }
}

/// Sheet A undocked in its own window, sheet B active in the main window.
/// Each window's canvas has its sheet's wire selected.
fn fixture() -> Fixture {
    let a = PathBuf::from("/w/ov/a.snxsch");
    let b = PathBuf::from("/w/ov/b.snxsch");
    let mut app = fx::app_workspace("/w/ov", &["a.snxsch", "b.snxsch"]);
    let main = iced::window::Id::unique();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: a.clone(),
            title: "a".to_string(),
        },
    );
    let mut slots = Vec::new();
    for (path, net) in [(&a, "A_NET"), (&b, "B_NET")] {
        let sheet = fx::sheet_with_net("R1", net, &[]);
        let wire = SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Wire);
        let mut slot = crate::canvas::CanvasSlot::new();
        slot.set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
        slot.selected = vec![wire];
        slots.push(slot);
        fx::open_with(&mut app.document_state, path, sheet);
        app.document_state.tabs.push(tab(path));
    }
    let main_slot = slots.pop().expect("b");
    app.interaction_state.canvas = main_slot;
    app.interaction_state
        .canvases
        .insert(window, slots.pop().expect("a"));
    app.document_state.active_tab = 1;
    app.document_state.active_path = Some(b.clone());
    Fixture {
        app,
        main,
        window,
        a,
        b,
    }
}

fn wire_ends(app: &Signex, path: &Path) -> Vec<(f64, f64, f64, f64)> {
    app.document_state.engines[path]
        .document()
        .wires
        .iter()
        .map(|w| (w.start.x, w.start.y, w.end.x, w.end.y))
        .collect()
}

fn scoped_bar(window: iced::window::Id, inner: ActiveBarMsg) -> Message {
    Message::ActiveBar(ActiveBarMsg::ScopedToWindow(window, Box::new(inner)))
}

fn scoped_menu(window: iced::window::Id, inner: ContextMenuMsg) -> Message {
    Message::ContextMenu(ContextMenuMsg::ScopedToWindow(window, Box::new(inner)))
}

#[test]
fn an_active_bar_action_from_an_undocked_window_acts_on_its_sheet() {
    let mut f = fixture();
    let (a_before, b_before) = (wire_ends(&f.app, &f.a), wire_ends(&f.app, &f.b));
    let _ = f.app.update(scoped_bar(
        f.window,
        ActiveBarMsg::Action(ActiveBarAction::RotateSelection),
    ));
    assert_ne!(wire_ends(&f.app, &f.a), a_before, "A's wire rotated");
    assert_eq!(wire_ends(&f.app, &f.b), b_before, "B is untouched");
    assert_eq!(
        f.app.document_state.active_path.as_deref(),
        Some(f.b.as_path()),
        "the main window keeps its sheet"
    );
    assert_eq!(f.app.document_state.active_tab, 1);
}

#[test]
fn a_context_menu_action_from_an_undocked_window_acts_on_its_sheet() {
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    let menu = f.app.interaction_state.context_menu.as_ref().expect("menu");
    assert_eq!(menu.window, Some(f.window));
    let _ = f.app.update(scoped_menu(
        f.window,
        ContextMenuMsg::Action(ContextAction::Delete),
    ));
    assert!(wire_ends(&f.app, &f.a).is_empty(), "A's wire is deleted");
    assert_eq!(wire_ends(&f.app, &f.b).len(), 1, "B keeps its wire");
    assert!(f.app.interaction_state.context_menu.is_none());
}

#[test]
fn an_undocked_windows_context_menu_paints_in_that_window_only() {
    let mut f = fixture();
    let _ = f
        .app
        .update(scoped_menu(f.window, ContextMenuMsg::Show(5.0, 5.0)));
    assert!(!f.app.context_menu_overlay(f.window).is_empty());
    assert!(f.app.context_menu_overlay(f.main).is_empty());
}

#[test]
fn the_main_windows_context_menu_does_not_paint_in_an_undocked_window() {
    let mut f = fixture();
    let _ = f
        .app
        .update(Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)));
    assert!(!f.app.context_menu_overlay(f.main).is_empty());
    assert!(f.app.context_menu_overlay(f.window).is_empty());
}

#[test]
fn an_undocked_windows_dropdown_paints_in_that_window_only() {
    let mut f = fixture();
    let _ = f.app.update(scoped_bar(
        f.window,
        ActiveBarMsg::ToggleMenu(ActiveBarMenu::Align),
    ));
    assert!(!f.app.active_bar_menu_overlay(f.window).is_empty());
    assert!(f.app.active_bar_menu_overlay(f.main).is_empty());
}

#[test]
fn the_main_windows_dropdown_does_not_paint_in_an_undocked_window() {
    let mut f = fixture();
    let _ = f.app.update(Message::ActiveBar(ActiveBarMsg::ToggleMenu(
        ActiveBarMenu::Align,
    )));
    assert!(!f.app.active_bar_menu_overlay(f.main).is_empty());
    assert!(f.app.active_bar_menu_overlay(f.window).is_empty());
}
