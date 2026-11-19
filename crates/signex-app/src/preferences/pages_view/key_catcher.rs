//! A wrapper that turns a key press into a page message before the app's
//! key handling sees it. The app routes only the keys no widget captured
//! (`event::listen_with` on `Status::Ignored`), so a key caught here never
//! reaches the global Esc that closes Preferences. Used for Tab and Esc on
//! the snap preview's pitch labels and for Esc on an expanded preview.

use iced::advanced::layout::Limits;
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Layout, Shell, overlay, renderer};
use iced::keyboard::{self, Key, Modifiers};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// `content`, plus `on_key`: the message a key press means here, if any.
pub(in crate::preferences) fn catch_keys<'a, Message: 'a>(
    content: Element<'a, Message>,
    on_key: impl Fn(&Key, Modifiers) -> Option<Message> + 'a,
) -> Element<'a, Message> {
    KeyCatcher {
        content,
        on_key: Box::new(on_key),
    }
    ._boxed()
}

struct KeyCatcher<'a, Message> {
    content: Element<'a, Message>,
    on_key: Box<dyn Fn(&Key, Modifiers) -> Option<Message> + 'a>,
}

impl<Message> widget::Meta for KeyCatcher<'_, Message> {}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for KeyCatcher<'_, Message> {
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

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) {
        self.content.layout(tree, renderer, limits);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
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
        renderer: &iced::Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // The content first: a focused field keeps the keys it handles.
        self.content
            .update(tree, event, layout, cursor, renderer, shell, viewport);
        if shell.is_event_captured() {
            return;
        }
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event
            && let Some(message) = (self.on_key)(key, *modifiers)
        {
            shell.publish(message);
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content
            .overlay(tree, layout, renderer, viewport, translation, window)
    }
}
