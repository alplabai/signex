//! The canvas program and the view function of the widget.

use iced::widget::canvas::{self, Canvas, Event};
use iced::{Element, Length, Rectangle, Renderer, Theme, mouse};
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
    type State = ();

    fn update(
        &self,
        _state: &mut (),
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
        hover_transition(self.hover.hovered_callout(), under_pointer).map(canvas::Action::publish)
    }

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
    Element::new(
        Canvas::new(program)
            .width(Length::Fill)
            .height(Length::Fixed(height)),
    )
}
