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
use iced::advanced::widget::{self, Operation, Tree, tree};
use iced::advanced::{Shell, Widget, mouse, overlay, renderer};
use iced::{Event, Length, Rectangle, Size, Vector};

/// Content whose widget state is valid only for `key`.
pub(super) struct Keyed<Key, Content> {
    key: Key,
    content: Content,
}

pub(super) fn keyed<Key, Content>(key: Key, content: Content) -> Keyed<Key, Content> {
    Keyed { key, content }
}

impl<Key, Content> widget::Meta for Keyed<Key, Content> {}

impl<Key, Content, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Keyed<Key, Content>
where
    Key: Copy + PartialEq + 'static,
    Content: Widget<Message, Theme, Renderer>,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Key>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(self.key)
    }

    fn diff(&mut self, tree: &mut Tree) {
        let same_key = *tree.state.downcast_ref::<Key>() == self.key;
        if !same_key {
            // Drop the stranger's state, including the whole child subtree,
            // so the content starts fresh.
            tree.state = tree::State::new(self.key);
            tree.children.clear();
        }
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        self.content.layout(&mut tree.children[0], renderer, limits);

        tree.size = tree.children[0].size;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .operate(&mut tree.children[0], layout, viewport, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
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
        cursor: mouse::Cursor,
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
        cursor: mouse::Cursor,
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
