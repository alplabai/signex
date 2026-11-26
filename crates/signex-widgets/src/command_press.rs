//! `CommandPress` — wraps a pressable control and sends a second message
//! when it is clicked with Ctrl held (Cmd on macOS). A plain click reaches
//! the control as before; a Ctrl+click is taken here, so the control's own
//! message is not sent. A mouse event carries no modifiers, so the held
//! keys are tracked from `ModifiersChanged` in this widget's tree state.

use iced::advanced::layout::Limits;
use iced::advanced::widget::{self, Tree, Widget, tree};
use iced::advanced::{Layout, Shell, overlay, renderer};
use iced::keyboard::{self, Modifiers};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

pub struct CommandPress<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    on_command_press: Message,
}

impl<'a, Message, Theme, Renderer> CommandPress<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// Wrap `content`; `on_command_press` is sent on a left press over it
    /// with Ctrl (Cmd on macOS) held.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        on_command_press: Message,
    ) -> Self {
        Self {
            content: content.into(),
            on_command_press,
        }
    }
}

/// The modifier keys held, as the last `ModifiersChanged` said.
#[derive(Debug, Default)]
struct State {
    modifiers: Modifiers,
}

impl<Message, Theme, Renderer> widget::Meta for CommandPress<'_, Message, Theme, Renderer> where
    Renderer: iced::advanced::Renderer
{
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for CommandPress<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        self.content.layout(&mut tree.children[0], renderer, limits);
        tree.size = tree.children[0].size;
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
            .operate(&mut tree.children[0], layout, viewport, renderer, operation);
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
        let state: &mut State = tree.state.downcast_mut();
        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = *modifiers;
        }
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) && state.modifiers.command()
            && cursor.is_over(layout.bounds())
        {
            shell.publish(self.on_command_press.clone());
            shell.capture_event();
            return;
        }
        self.content.update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
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
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
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
        self.content.draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
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
        self.content.overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
            window,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<CommandPress<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(w: CommandPress<'a, Message, Theme, Renderer>) -> Self {
        Widget::_boxed(w)
    }
}

// iced's headless `()` renderer exists only with debug assertions.
#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use crate::button;
    use iced::Widget as _;
    use iced::advanced::shell::{Bus, Waker};
    use iced::widget::Space;
    use iced::window::Headless;
    use iced::{Point, Theme};

    /// Click at `at` over a 40x20 button (message 7) wrapped to send 8 on a
    /// Ctrl+click, with `held` as the last modifier change, in a 200x100
    /// window; the messages sent.
    fn click(held: Modifiers, at: Point) -> Vec<u8> {
        let pressable: Element<'static, u8, Theme, ()> = button(Space::new().width(40).height(20))
            .on_press(7)
            .boxed();
        let mut element: Element<'static, u8, Theme, ()> = CommandPress::new(pressable, 8).into();
        let mut tree = Tree::new(&element);
        tree.diff(&mut element);
        let limits = Limits::new(Size::ZERO, Size::new(200.0, 100.0));
        element.layout(&mut tree, &(), &limits);
        let layout = Layout::new(tree.size);
        let viewport = Rectangle::with_size(Size::new(200.0, 100.0));
        let cursor = Cursor::Available(at);
        let mut messages = Bus::new();
        for event in [
            Event::Keyboard(keyboard::Event::ModifiersChanged(held)),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ] {
            // One shell per event, as the runtime does.
            let mut shell = Shell::new(&Headless, Waker::noop(), &mut messages);
            element.update(
                &mut tree,
                &event,
                layout,
                cursor,
                &(),
                &mut shell,
                &viewport,
            );
        }
        messages.drain().map(|(message, _)| message).collect()
    }

    #[test]
    fn a_plain_click_reaches_the_control() {
        assert_eq!(click(Modifiers::empty(), Point::new(5.0, 5.0)), vec![7]);
        assert_eq!(click(Modifiers::SHIFT, Point::new(5.0, 5.0)), vec![7]);
    }

    #[test]
    fn a_command_click_sends_the_second_message_only() {
        assert_eq!(click(Modifiers::COMMAND, Point::new(5.0, 5.0)), vec![8]);
    }

    #[test]
    fn a_command_click_elsewhere_sends_nothing() {
        assert!(click(Modifiers::COMMAND, Point::new(150.0, 80.0)).is_empty());
    }
}
