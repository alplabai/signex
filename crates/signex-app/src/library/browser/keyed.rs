//! A wrapper that ties its content's widget state to a key.
//!
//! iced hands widget state (text-input focus and cursor, scroll offset) to
//! children by position, and its `keyed_column` only realigns a single
//! insertion or removal. The browser grid slides a window over thousands of
//! rows, so a plain diff would give a row the state of whichever row used to
//! sit in its slot; a focused inline cell editor would then commit its text
//! to a different row (#1355). `Keyed` makes the state follow the key
//! instead: when the key at a slot changes, the content starts from fresh
//! state rather than inheriting a stranger's.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// Content whose widget state is valid only for `key`.
pub(super) struct Keyed<'a, Key, Message, Theme, Renderer> {
    key: Key,
    content: Element<'a, Message, Theme, Renderer>,
}

pub(super) fn keyed<'a, Key, Message, Theme, Renderer>(
    key: Key,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Keyed<'a, Key, Message, Theme, Renderer> {
    Keyed {
        key,
        content: content.into(),
    }
}

impl<Key, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Keyed<'_, Key, Message, Theme, Renderer>
where
    Key: Copy + PartialEq + 'static,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Key>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(self.key)
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        let same_key = tree.state.downcast_ref::<Key>() == &self.key;
        if same_key {
            tree.diff_children(std::slice::from_ref(&self.content));
        } else {
            tree.state = tree::State::new(self.key);
            tree.children = vec![Tree::new(&self.content)];
        }
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
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
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Key, Message, Theme, Renderer> From<Keyed<'a, Key, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Key: Copy + PartialEq + 'static,
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(keyed: Keyed<'a, Key, Message, Theme, Renderer>) -> Self {
        Element::new(keyed)
    }
}
