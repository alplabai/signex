//! Drawing a [`DashedOutline`]: iced's `Border` has no dash, so the outline
//! is a canvas stacked over the content it frames.

use iced::widget::canvas::{self, Canvas};
use iced::{Element, Length, Point, Rectangle, Size, Theme, Widget as _};

use super::DashedOutline;

impl<M> canvas::Program<M> for DashedOutline {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let w = self.width;
        let path = canvas::Path::rounded_rectangle(
            Point::new(w / 2.0, w / 2.0),
            Size::new((bounds.width - w).max(0.0), (bounds.height - w).max(0.0)),
            self.radius.into(),
        );
        let mut stroke = canvas::Stroke::default()
            .with_color(self.color)
            .with_width(w);
        stroke.line_dash = canvas::LineDash {
            segments: &self.dash,
            offset: 0,
        };
        frame.stroke(&path, stroke);
        vec![frame.into_geometry()]
    }
}

/// `content`, full width, with `outline` drawn along its edge. The canvas
/// takes no input, so clicks still reach `content`.
pub fn dashed_frame<'a, M: 'a>(content: Element<'a, M>, outline: DashedOutline) -> Element<'a, M> {
    let edge: Canvas<DashedOutline, M> = Canvas::new(outline);
    iced::widget::stack![
        content,
        edge.width(Length::Fill).height(Length::Fill).boxed(),
    ]
    .width(Length::Fill)
    .boxed()
}
