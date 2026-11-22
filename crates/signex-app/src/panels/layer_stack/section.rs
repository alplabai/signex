//! The two drawings of the Stack page (mockup row 105): the cross-section,
//! each row as tall as its share of the stack with the drill spans over it,
//! and an oblique preview of the board slab. Both follow the stack live. A row
//! with no thickness (paste, silkscreen) is drawn as a mark at its place.

use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::widget::container;
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, Widget as _, mouse};
use signex_types::pcb_stack::{LayerStack, StackLayerKind};

use super::{LayerStackFacts, TS};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{HAIRLINE, ti, tokens::resolve};

/// Height of the preview, in body-text ems.
const PREVIEW_EMS: f32 = 12.0;
/// Height of a mark for a row with no thickness, as a share of the drawing.
const MARK_SHARE: f32 = 0.006;
/// Physical material colours: the same in both themes, as on a real board.
const COPPER: Color = Color::from_rgb(0.78, 0.63, 0.17);
const DIELECTRIC: Color = Color::from_rgb(0.72, 0.61, 0.38);
const MASK: Color = Color::from_rgb(0.18, 0.42, 0.23);
const PASTE: Color = Color::from_rgb(0.62, 0.62, 0.65);
const SILK: Color = Color::from_rgb(0.95, 0.95, 0.95);
/// The side of the slab, darker than its top.
const SHADE: f32 = 0.7;

/// One row of the stack in the drawing, as shares of its height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Band {
    pub top: f32,
    pub height: f32,
    pub kind: StackLayerKind,
}

/// The rows of `stack`, top to bottom, to scale; a row with no thickness
/// takes a thin mark. The bands fill the drawing exactly.
pub(super) fn bands(stack: &LayerStack) -> Vec<Band> {
    let total = stack.total_thickness_mm();
    let marks = stack
        .layers
        .iter()
        .filter(|l| l.thickness_mm <= 0.0)
        .count();
    let drawn = (1.0 - MARK_SHARE * marks as f32).max(0.0);
    let mut top = 0.0;
    stack
        .layers
        .iter()
        .map(|layer| {
            let height = if layer.thickness_mm > 0.0 && total > 0.0 {
                (layer.thickness_mm / total) as f32 * drawn
            } else {
                MARK_SHARE
            };
            let band = Band {
                top,
                height,
                kind: layer.kind,
            };
            top += height;
            band
        })
        .collect()
}

fn colour(kind: StackLayerKind) -> Color {
    match kind {
        StackLayerKind::Signal | StackLayerKind::Plane => COPPER,
        StackLayerKind::Dielectric => DIELECTRIC,
        StackLayerKind::SolderMask => MASK,
        StackLayerKind::Paste => PASTE,
        StackLayerKind::Overlay => SILK,
    }
}

fn shaded(c: Color) -> Color {
    Color::from_rgb(c.r * SHADE, c.g * SHADE, c.b * SHADE)
}

/// A drill span over the section, as shares of its height.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Span {
    top: f32,
    bottom: f32,
}

/// Where each drill pair runs, from the copper bands it joins.
fn spans(facts: &LayerStackFacts, bands: &[Band]) -> Vec<Span> {
    let copper: Vec<&Band> = bands.iter().filter(|b| b.kind.is_copper()).collect();
    facts
        .drill_pairs
        .iter()
        .filter_map(|pair| {
            let first = copper.get(pair.span.0.checked_sub(1)?)?;
            let last = copper.get(pair.span.1.checked_sub(1)?)?;
            Some(Span {
                top: first.top,
                bottom: last.top + last.height,
            })
        })
        .collect()
}

struct CrossSection {
    bands: Vec<Band>,
    spans: Vec<Span>,
    drill: Color,
}

impl<M> canvas::Program<M> for CrossSection {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let (w, h) = (bounds.width, bounds.height);
        for band in &self.bands {
            frame.fill_rectangle(
                Point::new(0.0, band.top * h),
                Size::new(w, (band.height * h).max(HAIRLINE)),
                colour(band.kind),
            );
        }
        let count = self.spans.len() as f32;
        for (i, span) in self.spans.iter().enumerate() {
            let x = w * (i as f32 + 1.0) / (count + 1.0);
            let line = Path::line(Point::new(x, span.top * h), Point::new(x, span.bottom * h));
            frame.stroke(
                &line,
                Stroke::default()
                    .with_color(self.drill)
                    .with_width(2.0 * HAIRLINE),
            );
        }
        vec![frame.into_geometry()]
    }
}

struct Slab {
    bands: Vec<Band>,
}

impl<M> canvas::Program<M> for Slab {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let (w, h) = (bounds.width, bounds.height);
        // The slab's top face leans back; its front edge shows the layers.
        let (inset, front, edge) = (w * 0.18, h * 0.62, h * 0.22);
        let top = Path::new(|p| {
            p.move_to(Point::new(inset, 0.0));
            p.line_to(Point::new(w, 0.0));
            p.line_to(Point::new(w - inset, front));
            p.line_to(Point::new(0.0, front));
            p.close();
        });
        frame.fill(&top, MASK);
        for band in &self.bands {
            frame.fill_rectangle(
                Point::new(0.0, front + band.top * edge),
                Size::new(w - inset, (band.height * edge).max(HAIRLINE)),
                shaded(colour(band.kind)),
            );
        }
        vec![frame.into_geometry()]
    }
}

pub(super) fn cross_section<'a>(
    ctx: &'a PanelContext,
    facts: &'a LayerStackFacts,
) -> Element<'a, PanelMsg> {
    let bands = bands(&facts.stack);
    let spans = spans(facts, &bands);
    let program = CrossSection {
        bands,
        spans,
        drill: ti(ctx.tokens.text),
    };
    let drawing = canvas::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fill);
    framed(ctx, iced::Widget::boxed(drawing), Length::Fill)
}

pub(super) fn preview<'a>(
    ctx: &'a PanelContext,
    facts: &'a LayerStackFacts,
) -> Element<'a, PanelMsg> {
    let program = Slab {
        bands: bands(&facts.stack),
    };
    let height = Length::Fixed(TS.body * PREVIEW_EMS);
    let drawing = canvas::Canvas::new(program)
        .width(Length::Fill)
        .height(height);
    framed(ctx, iced::Widget::boxed(drawing), height)
}

/// A drawing inside a hairline frame.
fn framed<'a>(
    ctx: &PanelContext,
    drawing: Element<'a, PanelMsg>,
    height: Length,
) -> Element<'a, PanelMsg> {
    let line = resolve(&ctx.tokens).line_strong;
    container(drawing)
        .width(Length::Fill)
        .height(height)
        .style(move |_: &Theme| container::Style {
            border: iced::Border {
                color: line,
                width: HAIRLINE,
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_are_to_scale_and_fill_the_drawing() {
        let stack = LayerStack::derived(&[], 1.6);
        let shown = bands(&stack);
        assert_eq!(shown.len(), stack.layers.len());
        let end = shown.last().map(|b| b.top + b.height).unwrap();
        assert!((end - 1.0).abs() < 1e-5, "{end}");
        let of = |kind| shown.iter().find(|b| b.kind == kind).unwrap().height;
        let ratio = of(StackLayerKind::Dielectric) / of(StackLayerKind::Signal);
        let want = stack
            .layers
            .iter()
            .find(|l| l.kind == StackLayerKind::Dielectric)
            .unwrap()
            .thickness_mm
            / signex_types::pcb_stack::DEFAULT_COPPER_MM;
        assert!(
            (f64::from(ratio) - want).abs() < 1e-2,
            "{ratio} against {want}"
        );
    }

    #[test]
    fn a_row_with_no_thickness_is_still_drawn() {
        let stack = LayerStack::derived(&[], 1.6);
        let shown = bands(&stack);
        let silk = shown
            .iter()
            .find(|b| b.kind == StackLayerKind::Overlay)
            .unwrap();
        assert!(silk.height > 0.0);
    }
}
