//! The canvas program and the view function of the widget.

use iced::widget::canvas::{self, Canvas, Event};
use iced::{Element, Length, Rectangle, Renderer, Theme, Widget, mouse};
use signex_types::theme::ThemeTokens;

use super::draw::Painter;
use super::geometry::Fit;
use super::hit::hit_test;
use super::hover::{IllustrationHover, IllustrationMsg, hover_transition};
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
    /// Whether the pointer was on one of this drawing's parts at the last
    /// move, so this drawing owns the model's callout hover.
    type State = bool;

    fn update(
        &self,
        is_owner: &mut bool,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<IllustrationMsg>> {
        if !matches!(
            event,
            Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft)
        ) {
            return None;
        }
        let under_pointer = cursor
            .position_in(bounds)
            .and_then(|p| hit_test(&self.illustration, &self.fit(bounds), &self.metrics, p));
        let was_owner = std::mem::replace(is_owner, under_pointer.is_some());
        hover_transition(self.hover.hovered_callout(), under_pointer, was_owner)
            .map(canvas::Action::publish)
    }

    fn draw(
        &self,
        _state: &bool,
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
    use crate::property_illustration::{FieldId, Part, Primitive, Style};
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

    /// Two drawings on one hover (Document Options: the grid and the
    /// sheet): moving over a part of the first keeps its callout steady,
    /// and the second, away from the pointer, publishes nothing.
    #[test]
    fn a_second_drawing_on_the_same_hover_does_not_flicker_the_callout() {
        let first = Rectangle::new(Point::ORIGIN, iced::Size::new(100.0, 100.0));
        let second = Rectangle::new(Point::new(0.0, 200.0), iced::Size::new(100.0, 100.0));
        let (mut first_owner, mut second_owner) = (false, false);
        let mut hover = IllustrationHover::default();
        let mut published = Vec::new();
        for x in [50.0, 50.5, 51.0, 50.5, 50.0] {
            let position = Point::new(x, 70.0);
            let event = Event::Mouse(mouse::Event::CursorMoved { position });
            let cursor = mouse::Cursor::Available(position);
            let messages = [
                program("grid", &hover).update(&mut first_owner, &event, first, cursor),
                program("sheet", &hover).update(&mut second_owner, &event, second, cursor),
            ];
            for action in messages.into_iter().flatten() {
                let (message, _, _) = action.into_inner();
                let message = message.expect("a hover action publishes");
                published.push(message.clone());
                hover.update(message);
            }
            assert_eq!(hover.hovered_callout(), Some(&FieldId::new("grid")));
        }
        assert_eq!(
            published,
            vec![IllustrationMsg::CalloutHovered(FieldId::new("grid"))],
            "one enter, then nothing while the pointer stays on the part"
        );
        // Leaving the first drawing clears its own callout once.
        let position = Point::new(50.0, 150.0);
        let event = Event::Mouse(mouse::Event::CursorMoved { position });
        let cursor = mouse::Cursor::Available(position);
        let left = program("grid", &hover).update(&mut first_owner, &event, first, cursor);
        let message = left.and_then(|a| a.into_inner().0);
        assert_eq!(message, Some(IllustrationMsg::CalloutLeft));
    }
}
