//! Mockup row 207 in the window (#1307): a document tab let go on an edge of
//! the document area splits there, as a second view while Ctrl is held; a
//! press on the other pane moves the focus; the split is kept for the
//! project, comes back once it is reopened and is forgotten on Untile. The
//! kept splits go to a recorder, never to `prefs.json`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::app::handlers::menu::export::tests as fx;
use crate::app::{InputMsg, Message, Signex, TabInfo, TabKind, TileLayout, UiMsg, WindowMsg};
use crate::document_panes::DocumentPanesStore;

const DIR: &str = "/w/panes";
const SHEETS: [&str; 3] = ["power.snxsch", "mcu.snxsch", "io.snxsch"];

#[derive(Default)]
struct Recorder {
    writes: Mutex<Vec<serde_json::Value>>,
}

impl DocumentPanesStore for Recorder {
    fn write(&self, value: &serde_json::Value) -> Result<(), String> {
        self.writes.lock().unwrap().push(value.clone());
        Ok(())
    }
}

fn path(name: &str) -> PathBuf {
    PathBuf::from(DIR).join(name)
}

/// Three sheets of one project in the main window, `io` active, the main
/// window known and laid out.
fn app() -> (Signex, iced::window::Id) {
    let mut app = fx::app_workspace(DIR, &SHEETS);
    let project = app.document_state.projects[0].id;
    for name in SHEETS {
        fx::open_with(
            &mut app.document_state,
            &path(name),
            fx::sheet_with_net("R1", "N1", &[]),
        );
        app.document_state.tabs.push(TabInfo {
            title: name.to_string(),
            path: path(name),
            project_id: Some(project),
            kind: TabKind::Schematic,
        });
    }
    app.document_state.active_tab = 2;
    app.document_state.active_path = Some(path("io.snxsch"));
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.window_size = (1600.0, 1000.0);
    let store: Arc<dyn DocumentPanesStore> = Arc::new(Recorder::default());
    app.ui_state.workspace_layouts.panes.store = store;
    (app, main)
}

/// Drag tab `tab` from the strip and let go at the share `(fx, fy)` of the
/// document area.
fn drop_tab(app: &mut Signex, main: iced::window::Id, tab: usize, at: (f32, f32)) {
    let area = app.document_area_in(None);
    app.ui_state.tab_dragging = Some((tab, area.x, area.y - 20.0));
    app.interaction_state.last_mouse_pos =
        (area.x + area.width * at.0, area.y + area.height * at.1);
    let _ = app.update(Message::Ui(UiMsg::DragEnd(main)));
}

fn hold_ctrl(app: &mut Signex, main: iced::window::Id, held: bool) {
    let modifiers = if held {
        iced::keyboard::Modifiers::CTRL
    } else {
        iced::keyboard::Modifiers::empty()
    };
    let _ = app.update(Message::Input(InputMsg::KeyInput {
        window: main,
        event: iced::keyboard::Event::ModifiersChanged(modifiers),
    }));
}

fn kept_entry(app: &Signex) -> Option<serde_json::Value> {
    let key = path("ProjectA.snxprj").display().to_string();
    app.ui_state.workspace_layouts.panes.kept.get(&key).cloned()
}

/// Decided as drawn: a drop on the right edge moves the tab into a new pane
/// there, which has the focus; the others stay in the first pane. The drop
/// is kept for the project.
#[test]
fn a_tab_dropped_on_the_right_edge_moves_into_a_new_pane() {
    let (mut app, main) = app();
    drop_tab(&mut app, main, 0, (0.95, 0.5));
    let view = app.tiled_pane_view().expect("split");
    assert_eq!(view.layout, TileLayout::SideBySide);
    assert_eq!(
        view.tabs,
        [2, 0],
        "power moved right; the left pane keeps showing io, which was active"
    );
    assert_eq!(view.focused, 1);
    assert_eq!(
        app.document_state.active_tab, 0,
        "the dropped tab is active"
    );
    assert!(app.ui_state.tab_dragging.is_none());
    let kept = kept_entry(&app).expect("kept for the project");
    assert_eq!(kept["direction"], "side_by_side");
    assert_eq!(kept["panes"][1]["tabs"][0]["path"], "power.snxsch");

    // The middle keeps the tab where it is; nothing changes.
    drop_tab(&mut app, main, 2, (0.5, 0.5));
    assert_eq!(app.tiled_pane_view().expect("split").tabs, [2, 0]);
}

/// With Ctrl held the drop opens a second view: the active sheet is drawn
/// in both panes, the left one on a pane canvas of its own, and a press on
/// the left pane moves the focus without changing the active tab.
#[test]
fn a_ctrl_drop_opens_a_second_view_and_a_press_moves_the_focus() {
    let (mut app, main) = app();
    hold_ctrl(&mut app, main, true);
    drop_tab(&mut app, main, 2, (0.95, 0.5));
    hold_ctrl(&mut app, main, false);
    let view = app.tiled_pane_view().expect("split");
    assert_eq!((view.tabs.clone(), view.focused), (vec![2, 2], 1));
    assert!(
        app.interaction_state
            .tile_panes
            .schematic
            .contains_key(&path("io.snxsch")),
        "the unfocused view draws on its own canvas"
    );

    let _ = app.update(Message::Window(WindowMsg::TilePanePressed(0)));
    let view = app.tiled_pane_view().expect("split");
    assert_eq!(view.focused, 0);
    assert_eq!(app.document_state.active_tab, 2);
}

/// A drop across the panes is not offered; with no drop target nothing
/// splits.
#[test]
fn a_drop_across_the_panes_is_not_offered() {
    let (mut app, main) = app();
    drop_tab(&mut app, main, 0, (0.95, 0.5));
    app.ui_state.tab_dragging = Some((1, 0.0, 0.0));
    let area = app.document_area_in(None);
    app.interaction_state.last_mouse_pos = (area.x + area.width * 0.5, area.y + area.height * 0.97);
    assert_eq!(app.pane_drop_target(), None, "the bottom edge runs across");
    let _ = app.update(Message::Ui(UiMsg::DragEnd(main)));
    assert_eq!(app.tiled_pane_view().expect("split").tabs.len(), 2);
}

/// The kept split comes back when the project's documents are open again,
/// once; Untile merges the panes and forgets it.
#[test]
fn the_kept_split_comes_back_once_and_untile_forgets_it() {
    let (mut first, main) = app();
    drop_tab(&mut first, main, 0, (0.95, 0.5));
    let kept = first.ui_state.workspace_layouts.panes.kept.clone();

    let (mut reopened, _) = app();
    reopened.ui_state.workspace_layouts.panes.kept = kept;
    reopened.settle_document_tiling();
    let view = reopened.tiled_pane_view().expect("the split came back");
    assert_eq!(view.tabs, [2, 0], "io stays left as the active tab");
    assert_eq!(view.focused, 0);

    let _ = reopened.update(Message::Window(WindowMsg::UntileRequested));
    assert!(reopened.tiled_pane_view().is_none());
    assert_eq!(kept_entry(&reopened), None, "Untile forgets the split");
    reopened.settle_document_tiling();
    assert!(
        reopened.tiled_pane_view().is_none(),
        "not brought back again"
    );
}

/// Closing the only tab of the right pane ends the split, but the project
/// keeps it for the next time.
#[test]
fn closing_a_panes_only_tab_ends_the_split_and_keeps_it() {
    let (mut app, main) = app();
    drop_tab(&mut app, main, 0, (0.95, 0.5));
    app.document_state.tabs.remove(0);
    app.document_state.active_tab = 0;
    app.settle_document_tiling();
    assert!(app.tiled_pane_view().is_none());
    assert_eq!(app.ui_state.document_tiling, None);
    assert!(kept_entry(&app).is_some());
}
