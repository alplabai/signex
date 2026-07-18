//! Integration test for PCB app-event to dirty-flag routing.

use signex_gfx::scene::{DirtyFlags, Scene};
use signex_renderer::pcb::{
    DrcMarkerInput, PcbAppEvent, PcbRenderer, PcbSnapshot, RatsnestInput, dirty_flags_for_events,
};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;
use signex_types::pcb::PcbBoard;
use signex_types::violation::Severity;

fn fixture_board() -> PcbBoard {
    serde_json::from_str(include_str!("fixtures/pcb_vertical_slice_fixture.json"))
        .expect("valid PCB vertical slice fixture")
}

#[test]
fn pcb_event_flow_updates_only_expected_scene_families() {
    let board = fixture_board();
    let snapshot = PcbSnapshot::from_board(&board)
        .with_ratsnest_lines(vec![RatsnestInput {
            p0: [2.0, 2.0],
            p1: [7.0, 4.0],
            net: 2,
        }])
        .with_drc_markers(vec![DrcMarkerInput {
            center: [7.0, 4.0],
            radius_mm: 0.3,
            severity: Severity::Error,
            violation_type: None,
        }]);

    let theme = ResolvedTheme::builtin_default();
    let mut scene = Scene::default();

    let dirty = dirty_flags_for_events(&[
        PcbAppEvent::TraceEdited,
        PcbAppEvent::ZoneRefilled,
        PcbAppEvent::DrcResultsUpdated,
    ]);

    assert!(dirty.contains(DirtyFlags::LINES));
    assert!(dirty.contains(DirtyFlags::POLYGONS));
    assert!(dirty.contains(DirtyFlags::OVERLAY));
    assert!(!dirty.contains(DirtyFlags::CIRCLES));
    assert!(!dirty.contains(DirtyFlags::TEXT));

    PcbRenderer::build_scene(&snapshot, &theme, dirty, &mut scene);

    assert_eq!(scene.lines.len(), 2);
    assert_eq!(scene.circles.len(), 0);
    assert_eq!(scene.polygons.len(), 4);
    assert_eq!(scene.overlay_lines.len(), 2);
    assert_eq!(scene.overlay_circles.len(), 1);
    assert_eq!(scene.overlay_polygons.len(), 1);
}

#[test]
fn camera_only_event_does_not_request_geometry_uploads() {
    let dirty = dirty_flags_for_events(&[PcbAppEvent::CameraMoved]);
    assert_eq!(dirty, DirtyFlags::empty());
}
