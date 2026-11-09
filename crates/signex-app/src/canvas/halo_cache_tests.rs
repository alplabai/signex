//! The drag-time selection halos: the cached halo carried by the drag delta
//! is the halo lowered at the shifted sheet.

use signex_gfx::primitive::line::LineSegment;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_renderer::schematic::{SchematicRenderer, SchematicSnapshot, ViewRenderer};
use signex_renderer::theme::ResolvedTheme;
use signex_types::schematic::Aabb;

use super::halo_cache::{ShiftPlan, shift_plan};
use super::render_bench_tests::{Variant, generate};
use super::shift_selection::shift_snapshot_for_selection;
use super::tests::test_prefs;
use super::*;
use crate::schematic_runtime::selection::{overlay_inputs_for_boxes, selection_boxes};
use crate::schematic_runtime::{Designators, SchematicRenderCache, ScreenTransform, WireWidths};

const EPS_MM: f32 = 1e-3;

fn graphite_ink() -> GraphiteCanvasInk {
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    GraphiteCanvasInk::from_tokens(&tokens).expect("graphite")
}

fn colors() -> signex_types::theme::CanvasColors {
    signex_types::theme::canvas_colors(signex_types::theme::ThemeId::GraphiteLight)
}

fn transform() -> ScreenTransform {
    ScreenTransform {
        offset_x: 40.0,
        offset_y: 25.0,
        scale: 5.6,
        device_px_per_logical_px: 1.0,
    }
}

/// The overlay primitives `boxes` lower to.
fn lowered(boxes: &[Aabb], graphite: Option<&GraphiteCanvasInk>) -> Scene {
    let snapshot = SchematicSnapshot {
        overlays: overlay_inputs_for_boxes(boxes, &transform(), &colors(), graphite),
        ..Default::default()
    };
    let mut scene = Scene::default();
    SchematicRenderer::build_scene(
        &snapshot,
        &ResolvedTheme::from_canvas_colors(colors()),
        DirtyFlags::OVERLAY,
        &mut scene,
    );
    scene
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= EPS_MM
}

fn line_matches(shifted: &LineSegment, fresh: &LineSegment, d: (f32, f32)) -> bool {
    near(shifted.p0[0] + d.0, fresh.p0[0])
        && near(shifted.p0[1] + d.1, fresh.p0[1])
        && near(shifted.p1[0] + d.0, fresh.p1[0])
        && near(shifted.p1[1] + d.1, fresh.p1[1])
        && shifted.width == fresh.width
        && shifted.color == fresh.color
        && shifted.style == fresh.style
}

fn assert_translated(cached: &Scene, fresh: &Scene, d: (f32, f32)) {
    assert!(!fresh.overlay_lines.is_empty() || !fresh.overlay_polygons.is_empty());
    assert_eq!(cached.overlay_lines.len(), fresh.overlay_lines.len());
    for (a, b) in cached.overlay_lines.iter().zip(&fresh.overlay_lines) {
        assert!(line_matches(a, b, d));
    }
    assert_eq!(cached.overlay_polygons.len(), fresh.overlay_polygons.len());
    for (a, b) in cached.overlay_polygons.iter().zip(&fresh.overlay_polygons) {
        assert_eq!(a.fill_color, b.fill_color);
        assert_eq!(a.vertices.len(), b.vertices.len());
        for (p, q) in a.vertices.iter().zip(&b.vertices) {
            assert!(near(p[0] + d.0, q[0]) && near(p[1] + d.1, q[1]));
        }
    }
    assert_eq!(cached.overlay_circles.len(), fresh.overlay_circles.len());
    for (a, b) in cached.overlay_circles.iter().zip(&fresh.overlay_circles) {
        assert!(near(a.center[0] + d.0, b.center[0]) && near(a.center[1] + d.1, b.center[1]));
        assert_eq!(a.radius, b.radius);
    }
}

#[test]
fn a_cached_halo_carried_by_the_drag_delta_is_the_halo_at_the_shifted_sheet() {
    // Arrange: a block of parts, wires and labels dragged by an off-grid delta.
    let g = generate(6, 5, Variant::from_env());
    let (dx, dy) = (13.37, -7.91);
    let moved = shift_snapshot_for_selection(&g.sheet, &g.block, dx, dy);
    let designators = Designators::new();
    let widths = WireWidths::default();
    let base = selection_boxes(&g.sheet, &g.block, &designators, widths);
    let now = selection_boxes(&moved, &g.block, &designators, widths);

    // Act
    let plan = shift_plan(&base, &now);

    // Assert: every item took the one shift, so one cached halo set serves.
    assert!(plan.shifted.iter().all(|&s| s), "{:?}", plan.shifted);
    assert!((plan.shift.0 - dx).abs() < 1e-6 && (plan.shift.1 - dy).abs() < 1e-6);
    let boxes = |v: &[Option<Aabb>]| v.iter().flatten().copied().collect::<Vec<_>>();
    let ink = graphite_ink();
    for graphite in [Some(&ink), None] {
        let cached = lowered(&boxes(&base), graphite);
        let fresh = lowered(&boxes(&now), graphite);
        assert_translated(&cached, &fresh, (dx as f32, dy as f32));
    }
}

#[test]
fn an_item_that_changed_shape_or_was_held_is_not_carried_by_the_shift() {
    // Arrange
    let b = |x: f64, w: f64| {
        Some(Aabb {
            min_x: x,
            min_y: 0.0,
            max_x: x + w,
            max_y: 2.0,
        })
    };
    let base = [b(0.0, 4.0), b(10.0, 4.0), b(20.0, 4.0), b(30.0, 4.0), None];
    // Held still, moved by 5, stretched, moved by 5, gone.
    let now = [b(0.0, 4.0), b(15.0, 4.0), b(25.0, 9.0), b(35.0, 4.0), None];

    // Act
    let plan = shift_plan(&base, &now);

    // Assert: the held item does not decide the shift.
    assert_eq!(
        plan,
        ShiftPlan {
            shift: (5.0, 0.0),
            shifted: vec![false, true, false, true, false],
        }
    );
}

#[test]
fn the_halos_are_lowered_once_per_selection_zoom_and_theme_not_per_drag_step() {
    // Arrange
    let g = generate(6, 5, Variant::from_env());
    let overrides = Default::default();
    let filters = Default::default();
    let mut slot = CanvasSlot::new();
    slot.set_render_cache(Some(SchematicRenderCache::from_sheet_at(&g.sheet, 3)));
    slot.selected = g.block.clone();
    *slot.camera_mut() = Camera {
        offset: iced::Point::new(10.0, 10.0),
        scale: 5.6,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
    let renderer = crate::layout_probe::headless_renderer();
    let frame = |slot: &CanvasSlot, dx: f64, graphite: bool| {
        let mut prefs = test_prefs(&overrides, &filters);
        prefs.graphite = graphite.then(graphite_ink);
        let moved = shift_snapshot_for_selection(&g.sheet, &slot.selected, dx, dx * 0.5);
        let drag = SchematicCanvas::new(slot, prefs)
            .draw_drag_selection(&renderer, bounds, &moved)
            .expect("known revision");
        drag.halos.map(|(_, shift)| shift)
    };

    // Act
    let first = frame(&slot, 3.0, true);
    let second = frame(&slot, 9.0, true);
    let after_steps = slot.halo_tessellations();
    let _ = frame(&slot, 9.0, false);
    let after_theme = slot.halo_tessellations();
    slot.selected.pop();
    let _ = frame(&slot, 9.0, false);
    let after_selection = slot.halo_tessellations();

    // Assert
    assert_eq!(after_steps, 1, "two drag steps tessellate the halos once");
    assert_ne!(first, second, "the cached halos are drawn under the delta");
    let (a, b) = (first.expect("halos"), second.expect("halos"));
    assert!((a.x - 3.0 * 5.6).abs() < 1e-3 && (b.x - 9.0 * 5.6).abs() < 1e-3);
    assert_eq!(after_theme, 2, "a theme change lowers them again");
    assert_eq!(after_selection, 3, "so does a new selection");
}
