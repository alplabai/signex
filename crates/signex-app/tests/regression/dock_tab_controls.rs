//! Document-tab close button, dock-tab right-click menu, and the dock drop
//! preview (#1893).

use signex_app::app::{InputMsg, Message, Signex, TabInfo, TabKind, UiMsg};
use signex_app::dock::{DockMessage, DockTabAction, PanelPosition};
use signex_app::tab_bar::TabMessage;
use std::path::PathBuf;

use crate::primitive_dirty_quit::{edit_description_via_panel, open_footprint_editor};

/// Opens one kind of context menu on the app.
type MenuOpener = dyn Fn(&mut Signex);

fn app_with_main_window() -> (Signex, iced::window::Id) {
    let (mut app, _t) = Signex::new();
    let id = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(id);
    app.ui_state.window_size = (1200.0, 800.0);
    (app, id)
}

fn push_tab(app: &mut Signex, name: &str) {
    let path = PathBuf::from(name);
    app.document_state.tabs.push(TabInfo {
        title: name.into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

fn close_x(app: &mut Signex, window_id: iced::window::Id, idx: usize) {
    let _ = app.update(Message::Tab {
        window_id,
        msg: TabMessage::CloseClicked(idx),
    });
}

#[test]
fn the_close_x_closes_exactly_that_clean_tab() {
    let (mut app, win) = app_with_main_window();
    push_tab(&mut app, "one.snxfpt");
    push_tab(&mut app, "two.snxfpt");
    push_tab(&mut app, "three.snxfpt");

    close_x(&mut app, win, 1);

    let titles: Vec<_> = app
        .document_state
        .tabs
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(titles, ["one.snxfpt", "three.snxfpt"]);
}

#[test]
fn the_close_x_on_a_dirty_tab_keeps_the_draft_like_the_menu_close() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("a.snxfpt");
    let (mut app, win) = app_with_main_window();
    open_footprint_editor(&mut app, &path);
    edit_description_via_panel(&mut app);
    assert!(app.document_state.is_document_dirty(&path));

    close_x(&mut app, win, 0);

    assert!(app.document_state.tabs.is_empty());
    // Parked, not discarded: Save All / reopen still see the edits.
    assert!(app.document_state.is_document_dirty(&path));
    assert!(app.document_state.footprint_editors.contains_key(&path));
}

fn left_panels(app: &Signex) -> Vec<signex_app::panels::PanelKind> {
    app.document_state
        .dock
        .panel_kinds(PanelPosition::Left)
        .to_vec()
}

fn open_menu(app: &mut Signex, pos: PanelPosition, idx: usize) {
    let _ = app.update(Message::Dock(DockMessage::TabContextMenuRequested(
        pos, idx,
    )));
}

fn pick(app: &mut Signex, action: DockTabAction) {
    let _ = app.update(Message::Dock(DockMessage::TabMenuActionPicked(action)));
}

#[test]
fn right_click_on_a_dock_tab_opens_its_menu() {
    let (mut app, _) = app_with_main_window();
    assert!(!left_panels(&app).is_empty());
    open_menu(&mut app, PanelPosition::Left, 0);
    let menu = app.document_state.dock.tab_menu.expect("menu open");
    assert_eq!(menu.position, PanelPosition::Left);
    assert_eq!(menu.kind, left_panels(&app)[0]);
}

#[test]
fn dock_menu_close_removes_the_panel_and_dismisses() {
    let (mut app, _) = app_with_main_window();
    let before = left_panels(&app).len();
    open_menu(&mut app, PanelPosition::Left, 0);
    pick(&mut app, DockTabAction::Close);
    assert_eq!(left_panels(&app).len(), before - 1);
    assert!(app.document_state.dock.tab_menu.is_none());
}

#[test]
fn dock_menu_float_leaves_a_resting_floating_panel() {
    let (mut app, _) = app_with_main_window();
    let kind = left_panels(&app)[0];
    open_menu(&mut app, PanelPosition::Left, 0);
    pick(&mut app, DockTabAction::Float);
    let fp = app
        .document_state
        .dock
        .floating
        .last()
        .expect("floating panel");
    assert_eq!(fp.kind, kind);
    assert!(!fp.dragging);
    assert!(app.document_state.dock.tab_menu.is_none());
}

#[test]
fn dock_menu_move_sends_the_panel_to_the_other_region() {
    let (mut app, _) = app_with_main_window();
    let kind = left_panels(&app)[0];
    open_menu(&mut app, PanelPosition::Left, 0);
    pick(&mut app, DockTabAction::MoveTo(PanelPosition::Bottom));
    let dock = &app.document_state.dock;
    assert!(dock.panel_kinds(PanelPosition::Bottom).contains(&kind));
    assert!(!dock.panel_kinds(PanelPosition::Left).contains(&kind));
}

#[test]
fn dismissing_or_escaping_closes_the_dock_menu() {
    let (mut app, win) = app_with_main_window();
    open_menu(&mut app, PanelPosition::Left, 0);
    let _ = app.update(Message::Dock(DockMessage::TabMenuDismissed));
    assert!(app.document_state.dock.tab_menu.is_none());

    open_menu(&mut app, PanelPosition::Left, 0);
    let _ = app.update(Message::Input(InputMsg::EscapePressed {
        window: Some(win),
    }));
    assert!(app.document_state.dock.tab_menu.is_none());
}

fn drag_to(app: &mut Signex, x: f32, y: f32) {
    let _ = app.update(Message::Ui(UiMsg::DragMove(x, y)));
}

#[test]
fn dragging_a_floating_panel_previews_and_release_docks_there() {
    let (mut app, _) = app_with_main_window();
    let kind = left_panels(&app)[0];
    let _ = app.update(Message::Dock(DockMessage::UndockPanel(
        PanelPosition::Left,
        0,
    )));
    assert!(app.document_state.dock.floating[0].dragging);

    drag_to(&mut app, 600.0, 300.0);
    assert_eq!(app.document_state.dock.drop_preview, None);

    drag_to(&mut app, 1190.0, 300.0);
    assert_eq!(
        app.document_state.dock.drop_preview,
        Some(PanelPosition::Right)
    );

    let _ = app.update(Message::Ui(UiMsg::DragEnd(iced::window::Id::unique())));
    let dock = &app.document_state.dock;
    assert_eq!(dock.drop_preview, None);
    assert!(dock.floating.is_empty());
    assert!(dock.panel_kinds(PanelPosition::Right).contains(&kind));
}

#[test]
fn a_tab_dragged_out_of_a_region_previews_from_the_moment_it_floats() {
    let (mut app, _) = app_with_main_window();
    let kind = left_panels(&app)[0];
    // Arm the tab drag at (50,50), then sweep far enough to undock it
    // while ending over the bottom edge.
    app.interaction_state.last_mouse_pos = (50.0, 50.0);
    let _ = app.update(Message::Dock(DockMessage::TabDragStart(
        PanelPosition::Left,
        0,
    )));
    drag_to(&mut app, 600.0, 790.0);

    assert_eq!(app.document_state.dock.floating.len(), 1);
    assert_eq!(
        app.document_state.dock.drop_preview,
        Some(PanelPosition::Bottom)
    );

    let _ = app.update(Message::Ui(UiMsg::DragEnd(iced::window::Id::unique())));
    let dock = &app.document_state.dock;
    assert_eq!(dock.drop_preview, None);
    assert!(dock.panel_kinds(PanelPosition::Bottom).contains(&kind));
}

#[test]
fn escape_mid_drag_clears_the_preview_and_leaves_the_panel_floating() {
    let (mut app, _) = app_with_main_window();
    let _ = app.update(Message::Dock(DockMessage::UndockPanel(
        PanelPosition::Left,
        0,
    )));
    drag_to(&mut app, 10.0, 300.0);
    assert_eq!(
        app.document_state.dock.drop_preview,
        Some(PanelPosition::Left)
    );

    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    let dock = &app.document_state.dock;
    assert_eq!(dock.drop_preview, None);
    assert_eq!(dock.floating.len(), 1);
    assert!(!dock.floating[0].dragging);
}

// One right-click menu at a time.

fn open_menus(app: &Signex) -> Vec<&'static str> {
    let i = &app.interaction_state;
    let mut open = Vec::new();
    if i.context_menu.is_some() {
        open.push("canvas");
    }
    if i.tab_context_menu.is_some() {
        open.push("document-tab");
    }
    if i.project_tree_context_menu.is_some() {
        open.push("project-tree");
    }
    if app.document_state.dock.tab_menu.is_some() {
        open.push("dock-tab");
    }
    if app.library.components_panel.row_menu.is_some() {
        open.push("components-row");
    }
    open
}

#[test]
fn opening_any_context_menu_closes_the_others() {
    use signex_app::app::ContextMenuMsg;
    use signex_app::library::LibraryMessage;
    use signex_app::library::state::ComponentsPanelRowKey;

    let (mut app, _) = app_with_main_window();
    push_tab(&mut app, "one.snxfpt");
    let canvas = |a: &mut Signex| {
        let _ = a.update(Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)));
    };
    let doc_tab = |a: &mut Signex| {
        let _ = a.update(Message::ContextMenu(ContextMenuMsg::ShowTab(0)));
    };
    let tree = |a: &mut Signex| {
        let _ = a.update(Message::ContextMenu(ContextMenuMsg::ShowProjectTree(None)));
    };
    let dock_tab = |a: &mut Signex| open_menu(a, PanelPosition::Left, 0);
    let row = |a: &mut Signex| {
        let key = ComponentsPanelRowKey {
            library_path: PathBuf::from("lib.snxlib"),
            table: "parts".into(),
            row_id: signex_library::RowId(uuid::Uuid::now_v7()),
        };
        let _ = a.update(Message::Library(
            LibraryMessage::ComponentsPanelRowRightClicked(key),
        ));
    };
    let openers: [(&str, &MenuOpener); 5] = [
        ("canvas", &canvas),
        ("document-tab", &doc_tab),
        ("project-tree", &tree),
        ("dock-tab", &dock_tab),
        ("components-row", &row),
    ];
    for (first, open_first) in openers {
        for (second, open_second) in openers {
            if first == second {
                continue;
            }
            open_first(&mut app);
            assert_eq!(open_menus(&app), [first], "opening {first}");
            open_second(&mut app);
            assert_eq!(open_menus(&app), [second], "{second} over {first}");
        }
    }
}

#[test]
fn a_dock_menu_acts_on_its_panel_even_after_the_tabs_shift() {
    let (mut app, _) = app_with_main_window();
    let panels = left_panels(&app);
    assert!(panels.len() >= 2, "default layout has two left tabs");
    let (a, b) = (panels[0], panels[1]);
    open_menu(&mut app, PanelPosition::Left, 1);

    // Tab A goes away while the menu is open; B slides into index 0.
    let _ = app.update(Message::Dock(DockMessage::ClosePanel(
        PanelPosition::Left,
        0,
    )));
    pick(&mut app, DockTabAction::Close);

    let left = left_panels(&app);
    assert!(!left.contains(&a) && !left.contains(&b), "B was closed");
    assert_eq!(left.len(), panels.len() - 2);
}

#[test]
fn a_dock_menu_for_a_vanished_panel_just_dismisses() {
    let (mut app, _) = app_with_main_window();
    let before = left_panels(&app).len();
    open_menu(&mut app, PanelPosition::Left, 0);
    let _ = app.update(Message::Dock(DockMessage::ClosePanel(
        PanelPosition::Left,
        0,
    )));
    // Re-open nothing: the stored panel is gone, so the pick is a no-op.
    pick(&mut app, DockTabAction::Close);
    assert_eq!(left_panels(&app).len(), before - 1);
    assert!(app.document_state.dock.tab_menu.is_none());
}

/// Review of #1906: a panel dragged past the window edge detaches into its
/// own window, and the drop preview that move showed goes with it.
#[test]
fn detaching_a_dragged_panel_clears_the_drop_preview() {
    let (mut app, _) = app_with_main_window();
    let _undock = app.update(Message::Dock(DockMessage::UndockPanel(
        PanelPosition::Left,
        0,
    )));
    drag_to(&mut app, -20.0, 300.0);
    assert_eq!(
        app.document_state.dock.drop_preview,
        Some(PanelPosition::Left)
    );
    let _detach = app.update(Message::Window(
        signex_app::app::WindowMsg::DetachFloatingPanel(0),
    ));
    assert!(app.document_state.dock.floating.is_empty());
    assert_eq!(app.document_state.dock.drop_preview, None);
}
