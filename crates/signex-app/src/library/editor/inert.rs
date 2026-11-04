//! `Inert` — wraps editing controls of a Component Preview tab that is
//! open on a read-only library (the stock library, issue #329).
//!
//! The child renders exactly as it would normally, dimmed, but takes no
//! input: no clicks, no typing, no hover state, no focus. Only wheel
//! scrolling still reaches it so a long parameter list stays readable.
//! One wrapper covers every control in a tab, so a control added to the
//! tab later is read-only by construction instead of by remembering to
//! thread a flag through it. The dispatcher guard stays as the backstop.

use iced::advanced::layout::Limits;
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Layout, Shell, renderer};
use iced::mouse::{self, Cursor};
use iced::{Background, Color, Element, Event, Length, Rectangle, Size};

pub struct Inert<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    veil: Color,
}

impl<'a, Message, Theme, Renderer> Inert<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// Wrap `content`, fading it with the (theme-token) `veil`.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>, veil: Color) -> Self {
        Self {
            content: content.into(),
            veil,
        }
    }
}

/// Wrap `content` so it ignores input when `read_only`, fading it with
/// the theme-token `veil`; otherwise hand it back untouched.
pub fn unless_read_only_veiled<'a, Message: 'a>(
    read_only: bool,
    veil: Color,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let content = content.into();
    if read_only {
        Inert::new(content, veil).into()
    } else {
        content
    }
}

impl<Message, Theme, Renderer> widget::Meta for Inert<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn is_void(&self) -> bool {
        widget::Meta::is_void(&self.content)
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Inert<'_, Message, Theme, Renderer>
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
        self.content.diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        self.content.layout(tree, renderer, limits);
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
        if matches!(event, Event::Mouse(mouse::Event::WheelScrolled { .. })) {
            self.content
                .update(tree, event, layout, cursor, renderer, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout,
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
        layout: Layout,
        _cursor: Cursor,
        viewport: &Rectangle,
    ) {
        // `Cursor::Unavailable` so no hover or pressed styling shows.
        self.content.draw(
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
            Background::Color(self.veil),
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
        Widget::_boxed(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Widget as _;
    use iced::advanced::shell::{Bus, Waker};
    use iced::widget::{Space, button};
    use iced::window::Headless;
    use iced::{Point, Theme};

    /// Click the middle of a button built by `wrap` and return the
    /// messages it produced. `()` is iced's headless renderer.
    fn click(
        wrap: impl FnOnce(Element<'static, u8, Theme, ()>) -> Element<'static, u8, Theme, ()>,
    ) -> Vec<u8> {
        let pressable: Element<'static, u8, Theme, ()> = button(Space::new().width(40).height(20))
            .on_press(7)
            .boxed();
        let mut element = wrap(pressable);
        let mut tree = Tree::new(&element);
        tree.diff(&mut element);
        let limits = Limits::new(Size::ZERO, Size::new(200.0, 100.0));
        element.layout(&mut tree, &(), &limits);
        let layout = Layout::new(tree.size);
        let viewport = Rectangle::with_size(Size::new(200.0, 100.0));
        let cursor = Cursor::Available(Point::new(5.0, 5.0));
        let mut messages = Bus::new();
        for event in [
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ] {
            // One shell per event, as the runtime does: a captured
            // press would otherwise mask the release.
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
    fn a_plain_button_fires_on_click() {
        assert_eq!(click(|e| e), vec![7]);
    }

    #[test]
    fn a_veiled_wrapper_stays_inert() {
        let veil = Color::from_rgba(0.5, 0.5, 0.5, 0.5);
        assert!(click(|e| Inert::new(e, veil).into()).is_empty());
    }
}
