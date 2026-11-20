//! Graphite mockup #55 — the Filter panel on a schematic tab edits the filter a box select
//! picks with: Wires toggled off there, a box over two wires selects none.

use signex_app::active_bar::SelectionFilter;
use signex_app::app::{Message, Signex, TabInfo, TabKind};
use signex_app::canvas::{BoxDirection, CanvasEvent};
use signex_app::dock::DockMessage;
use signex_app::panels::{PanelMsg, SchFilterMsg};
use signex_types::schematic::{Point, SchematicSheet, Wire};
use std::path::PathBuf;

fn wire(a: (f64, f64), b: (f64, f64)) -> Wire {
    Wire {
        color: None,
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        stroke_width: 0.0,
        locked: false,
    }
}

fn app_with_wires() -> Signex {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        wires: vec![wire((5.0, 5.0), (15.0, 5.0)), wire((5.0, 8.0), (15.0, 8.0))],
        ..Default::default()
    };
    let path = PathBuf::from("filter-panel.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet.clone()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "filter-panel".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    app
}

fn box_select(app: &mut Signex) -> usize {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::BoxSelect {
        x1: 0.0,
        y1: 0.0,
        x2: 30.0,
        y2: 10.0,
        direction: BoxDirection::LeftToRight,
    }));
    app.interaction_state.active_canvas().selected.len()
}

#[test]
fn wires_off_in_the_filter_panel_keeps_a_box_off_the_wires() {
    let mut app = app_with_wires();
    assert_eq!(box_select(&mut app), 2, "fixture: the box takes both wires");

    let toggled = PanelMsg::SchFilter(SchFilterMsg::KindToggled(SelectionFilter::Wires));
    let _ = app.update(Message::Dock(DockMessage::Panel(toggled)));
    assert!(
        !app.interaction_state
            .selection_filters
            .contains(&SelectionFilter::Wires)
    );
    assert_eq!(box_select(&mut app), 0);
}
