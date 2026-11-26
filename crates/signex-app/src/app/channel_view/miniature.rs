//! A channel card's drawing (#2658): the compiled channel's wires, buses
//! and part bodies scaled to fit the card, with the designators and net
//! names that channel has, drawn at a readable size whatever the scale.
//! The data is built by the compile (`signex_net::sheet_miniature`); this
//! only draws it.

use std::collections::HashSet;
use std::sync::Arc;

use iced::alignment::{Horizontal, Vertical};
use iced::widget::canvas;
use iced::{Color, Element, Length, Point, Rectangle, Size, Theme, Widget as _, mouse};
use signex_net::{MiniatureText, SheetMiniature};
use signex_types::schematic::Aabb;

use crate::panels::PanelContext;
use crate::styles::{HAIRLINE, Radii, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
/// Height of the drawing: 120 px at the 12 px body.
pub(super) const MINIATURE_H: f32 = TS.body * 10.0;
/// Clear space inside the drawing's edge: 6 px at the 12 px body.
const INSET: f32 = TS.body * 0.5;
/// Size of a designator or net name on the drawing: 9 px at the 12 px
/// body. It does not scale with the drawing, so a small card stays legible.
const LABEL: f32 = TS.body * 0.75;

/// Where the sheet sits in the drawing: px per mm and where the top-left
/// corner of the miniature's bounds lands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Fit {
    pub scale: f32,
    pub origin: Point,
    /// The bounds' top-left corner in sheet mm.
    min: (f64, f64),
}

impl Fit {
    /// `bounds` fitted into `area` inside the inset, its shape kept, centred.
    pub(super) fn of(bounds: &Aabb, area: Size) -> Self {
        let w = (bounds.max_x - bounds.min_x).max(1.0) as f32;
        let h = (bounds.max_y - bounds.min_y).max(1.0) as f32;
        let room = Size::new(
            (area.width - 2.0 * INSET).max(1.0),
            (area.height - 2.0 * INSET).max(1.0),
        );
        let scale = (room.width / w).min(room.height / h);
        Self {
            scale,
            origin: Point::new(
                (area.width - w * scale) / 2.0,
                (area.height - h * scale) / 2.0,
            ),
            min: (bounds.min_x, bounds.min_y),
        }
    }

    /// A sheet point in the drawing.
    pub(super) fn at(&self, p: signex_types::schematic::Point) -> Point {
        Point::new(
            self.origin.x + (p.x - self.min.0) as f32 * self.scale,
            self.origin.y + (p.y - self.min.1) as f32 * self.scale,
        )
    }
}

/// The colours of the drawing, the schematic canvas's own.
#[derive(Debug, Clone, Copy)]
struct Ink {
    paper: Color,
    wire: Color,
    bus: Color,
    body: Color,
    designator: Color,
    net: Color,
    power: Color,
    /// A designator whose part the active variant leaves out (#2662).
    left_out: Color,
}

/// The drawing. Stateless; rebuilt by every view.
struct Miniature {
    data: Arc<SheetMiniature>,
    ink: Ink,
    /// The designators drawn in `Ink::left_out`.
    left_out: HashSet<String>,
}

impl<M> canvas::Program<M> for Miniature {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let ground = canvas::Path::rounded_rectangle(
            Point::ORIGIN,
            bounds.size(),
            Radii::GRAPHITE.control.into(),
        );
        frame.fill(&ground, self.ink.paper);
        let fit = Fit::of(&self.data.bounds, bounds.size());
        let line = |color: Color, width: f32| {
            canvas::Stroke::default()
                .with_color(color)
                .with_width(width)
        };
        for (a, b) in &self.data.wires {
            let path = canvas::Path::line(fit.at(*a), fit.at(*b));
            frame.stroke(&path, line(self.ink.wire, HAIRLINE));
        }
        for (a, b) in &self.data.buses {
            let path = canvas::Path::line(fit.at(*a), fit.at(*b));
            frame.stroke(&path, line(self.ink.bus, 2.0 * HAIRLINE));
        }
        for body in &self.data.bodies {
            let corner = fit.at(signex_types::schematic::Point::new(body.min_x, body.min_y));
            let size = Size::new(
                (body.max_x - body.min_x) as f32 * fit.scale,
                (body.max_y - body.min_y) as f32 * fit.scale,
            );
            let path = canvas::Path::rectangle(corner, size);
            frame.stroke(&path, line(self.ink.body, HAIRLINE));
        }
        for t in &self.data.nets {
            let color = if t.is_power {
                self.ink.power
            } else {
                self.ink.net
            };
            label(&mut frame, &fit, t, color, Vertical::Bottom);
        }
        for t in &self.data.designators {
            let color = designator_ink(&self.ink, &self.left_out, &t.text);
            label(&mut frame, &fit, t, color, Vertical::Center);
        }
        vec![frame.into_geometry()]
    }
}

/// The ink of the designator `text` on the drawing: greyed when its part is
/// one the variant leaves out (`left_out`, from `fit::left_out`).
fn designator_ink(ink: &Ink, left_out: &HashSet<String>, text: &str) -> Color {
    if left_out.contains(text) {
        ink.left_out
    } else {
        ink.designator
    }
}

/// One name, centred across its point.
fn label(frame: &mut canvas::Frame, fit: &Fit, t: &MiniatureText, color: Color, y: Vertical) {
    frame.fill_text(canvas::Text {
        content: t.text.clone(),
        position: fit.at(t.at),
        color,
        size: iced::Pixels(LABEL),
        font: crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT),
        align_x: Horizontal::Center.into(),
        align_y: y,
        ..canvas::Text::default()
    });
}

/// The drawing of `data`, as wide as the card; the designators in
/// `left_out` are greyed.
pub(super) fn view<'a, M: 'a>(
    ctx: &PanelContext,
    data: &Arc<SheetMiniature>,
    left_out: HashSet<String>,
) -> Element<'a, M> {
    let colors = &ctx.canvas_colors;
    let program = Miniature {
        data: Arc::clone(data),
        ink: Ink {
            paper: ti(colors.paper),
            wire: ti(colors.wire),
            bus: ti(colors.bus),
            body: ti(colors.body),
            designator: ti(colors.reference),
            net: ti(colors.net_label),
            power: ti(colors.power),
            left_out: super::fit::faded(resolve(&ctx.tokens).ink4),
        },
        left_out,
    };
    canvas(program)
        .width(Length::Fill)
        .height(Length::Fixed(MINIATURE_H))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(w: f64, h: f64) -> Aabb {
        Aabb::new(10.0, 20.0, 10.0 + w, 20.0 + h)
    }

    /// The whole miniature lands inside the drawing, inside the inset.
    #[test]
    fn the_fit_keeps_every_corner_inside_the_inset() {
        let area = Size::new(240.0, MINIATURE_H);
        for b in [bounds(100.0, 20.0), bounds(20.0, 100.0), bounds(50.0, 50.0)] {
            let fit = Fit::of(&b, area);
            for (x, y) in [(b.min_x, b.min_y), (b.max_x, b.max_y)] {
                let p = fit.at(signex_types::schematic::Point::new(x, y));
                assert!(
                    p.x >= INSET - 1e-3 && p.x <= area.width - INSET + 1e-3,
                    "{p:?}"
                );
                assert!(
                    p.y >= INSET - 1e-3 && p.y <= area.height - INSET + 1e-3,
                    "{p:?}"
                );
            }
        }
    }

    /// #2662: a part the variant leaves out in this channel draws its
    /// designator greyed on the card; a fitted one keeps the designator ink.
    #[test]
    fn a_left_out_part_draws_its_designator_greyed() {
        use crate::panels::compiled_design::{ChannelPartFit, ChannelTabView};
        use signex_types::project::PartRef;
        use signex_types::schematic::ChannelOccurrence;
        let fit = |designator: &str, is_fitted: bool| ChannelPartFit {
            part: PartRef::new(designator.to_string(), None),
            is_fitted,
        };
        let tab = ChannelTabView {
            channel: ChannelOccurrence {
                name: "CH2".into(),
                index: 2,
                number: 2,
                of: 2,
            },
            parts: Some(2),
            nets: Some(1),
            designators: vec!["CH2_C6".into(), "CH2_R5".into()],
            miniature: None,
            fits: vec![fit("CH2_C6", false), fit("CH2_R5", true)],
        };
        let grey = Color::from_rgb(0.5, 0.5, 0.5);
        let ink = Ink {
            paper: Color::WHITE,
            wire: Color::BLACK,
            bus: Color::BLACK,
            body: Color::BLACK,
            designator: Color::BLACK,
            net: Color::BLACK,
            power: Color::BLACK,
            left_out: grey,
        };
        let left_out = super::super::fit::left_out(&tab);
        assert_eq!(designator_ink(&ink, &left_out, "CH2_C6"), grey);
        assert_eq!(designator_ink(&ink, &left_out, "CH2_R5"), Color::BLACK);
    }

    /// The sheet keeps its shape and sits in the middle.
    #[test]
    fn the_fit_keeps_the_shape_and_centres_it() {
        let area = Size::new(240.0, MINIATURE_H);
        let b = bounds(100.0, 20.0);
        let fit = Fit::of(&b, area);
        let a = fit.at(signex_types::schematic::Point::new(b.min_x, b.min_y));
        let z = fit.at(signex_types::schematic::Point::new(b.max_x, b.max_y));
        let ratio = (z.x - a.x) / (z.y - a.y);
        assert!((ratio - 5.0).abs() < 1e-3, "{ratio}");
        assert!(((a.x + z.x) / 2.0 - area.width / 2.0).abs() < 1e-3);
        assert!(((a.y + z.y) / 2.0 - area.height / 2.0).abs() < 1e-3);
    }
}
