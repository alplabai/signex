//! #1256 — the Measure tool (`measure_distance`, Ctrl+M) and zoom to the
//! selection (`zoom_to_selection_area`), driven through `Signex::update`.

use signex_app::app::{Message, Signex, TabInfo, TabKind, Tool, UiMsg};
use signex_app::canvas::CanvasEvent;
use signex_app::keymap::{KeyStroke, KeyToken, Modifiers};
use signex_types::coord::Unit;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Wire};
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

/// An app on the Altium keymap holding `wires`, 1 mm grid, nothing selected.
fn app_with(wires: Vec<Wire>) -> Signex {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        wires,
        ..Default::default()
    };
    let path = PathBuf::from("measure-and-zoom.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.grid_size_mm = 1.0;
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "measure-and-zoom".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    let sheet = app.document_state.engines[&path].document().clone();
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    app
}

fn ctrl_m() -> Message {
    Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: KeyStroke {
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::default()
            },
            key: KeyToken::Character("m".into()),
        },
    })
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

fn logged(fragment: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(fragment))
}

#[test]
fn ctrl_m_arms_the_measure_tool() {
    let mut app = app_with(vec![]);
    let _ = app.update(ctrl_m());
    assert_eq!(app.interaction_state.current_tool, Tool::Measure);
}

#[test]
fn two_clicks_measure_dx_dy_and_distance_without_touching_the_document() {
    let mut app = app_with(vec![wire((0.0, 0.0), (5.0, 0.0))]);
    let _ = signex_app::diagnostics::init_logging();
    let before = app
        .document_state
        .active_engine()
        .unwrap()
        .document()
        .clone();
    let _ = app.update(ctrl_m());

    click(&mut app, 1.0, 1.0);
    assert!(app.interaction_state.active_canvas().measure.is_none());
    assert!(app.interaction_state.active_canvas().shape_anchor.is_some());
    click(&mut app, 4.0, 5.0);

    let result = app
        .interaction_state
        .active_canvas()
        .measure
        .clone()
        .unwrap();
    assert_eq!(
        (result.a, result.b),
        (Point::new(1.0, 1.0), Point::new(4.0, 5.0))
    );
    assert!(logged("dx 3.000 mm dy 4.000 mm dist 5.000 mm"));

    let engine = app.document_state.active_engine().unwrap();
    assert_eq!(engine.document().wires.len(), before.wires.len());
    assert!(!engine.can_undo(), "measuring must not add an undo entry");
    assert!(
        !app.document_state
            .is_document_dirty(&app.document_state.tabs[0].path)
    );
}

#[test]
fn the_result_follows_the_active_unit() {
    let mut app = app_with(vec![]);
    let _ = signex_app::diagnostics::init_logging();
    app.ui_state.unit = Unit::Mil;
    let _ = app.update(ctrl_m());
    click(&mut app, 0.0, 0.0);
    click(&mut app, 2.0, 0.0);
    assert!(logged("dx 78.74 mil dy 0.00 mil dist 78.74 mil"));
}

#[test]
fn a_third_click_starts_a_new_measurement_and_esc_cancels() {
    let mut app = app_with(vec![]);
    let _ = app.update(ctrl_m());
    click(&mut app, 0.0, 0.0);
    click(&mut app, 3.0, 0.0);
    click(&mut app, 10.0, 10.0);
    assert!(app.interaction_state.active_canvas().measure.is_none());
    assert!(app.interaction_state.active_canvas().shape_anchor.is_some());

    let _ = app.update(Message::Input(signex_app::app::InputMsg::EscapePressed {
        window: None,
    }));
    let canvas = app.interaction_state.active_canvas();
    assert!(canvas.shape_anchor.is_none() && canvas.measure.is_none());
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
}

#[test]
fn measure_clicks_snap_to_the_grid() {
    let mut app = app_with(vec![]);
    app.ui_state.snap_enabled = true;
    let _ = app.update(ctrl_m());
    click(&mut app, 0.2, 0.2);
    click(&mut app, 3.4, 4.1);
    let result = app
        .interaction_state
        .active_canvas()
        .measure
        .clone()
        .unwrap();
    assert_eq!(
        (result.a, result.b),
        (Point::new(0.0, 0.0), Point::new(3.0, 4.0))
    );
}

#[test]
fn zoom_to_selection_frames_only_the_selected_items() {
    let near = wire((10.0, 10.0), (30.0, 10.0));
    let far = wire((100.0, 80.0), (100.0, 90.0));
    let selected = vec![SelectedItem::new(near.uuid, SelectedKind::Wire)];
    let mut app = app_with(vec![near, far]);
    app.interaction_state.active_canvas_mut().selected = selected;

    let _ = app.update(Message::CanvasEvent(CanvasEvent::FitSelection));

    let fit = app
        .interaction_state
        .active_canvas()
        .pending_fit
        .get()
        .unwrap();
    // A 20 mm wire plus its stroke and the 2 mm margin each side; the far
    // wire at x = 100 is not in the frame. 16 mm is the minimum side.
    assert!(fit.width > 24.0 && fit.width < 26.0, "{fit:?}");
    assert_eq!(fit.height, 16.0);
    assert!((fit.x + fit.width / 2.0 - 20.0).abs() < 0.01, "{fit:?}");
    assert!((fit.y + fit.height / 2.0 - 10.0).abs() < 0.01, "{fit:?}");
}

#[test]
fn zoom_to_selection_with_nothing_selected_is_a_logged_no_op() {
    let mut app = app_with(vec![wire((0.0, 0.0), (5.0, 0.0))]);
    let _ = signex_app::diagnostics::init_logging();
    let _ = app.update(Message::CanvasEvent(CanvasEvent::FitSelection));
    assert!(
        app.interaction_state
            .active_canvas()
            .pending_fit
            .get()
            .is_none()
    );
    assert!(logged("Zoom to selection: nothing is selected"));
}
