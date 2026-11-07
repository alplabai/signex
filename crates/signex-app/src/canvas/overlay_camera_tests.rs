//! The selection layer (halo, ERC markers, lock badges) is drawn in screen
//! space and cached. It must be drawn again at every camera, whichever message
//! moved it: the zoom pill sends View > Zoom In, which clears only the
//! background and content caches, and left the halo at its old place and size.

use iced::Rectangle;
use iced::widget::Widget as _;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind, Wire};
use signex_types::theme::ThemeId;

use super::*;
use crate::app::Signex;
use crate::menu_bar::MenuMessage;

const BOUNDS: Rectangle = Rectangle {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 600.0,
};

/// A Graphite app showing a one-wire schematic in its main window, the wire
/// selected and the canvas laid out at `BOUNDS`.
pub(crate) fn graphite_app_with_selected_wire() -> (Signex, iced::window::Id) {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.theme_id = ThemeId::GraphiteLight;
    app.ui_state.preferences_draft_theme = ThemeId::GraphiteLight;
    app.sync_panel_theme();

    let wire = Wire {
        uuid: uuid::Uuid::now_v7(),
        start: signex_types::schematic::Point::new(20.0, 20.0),
        end: signex_types::schematic::Point::new(60.0, 20.0),
        stroke_width: 0.25,
        unknown_columns: Default::default(),
        locked: false,
    };
    let selected = SelectedItem::new(wire.uuid, SelectedKind::Wire);
    let sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        wires: vec![wire],
        ..Default::default()
    };
    let path = std::path::PathBuf::from("halo-camera.snxsch");
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet.clone()).expect("engine"),
    );
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "halo-camera".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    let canvas = app.interaction_state.active_canvas_mut();
    canvas.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet_at(&sheet, 1),
    ));
    canvas.selected = vec![selected];
    canvas.set_viewport(BOUNDS);
    (app, main)
}

/// One frame of the canvas; how often the selection layer was tessellated.
fn selection_draws(app: &Signex, main: iced::window::Id) -> u32 {
    let slot = app.interaction_state.active_canvas();
    let before = slot.overlay_tessellations.get();
    let renderer = crate::layout_probe::headless_renderer();
    let _layers = SchematicCanvas::new(slot, app.canvas_view_prefs(main)).draw_surface(
        &CanvasState::default(),
        &renderer,
        BOUNDS,
        mouse::Cursor::Unavailable,
        true,
    );
    slot.overlay_tessellations.get() - before
}

fn camera(app: &Signex) -> (f32, f32, f32) {
    app.interaction_state.active_canvas().live_camera()
}

#[test]
fn the_halo_is_drawn_again_after_a_zoom_pill_zoom() {
    let (mut app, main) = graphite_app_with_selected_wire();
    assert!(
        app.canvas_view_prefs(main).graphite.is_some(),
        "a Graphite halo"
    );
    assert_eq!(selection_draws(&app, main), 1, "first frame draws");
    assert_eq!(
        selection_draws(&app, main),
        0,
        "an idle frame reuses the layer"
    );

    for zoom in [MenuMessage::ZoomIn, MenuMessage::ZoomOut] {
        let before = camera(&app);
        let _ = app.update(crate::app::Message::Menu(zoom.clone()));
        assert_ne!(camera(&app), before, "{zoom:?} moved the camera");
        assert_eq!(
            selection_draws(&app, main),
            1,
            "{zoom:?}: the halo stayed at the old camera"
        );
    }
}

/// A fit lands at the next canvas event and publishes nothing the overlay
/// cache listens to before the frame is drawn.
#[test]
fn the_halo_follows_a_fit_resolved_by_the_canvas() {
    let (mut app, main) = graphite_app_with_selected_wire();
    assert_eq!(selection_draws(&app, main), 1);
    let _ = app.update(crate::app::Message::CanvasEvent(CanvasEvent::FitAll));
    let before = camera(&app);
    assert!(
        app.interaction_state
            .active_canvas()
            .resolve_pending_fit(BOUNDS)
    );
    assert_ne!(camera(&app), before, "the fit moved the camera");
    assert_eq!(
        selection_draws(&app, main),
        1,
        "the halo stayed at the old fit"
    );
}

/// The camera key adds no tessellation to a frame whose camera did not move.
#[test]
fn an_unmoved_camera_keeps_the_cached_halo() {
    let (app, main) = graphite_app_with_selected_wire();
    assert_eq!(selection_draws(&app, main), 1);
    for _ in 0..3 {
        assert_eq!(selection_draws(&app, main), 0);
    }
}

/// A wheel zoom through the canvas widget moves the camera, and the next frame
/// draws the halo at it before any message is handled.
#[test]
fn the_halo_follows_a_wheel_zoom() {
    let (app, main) = graphite_app_with_selected_wire();
    assert_eq!(selection_draws(&app, main), 1);
    let before = camera(&app);
    let slot = app.interaction_state.active_canvas();
    let surface = SchematicSurface::new(SchematicCanvas::new(slot, app.canvas_view_prefs(main)));
    let messages = crate::layout_probe::messages_on_scroll_at(
        surface.boxed(),
        BOUNDS.size(),
        BOUNDS.center(),
        mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
    );
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::CanvasEvent(CanvasEvent::ViewMoved))),
        "the wheel zoomed"
    );
    assert_ne!(camera(&app), before);
    assert_eq!(selection_draws(&app, main), 1);
}
