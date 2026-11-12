//! The selection, hover and marquee, painted above the board (#2191).
//!
//! A separate canvas layer, so it sits over the GPU shader too and a
//! selection change never re-tessellates the board.

use iced::widget::canvas::{self, LineCap, LineDash, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse};
use pcb_geom::{PointNm, Shape, nm_to_mm};

use super::{BoardCamera, PcbCanvas};
use crate::app::Message;

/// Points per quarter turn of a pad corner or end cap.
const CORNER_STEPS: usize = 32;
/// Narrowest stroke for a highlighted track, in pixels.
const MIN_BAND_PX: f32 = 3.0;

/// The copper to paint, in board nm.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Highlight {
    pub selected: Vec<Shape>,
    pub hover: Option<Shape>,
    /// Bodies of selected footprints, outlined with a faint fill.
    pub bodies: Vec<Shape>,
    pub hover_body: Option<Shape>,
    /// Where a footprint drag or Move would put what it carries (#2240).
    pub ghost: Vec<Shape>,
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
        for shape in &canvas.highlight.bodies {
            paint(&mut frame, shape, &camera, view, accent, 0.08, 1.5);
        }
        if let Some(shape) = &canvas.highlight.hover_body {
            paint(&mut frame, shape, &camera, view, accent, 0.04, 1.0);
        }
        for shape in &canvas.highlight.ghost {
            paint(&mut frame, shape, &camera, view, accent, 0.3, 1.0);
        }
        if let Some((from, to)) = canvas.marquee.get() {
            let at = |p: signex_types::pcb::Point| Point::new(p.x as f32, p.y as f32);
            let window = is_window(from, to, camera.rotation().degrees());
            paint_marquee(&mut frame, &camera, at(from), at(to), window, accent);
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

fn screen(camera: &BoardCamera, p: PointNm) -> Point {
    let world = Point::new(nm_to_mm(p.x) as f32, nm_to_mm(p.y) as f32);
    camera.world_to_screen(world)
}

/// The screen box around a board box: all four corners, since a turned view
/// (#2316) can put any of them at the top left.
fn screen_extent(camera: &BoardCamera, min: PointNm, max: PointNm) -> Rectangle {
    let corners = [
        min,
        PointNm::new(max.x, min.y),
        PointNm::new(min.x, max.y),
        max,
    ]
    .map(|p| screen(camera, p));
    let (mut lo, mut hi) = (corners[0], corners[0]);
    for c in &corners[1..] {
        lo = Point::new(lo.x.min(c.x), lo.y.min(c.y));
        hi = Point::new(hi.x.max(c.x), hi.y.max(c.y));
    }
    Rectangle::new(lo, iced::Size::new(hi.x - lo.x, hi.y - lo.y))
}

fn paint(
    frame: &mut canvas::Frame,
    shape: &Shape,
    camera: &BoardCamera,
    view: Rectangle,
    accent: Color,
    fill_alpha: f32,
    outline_px: f32,
) {
    let aabb = shape.aabb();
    if !view.intersects(&screen_extent(camera, aabb.min, aabb.max)) {
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
                (nm_to_mm(radius) as f32 * camera.scale()).max(1.0),
            );
            frame.fill(&path, fill);
            frame.stroke(&path, outline);
        }
        Shape::Capsule { a, b, radius } => {
            let band = (2.0 * nm_to_mm(radius) as f32 * camera.scale()).max(MIN_BAND_PX);
            let path = Path::line(screen(camera, a), screen(camera, b));
            let stroke = Stroke::default()
                .with_width(band)
                .with_color(fill)
                .with_line_cap(LineCap::Round);
            frame.stroke(&path, stroke);
        }
        Shape::Rect { .. } | Shape::RoundRect { .. } => {
            let points = shape.outline(CORNER_STEPS);
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

/// Whether the box selects as a window, by `pcb_geom`'s own rule, judged on
/// screen for a view turned `view_deg`.
fn is_window(from: signex_types::pcb::Point, to: signex_types::pcb::Point, view_deg: f32) -> bool {
    match (
        pcb_geom::PointNm::from_mm(from, "marquee.from"),
        pcb_geom::PointNm::from_mm(to, "marquee.to"),
    ) {
        (Ok(from), Ok(to)) => {
            pcb_geom::turned_marquee_mode(from, to, f64::from(view_deg))
                == pcb_geom::RectSelect::Inside
        }
        _ => false,
    }
}

/// A window (left to right) is outlined solid, a crossing dashed. The box
/// runs along the screen, whatever the view's turn.
fn paint_marquee(
    frame: &mut canvas::Frame,
    camera: &BoardCamera,
    from: Point,
    to: Point,
    window: bool,
    accent: Color,
) {
    let (a, b) = (camera.world_to_screen(from), camera.world_to_screen(to));
    let top_left = Point::new(a.x.min(b.x), a.y.min(b.y));
    let size = iced::Size::new((a.x - b.x).abs(), (a.y - b.y).abs());
    let path = Path::rectangle(top_left, size);
    frame.fill(&path, Color { a: 0.12, ..accent });
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
