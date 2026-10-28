//! Tests for the upload tracker: dirty-only uploads, edge culling, re-cull on pan.
use super::tracker::{CULL_MARGIN_FRACTION, GEOMETRY_FLAGS, UploadTracker};
use super::{
    SceneUploadTarget, TextUploadParams, UploadCounters, UploadCulling, ViewportAabbMm,
    apply_dirty_uploads_with_culling,
};
use crate::primitive::arc::Arc as GpuArc;
use crate::primitive::circle::Circle;
use crate::primitive::line::LineSegment;
use crate::primitive::ordered::OrderedItem;
use crate::primitive::polygon::GpuPolygon;
use crate::primitive::text::TextItem;
use crate::scene::{DirtyFlags, Scene};
use std::sync::Arc;

#[derive(Default)]
struct CountingTarget {
    lines: usize,
    circles: usize,
    drawings: Option<usize>,
}

impl SceneUploadTarget for CountingTarget {
    type TextError = ();
    fn upload_lines(&mut self, lines: &[LineSegment]) {
        self.lines = lines.len();
    }
    fn upload_circles(&mut self, circles: &[Circle]) {
        self.circles = circles.len();
    }
    fn upload_arcs(&mut self, _: &[GpuArc]) {}
    fn upload_polygons(&mut self, _: &[GpuPolygon]) {}
    fn upload_top_polygons(&mut self, _: &[GpuPolygon]) {}
    fn upload_drawings(&mut self, drawings: &[OrderedItem]) {
        self.drawings = Some(drawings.len());
    }
    fn upload_drawings_front(&mut self, _: &[OrderedItem]) {}
    fn upload_texts(&mut self, _: &[TextItem], _: TextUploadParams) -> Result<(), ()> {
        Ok(())
    }
    fn refresh_grid(&mut self) {}
    fn upload_overlay_lines(&mut self, _: &[LineSegment]) {}
    fn upload_overlay_circles(&mut self, _: &[Circle]) {}
    fn upload_overlay_polygons(&mut self, _: &[GpuPolygon]) {}
    fn upload_erc_marker_lines(&mut self, _: &[LineSegment]) {}
    fn upload_erc_marker_circles(&mut self, _: &[Circle]) {}
    fn upload_erc_marker_polygons(&mut self, _: &[GpuPolygon]) {}
    fn refresh_theme(&mut self) {}
}

fn line_at(x: f32, color: [f32; 4]) -> LineSegment {
    LineSegment {
        p0: [x, 0.0],
        p1: [x + 1.0, 0.0],
        width: 0.2,
        color,
        style: 0,
        _pad: 0,
    }
}

const WHITE: [f32; 4] = [1.0; 4];

fn scene_with_lines(xs: &[f32]) -> Scene {
    let mut scene = Scene {
        lines: xs.iter().map(|&x| line_at(x, WHITE)).collect(),
        ..Default::default()
    };
    scene.circles.push(Circle {
        center: [0.0, 0.0],
        radius: 1.0,
        stroke_width: 0.1,
        color: WHITE,
    });
    scene
}

fn view(min_x: f32, max_x: f32) -> Option<ViewportAabbMm> {
    Some(ViewportAabbMm::new([min_x, -5.0], [max_x, 5.0]))
}

/// Plan and apply one frame; returns the counters and what the target saw.
fn frame(
    tracker: &mut UploadTracker,
    scene: &Arc<Scene>,
    generation: u64,
    visible: Option<ViewportAabbMm>,
) -> (UploadCounters, CountingTarget) {
    let plan = tracker.plan(scene, Some(generation), visible);
    let mut target = CountingTarget::default();
    let counters = apply_dirty_uploads_with_culling(
        scene,
        plan.dirty,
        &mut target,
        TextUploadParams::default(),
        plan.culling,
    )
    .expect("mock text upload cannot fail");
    (counters, target)
}

#[test]
fn first_frame_uploads_every_geometry_category() {
    let mut tracker = UploadTracker::default();
    let scene = Arc::new(scene_with_lines(&[0.0, 1.0]));
    let (counters, _) = frame(&mut tracker, &scene, 1, view(-5.0, 5.0));
    assert!(counters.line_uploads == 1 && counters.circle_uploads == 1);
    assert!(counters.arc_uploads == 1 && counters.polygon_uploads == 1);
    assert_eq!(counters.overlay_line_uploads, 1);
}

#[test]
fn unchanged_generation_uploads_nothing() {
    let mut tracker = UploadTracker::default();
    let scene = Arc::new(scene_with_lines(&[0.0]));
    frame(&mut tracker, &scene, 1, view(-5.0, 5.0));
    let (counters, _) = frame(&mut tracker, &scene, 1, view(-5.0, 5.0));
    assert!(counters.is_idle());
}

#[test]
fn editing_lines_reuploads_only_lines() {
    let mut tracker = UploadTracker::default();
    frame(
        &mut tracker,
        &Arc::new(scene_with_lines(&[0.0, 1.0])),
        1,
        view(-5.0, 5.0),
    );
    // Same circles, one extra line: generation bumped by the producer.
    let edited = Arc::new(scene_with_lines(&[0.0, 1.0, 2.0]));
    let (counters, target) = frame(&mut tracker, &edited, 2, view(-5.0, 5.0));
    assert_eq!(counters.line_uploads, 1);
    assert_eq!(counters.circle_uploads, 0);
    assert_eq!(counters.total_updates(), 1);
    assert_eq!(target.lines, 3);
}

#[test]
fn theme_change_reuploads_the_recoloured_category() {
    let mut tracker = UploadTracker::default();
    frame(
        &mut tracker,
        &Arc::new(scene_with_lines(&[0.0])),
        1,
        view(-5.0, 5.0),
    );
    let mut recoloured = scene_with_lines(&[0.0]);
    recoloured.lines[0].color = [0.1, 0.2, 0.3, 1.0];
    let (counters, _) = frame(&mut tracker, &Arc::new(recoloured), 2, view(-5.0, 5.0));
    assert_eq!(counters.line_uploads, 1);
    assert_eq!(counters.circle_uploads, 0);
}

#[test]
fn emptied_category_is_cleared_whatever_the_producer_dirty_flag_says() {
    let mut tracker = UploadTracker::default();
    let mut with_overlay = scene_with_lines(&[0.0]);
    with_overlay.overlay_lines.push(line_at(0.0, WHITE));
    frame(&mut tracker, &Arc::new(with_overlay), 1, view(-5.0, 5.0));
    // The next build no longer has the overlay and its `dirty` omits OVERLAY.
    let mut next = scene_with_lines(&[0.0]);
    next.dirty = DirtyFlags::LINES;
    let plan = tracker.plan(&Arc::new(next), Some(2), view(-5.0, 5.0));
    assert!(plan.dirty.contains(DirtyFlags::OVERLAY));
}

#[test]
fn culling_returns_after_a_degenerate_frame() {
    let mut tracker = UploadTracker::default();
    let scene = Arc::new(scene_with_lines(&[0.0, 1000.0]));
    let (_, full) = frame(&mut tracker, &scene, 1, None);
    assert_eq!(full.lines, 2);
    let (counters, culled) = frame(&mut tracker, &scene, 1, view(-5.0, 5.0));
    assert_eq!(counters.line_uploads, 1);
    assert_eq!(culled.lines, 1, "far line culled again");
}

#[test]
fn deep_zoom_in_restores_the_margin() {
    let mut tracker = UploadTracker::default();
    let scene = Arc::new(scene_with_lines(&[0.0, 8.0]));
    frame(&mut tracker, &scene, 1, view(-5.0, 5.0));
    // Window is -10..10; zooming to -1..1 is still inside it but 100x smaller.
    let (counters, target) = frame(&mut tracker, &scene, 1, view(-1.0, 1.0));
    assert_eq!(counters.line_uploads, 1, "window far larger than the view");
    assert_eq!(target.lines, 1, "x=8 now outside the tightened window");
}

#[test]
fn item_straddling_the_viewport_edge_is_kept() {
    let mut tracker = UploadTracker::default();
    // Line spans x 4.5..5.5 across the right edge (5.0); another is far away.
    let scene = Arc::new(scene_with_lines(&[4.5, 100.0]));
    let (_, target) = frame(&mut tracker, &scene, 1, view(-5.0, 5.0));
    assert_eq!(target.lines, 1, "edge line kept, far line culled");
}

#[test]
fn margin_keeps_an_item_just_outside_the_viewport() {
    let mut tracker = UploadTracker::default();
    // Visible width 10, margin 0.5 => resident window reaches x = 10.
    let scene = Arc::new(scene_with_lines(&[7.0, 12.0]));
    let (_, target) = frame(&mut tracker, &scene, 1, view(-5.0, 5.0));
    assert_eq!(target.lines, 1, "x=7 is inside the margin, x=12 is not");
    assert_eq!(CULL_MARGIN_FRACTION, 0.5);
}

#[test]
fn pan_inside_the_margin_does_not_reupload() {
    let mut tracker = UploadTracker::default();
    let scene = Arc::new(scene_with_lines(&[0.0, 7.0]));
    frame(&mut tracker, &scene, 1, view(-5.0, 5.0));
    let (counters, _) = frame(&mut tracker, &scene, 1, view(-2.0, 8.0));
    assert!(counters.is_idle(), "window -10..10 still covers -2..8");
}

#[test]
fn pan_past_the_margin_recull_brings_new_items_in() {
    let mut tracker = UploadTracker::default();
    let scene = Arc::new(scene_with_lines(&[0.0, 30.0]));
    let (_, first) = frame(&mut tracker, &scene, 1, view(-5.0, 5.0));
    assert_eq!(first.lines, 1);

    let (counters, panned) = frame(&mut tracker, &scene, 1, view(25.0, 35.0));
    assert_eq!(
        counters.line_uploads, 1,
        "same generation, view moved: re-cull"
    );
    assert_eq!(panned.lines, 1, "the line at x=30 is now resident");
    assert_eq!(panned.circles, 0, "the circle at the origin is culled away");
    assert!(GEOMETRY_FLAGS.contains(DirtyFlags::LINES));
}

#[test]
fn zooming_out_past_the_window_recull() {
    let mut tracker = UploadTracker::default();
    let scene = Arc::new(scene_with_lines(&[0.0, 12.0]));
    frame(&mut tracker, &scene, 1, view(-5.0, 5.0));
    let (counters, target) = frame(&mut tracker, &scene, 1, view(-20.0, 20.0));
    assert_eq!(counters.line_uploads, 1);
    assert_eq!(target.lines, 2);
}

#[test]
fn degenerate_view_uploads_unculled_once() {
    let mut tracker = UploadTracker::default();
    let scene = Arc::new(scene_with_lines(&[0.0, 1000.0]));
    let (_, target) = frame(&mut tracker, &scene, 1, None);
    assert_eq!(target.lines, 2);
    let plan = tracker.plan(&scene, Some(1), None);
    assert_eq!(plan.culling, UploadCulling::disabled());
    assert!(plan.dirty.is_empty());
}

#[test]
fn linear_cull_matches_an_exhaustive_filter() {
    let xs: Vec<f32> = (-50..50).map(|i| i as f32 * 0.7).collect();
    let scene = scene_with_lines(&xs);
    let window = ViewportAabbMm::new([-3.0, -1.0], [4.0, 1.0]);
    let mut target = CountingTarget::default();
    apply_dirty_uploads_with_culling(
        &scene,
        DirtyFlags::LINES,
        &mut target,
        TextUploadParams::default(),
        UploadCulling::viewport(window),
    )
    .unwrap();
    let expected = xs
        .iter()
        .filter(|&&x| x + 1.0 + 0.1 >= -3.0 && x - 0.1 <= 4.0)
        .count();
    assert_eq!(target.lines, expected);
}

// Drawings joined the tracker after the merge with #1371: a drawings-only edit
// must dirty DRAWINGS, and the first frame must upload them (#1579).
#[test]
fn drawings_upload_on_the_first_frame_and_on_a_drawings_only_edit() {
    let mut tracker = UploadTracker::default();
    let mut scene = scene_with_lines(&[0.0]);
    scene.drawings = vec![OrderedItem::Line(line_at(1.0, WHITE))];
    let (_, first) = frame(&mut tracker, &Arc::new(scene.clone()), 1, view(-10.0, 10.0));
    assert_eq!(first.drawings, Some(1));

    scene.drawings.push(OrderedItem::Line(line_at(2.0, WHITE)));
    let (counters, edited) = frame(&mut tracker, &Arc::new(scene), 2, view(-10.0, 10.0));
    assert_eq!(edited.drawings, Some(2));
    assert_eq!(counters.drawing_uploads, 1);
    assert_eq!(counters.line_uploads, 0, "only the drawings changed");
}
