//! Reports the height a widget is laid out at (#1355).
//!
//! The grid windows its rows from the height of its scrollable. The
//! scrollable only reports a viewport while its content overflows, so a
//! resize that makes the content fit is never published and the window
//! would go stale. This wrapper reads the height from layout on every
//! redraw instead, and publishes it whenever it differs from the height the
//! state already knows.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector, window};

/// Height changes smaller than this are layout rounding, not a resize.
const RESIZE_EPSILON: f32 = 0.5;

pub(super) struct SizeSensor<'a, Message, Theme, Renderer> {
    known_height: f32,
    on_height: Box<dyn Fn(f32) -> Message + 'a>,
    content: Element<'a, Message, Theme, Renderer>,
}

/// Wrap `content`; `on_height` is published when its laid-out height is not
/// `known_height`.
pub(super) fn sense_height<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    known_height: f32,
    on_height: impl Fn(f32) -> Message + 'a,
) -> SizeSensor<'a, Message, Theme, Renderer> {
    SizeSensor {
        known_height,
        on_height: Box::new(on_height),
        content: content.into(),
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SizeSensor<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<f32>()
    }

    /// The last height published, so a redraw before the state catches up
    /// does not publish it again.
    fn state(&self) -> tree::State {
        tree::State::new(f32::NAN)
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
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

        if matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            let height = layout.bounds().height;
            let last_published = *tree.state.downcast_ref::<f32>();
            let differs = |a: f32| a.is_nan() || (a - height).abs() > RESIZE_EPSILON;
            if differs(self.known_height) && differs(last_published) {
                *tree.state.downcast_mut::<f32>() = height;
                shell.publish((self.on_height)(height));
            }
        }
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

impl<'a, Message, Theme, Renderer> From<SizeSensor<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(sensor: SizeSensor<'a, Message, Theme, Renderer>) -> Self {
        Element::new(sensor)
    }
}
