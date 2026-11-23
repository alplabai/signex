//! The drawing on a Board Print page card (mockup row 116): a schematic board
//! in the page's layer colour, live with the print options. It is the same
//! sketch for every board, not a rendered thumbnail.

use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Widget as _};
use signex_output::{BoardLayerSet, BoardPrintOptions};
use signex_types::pcb_layer::{CopperLayer, InnerCopper, PcbLayer, TechnicalLayer};

use crate::app::Message;
use crate::pcb_canvas::layer_style::board_layer_colour;
use crate::styles::chrome_px;

/// The drawing's own coordinates, as mockup row 116 draws it.
const ART_W: f32 = 150.0;
const ART_H: f32 = 100.0;
/// Drawing px per art unit at the default UI size.
const ART_SCALE: f32 = chrome_px(11.0 / 12.0);
const BOARD_ORIGIN: (f32, f32) = (20.0, 15.0);
const BOARD_SIZE: (f32, f32) = (110.0, 70.0);
const BOARD_RADIUS: f32 = 4.0;
const TRACES: [&[(f32, f32)]; 3] = [
    &[(35.0, 30.0), (75.0, 30.0), (75.0, 55.0), (105.0, 55.0)],
    &[(35.0, 70.0), (95.0, 70.0)],
    &[(110.0, 30.0), (110.0, 70.0)],
];
/// Pads: x, y, width, height.
const PADS: [(f32, f32, f32, f32); 2] = [(40.0, 40.0, 14.0, 10.0), (88.0, 55.0, 10.0, 16.0)];
const HOLES: [(f32, f32); 4] = [(30.0, 25.0), (120.0, 25.0), (30.0, 75.0), (120.0, 75.0)];
const HOLE_R: f32 = 2.0;
const TRACE_W: f32 = 2.2;
const OUTLINE_W: f32 = 1.2;
/// The title block stub in the page's corner, and its inset from the edges.
const TITLE_SIZE: (f32, f32) = (34.0, 10.0);
const TITLE_INSET: f32 = 6.0;
/// Inks of the printed page, which is white in both themes.
const OUTLINE_INK: Color = Color::from_rgb8(0x99, 0x99, 0x99);
const HOLE_INK: Color = Color::from_rgb8(0x55, 0x55, 0x55);
const TITLE_INK: Color = Color::from_rgb8(0xBB, 0xBB, 0xBB);
/// The assembly colours are made for the dark board; on white paper they
/// take this share of their channels to stay legible.
const ASSEMBLY_ON_PAPER: f32 = 0.8;

/// What a card's drawing shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PageArt {
    pub ink: Color,
    pub mirrored: bool,
    pub outline: bool,
    pub drills: bool,
    pub title_block: bool,
}

impl PageArt {
    /// The drawing of a page of `sets`; a page of several sets takes the
    /// first set's colour.
    pub(super) fn of(options: &BoardPrintOptions, sets: &[BoardLayerSet], mirrored: bool) -> Self {
        let ink = match sets.first() {
            Some(set) if !options.monochrome => set_ink(*set),
            _ => Color::BLACK,
        };
        Self {
            ink,
            mirrored,
            outline: options.outline,
            drills: options.drill_marks,
            title_block: options.title_block,
        }
    }

    pub(super) fn view<'a>(self) -> Element<'a, Message> {
        canvas::Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    }
}

/// The layer whose colour stands for `set`.
fn set_layer(set: BoardLayerSet) -> PcbLayer {
    match set {
        BoardLayerSet::Top => PcbLayer::Copper(CopperLayer::Top),
        BoardLayerSet::Bottom => PcbLayer::Copper(CopperLayer::Bottom),
        BoardLayerSet::Inner => {
            PcbLayer::Copper(InnerCopper::new(1).map_or(CopperLayer::Top, CopperLayer::Inner))
        }
        BoardLayerSet::Mechanical => PcbLayer::Technical(TechnicalLayer::EdgeCuts),
        BoardLayerSet::AssemblyTop => PcbLayer::Technical(TechnicalLayer::AssemblyTop),
        BoardLayerSet::AssemblyBottom => PcbLayer::Technical(TechnicalLayer::AssemblyBottom),
    }
}

/// The opaque colour `set` draws in on white paper.
fn set_ink(set: BoardLayerSet) -> Color {
    let colour = board_layer_colour(set_layer(set));
    let share = if set.assembly_side().is_some() {
        ASSEMBLY_ON_PAPER
    } else {
        1.0
    };
    Color {
        r: colour.r * share,
        g: colour.g * share,
        b: colour.b * share,
        a: 1.0,
    }
}

/// An art x as drawn: a mirrored page reads from the other side.
pub(super) fn art_x(x: f32, mirrored: bool) -> f32 {
    if mirrored { ART_W - x } else { x }
}

/// Where the art sits in the card.
#[derive(Clone, Copy)]
struct Placement {
    origin: Point,
    scale: f32,
    mirrored: bool,
}

impl Placement {
    fn of(bounds: Size, mirrored: bool) -> Self {
        let scale = ART_SCALE
            .min(bounds.width / ART_W)
            .min(bounds.height / ART_H);
        let origin = Point::new(
            (bounds.width - ART_W * scale) / 2.0,
            (bounds.height - ART_H * scale) / 2.0,
        );
        Self {
            origin,
            scale,
            mirrored,
        }
    }

    fn at(self, (x, y): (f32, f32)) -> Point {
        Point::new(
            self.origin.x + art_x(x, self.mirrored) * self.scale,
            self.origin.y + y * self.scale,
        )
    }
}

/// The board outline; it is symmetric, so a mirrored page draws it the same.
fn draw_outline(frame: &mut Frame, place: Placement) {
    let top_left = Point::new(
        place.origin.x + BOARD_ORIGIN.0 * place.scale,
        place.origin.y + BOARD_ORIGIN.1 * place.scale,
    );
    let size = Size::new(BOARD_SIZE.0 * place.scale, BOARD_SIZE.1 * place.scale);
    let path = Path::rounded_rectangle(top_left, size, (BOARD_RADIUS * place.scale).into());
    frame.stroke(
        &path,
        Stroke::default()
            .with_width(OUTLINE_W * place.scale)
            .with_color(OUTLINE_INK),
    );
}

/// The copper: traces and pads in the page's ink.
fn draw_copper(frame: &mut Frame, place: Placement, ink: Color) {
    let stroke = Stroke::default()
        .with_width(TRACE_W * place.scale)
        .with_color(ink);
    for trace in TRACES {
        let path = Path::new(|b| {
            if let Some((first, rest)) = trace.split_first() {
                b.move_to(place.at(*first));
                for point in rest {
                    b.line_to(place.at(*point));
                }
            }
        });
        frame.stroke(&path, stroke);
    }
    for (x, y, w, h) in PADS {
        let path = Path::new(|b| {
            b.move_to(place.at((x, y)));
            b.line_to(place.at((x + w, y)));
            b.line_to(place.at((x + w, y + h)));
            b.line_to(place.at((x, y + h)));
            b.close();
        });
        frame.fill(&path, ink);
    }
}

/// The title block stub, in the page's corner on every page.
fn draw_title_block(frame: &mut Frame, bounds: Size, scale: f32) {
    let size = Size::new(TITLE_SIZE.0 * scale, TITLE_SIZE.1 * scale);
    let inset = TITLE_INSET * scale;
    let top_left = Point::new(
        bounds.width - size.width - inset,
        bounds.height - size.height - inset,
    );
    frame.stroke(
        &Path::rectangle(top_left, size),
        Stroke::default().with_width(1.0).with_color(TITLE_INK),
    );
}

impl canvas::Program<Message> for PageArt {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let size = bounds.size();
        let mut frame = Frame::new(renderer, size);
        let place = Placement::of(size, self.mirrored);
        if self.outline {
            draw_outline(&mut frame, place);
        }
        draw_copper(&mut frame, place, self.ink);
        if self.drills {
            for hole in HOLES {
                let path = Path::circle(place.at(hole), HOLE_R * place.scale);
                frame.fill(&path, HOLE_INK);
            }
        }
        if self.title_block {
            draw_title_block(&mut frame, size, place.scale);
        }
        vec![frame.into_geometry()]
    }
}
