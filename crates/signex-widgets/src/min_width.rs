//! Content as wide as its room, but never narrower than a floor. Inside a
//! sideways scroller the room is only a hint, so content that needs more
//! than the room is laid out at the floor and scrolls; content that needs
//! less fills the room. A `Fill` length with a minimum cannot do this: a
//! row hands its fill children no more than the room, so the last ones are
//! cut below their minimum (#3037).

use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Shell, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// `content` as wide as the room, at least `min_width`.
pub struct MinWidth<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// `[content]`.
    parts: [Element<'a, Message, Theme, Renderer>; 1],
    min_width: f32,
}

impl<'a, Message, Theme, Renderer> MinWidth<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// `content` filling its room, never narrower than `min_width`.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>, min_width: f32) -> Self {
        Self {
            parts: [content.into()],
            min_width,
        }
    }
}

/// The width the content takes: the room, at least `min_width`.
pub fn resolved_width(min_width: f32, limits: &Limits) -> f32 {
    limits.max.width.max(min_width).max(limits.min.width)
}

impl<Message, Theme, Renderer> widget::Meta for MinWidth<'_, Message, Theme, Renderer> where
    Renderer: iced::advanced::Renderer
{
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for MinWidth<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(self.parts.as_mut_slice());
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: self.parts[0].size().height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let width = resolved_width(self.min_width, limits);
        let exact = Limits::with_flags(
            Size::new(width, limits.min.height),
            Size::new(width, limits.max.height),
            Size::new(false, limits.compression.height),
            Size::new(false, limits.infinite.height),
        );
        let child = &mut tree.children[0];
        self.parts[0].layout(child, renderer, &exact);
        child.translation = Vector::ZERO;
        tree.size = Size::new(width, child.size.height);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some((layout, state)) = layout.iter_mut(&mut tree.children).next() {
            self.parts[0].operate(state, layout, viewport, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Some((layout, state)) = layout.iter_mut(&mut tree.children).next() {
            self.parts[0].update(state, event, layout, cursor, renderer, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        layout
            .iter(&tree.children)
            .next()
            .map(|(layout, state)| {
                self.parts[0].mouse_interaction(state, layout, cursor, viewport, renderer)
            })
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        if let Some((layout, state)) = layout.iter(&tree.children).next() {
            self.parts[0].draw(state, renderer, theme, style, layout, cursor, viewport);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        match layout.iter_mut(&mut tree.children).next() {
            Some((layout, state)) => {
                self.parts[0].overlay(state, layout, renderer, viewport, translation, window)
            }
            None => Vec::new(),
        }
    }
}

impl<'a, Message, Theme, Renderer> From<MinWidth<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(width: MinWidth<'a, Message, Theme, Renderer>) -> Self {
        Widget::_boxed(width)
    }
}

// iced's headless `()` renderer exists only with debug assertions.
#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use iced::Theme;
    use iced::widget::{Space, container};

    type Part = Element<'static, (), Theme, ()>;

    /// The widget's width and its content's, laid out with `room` and the
    /// room as a hint only (`infinite`, as a sideways scroller gives it).
    fn laid_out(min_width: f32, room: f32, infinite: bool) -> (f32, f32) {
        let content: Part =
            iced::Widget::boxed(container(Space::new().width(10).height(10)).width(Length::Fill));
        let mut element: Element<'static, (), Theme, ()> = MinWidth::new(content, min_width).into();
        let mut tree = Tree::new(&element);
        tree.diff(&mut element);
        let limits = Limits::with_flags(
            Size::ZERO,
            Size::new(room, 100.0),
            Size::new(false, false),
            Size::new(infinite, false),
        );
        element.layout(&mut tree, &(), &limits);
        (tree.size.width, tree.children[0].size.width)
    }

    #[test]
    fn content_that_needs_less_fills_the_room() {
        assert_eq!(laid_out(100.0, 400.0, true), (400.0, 400.0));
        assert_eq!(laid_out(100.0, 400.0, false), (400.0, 400.0));
    }

    #[test]
    fn content_that_needs_more_keeps_its_floor() {
        assert_eq!(laid_out(500.0, 400.0, true), (500.0, 500.0));
    }
}
