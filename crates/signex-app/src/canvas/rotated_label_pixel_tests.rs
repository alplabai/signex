//! Rotated labels, rendered to pixels on the CPU canvas layer (#1444).
//!
//! The GPU glyph atlas cannot rotate, so `TextItem::is_axis_aligned` sends every
//! turned label to `renderer_scene_canvas`, which translates to the anchor and
//! rotates the canvas. These tests draw that path through iced's real wgpu
//! renderer (the harness of `gpu_mount_tests`) and read the pixels back, for a
//! label at 0, 90, 180 and 270 degrees:
//!
//! * **anchor** — the glyph sits on the side of the anchor its direction
//!   implies and touches it (left/bottom alignment pivots there);
//! * **orientation** — an `L` keeps its stem on the reading start and its foot
//!   on the baseline side, so a mirrored or wrongly-signed turn is caught.
//!
//! Measurements are in the label's own frame (`d` along the text, `u` up from
//! the baseline), so one set of bounds covers all four angles. Tolerances are
//! pixel-scale slack for glyph side bearings and anti-aliasing, so they hold on
//! any adapter and font rasteriser.

use iced::widget::canvas;
use iced::{Point, Rectangle, Renderer, Theme, mouse};
use signex_gfx::primitive::text::{TextHAlign, TextItem, TextVAlign};
use signex_gfx::scene::Scene;

use super::gpu_mount_tests::{Frame, HEIGHT, WIDTH, gpu, render};
use crate::renderer_scene_canvas::{
    SceneDrawOptions, draw_rotated_texts, draw_scene_with_world_to_screen,
};

/// Glyph height in logical pixels. Large so the measurements dwarf the slack.
const GLYPH_PX: f32 = 36.0;
/// Slack for side bearings, descent and anti-aliasing, in pixels.
const SLACK: f32 = 3.0;

struct OneLabel(TextItem);

impl canvas::Program<crate::app::Message> for OneLabel {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let scene = Scene {
            texts: vec![self.0.clone()],
            ..Scene::default()
        };
        // World is pixels here (1 px per mm), so the anchor is where we say.
        let options = SceneDrawOptions {
            scale_px_per_mm: 1.0,
            min_stroke_px: 0.5,
            text_min_px: 1.0,
            text_max_px: 400.0,
        };
        let to_screen = |p: [f32; 2]| Point::new(p[0], p[1]);
        // The 0 degree label belongs to the GPU atlas on the real surface, so
        // it is drawn through the full CPU replay as the unrotated baseline;
        // the turned ones go through the fallback the app mounts.
        if self.0.is_axis_aligned() {
            draw_scene_with_world_to_screen(&mut frame, &scene, to_screen, options);
        } else {
            draw_rotated_texts(&mut frame, &scene, to_screen, options);
        }
        vec![frame.into_geometry()]
    }
}

fn anchor() -> Point {
    Point::new(WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0)
}

fn label_at(degrees: f32) -> TextItem {
    let a = anchor();
    TextItem {
        content: "L".to_string(),
        position: [a.x, a.y],
        // `text_px` divides by MM_PER_EM, so this is GLYPH_PX pixels.
        size_mm: GLYPH_PX * signex_gfx::primitive::text::MM_PER_EM,
        color: [1.0; 4],
        rotation: degrees.to_radians(),
        h_align: TextHAlign::Left,
        v_align: TextVAlign::Bottom,
        ..Default::default()
    }
}

/// Lit pixels as `(d, u)`: distance along the text direction and up from the
/// baseline, both measured from the anchor. Screen y is down and rotation is
/// clockwise-positive, so direction is `(cos, sin)` and up is `(sin, -cos)`.
fn in_label_frame(frame: &Frame, degrees: f32) -> Vec<(f32, f32)> {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let a = anchor();
    let mut out = Vec::new();
    for y in 0..frame.height {
        for x in 0..frame.width {
            if frame.px(x, y)[..3].iter().any(|&c| c > 128) {
                let (dx, dy) = (x as f32 + 0.5 - a.x, y as f32 + 0.5 - a.y);
                out.push((dx * cos + dy * sin, dx * sin - dy * cos));
            }
        }
    }
    out
}

fn extent(points: &[(f32, f32)], pick: impl Fn(&(f32, f32)) -> f32) -> (f32, f32) {
    points
        .iter()
        .map(pick)
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
}

#[test]
fn quarter_turn_labels_land_on_their_anchor_and_keep_their_orientation() {
    let mut gpu = gpu();
    for degrees in [0.0, 90.0, 180.0, 270.0] {
        let element = canvas(OneLabel(label_at(degrees)))
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .into();
        let frame = render(&mut gpu, element);
        let lit = in_label_frame(&frame, degrees);
        assert!(
            lit.len() > 40,
            "{degrees} deg: label not drawn ({} px)",
            lit.len()
        );

        let (d_min, d_max) = extent(&lit, |p| p.0);
        let (u_min, u_max) = extent(&lit, |p| p.1);

        // Anchor: the glyph starts at the anchor along the text and rises from
        // the baseline just above it (Left / Bottom alignment).
        assert!(
            (-SLACK..=GLYPH_PX * 0.25).contains(&d_min),
            "{degrees} deg: text starts {d_min:.1}px along the text from the anchor"
        );
        assert!(
            (-SLACK..=GLYPH_PX * 0.35).contains(&u_min),
            "{degrees} deg: baseline is {u_min:.1}px above the anchor"
        );
        // Size: the glyph is upright-sized in its own frame, not squashed.
        assert!(
            (GLYPH_PX * 0.5..=GLYPH_PX * 1.1).contains(&(u_max - u_min)),
            "{degrees} deg: glyph height {:.1}px",
            u_max - u_min
        );
        assert!(
            (GLYPH_PX * 0.25..=GLYPH_PX * 0.8).contains(&(d_max - d_min)),
            "{degrees} deg: glyph width {:.1}px",
            d_max - d_min
        );

        // Orientation: an `L` carries its mass at the reading start (stem) and
        // at the baseline (foot), in the label's own frame.
        let n = lit.len() as f32;
        let (mean_d, mean_u) = lit
            .iter()
            .fold((0.0, 0.0), |(sd, su), (d, u)| (sd + d, su + u));
        let (mean_d, mean_u) = (mean_d / n, mean_u / n);
        assert!(
            mean_d < (d_min + d_max) / 2.0 - 1.0,
            "{degrees} deg: stem not at the reading start (mean d {mean_d:.1}, span {d_min:.1}..{d_max:.1})"
        );
        assert!(
            mean_u < (u_min + u_max) / 2.0 - 1.0,
            "{degrees} deg: foot not on the baseline side (mean u {mean_u:.1}, span {u_min:.1}..{u_max:.1})"
        );
    }
}
