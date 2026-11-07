//! Custom widget that positions its child at an absolute offset from the
//! parent's top-left corner and allows the child to extend past the parent's
//! bounds (i.e. off-screen). Used by modal dialogs and floating panels so the
//! user can drag them to any position including partially past the viewport.
//!
//! Unlike `iced::widget::Pin`, which shrinks the child's available space by
//! the offset amount (and would squeeze a fixed-size modal when positioned
//! near an edge), `Translate` passes the parent's full limits through to the
//! child and only translates the resulting layout node. Negative offsets are
//! allowed; no clipping is applied inside the widget.

use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Shell, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

pub struct Translate<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    offset: Vector,
}

impl<'a, Message, Theme, Renderer> Translate<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        offset: (f32, f32),
    ) -> Self {
        Self {
            content: content.into(),
            offset: Vector::new(offset.0, offset.1),
        }
    }
}

impl<Message, Theme, Renderer> widget::Meta for Translate<'_, Message, Theme, Renderer> where
    Renderer: iced::advanced::Renderer
{
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Translate<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        self.content.tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.state()
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        // Child gets the full parent limits — no shrinkage like Pin does.
        self.content.layout(&mut tree.children[0], renderer, limits);

        tree.children[0].translation = self.offset;
        tree.size = limits.resolve(Length::Fill, Length::Fill, tree.children[0].size);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some((child_layout, child_tree)) = layout.iter_mut(&mut tree.children).next() {
            self.content
                .operate(child_tree, child_layout, viewport, renderer, operation);
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
        if let Some((child_layout, child_tree)) = layout.iter_mut(&mut tree.children).next() {
            self.content.update(
                child_tree,
                event,
                child_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );
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
        let Some((child_layout, child_tree)) = layout.iter(&tree.children).next() else {
            return mouse::Interaction::default();
        };
        self.content
            .mouse_interaction(child_tree, child_layout, cursor, viewport, renderer)
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
        // The outer viewport passes straight through so the child can paint
        // outside this widget's own bounds (no double clip).
        if let Some((child_layout, child_tree)) = layout.iter(&tree.children).next() {
            self.content.draw(
                child_tree,
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                viewport,
            );
        }
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'a, Message, Theme, Renderer>> {
        let Some((child_layout, child_tree)) = layout.iter_mut(&mut tree.children).next() else {
            return Vec::new();
        };
        self.content.overlay(
            child_tree,
            child_layout,
            renderer,
            viewport,
            translation,
            window,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Translate<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(w: Translate<'a, Message, Theme, Renderer>) -> Self {
        iced::advanced::Widget::_boxed(w)
    }
}
