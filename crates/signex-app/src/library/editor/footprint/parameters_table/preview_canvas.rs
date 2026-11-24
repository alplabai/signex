//! The row under the parameters table (mockup row 189): the live bake
//! preview card, with each driving dimension's line and `name · value`
//! label, and beside it the narrower Live checks card. The row takes the
//! panel's height left under the table. The preview is drawn fresh from
//! the editor's state on every view, so each keystroke that re-solves
//! redraws it; there is no cache to clear.

use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{column, container, row};
use iced::{Border, Color, Element, Length, Point, Rectangle, Renderer, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_sketch::unit::Quantity;
use signex_types::theme::ThemeTokens;

use super::preview::{BakePreview, Mm, PreviewPad, of_editor};
use super::state::display_value;
use super::view::{Look, caption, unit_id};
use crate::app::FootprintDocument;
use crate::library::messages::LibraryMessage;
use crate::styles::Radii;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The least height of the preview and Live checks cards, in body text
/// heights; they grow into the panel's room.
pub(super) const LIVE_EM: f32 = 13.0;
/// Width shares of the preview and Live checks cards (the mockup's drawing
/// beside a narrower notes column).
pub(super) const PREVIEW_PORTION: u16 = 2;
pub(super) const CHECKS_PORTION: u16 = 1;
/// Edge and pad outline width, in caption heights.
const STROKE_EM: f32 = 0.1;
/// How strongly a pad is filled.
const PAD_FILL_ALPHA: f32 = 0.35;
/// Arrowhead length and half width of a dimension line, in caption heights.
const ARROW_EM: f32 = 0.45;
const ARROW_HALF_EM: f32 = 0.2;
/// A dimension label's horizontal and vertical padding, in caption heights,
/// and the average advance of one of its characters, in ems.
const LABEL_PAD_X_EM: f32 = 0.5;
const LABEL_PAD_Y_EM: f32 = 0.25;
const LABEL_ADVANCE_EM: f32 = 0.6;

/// The preview card and the Live checks card, side by side.
pub(super) fn live_row<'a>(
    editor: &FootprintDocument,
    problems: Vec<Localized>,
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let preview = of_editor(editor);
    let is_last_good = preview.as_ref().is_some_and(|p| p.is_last_good);
    let checks = super::checks::section(problems, &editor.state.pads, look, i18n);
    row![
        live_card(
            preview_card(preview, look, tokens, i18n),
            PREVIEW_PORTION,
            is_last_good,
            look
        ),
        live_card(checks, CHECKS_PORTION, false, look),
    ]
    .spacing(SP.m)
    .height(Length::Fill.min(TS.body * LIVE_EM))
    .boxed()
}

/// One of the two cards, `portion` shares wide, bordered in the error
/// colour when `is_marked`.
fn live_card<'a>(
    content: Element<'a, LibraryMessage>,
    portion: u16,
    is_marked: bool,
    look: Look,
) -> Element<'a, LibraryMessage> {
    let border = if is_marked { look.error } else { look.line };
    container(content)
        .padding(SP.s)
        .width(Length::FillPortion(portion))
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color: border,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn preview_card<'a>(
    preview: Option<BakePreview>,
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut card = column![caption(
        t!(i18n, ids::SKETCH_PARAMETERS_PREVIEW_TITLE),
        look.muted
    )]
    .spacing(SP.xs);
    let Some(preview) = preview else {
        return card
            .push(caption(
                t!(i18n, ids::SKETCH_PARAMETERS_PREVIEW_NONE),
                look.muted,
            ))
            .boxed();
    };
    if preview.is_last_good {
        card = card.push(caption(
            t!(i18n, ids::SKETCH_PARAMETERS_PREVIEW_LAST_GOOD),
            look.error,
        ));
    }
    if preview.pads_hidden {
        card = card.push(caption(
            t!(i18n, ids::SKETCH_PARAMETERS_PREVIEW_PADS_HIDDEN),
            look.error,
        ));
    }
    if preview.extent().is_none() {
        if !preview.pads_hidden {
            card = card.push(caption(
                t!(i18n, ids::SKETCH_PARAMETERS_PREVIEW_EMPTY),
                look.muted,
            ));
        }
        return card.boxed();
    }
    let labels = preview
        .dimensions
        .iter()
        .map(|d| dimension_label(d, i18n).into_string())
        .collect();
    let r = crate::styles::tokens::resolve(tokens);
    let drawing = PreviewDrawing {
        preview,
        labels,
        ground: r.field,
        edge: look.ink,
        pad: look.name,
        label: look.muted,
        last_good: look.error,
    };
    card.push(
        canvas::Canvas::new(drawing)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
    )
    .boxed()
}

/// `name = value unit`, or the value alone when no parameter drives it.
fn dimension_label(d: &super::preview::DrivingDimension, i18n: &I18n) -> Localized {
    let quantity = if d.is_angle {
        Quantity::angle(d.value)
    } else {
        Quantity::length(d.value)
    };
    let (value, unit) = display_value(quantity);
    let unit = t!(i18n, unit_id(unit)).into_string();
    match &d.name {
        Some(name) => t!(
            i18n,
            ids::SKETCH_PARAMETERS_PREVIEW_DIMENSION_NAMED,
            "name" => name.as_str(),
            "value" => value,
            "unit" => unit
        ),
        None => t!(
            i18n,
            ids::SKETCH_PARAMETERS_PREVIEW_DIMENSION,
            "value" => value,
            "unit" => unit
        ),
    }
}

/// The preview as a canvas program.
struct PreviewDrawing {
    preview: BakePreview,
    /// One label per driving dimension, in order.
    labels: Vec<String>,
    ground: Color,
    edge: Color,
    pad: Color,
    label: Color,
    last_good: Color,
}

impl canvas::Program<LibraryMessage> for PreviewDrawing {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.ground);
        let Some((lo, hi)) = self.preview.extent() else {
            return vec![frame.into_geometry()];
        };
        let inset = TS.body;
        let (w, h) = ((hi.0 - lo.0).max(1e-3), (hi.1 - lo.1).max(1e-3));
        let room = |px: f32| f64::from((px - 2.0 * inset).max(1.0));
        let scale = (room(bounds.width) / w).min(room(bounds.height) / h);
        // Centred in the card.
        let pad_x = (f64::from(bounds.width) - w * scale) / 2.0;
        let pad_y = (f64::from(bounds.height) - h * scale) / 2.0;
        let at = |p: Mm| {
            Point::new(
                (pad_x + (p.0 - lo.0) * scale) as f32,
                (pad_y + (p.1 - lo.1) * scale) as f32,
            )
        };
        let line = |points: &[Mm], close: bool| {
            Path::new(|b| {
                if let Some((first, rest)) = points.split_first() {
                    b.move_to(at(*first));
                    for p in rest {
                        b.line_to(at(*p));
                    }
                    if close {
                        b.close();
                    }
                }
            })
        };
        let stroke = |color: Color| {
            Stroke::default()
                .with_width(TS.caption * STROKE_EM)
                .with_color(color)
        };
        let fill = Color {
            a: PAD_FILL_ALPHA,
            ..self.pad
        };
        for pad in &self.preview.pads {
            let path = match pad {
                PreviewPad::Round { centre, radius } => {
                    Path::circle(at(*centre), (radius * scale) as f32)
                }
                PreviewPad::Outline(points) => line(points, true),
            };
            frame.fill(&path, fill);
            frame.stroke(&path, stroke(self.pad));
        }
        for edge in &self.preview.edges {
            frame.stroke(&line(edge, false), stroke(self.edge));
        }
        for (dimension, label) in self.preview.dimensions.iter().zip(&self.labels) {
            let color = if dimension.is_last_good {
                self.last_good
            } else {
                self.label
            };
            if let Some((a, b)) = dimension.span {
                let (a, b) = (at(a), at(b));
                frame.stroke(&Path::line(a, b), stroke(color));
                if a.distance(b) > 2.0 * TS.caption * ARROW_EM {
                    for (tip, tail) in [(a, b), (b, a)] {
                        let [p, q, r] = arrow_head(tip, tail);
                        let head = Path::new(|path| {
                            path.move_to(p);
                            path.line_to(q);
                            path.line_to(r);
                            path.close();
                        });
                        frame.fill(&head, color);
                    }
                }
            }
            let centre = at(dimension.at);
            let rect = label_rect(centre, label);
            let pill =
                Path::rounded_rectangle(rect.position(), rect.size(), Radii::GRAPHITE.card.into());
            frame.fill(&pill, self.ground);
            frame.stroke(&pill, stroke(color));
            frame.fill_text(canvas::Text {
                content: label.clone(),
                position: centre,
                color,
                size: iced::Pixels(TS.caption),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
        }
        vec![frame.into_geometry()]
    }
}

/// The arrowhead at `tip` of a dimension line coming from `tail`: the tip
/// and the two back corners.
pub(super) fn arrow_head(tip: Point, tail: Point) -> [Point; 3] {
    let (dx, dy) = (tail.x - tip.x, tail.y - tip.y);
    let len = dx.hypot(dy).max(f32::EPSILON);
    let (ux, uy) = (dx / len, dy / len);
    let (back, half) = (TS.caption * ARROW_EM, TS.caption * ARROW_HALF_EM);
    let base = Point::new(tip.x + ux * back, tip.y + uy * back);
    [
        tip,
        Point::new(base.x - uy * half, base.y + ux * half),
        Point::new(base.x + uy * half, base.y - ux * half),
    ]
}

/// The pill a dimension label reading `text` sits in, centred on `centre`.
pub(super) fn label_rect(centre: Point, text: &str) -> Rectangle {
    let chars = text.chars().count() as f32;
    let width = chars * TS.caption * LABEL_ADVANCE_EM + 2.0 * TS.caption * LABEL_PAD_X_EM;
    let height = TS.caption * (1.0 + 2.0 * LABEL_PAD_Y_EM);
    Rectangle::new(
        Point::new(centre.x - width / 2.0, centre.y - height / 2.0),
        iced::Size::new(width, height),
    )
}
