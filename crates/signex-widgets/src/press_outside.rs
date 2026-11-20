//! `PressOutside` — wraps a control and sends a message when the left mouse
//! button is pressed outside it: the press that takes focus away from a
//! text editor, which has no blur message of its own. The child sees every
//! event unchanged; the message is extra and the event is not captured.

use iced::advanced::layout::Limits;
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Layout, Shell, renderer};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size};

pub struct PressOutside<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    on_press: Option<Message>,
}

impl<'a, Message, Theme, Renderer> PressOutside<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// Wrap `content`; `on_press` is sent on a left press outside it, and
    /// `None` sends nothing.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        on_press: Option<Message>,
    ) -> Self {
        Self {
            content: content.into(),
            on_press,
        }
    }
}

impl<Message, Theme, Renderer> widget::Meta for PressOutside<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn is_void(&self) -> bool {
        widget::Meta::is_void(&self.content)
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for PressOutside<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        self.content.tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.state()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.content.diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        self.content.layout(tree, renderer, limits);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .operate(tree, layout, viewport, renderer, operation);
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
        if let (Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)), Some(message)) =
            (event, &self.on_press)
            && !cursor.is_over(layout.bounds())
        {
            shell.publish(message.clone());
        }
        self.content
            .update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
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
        self.content
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }
}

impl<'a, Message, Theme, Renderer> From<PressOutside<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(w: PressOutside<'a, Message, Theme, Renderer>) -> Self {
        Widget::_boxed(w)
    }
}

// iced's headless `()` renderer exists only with debug assertions.
#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use iced::Widget as _;
    use iced::advanced::shell::{Bus, Waker};
    use iced::widget::Space;
    use iced::window::Headless;
    use iced::{Point, Theme};

    /// Press the left button at `at` over a 40x20 box wrapped with message
    /// 9, in a 200x100 window, and return what was sent.
    fn press_at(at: Point, armed: bool) -> Vec<u8> {
        let inner: Element<'static, u8, Theme, ()> = Space::new().width(40).height(20).into();
        let mut element: Element<'static, u8, Theme, ()> =
            PressOutside::new(inner, armed.then_some(9)).into();
        let mut tree = Tree::new(&element);
        tree.diff(&mut element);
        let limits = Limits::new(Size::ZERO, Size::new(200.0, 100.0));
        element.layout(&mut tree, &(), &limits);
        let layout = Layout::new(tree.size);
        let viewport = Rectangle::with_size(Size::new(200.0, 100.0));
        let mut messages = Bus::new();
        let mut shell = Shell::new(&Headless, Waker::noop(), &mut messages);
        element.update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            layout,
            Cursor::Available(at),
            &(),
            &mut shell,
            &viewport,
        );
        messages.drain().map(|(message, _)| message).collect()
    }

    #[test]
    fn a_press_outside_sends_the_message_and_one_inside_does_not() {
        assert_eq!(press_at(Point::new(150.0, 80.0), true), vec![9]);
        assert!(press_at(Point::new(5.0, 5.0), true).is_empty());
        assert!(press_at(Point::new(150.0, 80.0), false).is_empty());
    }
}
