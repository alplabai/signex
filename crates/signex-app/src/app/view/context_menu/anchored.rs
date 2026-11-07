//! A menu card laid at the point it was opened, with its optional submenu
//! flyout. The placement runs in `layout`, on the card's real size and the
//! layer's real bounds (the window the overlay draws in), so a card opens
//! at the cursor and moves only to stay inside that window: no row-count
//! guess can make it jump (the canvas menu once opened over the menu bar
//! because a 22-row guess was taller than the space below the click).

use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Shell, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

use super::placement::{clamp_menu_origin, submenu_origin};

/// A card at `anchor`, and a flyout beside it.
pub(in crate::app::view) struct AnchoredMenu<'a, Message> {
    /// The card, then the flyout when there is one.
    children: Vec<Element<'a, Message>>,
    anchor: (f32, f32),
    /// Where the flyout's top sits below the card's top.
    flyout_offset: f32,
}

impl<'a, Message: 'a> AnchoredMenu<'a, Message> {
    /// `card` opened at the window point `anchor`.
    pub fn new(card: Element<'a, Message>, anchor: (f32, f32)) -> Self {
        Self {
            children: vec![card],
            anchor,
            flyout_offset: 0.0,
        }
    }

    /// Add `flyout` beside the card, its top `offset` below the card's top
    /// (level with the launcher row that opened it).
    pub fn flyout(mut self, flyout: Element<'a, Message>, offset: f32) -> Self {
        self.children.truncate(1);
        self.children.push(flyout);
        self.flyout_offset = offset;
        self
    }
}

impl<Message> widget::Meta for AnchoredMenu<'_, Message> {}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for AnchoredMenu<'_, Message> {
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(self.children.as_mut_slice());
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) {
        let window = limits.max;
        let loose = Limits::new(Size::ZERO, window);
        let window = (window.width, window.height);
        let mut card = (0.0, 0.0);
        let mut origin = self.anchor;
        for (i, (child, child_tree)) in self
            .children
            .iter_mut()
            .zip(tree.children.iter_mut())
            .enumerate()
        {
            child.layout(child_tree, renderer, &loose);
            let size = (child_tree.size.width, child_tree.size.height);
            let at = if i == 0 {
                card = size;
                origin = clamp_menu_origin(self.anchor, size, window);
                origin
            } else {
                submenu_origin(origin, card.0, size, self.flyout_offset, window)
            };
            child_tree.translation = Vector::new(at.0, at.1);
        }
        tree.size = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        for (child, (child_layout, child_tree)) in self
            .children
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
        {
            child.operate(child_tree, child_layout, viewport, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: Cursor,
        renderer: &iced::Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // The flyout draws on top, so it sees an event first.
        for (child, (child_layout, child_tree)) in self
            .children
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
            .rev()
        {
            child.update(
                child_tree,
                event,
                child_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );
            if shell.is_event_captured() {
                return;
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.children
            .iter()
            .zip(layout.iter(&tree.children))
            .rev()
            .map(|(child, (child_layout, child_tree))| {
                child.mouse_interaction(child_tree, child_layout, cursor, viewport, renderer)
            })
            .find(|interaction| *interaction != mouse::Interaction::None)
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        for (i, (child, (child_layout, child_tree))) in self
            .children
            .iter()
            .zip(layout.iter(&tree.children))
            .enumerate()
        {
            // The flyout gets its own layer: one layer draws every quad before
            // any text, so the card's labels would show through it.
            if i == 0 {
                child.draw(
                    child_tree,
                    renderer,
                    theme,
                    style,
                    child_layout,
                    cursor,
                    viewport,
                );
            } else {
                renderer.with_layer(*viewport, |renderer| {
                    child.draw(
                        child_tree,
                        renderer,
                        theme,
                        style,
                        child_layout,
                        cursor,
                        viewport,
                    );
                });
            }
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.children
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
            .flat_map(|(child, (child_layout, child_tree))| {
                child.overlay(
                    child_tree,
                    child_layout,
                    renderer,
                    viewport,
                    translation,
                    window,
                )
            })
            .collect()
    }
}
