//! `Inert` — wraps editing controls of a Component Preview tab that is
//! open on a read-only library (the stock library, issue #329).
//!
//! The child renders exactly as it would normally, dimmed, but takes no
//! input: no clicks, no typing, no hover state, no focus. Only wheel
//! scrolling still reaches it so a long parameter list stays readable.
//! One wrapper covers every control in a tab, so a control added to the
//! tab later is read-only by construction instead of by remembering to
//! thread a flag through it. The dispatcher guard stays as the backstop.

use iced::advanced::layout::{Limits, Node};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Layout, Shell, renderer};
use iced::mouse::{self, Cursor};
use iced::{Background, Color, Element, Event, Length, Rectangle, Size};

/// Opacity of the veil drawn over an inert child.
const VEIL_ALPHA: f32 = 0.35;

pub struct Inert<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> Inert<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

/// Wrap `content` so it ignores input when `read_only`; otherwise hand
/// it back untouched.
pub fn unless_read_only<'a, Message: 'a>(
    read_only: bool,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let content = content.into();
    if read_only {
        Inert::new(content).into()
    } else {
        content
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Inert<'_, Message, Theme, Renderer>
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
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
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
        if matches!(event, Event::Mouse(mouse::Event::WheelScrolled { .. })) {
            self.content.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        viewport: &Rectangle,
    ) {
        // `Cursor::Unavailable` so no hover or pressed styling shows.
        self.content.as_widget().draw(
            tree,
            renderer,
            theme,
            style,
            layout,
            Cursor::Unavailable,
            viewport,
        );
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                ..renderer::Quad::default()
            },
            Background::Color(Color {
                a: VEIL_ALPHA,
                ..Color::BLACK
            }),
        );
    }
}

impl<'a, Message, Theme, Renderer> From<Inert<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(w: Inert<'a, Message, Theme, Renderer>) -> Self {
        Self::new(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::layout::Layout;
    use iced::widget::{Space, button};
    use iced::{Point, Theme};

    /// Click the middle of a button built by `wrap` and return the
    /// messages it produced. `()` is iced's headless renderer.
    fn click(
        wrap: impl FnOnce(Element<'static, u8, Theme, ()>) -> Element<'static, u8, Theme, ()>,
    ) -> Vec<u8> {
        let pressable: Element<'static, u8, Theme, ()> =
            button(Space::new().width(40).height(20)).on_press(7).into();
        let mut element = wrap(pressable);
        let mut tree = Tree::new(element.as_widget());
        let limits = Limits::new(Size::ZERO, Size::new(200.0, 100.0));
        let node = element.as_widget_mut().layout(&mut tree, &(), &limits);
        let viewport = Rectangle::with_size(Size::new(200.0, 100.0));
        let cursor = Cursor::Available(Point::new(5.0, 5.0));
        let mut messages = Vec::new();
        for event in [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ] {
            // One shell per event, as the runtime does: a captured
            // press would otherwise mask the release.
            let mut shell = Shell::new(&mut messages);
            element.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                cursor,
                &(),
                &mut iced::advanced::clipboard::Null,
                &mut shell,
                &viewport,
            );
        }
        messages
    }

    #[test]
    fn a_plain_button_fires_on_click() {
        assert_eq!(click(|e| e), vec![7]);
    }

    #[test]
    fn a_button_inside_inert_ignores_clicks() {
        assert!(click(|e| Inert::new(e).into()).is_empty());
    }
}
