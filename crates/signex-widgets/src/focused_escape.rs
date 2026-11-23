//! `FocusedEscape` wraps a text box and sends a message when Esc is pressed
//! while the box holds the keys. The box still blurs itself; the key is
//! captured so nothing else answers the same press.

use iced::advanced::layout::Limits;
use iced::advanced::widget::operation::{Focusable, Operation};
use iced::advanced::widget::{self, Id, Tree, Widget};
use iced::advanced::{Layout, Shell, renderer};
use iced::keyboard::{self, key::Named};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size};

pub struct FocusedEscape<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    on_escape: Message,
}

impl<'a, Message, Theme, Renderer> FocusedEscape<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// Wrap `content`; `on_escape` is sent for an Esc it has the focus for.
    pub fn new(content: Element<'a, Message, Theme, Renderer>, on_escape: Message) -> Self {
        Self { content, on_escape }
    }
}

/// Finds whether any focusable widget it visits holds the focus.
#[derive(Default)]
struct HoldsFocus(bool);

impl Operation for HoldsFocus {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn focusable(&mut self, _id: Option<&Id>, _bounds: Rectangle, state: &mut dyn Focusable) {
        self.0 |= state.is_focused();
    }
}

fn is_escape(event: &Event) -> bool {
    matches!(
        event,
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::Escape),
            ..
        })
    )
}

impl<Message, Theme, Renderer> widget::Meta for FocusedEscape<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn is_void(&self) -> bool {
        widget::Meta::is_void(&self.content)
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for FocusedEscape<'_, Message, Theme, Renderer>
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
        // Read before the box sees the key: it blurs itself on Esc.
        let mut holds_focus = HoldsFocus::default();
        if is_escape(event) {
            self.content
                .operate(tree, layout, viewport, renderer, &mut holds_focus);
        }
        self.content
            .update(tree, event, layout, cursor, renderer, shell, viewport);
        if holds_focus.0 {
            shell.publish(self.on_escape.clone());
            shell.capture_event();
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

impl<'a, Message, Theme, Renderer> From<FocusedEscape<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(w: FocusedEscape<'a, Message, Theme, Renderer>) -> Self {
        Widget::_boxed(w)
    }
}
