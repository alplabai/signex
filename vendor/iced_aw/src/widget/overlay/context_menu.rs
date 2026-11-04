//! A modal for showing elements as an overlay on top of another.
//!
//! *This API requires the following crate features to be activated: ``context_menu``*
use crate::context_menu;
pub use crate::style::{
    context_menu::{Catalog, Style},
    status::{self, StyleFn},
};

use iced_core::{
    Border, Color, Element, Event, Layout, Point, Rectangle, Shell, Size, Widget, keyboard,
    layout::Limits,
    mouse::{self, Cursor},
    overlay, renderer, touch,
    widget::Tree,
    window,
};

/// The overlay of the [`ContextMenu`](crate::widget::ContextMenu).
#[allow(missing_debug_implementations)]
pub struct ContextMenuOverlay<
    'a,
    'b,
    Message,
    Theme = iced_widget::Theme,
    Renderer = iced_widget::Renderer,
> where
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: Catalog,
    'b: 'a,
{
    // The position of the element
    position: Point,
    /// The bounds of the window the overlay is displayed in.
    window: Rectangle,
    /// The layout of the content, computed on creation.
    layout: Layout,
    /// The state of the [`ContextMenuOverlay`].
    tree: &'a mut Tree,
    /// The content of the [`ContextMenuOverlay`].
    content: Element<'a, Message, Theme, Renderer>,
    /// The style of the [`ContextMenuOverlay`].
    class: &'a Theme::Class<'b>,
    /// The state shared between [`ContextMenu`](crate::widget::ContextMenu) and [`ContextMenuOverlay`].
    state: &'a mut context_menu::State,
}

impl<'a, 'b, Message, Theme, Renderer> ContextMenuOverlay<'a, 'b, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: renderer::Renderer,
    Theme: 'a + Catalog,
    'b: 'a,
{
    /// Creates a new [`ContextMenuOverlay`].
    ///
    /// The content is laid out right away, inside the given `window` size.
    pub(crate) fn new<C>(
        position: Point,
        window: Size,
        renderer: &Renderer,
        tree: &'a mut Tree,
        content: C,
        class: &'a <Theme as Catalog>::Class<'b>,
        state: &'a mut context_menu::State,
    ) -> Self
    where
        C: Into<Element<'a, Message, Theme, Renderer>>,
    {
        let mut content = content.into();

        let limits = Limits::new(Size::ZERO, window);
        content.layout(tree, renderer, &limits);

        let content_size = tree.size;

        // Try to stay inside borders
        let mut content_position = position;
        if content_position.x + content_size.width > window.width {
            content_position.x = f32::max(0.0, content_position.x - content_size.width);
        }
        if content_position.y + content_size.height > window.height {
            content_position.y = f32::max(0.0, content_position.y - content_size.height);
        }

        let layout = Layout::new(content_size).move_to(content_position);

        ContextMenuOverlay {
            position,
            window: Rectangle::new(Point::ORIGIN, window),
            layout,
            tree,
            content,
            class,
            state,
        }
    }

    /// Turn this [`ContextMenuOverlay`] into an overlay [`Element`](overlay::Element).
    #[must_use]
    pub fn overlay(self) -> overlay::Element<'a, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }
}

impl<'a, 'b, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for ContextMenuOverlay<'a, 'b, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: 'a + Catalog,
    'b: 'a,
{
    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        cursor: Cursor,
    ) {
        let bounds = self.window;

        let style_sheet = theme.style(self.class, status::Status::Active);

        // Background

        renderer.with_layer(bounds, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: (0.0).into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                style_sheet.background,
            );

            // Modal
            self.content.draw(
                self.tree,
                renderer,
                theme,
                style,
                self.layout,
                cursor,
                &bounds,
            );
        });
    }

    fn update(
        &mut self,
        event: &Event,
        cursor: iced_core::mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let layout_children = self.layout;

        let mut forward_event_to_children = true;
        let mut capture_event = false;

        match &event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
                if *key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    self.state.show = false;
                    forward_event_to_children = false;
                    shell.capture_event();
                }
            }

            Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left | mouse::Button::Right,
            ))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(layout_children.bounds()) {
                    capture_event = true;
                } else {
                    self.state.show = false;
                    forward_event_to_children = false;
                    shell.request_redraw();
                }
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                // close when released because because button send message on release
                self.state.show = false;

                capture_event = true;
            }

            Event::Window(window::Event::Resized { .. }) => {
                self.state.show = false;
                forward_event_to_children = false;
                capture_event = true;
            }

            _ => {}
        }

        if forward_event_to_children {
            self.content.update(
                self.tree,
                event,
                layout_children,
                cursor,
                renderer,
                shell,
                &self.window,
            );
        }
        if capture_event {
            shell.capture_event();
        }
    }

    fn operate(&mut self, renderer: &Renderer, operation: &mut dyn iced_core::widget::Operation) {
        self.content
            .operate(self.tree, self.layout, &self.window, renderer, operation);
    }

    fn mouse_interaction(&self, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        self.content
            .mouse_interaction(self.tree, self.layout, cursor, &self.window, renderer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_core::Point;
    use iced_widget::Widget as _;
    use iced_widget::text::Text;

    const WINDOW: Size = Size::new(1920.0, 1080.0);

    #[derive(Clone)]
    enum TestMessage {}

    fn create_test_tree() -> Tree {
        Tree::empty()
    }

    fn create_test_content() -> Element<'static, TestMessage, iced_widget::Theme, ()> {
        Text::new("Test content").boxed()
    }

    fn create_test_state() -> crate::context_menu::State {
        crate::context_menu::State::new()
    }

    #[test]
    fn context_menu_overlay_new_creates_instance() {
        // Test basic creation of overlay
        let mut tree = create_test_tree();
        let content = create_test_content();
        let class = &<iced_widget::Theme as Catalog>::default();
        let mut state = create_test_state();
        let position = Point::new(100.0, 100.0);

        let overlay =
            ContextMenuOverlay::new(position, WINDOW, &(), &mut tree, content, class, &mut state);

        assert_eq!(overlay.position, position);
    }

    #[test]
    fn context_menu_overlay_creates_with_different_positions() {
        // Test overlay creation with various positions
        let mut tree1 = create_test_tree();
        let mut tree2 = create_test_tree();
        let mut tree3 = create_test_tree();
        let content1 = create_test_content();
        let content2 = create_test_content();
        let content3 = create_test_content();
        let class = &<iced_widget::Theme as Catalog>::default();
        let mut state1 = create_test_state();
        let mut state2 = create_test_state();
        let mut state3 = create_test_state();

        let overlay1 = ContextMenuOverlay::new(
            Point::ORIGIN,
            WINDOW,
            &(),
            &mut tree1,
            content1,
            class,
            &mut state1,
        );
        let overlay2 = ContextMenuOverlay::new(
            Point::new(500.0, 300.0),
            WINDOW,
            &(),
            &mut tree2,
            content2,
            class,
            &mut state2,
        );
        let overlay3 = ContextMenuOverlay::new(
            Point::new(1000.0, 800.0),
            WINDOW,
            &(),
            &mut tree3,
            content3,
            class,
            &mut state3,
        );

        assert_eq!(overlay1.position, Point::ORIGIN);
        assert_eq!(overlay2.position, Point::new(500.0, 300.0));
        assert_eq!(overlay3.position, Point::new(1000.0, 800.0));
    }

    #[test]
    fn context_menu_overlay_converts_to_overlay_element() {
        // Test conversion to overlay::Element
        let mut tree = create_test_tree();
        let content = create_test_content();
        let class = &<iced_widget::Theme as Catalog>::default();
        let mut state = create_test_state();
        let position = Point::new(100.0, 100.0);

        let overlay =
            ContextMenuOverlay::new(position, WINDOW, &(), &mut tree, content, class, &mut state);
        let _overlay_element = overlay.overlay();

        // If we get here without panic, the conversion worked
    }

    #[test]
    fn context_menu_overlay_state_can_be_shown() {
        // Test that state can be set to shown
        let mut state = create_test_state();
        assert!(!state.show);

        state.show = true;
        assert!(state.show);
    }

    #[test]
    fn context_menu_overlay_state_can_be_hidden() {
        // Test that state can be set to hidden
        let mut state = create_test_state();
        state.show = true;
        assert!(state.show);

        state.show = false;
        assert!(!state.show);
    }

    #[test]
    fn context_menu_overlay_state_tracks_cursor_position() {
        // Test that state can track cursor position for overlay placement
        let mut state = create_test_state();
        assert_eq!(state.cursor_position, Point::ORIGIN);

        state.cursor_position = Point::new(123.4, 567.8);
        assert_eq!(state.cursor_position.x, 123.4);
        assert_eq!(state.cursor_position.y, 567.8);
    }

    #[test]
    fn context_menu_overlay_state_show_and_position_update_together() {
        // Test that state can track both show status and cursor position
        // This simulates what happens during a right-click event
        let mut state = create_test_state();
        assert!(!state.show);
        assert_eq!(state.cursor_position, Point::ORIGIN);

        // Simulate showing the overlay at a specific position
        state.show = true;
        state.cursor_position = Point::new(200.0, 300.0);

        assert!(state.show);
        assert_eq!(state.cursor_position, Point::new(200.0, 300.0));

        // Simulate hiding the overlay (position stays the same)
        state.show = false;
        assert!(!state.show);
        assert_eq!(state.cursor_position, Point::new(200.0, 300.0));
    }

    #[test]
    fn context_menu_overlay_position_can_be_negative() {
        // Test that overlay can be positioned with negative coordinates
        // (Though layout() will adjust it, the initial position can be negative)
        let mut tree = create_test_tree();
        let content = create_test_content();
        let class = &<iced_widget::Theme as Catalog>::default();
        let mut state = create_test_state();
        let position = Point::new(-50.0, -100.0);

        let overlay =
            ContextMenuOverlay::new(position, WINDOW, &(), &mut tree, content, class, &mut state);

        assert_eq!(overlay.position.x, -50.0);
        assert_eq!(overlay.position.y, -100.0);
    }

    #[test]
    fn context_menu_overlay_position_accuracy() {
        // Test that position is stored with floating-point precision
        let mut tree = create_test_tree();
        let content = create_test_content();
        let class = &<iced_widget::Theme as Catalog>::default();
        let mut state = create_test_state();
        let position = Point::new(123.456, 789.012);

        let overlay =
            ContextMenuOverlay::new(position, WINDOW, &(), &mut tree, content, class, &mut state);

        assert_eq!(overlay.position.x, 123.456);
        assert_eq!(overlay.position.y, 789.012);
    }
}
