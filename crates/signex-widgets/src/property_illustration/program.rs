//! The canvas program and the view function of the widget.

use iced::widget::canvas::{self, Canvas};
use iced::{Element, Length, Rectangle, Renderer, Theme, Widget, mouse};
use signex_types::theme::ThemeTokens;

use super::draw::Painter;
use super::geometry::Fit;
use super::hover::{IllustrationHover, IllustrationMsg};
use super::illustration::Illustration;
use super::metrics::Metrics;
use super::palette::Palette;

/// Draws one illustration; holds no state of its own (the hover lives in
/// the caller's [`IllustrationHover`]).
struct IllustrationProgram {
    illustration: Illustration,
    hover: IllustrationHover,
    palette: Palette,
    metrics: Metrics,
}

impl IllustrationProgram {
    fn fit(&self, bounds: Rectangle) -> Fit {
        Fit::new(self.illustration.view_size(), bounds.size())
    }
}

impl canvas::Program<IllustrationMsg> for IllustrationProgram {
    /// The drawing does not follow the pointer: only a hovered field row
    /// lights its parts.
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let painter = Painter {
            fit: self.fit(bounds),
            metrics: self.metrics,
            palette: &self.palette,
        };
        for part in self.illustration.visible_parts() {
            painter.part(&mut frame, part, self.hover.is_part_highlighted(part));
        }
        vec![frame.into_geometry()]
    }
}

/// A property illustration on a transparent ground, full panel width and
/// `illustration.height_in_em()` captions tall.
///
/// `text_size` is the caption step of the type scale; every stroke, arrow,
/// dash and label size is derived from it. Map the returned messages into
/// the panel's message and feed them to [`IllustrationHover::update`].
///
/// The illustration is taken by value: it is data the panel builds in
/// `view` from the selected object.
pub fn property_illustration<'a>(
    illustration: Illustration,
    hover: &IllustrationHover,
    tokens: &ThemeTokens,
    text_size: f32,
) -> Element<'a, IllustrationMsg> {
    let metrics = Metrics::new(text_size);
    let height = metrics.height(illustration.height_in_em());
    let program = IllustrationProgram {
        illustration,
        hover: hover.clone(),
        palette: Palette::from_tokens(tokens),
        metrics,
    };
    Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::property_illustration::{Part, Primitive, Style};
    use iced::widget::canvas::Program;
    use iced::{Point, Radians};
    use signex_types::theme::{ThemeId, theme_tokens};

    /// A drawing whose one part, an arc, is under (50, 70) of a 100 px box.
    fn program(field: &'static str, hover: &IllustrationHover) -> IllustrationProgram {
        let illustration = Illustration::new(100.0, 100.0).part(
            Part::new(
                Primitive::Arc {
                    center: Point::new(50.0, 50.0),
                    radius: 20.0,
                    start: Radians(0.0),
                    end: Radians(std::f32::consts::FRAC_PI_2),
                },
                Style::edge(),
            )
            .linked(field),
        );
        IllustrationProgram {
            illustration,
            hover: hover.clone(),
            palette: Palette::from_tokens(&theme_tokens(ThemeId::DEFAULT)),
            metrics: Metrics::new(10.0),
        }
    }

    /// Moving over a drawn part publishes nothing, so no field row lights.
    #[test]
    fn the_pointer_over_a_drawing_publishes_nothing() {
        let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(100.0, 100.0));
        let hover = IllustrationHover::default();
        for x in [50.0, 50.5, 51.0, 50.5, 50.0] {
            let position = Point::new(x, 70.0);
            let event = canvas::Event::Mouse(mouse::Event::CursorMoved { position });
            let cursor = mouse::Cursor::Available(position);
            assert!(program("grid", &hover)
                .update(&mut (), &event, bounds, cursor)
                .is_none());
        }
    }
}
