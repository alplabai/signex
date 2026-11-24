//! The board view (mockup row 193): every visible layer drawn bottom first
//! in the file frame (x right, y up), mirrored while flipped. The near face
//! draws at full strength and the far face darker; Highlight net dims what
//! is not on the net; with Drills on, the holes draw over everything as
//! rings. Wheel zooms at the pointer, a drag (left or middle button) pans,
//! a left click selects. The camera lives in the slice state; the only
//! canvas state is the press being dragged.

use iced::widget::canvas::{self, Canvas, Frame, LineDash, Path, Stroke};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Widget as _, mouse};
use pcb_geom::{PointNm, Shape};
use signex_gerber::{FileFunction, Polarity};

use super::GerberViewerMsg;
use super::state::{Camera, OpenPackage, anchors, is_drill};

/// The share of the canvas the fitted board takes.
const FIT_SHARE: f64 = 0.9;
/// How much one wheel line zooms.
const ZOOM_STEP: f64 = 1.2;
const MIN_ZOOM: f64 = 0.05;
const MAX_ZOOM: f64 = 20_000.0;
/// Points per quarter turn of a round shape.
const ROUND_STEPS: usize = 8;
/// Strength of the far face, and of what Highlight net dims.
const FAR_ALPHA: f32 = 0.45;
const DIM_ALPHA: f32 = 0.15;

/// Where the board sits on the canvas for one camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardFit {
    /// Pixels per nm.
    pub scale: f64,
    /// The file point at the canvas middle, in nm.
    pub centre: (f64, f64),
    /// The board's own middle, which the camera's pan is measured from.
    pub board_centre: (f64, f64),
    /// The canvas middle, in pixels.
    pub mid: (f64, f64),
    pub flipped: bool,
}

impl BoardFit {
    /// The fit of `bounds` (min, max in nm) on a `size` canvas.
    pub fn of(bounds: (PointNm, PointNm), camera: Camera, size: Size, flipped: bool) -> Self {
        let (min, max) = bounds;
        let width = ((max.x - min.x) as f64).max(1.0);
        let height = ((max.y - min.y) as f64).max(1.0);
        let fitted = (f64::from(size.width) / width).min(f64::from(size.height) / height);
        let board_centre = (
            (min.x as f64 + max.x as f64) / 2.0,
            (min.y as f64 + max.y as f64) / 2.0,
        );
        Self {
            scale: fitted * FIT_SHARE * camera.zoom,
            centre: (board_centre.0 + camera.pan.0, board_centre.1 + camera.pan.1),
            board_centre,
            mid: (f64::from(size.width) / 2.0, f64::from(size.height) / 2.0),
            flipped,
        }
    }

    fn sign(&self) -> f64 {
        if self.flipped { -1.0 } else { 1.0 }
    }

    pub fn to_screen(&self, p: PointNm) -> Point {
        let x = self.mid.0 + self.sign() * (p.x as f64 - self.centre.0) * self.scale;
        let y = self.mid.1 - (p.y as f64 - self.centre.1) * self.scale;
        Point::new(x as f32, y as f32)
    }

    pub fn to_world(&self, p: Point) -> (f64, f64) {
        (
            self.centre.0 + self.sign() * (f64::from(p.x) - self.mid.0) / self.scale,
            self.centre.1 - (f64::from(p.y) - self.mid.1) / self.scale,
        )
    }

    /// The camera that zooms `camera` by `factor` and keeps the file point
    /// under `at` where it is.
    pub fn zoomed_at(&self, camera: Camera, at: Point, factor: f64) -> Camera {
        let zoom = (camera.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let scale = self.scale * zoom / camera.zoom;
        let world = self.to_world(at);
        let centre = (
            world.0 - self.sign() * (f64::from(at.x) - self.mid.0) / scale,
            world.1 + (f64::from(at.y) - self.mid.1) / scale,
        );
        self.camera_at(zoom, centre)
    }

    /// The camera that moves the board by `(dx, dy)` pixels.
    pub fn panned(&self, camera: Camera, dx: f32, dy: f32) -> Camera {
        let centre = (
            self.centre.0 - self.sign() * f64::from(dx) / self.scale,
            self.centre.1 + f64::from(dy) / self.scale,
        );
        self.camera_at(camera.zoom, centre)
    }

    fn camera_at(&self, zoom: f64, centre: (f64, f64)) -> Camera {
        Camera {
            zoom,
            pan: (
                centre.0 - self.board_centre.0,
                centre.1 - self.board_centre.1,
            ),
        }
    }
}

/// The board view's inputs.
pub struct BoardView<'a> {
    pub open: &'a OpenPackage,
    pub background: Color,
    pub accent: Color,
    /// How far, in pixels, a click may land off an object, and a press may
    /// move and still be a click.
    pub slop: f32,
    /// One wheel line, in pixels, for a wheel that reports pixels.
    pub line: f32,
    /// The dash and gap of a non-plated hole's ring, in pixels.
    pub dash: [f32; 2],
}

/// A press on the board: where, with which button, the camera then, and
/// whether it has moved past the slop.
#[derive(Debug, Clone, Copy)]
pub struct Press {
    at: Point,
    selects: bool,
    camera: Camera,
    moved: bool,
}

/// The canvas state: the press being held.
#[derive(Debug, Default)]
pub struct Gesture {
    press: Option<Press>,
}

impl BoardView<'_> {
    fn fit(&self, size: Size, camera: Camera) -> Option<BoardFit> {
        let bounds = self.open.bounds()?;
        Some(BoardFit::of(bounds, camera, size, self.open.flipped))
    }

    fn released(&self, press: Press, size: Size) -> Option<GerberViewerMsg> {
        if press.moved || !press.selects {
            return None;
        }
        let fit = self.fit(size, press.camera)?;
        let (x, y) = fit.to_world(press.at);
        Some(GerberViewerMsg::BoardClicked {
            at: PointNm::new(x.round() as i64, y.round() as i64),
            tolerance: (f64::from(self.slop) / fit.scale).round() as i64,
        })
    }

    fn dragged(&self, press: &mut Press, at: Point, size: Size) -> Option<GerberViewerMsg> {
        let (dx, dy) = (at.x - press.at.x, at.y - press.at.y);
        if !press.moved && dx.hypot(dy) < self.slop {
            return None;
        }
        press.moved = true;
        let fit = self.fit(size, press.camera)?;
        Some(GerberViewerMsg::ViewChanged(fit.panned(
            press.camera,
            dx,
            dy,
        )))
    }

    fn scrolled(
        &self,
        delta: mouse::ScrollDelta,
        at: Point,
        size: Size,
    ) -> Option<GerberViewerMsg> {
        let lines = match delta {
            mouse::ScrollDelta::Lines { y, .. } => f64::from(y),
            mouse::ScrollDelta::Pixels { y, .. } => f64::from(y / self.line.max(1.0)),
        };
        let camera = self.open.camera;
        let fit = self.fit(size, camera)?;
        Some(GerberViewerMsg::ViewChanged(fit.zoomed_at(
            camera,
            at,
            ZOOM_STEP.powf(lines),
        )))
    }
}

impl canvas::Program<GerberViewerMsg> for BoardView<'_> {
    type State = Gesture;

    fn update(
        &self,
        state: &mut Gesture,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<GerberViewerMsg>> {
        let canvas::Event::Mouse(event) = event else {
            return None;
        };
        let size = bounds.size();
        let message = match *event {
            mouse::Event::ButtonPressed(button @ (mouse::Button::Left | mouse::Button::Middle)) => {
                state.press = Some(Press {
                    at: cursor.position_in(bounds)?,
                    selects: button == mouse::Button::Left,
                    camera: self.open.camera,
                    moved: false,
                });
                return Some(canvas::Action::capture());
            }
            mouse::Event::CursorMoved { position } => {
                let press = state.press.as_mut()?;
                let at = Point::new(position.x - bounds.x, position.y - bounds.y);
                self.dragged(press, at, size)
            }
            mouse::Event::ButtonReleased(mouse::Button::Left | mouse::Button::Middle) => {
                let press = state.press.take()?;
                self.released(press, size)
            }
            mouse::Event::WheelScrolled { delta } => {
                self.scrolled(delta, cursor.position_in(bounds)?, size)
            }
            _ => None,
        }?;
        Some(canvas::Action::publish(message).and_capture())
    }

    fn draw(
        &self,
        _state: &Gesture,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.background);
        if let Some(fit) = self.fit(bounds.size(), self.open.camera) {
            for layer in self.open.draw_order() {
                self.draw_layer(&mut frame, &fit, layer);
            }
            self.draw_selection(&mut frame, &fit);
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Gesture,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match state.press {
            Some(press) if press.moved => mouse::Interaction::Grabbing,
            _ if cursor.is_over(bounds) => mouse::Interaction::Crosshair,
            _ => mouse::Interaction::default(),
        }
    }
}

impl BoardView<'_> {
    fn draw_layer(&self, frame: &mut Frame, fit: &BoardFit, layer: usize) {
        let Some(look) = self.open.looks.get(layer).filter(|look| look.visible) else {
            return;
        };
        let entry = &self.open.package.layers[layer];
        let function = entry.function.as_ref();
        let strength = if self.open.is_near_side(layer) {
            1.0
        } else {
            FAR_ALPHA
        };
        let [r, g, b] = look.colour;
        let ink = Color {
            a: strength,
            ..Color::from_rgb8(r, g, b)
        };
        let as_rings = self.open.drills_on_top && is_drill(function);
        for object in &entry.image.objects {
            let on_net = match &self.open.net {
                Some(net) => object.attributes.net.as_deref() == Some(net.as_str()),
                None => true,
            };
            let colour = match (object.polarity, on_net) {
                (Polarity::Clear, _) => self.background,
                (Polarity::Dark, true) => ink,
                (Polarity::Dark, false) => Color {
                    a: DIM_ALPHA,
                    ..ink
                },
            };
            if object.shapes.is_empty() {
                self.draw_marker(frame, fit, &anchors(&object.kind), colour);
            }
            for shape in &object.shapes {
                let path = shape_path(fit, shape);
                if as_rings {
                    frame.stroke(&path, self.ring(function, colour));
                } else {
                    frame.fill(&path, colour);
                }
            }
        }
    }

    /// A plated hole's light ring, or a non-plated hole's dashed one.
    fn ring(&self, function: Option<&FileFunction>, colour: Color) -> Stroke<'_> {
        let width = self.slop / 2.0;
        let stroke = Stroke::default().with_color(colour).with_width(width);
        match function {
            Some(FileFunction::Drill { plated: false, .. }) => Stroke {
                line_dash: LineDash {
                    segments: &self.dash,
                    offset: 0,
                },
                ..stroke
            },
            _ => stroke,
        }
    }

    /// A cross where an object that has no shape sits, so it is not lost.
    fn draw_marker(&self, frame: &mut Frame, fit: &BoardFit, points: &[PointNm], colour: Color) {
        let half = self.slop;
        let stroke = Stroke::default()
            .with_color(colour)
            .with_width(self.slop / 3.0);
        for point in points {
            let at = fit.to_screen(*point);
            let cross = Path::new(|b| {
                b.move_to(Point::new(at.x - half, at.y - half));
                b.line_to(Point::new(at.x + half, at.y + half));
                b.move_to(Point::new(at.x - half, at.y + half));
                b.line_to(Point::new(at.x + half, at.y - half));
            });
            frame.stroke(&cross, stroke);
        }
    }

    fn draw_selection(&self, frame: &mut Frame, fit: &BoardFit) {
        let Some(object) = self.open.selected.and_then(|at| self.open.object(at)) else {
            return;
        };
        let stroke = Stroke::default()
            .with_color(self.accent)
            .with_width(self.slop / 3.0);
        for shape in &object.shapes {
            frame.stroke(&shape_path(fit, shape), stroke);
        }
        if object.shapes.is_empty() {
            self.draw_marker(frame, fit, &anchors(&object.kind), self.accent);
        }
    }
}

/// The closed outline of `shape` on the canvas.
fn shape_path(fit: &BoardFit, shape: &Shape) -> Path {
    let ring = shape.ring(ROUND_STEPS);
    Path::new(|b| {
        let mut points = ring.iter().map(|p| fit.to_screen(*p));
        if let Some(first) = points.next() {
            b.move_to(first);
            for point in points {
                b.line_to(point);
            }
            b.close();
        }
    })
}

/// The board view as a widget filling its place.
pub fn board_view(view: BoardView<'_>) -> Element<'_, GerberViewerMsg> {
    Canvas::new(view)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}
