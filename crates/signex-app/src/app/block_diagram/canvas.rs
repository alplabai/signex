//! The diagram canvas (mockup row 171): groups as frames under the blocks,
//! the connections in right angles with their labels, then the blocks with
//! their names, functions, notes and port glyphs, the selection in the
//! accent. A power connection draws in the error colour and a bus thicker
//! in the accent, as the mockup's legend reads. Wheel zooms at the pointer;
//! a drag on empty canvas (left or middle button) pans; a left drag on a
//! block moves it and lands as one edit on release; a click picks, with
//! Shift it adds to the selection; a double click on a block opens its
//! sheet. With the Connect tool a click picks a block or port as an end.

use std::time::Instant;

use iced::widget::canvas::{self, Canvas, Frame, LineDash, Path, Stroke};
use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Widget as _, alignment,
    keyboard, mouse,
};
use signex_types::block_diagram::{
    Block, BlockDiagram, BlockShape, Connection, Endpoint, Interface, LinkState,
};
use signex_types::schematic::Point as MmPoint;
use uuid::Uuid;

use super::TabEvent;
use super::canvas_review::PreviewTags;
use super::state::{BlockDiagramEditor, Camera, DiagramItem, DiagramTool};
use crate::double_click::DoubleClick;

/// The share of the canvas the fitted diagram takes.
const FIT_SHARE: f64 = 0.85;
const ZOOM_STEP: f64 = 1.2;
const MIN_ZOOM: f64 = 0.05;
const MAX_ZOOM: f64 = 50.0;
/// A port glyph's half size, in body-text ems.
const PORT_EMS: f32 = 0.3;

/// The colours and sizes the canvas draws with, from the theme tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasLook {
    pub background: Color,
    pub block_fill: Color,
    pub edge: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub accent: Color,
    pub power: Color,
    pub unlinked: Color,
    /// The Re-sync review's previews: an added block, a moved one.
    pub added: Color,
    pub moved: Color,
    /// Body text size, px.
    pub text_size: f32,
    /// How far, in px, a click may land off an object and still pick it.
    pub slop: f32,
    /// Show GND: ground connections are drawn and picked only when on.
    pub show_ground: bool,
}

impl CanvasLook {
    /// Whether `link` is drawn and picked: a ground only with Show GND.
    pub fn shows(&self, link: &Connection) -> bool {
        self.show_ground || !super::is_ground_link(link)
    }
}

/// Where the diagram sits on the canvas for one camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    /// Pixels per mm.
    pub scale: f64,
    /// The diagram point at the canvas middle, mm.
    pub centre: (f64, f64),
    /// The box zoom 1 fits, mm; the pan is measured from its middle.
    pub home: [f64; 4],
    /// The canvas middle, px.
    pub mid: (f64, f64),
}

impl Fit {
    /// The fit of `diagram` on a `size` canvas.
    pub fn of(diagram: &BlockDiagram, camera: Camera, size: Size) -> Self {
        let home = camera.home_for(diagram);
        let [_, _, w, h] = home;
        let fitted = (f64::from(size.width) / w).min(f64::from(size.height) / h);
        let middle = home_middle(home);
        Self {
            scale: fitted * FIT_SHARE * camera.zoom,
            centre: (middle.0 + camera.pan.0, middle.1 + camera.pan.1),
            home,
            mid: (f64::from(size.width) / 2.0, f64::from(size.height) / 2.0),
        }
    }

    pub fn to_screen(self, p: MmPoint) -> Point {
        Point::new(
            (self.mid.0 + (p.x - self.centre.0) * self.scale) as f32,
            (self.mid.1 + (p.y - self.centre.1) * self.scale) as f32,
        )
    }

    pub fn to_mm(self, p: Point) -> MmPoint {
        MmPoint::new(
            self.centre.0 + (f64::from(p.x) - self.mid.0) / self.scale,
            self.centre.1 + (f64::from(p.y) - self.mid.1) / self.scale,
        )
    }

    /// A camera at `zoom` looking at `centre`, holding this fit's box.
    fn camera_at(self, zoom: f64, centre: (f64, f64)) -> Camera {
        let middle = home_middle(self.home);
        Camera {
            zoom,
            pan: (centre.0 - middle.0, centre.1 - middle.1),
            home: Some(self.home),
        }
    }

    /// The camera that zooms by `factor` and keeps the point under `at`.
    pub fn zoomed_at(self, camera: Camera, at: Point, factor: f64) -> Camera {
        let zoom = (camera.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let scale = self.scale * zoom / camera.zoom;
        let under = self.to_mm(at);
        let centre = (
            under.x - (f64::from(at.x) - self.mid.0) / scale,
            under.y - (f64::from(at.y) - self.mid.1) / scale,
        );
        self.camera_at(zoom, centre)
    }

    /// The camera that moves the view by `(dx, dy)` px.
    pub fn panned(self, camera: Camera, dx: f32, dy: f32) -> Camera {
        let centre = (
            self.centre.0 - f64::from(dx) / self.scale,
            self.centre.1 - f64::from(dy) / self.scale,
        );
        self.camera_at(camera.zoom, centre)
    }

    fn mm(self, px: f32) -> f64 {
        f64::from(px) / self.scale
    }
}

/// The middle of a `[x, y, width, height]` box.
fn home_middle([x, y, w, h]: [f64; 4]) -> (f64, f64) {
    (x + w / 2.0, y + h / 2.0)
}

/// What the canvas reads.
pub struct DiagramCanvas<'a> {
    pub editor: &'a BlockDiagramEditor,
    pub look: CanvasLook,
    /// The Re-sync review's tag words.
    pub tags: PreviewTags,
}

/// What a held button is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Gesture {
    /// Panning from where the press landed, with the camera then.
    Pan { from: Point, camera: Camera },
    /// Moving `block`, grabbed at `from`; `moved` once past the slop.
    Move {
        block: Uuid,
        from: Point,
        offset: (f32, f32),
        moved: bool,
    },
}

/// The canvas state: the gesture held, the modifiers and the last click.
#[derive(Debug, Default)]
pub struct CanvasState {
    gesture: Option<Gesture>,
    modifiers: keyboard::Modifiers,
    clicks: DoubleClick<Uuid>,
}

impl DiagramCanvas<'_> {
    fn diagram(&self) -> &BlockDiagram {
        &self.editor.diagram
    }

    fn fit(&self, size: Size) -> Fit {
        Fit::of(self.diagram(), self.editor.camera, size)
    }

    /// The object under `at`: a port, a block, a connection, a group frame.
    fn hit(&self, fit: Fit, at: Point) -> Option<DiagramItem> {
        let p = fit.to_mm(at);
        let slop = fit.mm(self.look.slop);
        let d = self.diagram();
        if let Some((_, port)) = d.port_at(p, slop) {
            return Some(DiagramItem::Port(port));
        }
        if let Some(block) = d.block_at(p) {
            return Some(DiagramItem::Block(block));
        }
        let shown = |id| d.connection(id).is_some_and(|l| self.look.shows(l));
        if let Some(link) = d.connection_at(p, slop).filter(|id| shown(*id)) {
            return Some(DiagramItem::Connection(link));
        }
        d.group_at(p, slop).map(DiagramItem::Group)
    }

    /// The end a Connect click picks: a port with its block, or a block.
    fn end_at(&self, fit: Fit, at: Point) -> Option<Endpoint> {
        let p = fit.to_mm(at);
        let d = self.diagram();
        if let Some((block, port)) = d.port_at(p, fit.mm(self.look.slop)) {
            return Some(Endpoint {
                block,
                port: Some(port),
            });
        }
        d.block_at(p).map(|block| Endpoint { block, port: None })
    }

    fn pressed(
        &self,
        state: &mut CanvasState,
        fit: Fit,
        at: Point,
        button: mouse::Button,
    ) -> Option<TabEvent> {
        if button == mouse::Button::Middle {
            state.gesture = Some(Gesture::Pan {
                from: at,
                camera: self.editor.camera,
            });
            return None;
        }
        if self.editor.tool == DiagramTool::Connect {
            return self.end_at(fit, at).map(TabEvent::EndPicked);
        }
        match self.hit(fit, at) {
            Some(DiagramItem::Block(block)) => {
                if state.clicks.register(block, Instant::now()) {
                    return Some(TabEvent::BlockOpened(block));
                }
                state.gesture = Some(Gesture::Move {
                    block,
                    from: at,
                    offset: (0.0, 0.0),
                    moved: false,
                });
                None
            }
            Some(item) => Some(TabEvent::ItemPicked {
                item: Some(item),
                add: state.modifiers.shift(),
            }),
            None => {
                state.gesture = Some(Gesture::Pan {
                    from: at,
                    camera: self.editor.camera,
                });
                (!state.modifiers.shift()).then_some(TabEvent::ItemPicked {
                    item: None,
                    add: false,
                })
            }
        }
    }

    fn dragged(&self, state: &mut CanvasState, fit: Fit, at: Point) -> Option<TabEvent> {
        match state.gesture.as_mut()? {
            Gesture::Pan { from, camera } => {
                let base = Fit::of(
                    self.diagram(),
                    *camera,
                    Size::new((fit.mid.0 * 2.0) as f32, (fit.mid.1 * 2.0) as f32),
                );
                Some(TabEvent::ViewChanged(base.panned(
                    *camera,
                    at.x - from.x,
                    at.y - from.y,
                )))
            }
            Gesture::Move {
                from,
                offset,
                moved,
                ..
            } => {
                let (dx, dy) = (at.x - from.x, at.y - from.y);
                if !*moved && dx.hypot(dy) < self.look.slop {
                    return None;
                }
                *moved = true;
                *offset = (dx, dy);
                None
            }
        }
    }

    fn released(&self, state: &mut CanvasState, fit: Fit) -> Option<TabEvent> {
        match state.gesture.take()? {
            Gesture::Pan { .. } => None,
            Gesture::Move {
                block,
                offset,
                moved,
                ..
            } => {
                if !moved {
                    return Some(TabEvent::ItemPicked {
                        item: Some(DiagramItem::Block(block)),
                        add: state.modifiers.shift(),
                    });
                }
                let at = self.diagram().block(block)?.position;
                let to = MmPoint::new(at.x + fit.mm(offset.0), at.y + fit.mm(offset.1));
                Some(TabEvent::BlockMoved { block, to })
            }
        }
    }
}

impl canvas::Program<TabEvent> for DiagramCanvas<'_> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<TabEvent>> {
        let fit = self.fit(bounds.size());
        let event = match event {
            canvas::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.modifiers = *modifiers;
                return None;
            }
            canvas::Event::Mouse(event) => *event,
            _ => return None,
        };
        let message = match event {
            mouse::Event::ButtonPressed(button @ (mouse::Button::Left | mouse::Button::Middle)) => {
                let at = cursor.position_in(bounds)?;
                let message = self.pressed(state, fit, at, button);
                let action =
                    message.map_or_else(canvas::Action::request_redraw, canvas::Action::publish);
                return Some(action.and_capture());
            }
            mouse::Event::CursorMoved { position } => {
                let at = Point::new(position.x - bounds.x, position.y - bounds.y);
                let message = self.dragged(state, fit, at);
                if message.is_none()
                    && matches!(state.gesture, Some(Gesture::Move { moved: true, .. }))
                {
                    return Some(canvas::Action::request_redraw().and_capture());
                }
                message
            }
            mouse::Event::ButtonReleased(mouse::Button::Left | mouse::Button::Middle) => {
                let message = self.released(state, fit);
                if message.is_none() {
                    return Some(canvas::Action::request_redraw());
                }
                message
            }
            mouse::Event::WheelScrolled { delta } => {
                let lines = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => f64::from(y),
                    mouse::ScrollDelta::Pixels { y, .. } => {
                        f64::from(y / (self.look.text_size * 1.5).max(1.0))
                    }
                };
                let at = cursor.position_in(bounds)?;
                Some(TabEvent::ViewChanged(fit.zoomed_at(
                    self.editor.camera,
                    at,
                    ZOOM_STEP.powf(lines),
                )))
            }
            _ => None,
        }?;
        Some(canvas::Action::publish(message).and_capture())
    }

    fn draw(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let size = bounds.size();
        let fit = self.fit(size);
        let mut frame = Frame::new(renderer, size);
        frame.fill_rectangle(Point::ORIGIN, size, self.look.background);
        let moving = match state.gesture {
            Some(Gesture::Move {
                block,
                offset,
                moved: true,
                ..
            }) => Some((block, offset)),
            _ => None,
        };
        let painter = Painter {
            canvas: self,
            fit,
            moving,
        };
        painter.paint(&mut frame);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match state.gesture {
            Some(Gesture::Pan { .. }) => mouse::Interaction::Grabbing,
            Some(Gesture::Move { moved: true, .. }) => mouse::Interaction::Grabbing,
            _ if !cursor.is_over(bounds) => mouse::Interaction::default(),
            _ if self.editor.tool == DiagramTool::Connect => mouse::Interaction::Crosshair,
            _ => mouse::Interaction::default(),
        }
    }
}

/// One frame's drawing.
struct Painter<'a, 'b> {
    canvas: &'a DiagramCanvas<'b>,
    fit: Fit,
    /// The block being dragged and how far, px.
    moving: Option<(Uuid, (f32, f32))>,
}

impl Painter<'_, '_> {
    fn look(&self) -> CanvasLook {
        self.canvas.look
    }

    fn selected(&self, item: DiagramItem) -> bool {
        self.canvas.editor.selection.contains(&item)
    }

    fn paint(&self, frame: &mut Frame) {
        let moved = self.moved_diagram();
        let diagram = moved.as_ref().unwrap_or(self.canvas.diagram());
        for group in &diagram.groups {
            self.draw_group(frame, diagram, group);
        }
        let look = self.look();
        for link in diagram.connections.iter().filter(|l| look.shows(l)) {
            self.draw_connection(frame, diagram, link);
        }
        for block in &diagram.blocks {
            self.draw_block(frame, block);
        }
        if let Some(review) = &self.canvas.editor.review {
            let tags = &self.canvas.tags;
            super::canvas_review::draw_review(frame, self.fit, diagram, review, look, tags);
        }
        if let Some(end) = self.canvas.editor.connect_from {
            self.draw_connect_start(frame, diagram, end);
        }
    }

    /// The diagram as drawn while a block is dragged: that block where the
    /// pointer has it. `None` while nothing is dragged.
    fn moved_diagram(&self) -> Option<BlockDiagram> {
        let (id, (dx, dy)) = self.moving?;
        let mut shown = self.canvas.diagram().clone();
        let block = shown.blocks.iter_mut().find(|b| b.id == id)?;
        block.position.x += self.fit.mm(dx);
        block.position.y += self.fit.mm(dy);
        Some(shown)
    }

    fn text(&self, frame: &mut Frame, content: &str, at: Point, color: Color, size: f32) {
        if content.is_empty() {
            return;
        }
        frame.fill_text(canvas::Text {
            content: content.to_string(),
            position: at,
            color,
            size: iced::Pixels(size),
            align_x: iced::advanced::text::Alignment::Center,
            align_y: alignment::Vertical::Center,
            ..canvas::Text::default()
        });
    }

    fn draw_group(
        &self,
        frame: &mut Frame,
        diagram: &BlockDiagram,
        group: &signex_types::block_diagram::BlockGroup,
    ) {
        let Some(f) = diagram.group_frame(group.id) else {
            return;
        };
        let look = self.look();
        let top_left = self.fit.to_screen(MmPoint::new(f.x, f.y));
        let size = Size::new(
            (f.width * self.fit.scale) as f32,
            (f.height * self.fit.scale) as f32,
        );
        let selected = self.selected(DiagramItem::Group(group.id));
        let color = match (selected, group.color) {
            (true, _) => look.accent,
            (false, Some(c)) => crate::styles::ti(c),
            (false, None) => look.edge,
        };
        let dash = [look.slop * 0.8, look.slop * 0.5];
        let stroke = Stroke {
            line_dash: LineDash {
                segments: &dash,
                offset: 0,
            },
            ..Stroke::default()
                .with_color(color)
                .with_width(if selected { 2.0 } else { 1.0 })
        };
        frame.stroke(&Path::rectangle(top_left, size), stroke);
        let label = Point::new(
            top_left.x + size.width / 2.0,
            top_left.y + look.text_size * 0.7,
        );
        self.text(frame, &group.name, label, color, look.text_size * 0.9);
    }

    fn connection_color(&self, link: &Connection) -> (Color, f32) {
        let look = self.look();
        match link.interface {
            Interface::Power => (look.power, 2.0),
            Interface::Bus => (look.accent, 3.0),
            _ => (look.text_secondary, 1.5),
        }
    }

    fn draw_connection(&self, frame: &mut Frame, diagram: &BlockDiagram, link: &Connection) {
        let Some(route) = diagram.route(link) else {
            return;
        };
        let (mut color, width) = self.connection_color(link);
        if self.selected(DiagramItem::Connection(link.id)) {
            color = self.look().accent;
        }
        let points: Vec<Point> = route.iter().map(|p| self.fit.to_screen(*p)).collect();
        let path = Path::new(|b| {
            if let Some(first) = points.first() {
                b.move_to(*first);
                for point in &points[1..] {
                    b.line_to(*point);
                }
            }
        });
        frame.stroke(&path, Stroke::default().with_color(color).with_width(width));
        if let [.., before, last] = points.as_slice() {
            frame.fill(&arrow_head(*before, *last, self.look().slop * 0.8), color);
        }
        if let Some(at) = diagram.label_point(link) {
            let at = self.fit.to_screen(at);
            let above = Point::new(at.x, at.y - self.look().text_size * 0.7);
            self.text(
                frame,
                &link.label,
                above,
                color,
                self.look().text_size * 0.85,
            );
        }
    }

    fn draw_block(&self, frame: &mut Frame, block: &Block) {
        let look = self.look();
        let b = block.bounds();
        let top_left = self.fit.to_screen(MmPoint::new(b.x, b.y));
        let size = Size::new(
            (b.width * self.fit.scale) as f32,
            (b.height * self.fit.scale) as f32,
        );
        let outline = shape_path(block.shape, top_left, size);
        let fill = block.color.map_or(look.block_fill, |c| Color {
            a: 0.18,
            ..crate::styles::ti(c)
        });
        frame.fill(&outline, fill);
        let selected = self.selected(DiagramItem::Block(block.id));
        let unlinked = block
            .sheet
            .as_ref()
            .is_some_and(|link| link.state == LinkState::Unlinked);
        let edge = match (selected, unlinked, block.color) {
            (true, _, _) => look.accent,
            (false, true, _) => look.unlinked,
            (false, false, Some(c)) => crate::styles::ti(c),
            (false, false, None) => look.edge,
        };
        frame.stroke(
            &outline,
            Stroke::default()
                .with_color(edge)
                .with_width(if selected { 2.0 } else { 1.0 }),
        );
        let cx = top_left.x + size.width / 2.0;
        let line = look.text_size * 1.3;
        let middle = top_left.y + size.height / 2.0;
        let has_function = !block.function.is_empty();
        let name_y = if has_function {
            middle - line / 2.0
        } else {
            middle
        };
        self.text(
            frame,
            &block.name,
            Point::new(cx, name_y),
            look.text,
            look.text_size,
        );
        if has_function {
            let at = Point::new(cx, middle + line / 2.0);
            self.text(
                frame,
                &block.function,
                at,
                look.text_secondary,
                look.text_size * 0.85,
            );
        }
        if !block.note.is_empty() {
            let at = Point::new(cx, top_left.y + size.height + line * 0.7);
            self.text(
                frame,
                &block.note,
                at,
                look.text_secondary,
                look.text_size * 0.8,
            );
        }
        for port in &block.ports {
            let at = self.fit.to_screen(block.port_point(port));
            let half = look.text_size * PORT_EMS;
            let selected = self.selected(DiagramItem::Port(port.id));
            let color = if selected { look.accent } else { edge };
            frame.fill_rectangle(
                Point::new(at.x - half, at.y - half),
                Size::new(half * 2.0, half * 2.0),
                color,
            );
        }
    }

    fn draw_connect_start(&self, frame: &mut Frame, diagram: &BlockDiagram, end: Endpoint) {
        let Some(block) = diagram.block(end.block) else {
            return;
        };
        let at = match end.port.and_then(|p| block.port(p)) {
            Some(port) => block.port_point(port),
            None => block.bounds().centre(),
        };
        let ring = Path::circle(self.fit.to_screen(at), self.look().slop);
        frame.stroke(
            &ring,
            Stroke::default()
                .with_color(self.look().accent)
                .with_width(2.0),
        );
    }
}

/// The outline of a block of `shape` in its box.
fn shape_path(shape: BlockShape, top_left: Point, size: Size) -> Path {
    match shape {
        BlockShape::Rectangle => Path::rectangle(top_left, size),
        BlockShape::RoundedRectangle => {
            let radius = (size.width.min(size.height) * 0.15).max(1.0);
            Path::new(|b| b.rounded_rectangle(top_left, size, radius.into()))
        }
        BlockShape::Ellipse => {
            let centre = Point::new(
                top_left.x + size.width / 2.0,
                top_left.y + size.height / 2.0,
            );
            Path::new(|b| {
                b.ellipse(canvas::path::arc::Elliptical {
                    center: centre,
                    radii: iced::Vector::new(size.width / 2.0, size.height / 2.0),
                    rotation: iced::Radians(0.0),
                    start_angle: iced::Radians(0.0),
                    end_angle: iced::Radians(std::f32::consts::TAU),
                });
            })
        }
        BlockShape::Hexagon => {
            let inset = (size.height / 2.0).min(size.width / 4.0);
            let (x, y, w, h) = (top_left.x, top_left.y, size.width, size.height);
            Path::new(|b| {
                b.move_to(Point::new(x + inset, y));
                b.line_to(Point::new(x + w - inset, y));
                b.line_to(Point::new(x + w, y + h / 2.0));
                b.line_to(Point::new(x + w - inset, y + h));
                b.line_to(Point::new(x + inset, y + h));
                b.line_to(Point::new(x, y + h / 2.0));
                b.close();
            })
        }
    }
}

/// A filled arrow head at `tip`, pointing along `from` -> `tip`.
fn arrow_head(from: Point, tip: Point, length: f32) -> Path {
    let (dx, dy) = (tip.x - from.x, tip.y - from.y);
    let norm = dx.hypot(dy).max(f32::EPSILON);
    let (ux, uy) = (dx / norm, dy / norm);
    let base = Point::new(tip.x - ux * length, tip.y - uy * length);
    let half = length * 0.45;
    Path::new(|b| {
        b.move_to(tip);
        b.line_to(Point::new(base.x - uy * half, base.y + ux * half));
        b.line_to(Point::new(base.x + uy * half, base.y - ux * half));
        b.close();
    })
}

/// The canvas as a widget filling its place.
pub fn diagram_canvas<'a>(
    editor: &'a BlockDiagramEditor,
    look: CanvasLook,
    tags: PreviewTags,
) -> Element<'a, TabEvent> {
    Canvas::new(DiagramCanvas { editor, look, tags })
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}
