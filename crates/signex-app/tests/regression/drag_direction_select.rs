//! #1257 — a selection box dragged left to right selects what it fully
//! contains, right to left what it touches. "Select by drag direction"
//! (default on) turns that on; off, the global Inside / Touching mode decides
//! for both directions.

use signex_app::app::{Message, Signex, TabInfo, TabKind};
use signex_app::canvas::{BoxDirection, CanvasEvent};
use signex_app::schematic_runtime::hit_test::SelectionMode;
use signex_types::schematic::{Point, SchematicSheet, Wire};
use std::path::PathBuf;

fn wire(a: (f64, f64), b: (f64, f64)) -> Wire {
    Wire {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        stroke_width: 0.0,
        locked: false,
    }
}

/// `inside` lies wholly in the box x 0..30, y 0..10; `crossing` pokes out of it.
fn app_with_wires() -> (Signex, uuid::Uuid, uuid::Uuid) {
    let inside = wire((5.0, 5.0), (15.0, 5.0));
    let crossing = wire((20.0, 5.0), (60.0, 5.0));
    let (inside_id, crossing_id) = (inside.uuid, crossing.uuid);
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        wires: vec![inside, crossing],
        ..Default::default()
    };
    let path = PathBuf::from("drag-direction.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet.clone()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "drag-direction".to_string(),
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
    (app, inside_id, crossing_id)
}

fn drag_box(app: &mut Signex, direction: BoxDirection) -> Vec<uuid::Uuid> {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::BoxSelect {
        x1: 0.0,
        y1: 0.0,
        x2: 30.0,
        y2: 10.0,
        direction,
    }));
    app.interaction_state
        .active_canvas()
        .selected
        .iter()
        .map(|i| i.uuid)
        .collect()
}

#[test]
fn the_preference_defaults_on() {
    let (app, _, _) = app_with_wires();
    assert!(app.ui_state.drag_direction_select);
}

#[test]
fn left_to_right_takes_only_what_the_box_contains() {
    let (mut app, inside, _) = app_with_wires();
    assert_eq!(drag_box(&mut app, BoxDirection::LeftToRight), vec![inside]);
}

#[test]
fn right_to_left_takes_everything_the_box_touches() {
    let (mut app, inside, crossing) = app_with_wires();
    let mut got = drag_box(&mut app, BoxDirection::RightToLeft);
    got.sort();
    let mut want = vec![inside, crossing];
    want.sort();
    assert_eq!(got, want);
}

#[test]
fn with_the_preference_off_the_global_mode_decides_for_both_directions() {
    let (mut app, inside, crossing) = app_with_wires();
    app.ui_state.drag_direction_select = false;

    app.ui_state.selection_mode = SelectionMode::Inside;
    assert_eq!(drag_box(&mut app, BoxDirection::RightToLeft), vec![inside]);

    app.ui_state.selection_mode = SelectionMode::Touching;
    let got = drag_box(&mut app, BoxDirection::LeftToRight);
    assert!(got.contains(&inside) && got.contains(&crossing));
}

#[test]
fn the_preference_round_trips_and_a_missing_key_is_on() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    assert!(signex_app::fonts::read_drag_direction_select_pref_at(&path));
    signex_app::fonts::write_drag_direction_select_pref_at(&path, false);
    assert!(!signex_app::fonts::read_drag_direction_select_pref_at(
        &path
    ));
    signex_app::fonts::write_drag_direction_select_pref_at(&path, true);
    assert!(signex_app::fonts::read_drag_direction_select_pref_at(&path));
    std::fs::write(&path, r#"{"drag_direction_select": "no"}"#).unwrap();
    assert!(signex_app::fonts::read_drag_direction_select_pref_at(&path));
}
