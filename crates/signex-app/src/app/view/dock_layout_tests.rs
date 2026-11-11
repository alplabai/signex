//! #2262: the main window's dock layout. The side docks run the full height;
//! the bottom dock sits under the canvas only; a side dock's tab strip shows
//! every tab inside the dock.

use crate::app::{Signex, TabInfo, TabKind};
use crate::dock::PanelPosition;
use crate::panels::PanelKind;
use signex_i18n::t;
use signex_types::schematic::SchematicSheet;

const WINDOW: iced::Size = iced::Size::new(1440.0, 900.0);
const LEFT_W: f32 = 240.0;
const RIGHT_W: f32 = 220.0;

/// A sheet tab open in the main window over the fresh Graphite dock layout.
fn app_with_sheet() -> (Signex, iced::window::Id) {
    let path = std::path::PathBuf::from("dock_layout.snxsch");
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "dock_layout".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.document_state.dock = crate::dock::graphite_first_run();
    app.ui_state.left_width = LEFT_W;
    app.ui_state.right_width = RIGHT_W;
    app.ui_state.bottom_height = crate::dock::GRAPHITE_BOTTOM_HEIGHT;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    (app, main)
}

fn label(app: &Signex, kind: PanelKind) -> String {
    t!(app.ui_state.i18n, kind.message()).to_string()
}

#[test]
fn the_bottom_dock_sits_under_the_canvas_and_the_side_docks_run_full_height() {
    let (app, main) = app_with_sheet();
    let messages = label(&app, PanelKind::Messages);
    let projects = label(&app, PanelKind::Projects);
    let texts = crate::layout_probe::lay_out_texts(app.view(main), WINDOW);
    let messages_rects: Vec<_> = texts.iter().filter(|t| t.text == messages).collect();
    let projects_rects: Vec<_> = texts.iter().filter(|t| t.text == projects).collect();
    assert!(!messages_rects.is_empty(), "the Messages dock is drawn");
    assert!(!projects_rects.is_empty(), "the Projects dock is drawn");
    for rect in &messages_rects {
        let b = rect.bounds;
        assert!(
            b.x >= LEFT_W,
            "Messages starts right of the left dock: {b:?}"
        );
        assert!(
            b.x + b.width <= WINDOW.width - RIGHT_W,
            "Messages ends left of the right dock: {b:?}"
        );
    }
    let bottom_dock_top = messages_rects
        .iter()
        .map(|t| t.bounds.y)
        .fold(f32::INFINITY, f32::min);
    let left_dock_bottom = projects_rects
        .iter()
        .map(|t| t.bounds.y + t.bounds.height)
        .fold(0.0, f32::max);
    assert!(
        left_dock_bottom > bottom_dock_top,
        "the left dock's tab strip ({left_dock_bottom}) runs past the top of the \
         bottom dock ({bottom_dock_top})"
    );
}

#[test]
fn a_crowded_side_dock_shows_every_tab_inside_the_dock() {
    let (mut app, main) = app_with_sheet();
    app.document_state
        .dock
        .add_panel(PanelPosition::Left, PanelKind::Library);
    let kinds = app
        .document_state
        .dock
        .panel_kinds(PanelPosition::Left)
        .to_vec();
    assert_eq!(kinds.len(), 4, "{kinds:?}");
    let names: Vec<String> = kinds.iter().map(|k| label(&app, *k)).collect();
    let texts = crate::layout_probe::lay_out_texts(app.view(main), WINDOW);
    for name in &names {
        // The tab is the lowest text of that name in the left dock.
        let tab = texts
            .iter()
            .filter(|t| &t.text == name && t.bounds.x < LEFT_W)
            .max_by(|a, b| a.bounds.y.total_cmp(&b.bounds.y))
            .unwrap_or_else(|| panic!("no {name} tab in the left dock"));
        assert!(
            tab.bounds.x + tab.bounds.width <= LEFT_W + 0.5,
            "the {name} tab runs past the dock edge: {:?}",
            tab.bounds
        );
    }
}
