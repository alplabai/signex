//! The net colour dialogs: the F5 palette (#71) and the custom picker (#72),
//! with the swatch both draw.

#[cfg(test)]
mod chooser_press_tests;
mod custom_picker;
mod palette;
mod row_flyout;

use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::widget::{Space, Widget, canvas as canvas_widget, container};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Theme, mouse};
use signex_types::theme::ThemeTokens;
use signex_widgets::button;

use crate::app::Message;
use crate::styles::{Radii, Spacing, chrome_px, ti, tokens::resolve};

const SP: Spacing = Spacing::DEFAULT;

/// Side of a palette swatch: 16 px at the default text size.
pub(super) const SWATCH: f32 = chrome_px(16.0 * 11.0 / 12.0);

/// A colour swatch `side` square, or the hatched "no colour" one.
pub(super) fn swatch<'a>(
    color: Option<Color>,
    side: f32,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let line = ti(tokens.border);
    match color {
        Some(fill) => container(Space::new())
            .width(Length::Fixed(side))
            .height(Length::Fixed(side))
            .style(move |_: &Theme| container::Style {
                background: Some(fill.into()),
                border: iced::Border {
                    width: 1.0,
                    radius: Radii::GRAPHITE.small.min(side / 4.0).into(),
                    color: line,
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
        None => hatched(Length::Fixed(side), Length::Fixed(side), tokens),
    }
}

/// A hatched area: diagonal hairlines in the faint ink inside a hairline
/// frame, the mark of "no colour".
pub(super) fn hatched<'a>(
    width: Length,
    height: Length,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    canvas_widget(Hatch {
        ink: resolve(tokens).ink4,
        edge: ti(tokens.border),
    })
    .width(width)
    .height(height)
    .boxed()
}

/// A swatch button that sends `message`; `selected` draws the accent ring.
pub(super) fn swatch_button<'a>(
    fill: Color,
    width: f32,
    height: f32,
    selected: bool,
    message: Message,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ring = if selected {
        ti(tokens.accent)
    } else {
        ti(tokens.border)
    };
    let ring_w = if selected { 2.0 } else { 1.0 };
    button(
        container(Space::new())
            .width(Length::Fixed(width))
            .height(Length::Fixed(height))
            .style(move |_: &Theme| container::Style {
                background: Some(fill.into()),
                border: iced::Border {
                    width: ring_w,
                    radius: Radii::GRAPHITE.small.min(height / 4.0).into(),
                    color: ring,
                    ..iced::Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding(0)
    .on_press(message)
    .style(|_: &Theme, _| iced::widget::button::Style {
        background: None,
        ..iced::widget::button::Style::default()
    })
    .boxed()
}

/// The hatch program: no state, draws from its bounds.
struct Hatch {
    ink: Color,
    edge: Color,
}

impl canvas::Program<Message> for Hatch {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let (w, h) = (bounds.width, bounds.height);
        let step = SP.s.max(1.0);
        // 45° lines `step` apart; the canvas clips them to its bounds.
        let mut o = step;
        while o < w + h {
            frame.stroke(
                &Path::line(Point::new(o, 0.0), Point::new(o - h, h)),
                Stroke::default().with_color(self.ink).with_width(1.0),
            );
            o += step;
        }
        let edge = Stroke::default().with_color(self.edge).with_width(1.0);
        frame.stroke(
            &Path::rectangle(
                Point::new(0.5, 0.5),
                iced::Size::new((w - 1.0).max(0.0), (h - 1.0).max(0.0)),
            ),
            edge,
        );
        vec![frame.into_geometry()]
    }
}
