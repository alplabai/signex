//! Mockup row 207 in the window (#1307), through `Signex::update`: Tile
//! Grid, the pane strips, the pane menu (Move Tab to Other Pane, Close
//! Pane, Sync Zoom), the splitters, a tab let go over the canvas, and the
//! view each pane keeps. The kept splits go to a recorder.

use crate::app::handlers::document_panes_tests::{app, drop_tab, kept_entry, path};
use crate::app::path_ops::settle_task;
use crate::app::{Message, PaneMsg, Signex, TileLayout, UiMsg, WindowMsg};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

fn pane(msg: PaneMsg) -> Message {
    Message::Window(WindowMsg::Pane(msg))
}

/// `message` and every task it leaves, run to the end.
fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    settle_task(app, task);
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

/// Ctrl held while `tab` is dropped at `at`: a second view.
fn second_view(app: &mut Signex, main: iced::window::Id, tab: usize) {
    app.ui_state.workspace_layouts.panes.second_view_held = true;
    drop_tab(app, main, tab, (0.95, 0.5));
    app.ui_state.workspace_layouts.panes.second_view_held = false;
}

/// View ▸ Window ▸ Tile Grid: one pane per document up to four, the active
/// one focused, kept for the project as `tile_grid`.
#[test]
fn the_tile_grid_row_puts_each_document_in_a_pane_of_a_grid() {
    let (mut app, _) = app();
    send(
        &mut app,
        Message::Menu(MenuMessage::Command(command("tile_windows_grid"))),
    );
    let view = app.tiled_pane_view().expect("tiled");
    assert_eq!(view.layout, TileLayout::Grid);
    assert_eq!((view.tabs.clone(), view.focused), (vec![0, 1, 2], 2));
    assert_eq!(app.ui_state.document_tiling, Some(TileLayout::Grid));
    let kept = kept_entry(&app).expect("kept");
    assert_eq!(kept["direction"], "tile_grid");
    // The focused pane is the last of three: the bottom row, full width.
    let area = app.document_area_in(None);
    let canvas = app.canvas_bounds();
    assert_eq!(canvas.width, area.width);
    assert!(canvas.y > area.y + area.height / 2.0 - 1.0, "{canvas:?}");
}

/// A tab on a pane's own strip makes it the active tab, in that pane.
#[test]
fn a_tab_pressed_on_a_pane_strip_shows_in_that_pane() {
    let (mut app, main) = app();
    drop_tab(&mut app, main, 0, (0.95, 0.5));
    assert_eq!(app.pane_tab_lists(), [vec![1, 2], vec![0]]);
    send(&mut app, pane(PaneMsg::TabPressed { pane: 0, tab: 1 }));
    let view = app.tiled_pane_view().expect("split");
    assert_eq!((view.tabs, view.focused), (vec![1, 0], 0));
    assert_eq!(app.document_state.active_tab, 1);
}

/// The ⋯ menu opens on its pane and closes again; a row picked there
/// focuses the pane and runs its command: Move Tab to Other Pane.
#[test]
fn the_pane_menu_moves_the_tab_of_its_pane_to_the_other_pane() {
    let (mut app, main) = app();
    drop_tab(&mut app, main, 0, (0.95, 0.5));
    send(&mut app, pane(PaneMsg::MenuToggled(0)));
    assert_eq!(app.ui_state.workspace_layouts.panes.menu, Some(0));
    send(&mut app, pane(PaneMsg::MenuToggled(0)));
    assert_eq!(app.ui_state.workspace_layouts.panes.menu, None);
    send(&mut app, pane(PaneMsg::MenuToggled(0)));
    send(
        &mut app,
        pane(PaneMsg::MenuCommandPicked {
            pane: 0,
            command: command("move_tab_to_other_pane"),
        }),
    );
    assert_eq!(app.ui_state.workspace_layouts.panes.menu, None);
    assert_eq!(
        app.pane_tab_lists(),
        [vec![1], vec![0, 2]],
        "io moved right"
    );
    let view = app.tiled_pane_view().expect("split");
    assert_eq!(view.focused, 1);
    assert_eq!(app.document_state.active_tab, 2);
    let kept = kept_entry(&app).expect("kept");
    assert_eq!(kept["panes"][1]["tabs"][1]["path"], "io.snxsch");
}

/// Close Pane hands the pane's tabs to the other one; down to one pane the
/// split ends and is forgotten, and no document closes.
#[test]
fn close_pane_merges_its_tabs_and_ends_a_split_of_two() {
    let (mut app, main) = app();
    drop_tab(&mut app, main, 0, (0.95, 0.5));
    send(&mut app, pane(PaneMsg::CloseRequested));
    assert!(app.tiled_pane_view().is_none());
    assert_eq!(app.ui_state.document_tiling, None);
    assert_eq!(app.document_state.tabs.len(), 3, "no document closed");
    assert_eq!(kept_entry(&app), None);
    send(&mut app, pane(PaneMsg::CloseRequested));
    assert_eq!(app.document_state.tabs.len(), 3, "refused, nothing changed");
}

/// Sync Zoom: the other view of the sheet takes the focused pane's zoom.
#[test]
fn sync_zoom_gives_the_other_pane_the_focused_zoom() {
    let (mut app, main) = app();
    second_view(&mut app, main, 2);
    app.interaction_state
        .canvas
        .set_pane_camera((40.0, 30.0, 6.0));
    send(&mut app, pane(PaneMsg::SyncZoomRequested));
    let other = &app.interaction_state.tile_panes.schematic[&path("io.snxsch")];
    assert_eq!(other.live_camera().2, 6.0);
}

/// Each pane keeps its own view: the focus moving to the other pane hands
/// the main canvas that pane's pan and zoom, and the pane left keeps the
/// view it had.
#[test]
fn each_pane_keeps_its_own_pan_and_zoom() {
    let (mut app, main) = app();
    drop_tab(&mut app, main, 0, (0.95, 0.5));
    let right = (12.0, 34.0, 5.0);
    app.interaction_state.canvas.set_pane_camera(right);
    let left = app.interaction_state.tile_panes.schematic[&path("io.snxsch")].live_camera();
    send(&mut app, Message::Window(WindowMsg::TilePanePressed(0)));
    assert_eq!(app.interaction_state.canvas.live_camera(), left);
    let kept = app.interaction_state.tile_panes.schematic[&path("power.snxsch")].live_camera();
    assert_eq!(kept, right, "the right pane kept its view");
    send(&mut app, Message::Window(WindowMsg::TilePanePressed(1)));
    assert_eq!(app.interaction_state.canvas.live_camera(), right);
}

/// Two views of one sheet keep their own selection, and Properties follows
/// the focused one.
#[test]
fn properties_follow_the_selection_of_the_focused_pane() {
    use signex_types::schematic::{SelectedItem, SelectedKind};
    let (mut app, main) = app();
    second_view(&mut app, main, 2);
    let symbol = app.document_state.engines[&path("io.snxsch")]
        .document()
        .symbols[0]
        .uuid;
    app.interaction_state.canvas.selected = vec![SelectedItem::new(symbol, SelectedKind::Symbol)];
    app.update_selection_info();
    assert_eq!(app.document_state.panel_ctx.selection_count, 1);
    send(&mut app, Message::Window(WindowMsg::TilePanePressed(0)));
    assert!(app.interaction_state.canvas.selected.is_empty());
    assert_eq!(app.document_state.panel_ctx.selection_count, 0);
    send(&mut app, Message::Window(WindowMsg::TilePanePressed(1)));
    assert_eq!(app.document_state.panel_ctx.selection_count, 1);
}

/// A splitter dragged resizes the panes and the sizes are kept; a double
/// click (Equal Pane Sizes) makes them equal again.
#[test]
fn a_dragged_splitter_resizes_the_panes_and_equal_undoes_it() {
    let (mut app, main) = app();
    drop_tab(&mut app, main, 0, (0.95, 0.5));
    let area = app.document_area_in(None);
    send(&mut app, pane(PaneMsg::SplitterPressed(0)));
    assert!(app.is_pointer_drag_active());
    let at = (area.x + area.width * 0.3, area.y + 100.0);
    send(&mut app, Message::Ui(UiMsg::DragMove(at.0, at.1)));
    send(&mut app, Message::Ui(UiMsg::DragEnd(main)));
    assert_eq!(app.ui_state.workspace_layouts.panes.splitter_drag, None);
    let canvas = app.canvas_bounds();
    assert!((canvas.x - at.0).abs() <= 1.0, "{canvas:?}");
    let kept = kept_entry(&app).expect("kept");
    assert_eq!(kept["shares"].as_array().map(Vec::len), Some(2), "{kept}");

    send(&mut app, pane(PaneMsg::EqualizeRequested));
    assert_eq!(app.ui_state.workspace_layouts.panes.shares, None);
    assert!((app.canvas_bounds().x - (area.x + (area.width / 2.0).floor())).abs() <= 1.0);
    assert!(kept_entry(&app).expect("kept").get("shares").is_none());
}

/// A tab let go over the canvas, which takes the release, still splits on
/// the edge it is over; over the tab strip the strip's own release handles
/// it and nothing changes here.
#[test]
fn a_tab_released_over_the_canvas_drops_on_the_pane_edge() {
    let (mut app, main) = app();
    let area = app.document_area_in(None);
    app.ui_state.tab_dragging = Some((0, area.x, area.y - 20.0));
    app.interaction_state.last_mouse_pos = (area.x + area.width * 0.5, area.y - 10.0);
    send(&mut app, pane(PaneMsg::TabReleased(main)));
    assert!(app.ui_state.tab_dragging.is_some(), "over the strip");
    assert!(app.tiled_pane_view().is_none());

    app.interaction_state.last_mouse_pos = (area.x + area.width * 0.95, area.y + area.height * 0.5);
    send(&mut app, pane(PaneMsg::TabReleased(main)));
    assert!(app.ui_state.tab_dragging.is_none());
    assert_eq!(app.tiled_pane_view().expect("split").tabs, [2, 0]);
}

/// The tile rows preview their panes: one per document, and Tile Grid's.
#[test]
fn the_tile_rows_preview_where_each_document_lands() {
    let (app, _) = app();
    let previews = app.tile_previews();
    let vertical = previews.vertical.expect("two documents or more");
    assert_eq!(vertical.layout, TileLayout::SideBySide);
    assert_eq!(vertical.panes.len(), 3);
    let title = vertical.title.replace(['\u{2068}', '\u{2069}'], "");
    assert_eq!(title, "Tile Vertically · 3 open documents");
    let grid = previews.grid.expect("grid");
    assert_eq!(
        grid.panes,
        [vec!["power.snxsch"], vec!["mcu.snxsch"], vec!["io.snxsch"]]
    );
    let ctx = crate::menu_bar::MenuContext {
        tile_previews: app.tile_previews(),
        ..crate::menu_bar::MenuContext::default()
    };
    let model = crate::menu_bar::model::menu_bar_model(&ctx);
    let view = model
        .menus
        .iter()
        .find(|m| m.title == "View")
        .expect("View");
    let window = view
        .entries
        .iter()
        .find_map(|entry| match entry {
            crate::menu_bar::model::MenuEntry::Submenu(menu) if menu.title == "Window" => {
                Some(menu)
            }
            _ => None,
        })
        .expect("Window");
    let grid_row = window
        .entries
        .iter()
        .find_map(|entry| match entry {
            crate::menu_bar::model::MenuEntry::Item(item) if item.label == "Tile Grid" => {
                Some(item)
            }
            _ => None,
        })
        .expect("Tile Grid");
    assert_eq!(
        grid_row.preview.as_ref().map(|p| p.layout),
        Some(TileLayout::Grid)
    );
}

/// Each pane has a stable id (mockup row 207): an earlier pane closing moves
/// the later panes' places, not the focus, so two views of one sheet keep
/// their own pan and zoom rather than swapping them.
#[test]
fn closing_an_earlier_pane_keeps_each_view_with_its_pane() {
    let (mut app, main) = app();
    let io = path("io.snxsch");
    // power alone on the left, then io's pane focused again.
    drop_tab(&mut app, main, 0, (0.05, 0.5));
    send(&mut app, Message::Window(WindowMsg::TilePanePressed(1)));
    assert_eq!(
        app.document_state.tabs[app.document_state.active_tab].path,
        io
    );
    // A second view of io on the right takes the focus.
    second_view(&mut app, main, 2);
    let view = app.tiled_pane_view().expect("three panes");
    assert_eq!((view.tabs.len(), view.focused), (3, 2));
    let focused_id = view.focused_id.expect("a split pane");
    let (left, right) = ((3.0, 4.0, 7.0), (12.0, 34.0, 5.0));
    app.interaction_state.canvas.set_pane_camera(right);
    app.interaction_state
        .tile_panes
        .schematic
        .get_mut(&io)
        .expect("the other view of io")
        .set_pane_camera(left);

    // Closing power's tab closes the first pane.
    let task = app.close_tab_now(0);
    settle_task(&mut app, task);
    let view = app.tiled_pane_view().expect("two panes");
    assert_eq!((view.tabs.len(), view.focused), (2, 1), "its place moved");
    assert_eq!(view.focused_id, Some(focused_id), "its id did not");
    let other = app.interaction_state.tile_panes.schematic[&io].live_camera();
    assert_eq!(other, left, "the other view kept its own view");
    assert_ne!(
        app.interaction_state.canvas.live_camera(),
        left,
        "the focused view did not take the other pane's"
    );
}
