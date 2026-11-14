//! The click-outside layer under a right-click menu (#2349).
//!
//! A left press anywhere outside the card closes the menu and goes no
//! further. A right press closes it too, but passes through, so whatever
//! lies under the pointer opens its own menu at the new spot. Wheel events
//! stop here, and the pointer is claimed so nothing under the menu hovers.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, Widget, tree};
use iced::advanced::{Shell, renderer};
use iced::mouse::{self, Cursor};
use iced::{Color, Event, Length, Rectangle, Size};

/// A window-sized layer painted in `scrim` that sends `on_dismiss`.
pub(in crate::app::view) struct MenuBackdrop<Message> {
    on_dismiss: Message,
    scrim: Color,
}

impl<Message> MenuBackdrop<Message> {
    pub fn new(on_dismiss: Message, scrim: Color) -> Self {
        Self { on_dismiss, scrim }
    }
}

/// Set while a right press is being delivered: the layer then reports no
/// pointer claim, so the `Stack` hands the press to the layers below with
/// the cursor still on them.
#[derive(Default)]
struct State {
    passing_right_press: bool,
}

impl<Message> widget::Meta for MenuBackdrop<Message> {}

impl<Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for MenuBackdrop<Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, Length::Fill, Length::Fill);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: Cursor,
        _renderer: &iced::Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        state.passing_right_press = false;
        let Event::Mouse(mouse_event) = event else {
            return;
        };
        match mouse_event {
            // Over the card the cursor is levitating: the card had it.
            mouse::Event::ButtonPressed(mouse::Button::Left) if cursor.is_over(layout.bounds()) => {
                shell.publish(self.on_dismiss.clone());
                shell.capture_event();
            }
            mouse::Event::ButtonPressed(mouse::Button::Right)
                if cursor.is_over(layout.bounds()) =>
            {
                shell.publish(self.on_dismiss.clone());
                state.passing_right_press = true;
            }
            // Wherever the pointer is: the menu owns the window's wheel.
            mouse::Event::WheelScrolled { .. } => shell.capture_event(),
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        if !state.passing_right_press && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Idle
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                ..renderer::Quad::default()
            },
            self.scrim,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::messages_for_events;
    use iced::widget::{Space, Stack, mouse_area};
    use iced::{Element, Point, Widget as _};

    #[derive(Debug, Clone, PartialEq)]
    enum Probe {
        Dismissed,
        BelowEntered,
        BelowLeftPressed,
        BelowRightPressed,
        BelowScrolled,
    }

    const WINDOW: Size = Size::new(400.0, 300.0);
    const AT: Point = Point::new(200.0, 150.0);

    /// The backdrop over a window-sized area that reports every press,
    /// hover and wheel it gets.
    fn over_a_listener() -> Element<'static, Probe> {
        let below = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
            .on_enter(Probe::BelowEntered)
            .on_press(Probe::BelowLeftPressed)
            .on_right_press(Probe::BelowRightPressed)
            .on_scroll(|_| Probe::BelowScrolled);
        let stack: Stack<Element<'static, Probe>> = Stack::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .push(below.boxed())
            .push(MenuBackdrop::new(Probe::Dismissed, Color::TRANSPARENT).boxed());
        stack.boxed()
    }

    fn after(events: &[Event]) -> Vec<Probe> {
        messages_for_events(over_a_listener(), WINDOW, AT, events)
    }

    #[test]
    fn a_left_press_dismisses_and_stops_there() {
        let got = after(&[Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Left,
        ))]);
        assert_eq!(got, vec![Probe::Dismissed]);
    }

    #[test]
    fn a_right_press_dismisses_and_reaches_what_is_under_it() {
        let got = after(&[Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Right,
        ))]);
        // The layer below sees the cursor for this press (so it may report an
        // enter as well); the dismiss is published first.
        assert_eq!(got.first(), Some(&Probe::Dismissed), "{got:?}");
        assert!(got.contains(&Probe::BelowRightPressed), "{got:?}");
        assert!(!got.contains(&Probe::BelowLeftPressed), "{got:?}");
    }

    #[test]
    fn the_hover_stops_at_the_backdrop() {
        let got = after(&[]);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn the_wheel_stops_at_the_backdrop() {
        let got = after(&[Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
        })]);
        assert!(got.is_empty(), "{got:?}");
    }
}
