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

use iced::advanced::layout::{Limits, Node};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, overlay, renderer};
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

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Translate<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        // Child gets the full parent limits — no shrinkage like Pin does.
        let child = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, limits)
            .translate(self.offset);

        let size = limits.resolve(Length::Fill, Length::Fill, child.size());
        Node::with_children(size, vec![child])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some(child_layout) = layout.children().next() {
            self.content
                .as_widget_mut()
                .operate(tree, child_layout, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Some(child_layout) = layout.children().next() {
            self.content.as_widget_mut().update(
                tree,
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        layout
            .children()
            .next()
            .map(|child_layout| {
                self.content.as_widget().mouse_interaction(
                    tree,
                    child_layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        // Pass the outer viewport straight through so the child can render
        // anywhere inside the window, including outside the Translate's own
        // bounds (otherwise we'd clip off-screen portions twice).
        if let Some(child_layout) = layout.children().next() {
            self.content.as_widget().draw(
                tree,
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
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let mut children = layout.children();
        let child_layout = children.next()?;
        self.content
            .as_widget_mut()
            .overlay(tree, child_layout, renderer, viewport, translation)
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
        Self::new(w)
    }
}

#[allow(dead_code)]
pub fn translate<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    offset: (f32, f32),
) -> Translate<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    Translate::new(content, offset)
}
