//! The toolbar's Side button (m175): a click looks from the right, a
//! Shift+click from the left. Mouse events carry no modifiers, so the
//! wrapper keeps whether Shift is held in its own widget state, from the
//! keyboard's modifier changes; a Shift press publishes the left message
//! and is captured, any other event goes on to the button inside.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::Meta;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Shell, Widget, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector, keyboard};
use signex_gfx::camera3d::ViewPreset;

/// The preset the Side button looks from: the right side, or the left side
/// with Shift held.
pub(super) const fn side_preset(shift: bool) -> ViewPreset {
    if shift {
        ViewPreset::Left
    } else {
        ViewPreset::Right
    }
}

/// Whether Shift is held, as the keyboard last said.
#[derive(Debug, Default, Clone, Copy)]
struct ShiftHeld(bool);

impl ShiftHeld {
    /// Follow `event`; `true` when it is a left press over the button with
    /// Shift held, which the wrapper takes for itself.
    fn takes(&mut self, event: &Event, over: bool) -> bool {
        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                self.0 = modifiers.shift();
                false
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => self.0 && over,
            _ => false,
        }
    }
}

/// `content` (the Side button, which publishes the right side) wrapped so a
/// Shift+click publishes `shifted` instead.
pub(super) struct SideButton<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    shifted: Message,
}

pub(super) fn side_button<'a, Message, Theme, Renderer>(
    content: Element<'a, Message, Theme, Renderer>,
    shifted: Message,
) -> SideButton<'a, Message, Theme, Renderer> {
    SideButton { content, shifted }
}

impl<Message, Theme, Renderer> Meta for SideButton<'_, Message, Theme, Renderer> {}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SideButton<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ShiftHeld>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ShiftHeld::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        if tree.children.len() != 1 {
            tree.children = vec![Tree::new(&self.content)];
        }
        tree.children[0].diff(&mut self.content);
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
        let over = cursor.is_over(layout.bounds());
        if tree.state.downcast_mut::<ShiftHeld>().takes(event, over) {
            shell.publish(self.shifted.clone());
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

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Point;

    fn shift(held: bool) -> Event {
        let modifiers = if held {
            keyboard::Modifiers::SHIFT
        } else {
            keyboard::Modifiers::empty()
        };
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers))
    }

    fn left_press() -> Event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    }

    #[test]
    fn side_looks_from_the_right_and_shift_from_the_left() {
        assert_eq!(side_preset(false), ViewPreset::Right);
        assert_eq!(side_preset(true), ViewPreset::Left);
    }

    /// A press with Shift held is taken (the left side); without Shift, or
    /// off the button, it goes on to the button (the right side).
    #[test]
    fn a_shift_click_is_taken_and_a_plain_click_goes_on() {
        let mut held = ShiftHeld::default();
        assert!(!held.takes(&left_press(), true), "no Shift yet");
        assert!(!held.takes(&shift(true), true));
        assert!(held.takes(&left_press(), true), "Shift+click");
        assert!(!held.takes(&left_press(), false), "off the button");
        let moved = Event::Mouse(mouse::Event::CursorMoved {
            position: Point::ORIGIN,
        });
        assert!(!held.takes(&moved, true));
        assert!(!held.takes(&shift(false), true));
        assert!(!held.takes(&left_press(), true), "Shift released");
    }
}
