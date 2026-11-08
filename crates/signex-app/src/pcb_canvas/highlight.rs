//! The selection, hover and marquee, painted above the board (#2191).
//!
//! A separate canvas layer, so it sits over the GPU shader too and a
//! selection change never re-tessellates the board.

use iced::widget::canvas::{self, LineCap, LineDash, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse};
use pcb_geom::{PointNm, Shape, nm_to_mm};

use super::PcbCanvas;
use crate::app::Message;
use crate::canvas::Camera;

/// Ellipse outline resolution.
const ELLIPSE_STEPS: usize = 32;
/// Narrowest stroke for a highlighted track, in pixels.
const MIN_BAND_PX: f32 = 3.0;

/// The copper to paint, in board nm.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Highlight {
    pub selected: Vec<Shape>,
    pub hover: Option<Shape>,
}

/// Paints [`PcbCanvas`]'s highlight and marquee; takes no input.
pub struct HighlightLayer<'a>(pub(super) &'a PcbCanvas);

impl canvas::Program<Message> for HighlightLayer<'_> {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        _event: &iced::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let canvas = self.0;
        let camera = canvas.camera_ref();
        let accent = crate::render_config::to_iced(&canvas.canvas_colors.selection);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let view = Rectangle::with_size(bounds.size());
        for shape in &canvas.highlight.selected {
            paint(&mut frame, shape, &camera, view, accent, 0.45, 1.5);
        }
        if let Some(shape) = &canvas.highlight.hover {
            paint(&mut frame, shape, &camera, view, accent, 0.2, 1.0);
        }
        if let Some((from, to)) = canvas.marquee.get() {
            let at = |p: signex_types::pcb::Point| Point::new(p.x as f32, p.y as f32);
            paint_marquee(&mut frame, &camera, at(from), at(to), accent);
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        mouse::Interaction::default()
    }
}

fn screen(camera: &Camera, p: PointNm) -> Point {
    let world = Point::new(nm_to_mm(p.x) as f32, nm_to_mm(p.y) as f32);
    camera.world_to_screen(world, Rectangle::with_size(iced::Size::ZERO))
}

fn paint(
    frame: &mut canvas::Frame,
    shape: &Shape,
    camera: &Camera,
    view: Rectangle,
    accent: Color,
    fill_alpha: f32,
    outline_px: f32,
) {
    let aabb = shape.aabb();
    let (lo, hi) = (screen(camera, aabb.min), screen(camera, aabb.max));
    let extent = Rectangle::new(lo, iced::Size::new(hi.x - lo.x, hi.y - lo.y));
    if !view.intersects(&extent) {
        return;
    }
    let fill = Color {
        a: fill_alpha,
        ..accent
    };
    let outline = Stroke::default().with_width(outline_px).with_color(accent);
    match *shape {
        Shape::Disc { center, radius } => {
            let path = Path::circle(
                screen(camera, center),
                (nm_to_mm(radius) as f32 * camera.scale).max(1.0),
            );
            frame.fill(&path, fill);
            frame.stroke(&path, outline);
        }
        Shape::Capsule { a, b, radius } => {
            let band = (2.0 * nm_to_mm(radius) as f32 * camera.scale).max(MIN_BAND_PX);
            let path = Path::line(screen(camera, a), screen(camera, b));
            let stroke = Stroke::default()
                .with_width(band)
                .with_color(fill)
                .with_line_cap(LineCap::Round);
            frame.stroke(&path, stroke);
        }
        Shape::Rect { .. } | Shape::Ellipse { .. } => {
            let points = shape.outline(ELLIPSE_STEPS);
            let path = Path::new(|b| {
                let mut ring = points.iter().map(|p| screen(camera, *p));
                if let Some(first) = ring.next() {
                    b.move_to(first);
                    ring.for_each(|p| b.line_to(p));
                    b.close();
                }
            });
            frame.fill(&path, fill);
            frame.stroke(&path, outline);
        }
    }
}

/// A window (left to right) is outlined solid, a crossing dashed.
fn paint_marquee(
    frame: &mut canvas::Frame,
    camera: &Camera,
    from: Point,
    to: Point,
    accent: Color,
) {
    let (a, b) = (
        camera.world_to_screen(from, Rectangle::with_size(iced::Size::ZERO)),
        camera.world_to_screen(to, Rectangle::with_size(iced::Size::ZERO)),
    );
    let top_left = Point::new(a.x.min(b.x), a.y.min(b.y));
    let size = iced::Size::new((a.x - b.x).abs(), (a.y - b.y).abs());
    let path = Path::rectangle(top_left, size);
    frame.fill(&path, Color { a: 0.12, ..accent });
    let window = to.x > from.x;
    let dash = [6.0_f32, 4.0];
    let stroke = Stroke {
        line_dash: if window {
            LineDash::default()
        } else {
            LineDash {
                segments: &dash,
                offset: 0,
            }
        },
        ..Stroke::default().with_width(1.0).with_color(accent)
    };
    frame.stroke(&path, stroke);
}
