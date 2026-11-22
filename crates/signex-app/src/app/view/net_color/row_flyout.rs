//! A list row with a card floating beside it (mockup row 71). The card is an
//! overlay, so it takes no room in the list and the rows below stay put. It
//! stays inside the palette's own window, which cannot draw past its edge.

use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Overlay, Shell, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

use crate::app::view::context_menu::clamp_menu_origin;

/// `row`, with `card` floating level with it at the window's right edge.
pub(super) struct RowFlyout<'a, Message> {
    /// The row, then the card.
    children: [Element<'a, Message>; 2],
}

impl<'a, Message> RowFlyout<'a, Message> {
    pub(super) fn new(row: Element<'a, Message>, card: Element<'a, Message>) -> Self {
        Self {
            children: [row, card],
        }
    }
}

/// Top-left of a `card` beside `row`, all in window coordinates: level with
/// the row, held inside the visible `list` when the row scrolls out, and
/// against the window's right edge.
fn chooser_origin(row: Rectangle, card: Size, list: Rectangle, window: Size) -> Point {
    let top = row.y.min(list.y + list.height - card.height).max(list.y);
    let (x, y) = clamp_menu_origin(
        (window.width, top),
        (card.width, card.height),
        (window.width, window.height),
    );
    Point::new(x, y)
}

impl<Message> widget::Meta for RowFlyout<'_, Message> {}

impl<Message> Widget<Message, Theme, Renderer> for RowFlyout<'_, Message> {
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(self.children.as_mut_slice());
    }

    fn size(&self) -> Size<Length> {
        self.children[0].size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let [row, card] = &mut self.children;
        row.layout(&mut tree.children[0], renderer, limits);
        let loose = Limits::new(Size::ZERO, Size::INFINITE);
        card.layout(&mut tree.children[1], renderer, &loose);
        tree.size = tree.children[0].size;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some((row_layout, row_tree)) = layout.iter_mut(&mut tree.children).next() {
            self.children[0].operate(row_tree, row_layout, viewport, renderer, operation);
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
        if let Some((row_layout, row_tree)) = layout.iter_mut(&mut tree.children).next() {
            self.children[0].update(
                row_tree, event, row_layout, cursor, renderer, shell, viewport,
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
        layout.iter(&tree.children).next().map_or(
            mouse::Interaction::None,
            |(row_layout, row_tree)| {
                self.children[0].mouse_interaction(row_tree, row_layout, cursor, viewport, renderer)
            },
        )
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
        if let Some((row_layout, row_tree)) = layout.iter(&tree.children).next() {
            self.children[0].draw(
                row_tree, renderer, theme, style, row_layout, cursor, viewport,
            );
        }
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
        let row_bounds = layout.bounds() + translation;
        let list = *viewport + translation;
        let [row, card] = &mut self.children;
        let mut laid_out = layout.iter_mut(&mut tree.children);
        let (Some((row_layout, row_tree)), Some((card_layout, card_tree))) =
            (laid_out.next(), laid_out.next())
        else {
            return Vec::new();
        };
        let origin = chooser_origin(row_bounds, card_layout.size(), list, window);
        let mut overlays = row.overlay(
            row_tree,
            row_layout,
            renderer,
            viewport,
            translation,
            window,
        );
        overlays.push(overlay::Element::new(Box::new(CardLayer {
            card,
            tree: card_tree,
            layout: card_layout.move_to(origin),
            window,
        })));
        overlays
    }
}

/// The floating card, laid out at its window position.
struct CardLayer<'a, 'b, Message> {
    card: &'a mut Element<'b, Message>,
    tree: &'a mut Tree,
    layout: Layout,
    window: Size,
}

impl<Message> Overlay<Message, Theme, Renderer> for CardLayer<'_, '_, Message> {
    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        cursor: Cursor,
    ) {
        use iced::advanced::Renderer as _;
        let window = Rectangle::with_size(self.window);
        renderer.with_layer(window, |renderer| {
            self.card.draw(
                self.tree,
                renderer,
                theme,
                style,
                self.layout,
                cursor,
                &window,
            );
        });
    }

    fn operate(&mut self, renderer: &Renderer, operation: &mut dyn widget::Operation) {
        let window = Rectangle::with_size(self.window);
        self.card
            .operate(self.tree, self.layout, &window, renderer, operation);
    }

    fn update(
        &mut self,
        event: &Event,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let window = Rectangle::with_size(self.window);
        self.card.update(
            self.tree,
            event,
            self.layout,
            cursor,
            renderer,
            shell,
            &window,
        );
    }

    /// The card's padding claims the pointer too, so a press there never
    /// reaches the row beneath it.
    fn mouse_interaction(&self, cursor: Cursor, renderer: &Renderer) -> mouse::Interaction {
        let window = Rectangle::with_size(self.window);
        match self
            .card
            .mouse_interaction(self.tree, self.layout, cursor, &window, renderer)
        {
            mouse::Interaction::None if cursor.is_over(self.layout.bounds()) => {
                mouse::Interaction::Idle
            }
            interaction => interaction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{headless_renderer, lay_out_texts};
    use iced::advanced::shell::{Bus, Waker};
    use iced::widget::{Column, Space, Widget as _, container};
    use iced_runtime::UserInterface;
    use iced_runtime::user_interface::Cache;
    use signex_i18n::UserText;
    use signex_widgets::{button, text};

    const WINDOW: Size = Size::new(360.0, 300.0);
    const ROW_H: f32 = 24.0;
    const CARD: Size = Size::new(200.0, 90.0);

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Hit {
        Row,
        Card,
    }

    fn fixture<'a>() -> Element<'a, Hit> {
        let row = button(Space::new())
            .width(Length::Fill)
            .height(Length::Fixed(ROW_H))
            .on_press(Hit::Row);
        let card = container(
            button(Space::new())
                .width(Length::Fixed(CARD.width / 2.0))
                .height(Length::Fixed(CARD.height / 2.0))
                .on_press(Hit::Card),
        )
        .center_x(Length::Fixed(CARD.width))
        .center_y(Length::Fixed(CARD.height));
        RowFlyout::new(row.boxed(), card.boxed()).boxed()
    }

    fn clicked(at: Point) -> Vec<Hit> {
        let mut renderer = headless_renderer();
        let mut ui = UserInterface::build(fixture(), WINDOW, Cache::default(), &mut renderer);
        let mut bus = Bus::new();
        let events = [
            mouse::Event::CursorMoved { position: at },
            mouse::Event::ButtonPressed(mouse::Button::Left),
            mouse::Event::ButtonReleased(mouse::Button::Left),
        ]
        .map(Event::Mouse);
        for event in events {
            let _ = ui.update(
                &iced::window::Headless,
                &Waker::noop(),
                std::slice::from_ref(&event),
                Cursor::Available(at),
                &mut renderer,
                &mut bus,
            );
        }
        bus.drain().map(|(hit, _)| hit).collect()
    }

    fn y_of(element: Element<'_, Hit>, label: &str) -> f32 {
        lay_out_texts(element, WINDOW)
            .into_iter()
            .find(|laid| laid.text == label)
            .map(|laid| laid.bounds.y)
            .expect("the label is laid out")
    }

    #[test]
    fn the_open_chooser_leaves_the_next_row_where_it_was() {
        let label = |s: &'static str| -> Element<'static, Hit> {
            text(UserText::new(s.to_string())).boxed()
        };
        let plain = Column::new().push(label("first")).push(label("next"));
        let tall = container(Space::new()).height(Length::Fixed(CARD.height));
        let open = Column::new()
            .push(RowFlyout::new(label("first"), tall.boxed()).boxed())
            .push(label("next"));
        assert_eq!(y_of(open.boxed(), "next"), y_of(plain.boxed(), "next"));
    }

    #[test]
    fn the_chooser_sits_level_with_its_row_at_the_right_edge() {
        let list = Rectangle::new(Point::new(0.0, 40.0), Size::new(360.0, 200.0));
        let row = Rectangle::new(Point::new(0.0, 100.0), Size::new(360.0, ROW_H));
        let at = chooser_origin(row, CARD, list, WINDOW);
        assert_eq!(at.y, 100.0);
        assert!(at.x + CARD.width <= WINDOW.width);
        assert!(at.x > 0.0, "the names at the left stay readable");
    }

    #[test]
    fn the_chooser_stays_in_the_list_when_its_row_scrolls_out() {
        let list = Rectangle::new(Point::new(0.0, 40.0), Size::new(360.0, 200.0));
        let above = Rectangle::new(Point::new(0.0, -50.0), Size::new(360.0, ROW_H));
        assert_eq!(chooser_origin(above, CARD, list, WINDOW).y, 40.0);
        let below = Rectangle::new(Point::new(0.0, 500.0), Size::new(360.0, ROW_H));
        assert_eq!(
            chooser_origin(below, CARD, list, WINDOW).y,
            40.0 + 200.0 - CARD.height
        );
    }

    #[test]
    fn a_press_on_the_card_reaches_the_card_and_not_the_row_under_it() {
        let origin = chooser_origin(
            Rectangle::new(Point::ORIGIN, Size::new(WINDOW.width, ROW_H)),
            CARD,
            Rectangle::with_size(WINDOW),
            WINDOW,
        );
        let centre = Point::new(origin.x + CARD.width / 2.0, origin.y + CARD.height / 2.0);
        assert_eq!(clicked(centre), vec![Hit::Card]);
        let padding_over_row = Point::new(origin.x + 2.0, ROW_H / 2.0);
        assert_eq!(clicked(padding_over_row), Vec::<Hit>::new());
        assert_eq!(clicked(Point::new(4.0, ROW_H / 2.0)), vec![Hit::Row]);
    }
}
