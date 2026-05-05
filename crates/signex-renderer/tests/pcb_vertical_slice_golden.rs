//! Deterministic fixture and golden checks for Milestone B vertical slice 01.

use serde::Deserialize;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_renderer::pcb::{DrcMarkerInput, PcbRenderer, PcbSnapshot, RatsnestInput};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;
use signex_types::pcb::PcbBoard;
use signex_types::violation::Severity;

#[derive(Debug, Deserialize)]
struct PcbVerticalSliceGolden {
    lines: usize,
    circles: usize,
    polygons: usize,
}

fn fixture_board() -> PcbBoard {
    serde_json::from_str(include_str!("fixtures/pcb_vertical_slice_fixture.json"))
        .expect("valid PCB vertical slice fixture")
}

fn golden_baseline() -> PcbVerticalSliceGolden {
    serde_json::from_str(include_str!("golden/pcb_vertical_slice_golden.json"))
        .expect("valid PCB vertical slice golden")
}

#[test]
fn pcb_vertical_slice_matches_golden_counts() {
    let board = fixture_board();
    let golden = golden_baseline();
    let snapshot = PcbSnapshot::from_board(&board);
    let theme = ResolvedTheme::builtin_default();
    let mut scene = Scene::default();

    PcbRenderer::build_scene(
        &snapshot,
        &theme,
        DirtyFlags::LINES | DirtyFlags::CIRCLES | DirtyFlags::POLYGONS,
        &mut scene,
    );

    assert_eq!(scene.lines.len(), golden.lines);
    assert_eq!(scene.circles.len(), golden.circles);
    assert_eq!(scene.polygons.len(), golden.polygons);
}

#[test]
fn pcb_vertical_slice_dirty_paths_are_family_scoped() {
    let board = fixture_board();
    let snapshot = PcbSnapshot::from_board(&board);
    let theme = ResolvedTheme::builtin_default();
    let mut scene = Scene::default();

    PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::LINES, &mut scene);
    assert_eq!(scene.lines.len(), 2);
    assert_eq!(scene.circles.len(), 0);
    assert_eq!(scene.polygons.len(), 0);

    PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::CIRCLES, &mut scene);
    assert_eq!(scene.lines.len(), 2);
    assert_eq!(scene.circles.len(), 1);
    assert_eq!(scene.polygons.len(), 0);

    PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::POLYGONS, &mut scene);
    assert_eq!(scene.lines.len(), 2);
    assert_eq!(scene.circles.len(), 1);
    assert_eq!(scene.polygons.len(), 4);
}

#[test]
fn pcb_vertical_slice_overlay_paths_emit_ratsnest_and_drc() {
    let board = fixture_board();
    let snapshot = PcbSnapshot::from_board(&board)
        .with_ratsnest_lines(vec![RatsnestInput {
            p0: [0.0, 0.0],
            p1: [3.0, 1.5],
            net: 1,
        }])
        .with_drc_markers(vec![
            DrcMarkerInput {
                center: [8.0, 3.0],
                radius_mm: 0.35,
                severity: Severity::Error,
                violation_type: None,
            },
            DrcMarkerInput {
                center: [6.0, 2.0],
                radius_mm: 0.3,
                severity: Severity::Warning,
                violation_type: None,
            },
        ]);

    let theme = ResolvedTheme::builtin_default();
    let mut scene = Scene::default();

    PcbRenderer::build_scene(&snapshot, &theme, DirtyFlags::OVERLAY, &mut scene);

    assert_eq!(scene.overlay_lines.len(), 3);
    assert_eq!(scene.overlay_circles.len(), 2);
    assert_eq!(scene.overlay_polygons.len(), 2);
}
