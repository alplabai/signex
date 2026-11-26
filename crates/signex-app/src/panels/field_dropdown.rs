//! A field with its menu floating under it (#2674, mockup row 151): the
//! card opens under the field, right-aligned to it, as an overlay over the
//! dock, so it takes no room in the panel and the rows below stay put. A
//! press outside the card and the field closes it.

use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Overlay, Shell, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

/// `field`, with `card` floating under it.
pub(crate) struct FieldDropdown<'a, Message> {
    /// The field, then the card.
    children: [Element<'a, Message>; 2],
    gap: f32,
    on_outside_press: Option<Message>,
}

impl<'a, Message> FieldDropdown<'a, Message> {
    /// `card` opens `gap` under `field`.
    pub(crate) fn new(field: Element<'a, Message>, card: Element<'a, Message>, gap: f32) -> Self {
        Self {
            children: [field, card],
            gap,
            on_outside_press: None,
        }
    }

    /// Publishes `message` on a press outside both the card and the field
    /// (a press on the field is the field's own). The press still reaches
    /// whatever is under it.
    pub(crate) fn on_outside_press(mut self, message: Message) -> Self {
        self.on_outside_press = Some(message);
        self
    }
}

/// Top-left of a `card` under `field`, in window coordinates: its right edge
/// on the field's, `gap` below it; above the field when there is no room
/// below; always inside the window, even for a field the panel has
/// scrolled down past the window's edge (#3041).
fn menu_origin(field: Rectangle, card: Size, gap: f32, window: Size) -> Point {
    let x = (field.x + field.width - card.width)
        .min(window.width - card.width)
        .max(0.0);
    let below = field.y + field.height + gap;
    let above = field.y - gap - card.height;
    let y = if below + card.height <= window.height || above < 0.0 {
        below
    } else {
        above
    };
    Point::new(x, y.min(window.height - card.height).max(0.0))
}

fn is_pointer_press(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse::Event::ButtonPressed(_))
            | Event::Touch(iced::touch::Event::FingerPressed { .. })
    )
}

impl<Message> widget::Meta for FieldDropdown<'_, Message> {}

impl<Message: Clone> Widget<Message, Theme, Renderer> for FieldDropdown<'_, Message> {
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(self.children.as_mut_slice());
    }

    fn size(&self) -> Size<Length> {
        self.children[0].size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let [field, card] = &mut self.children;
        field.layout(&mut tree.children[0], renderer, limits);
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
        if let Some((field_layout, field_tree)) = layout.iter_mut(&mut tree.children).next() {
            self.children[0].operate(field_tree, field_layout, viewport, renderer, operation);
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
        if let Some((field_layout, field_tree)) = layout.iter_mut(&mut tree.children).next() {
            self.children[0].update(
                field_tree,
                event,
                field_layout,
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
        layout.iter(&tree.children).next().map_or(
            mouse::Interaction::None,
            |(field_layout, field_tree)| {
                self.children[0].mouse_interaction(
                    field_tree,
                    field_layout,
                    cursor,
                    viewport,
                    renderer,
                )
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
        if let Some((field_layout, field_tree)) = layout.iter(&tree.children).next() {
            self.children[0].draw(
                field_tree,
                renderer,
                theme,
                style,
                field_layout,
                cursor,
                viewport,
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
        let field_bounds = layout.bounds() + translation;
        let [field, card] = &mut self.children;
        let mut laid_out = layout.iter_mut(&mut tree.children);
        let (Some((field_layout, field_tree)), Some((card_layout, card_tree))) =
            (laid_out.next(), laid_out.next())
        else {
            return Vec::new();
        };
        let origin = menu_origin(field_bounds, card_layout.size(), self.gap, window);
        let mut overlays = field.overlay(
            field_tree,
            field_layout,
            renderer,
            viewport,
            translation,
            window,
        );
        overlays.push(overlay::Element::new(Box::new(CardLayer {
            card,
            tree: card_tree,
            layout: card_layout.move_to(origin),
            field: field_bounds,
            window,
            on_outside_press: self.on_outside_press.as_ref(),
        })));
        overlays
    }
}

/// The floating card, laid out at its window position.
struct CardLayer<'a, 'b, Message> {
    card: &'a mut Element<'b, Message>,
    tree: &'a mut Tree,
    layout: Layout,
    /// The field it opened from, in window coordinates.
    field: Rectangle,
    window: Size,
    on_outside_press: Option<&'a Message>,
}

impl<Message: Clone> Overlay<Message, Theme, Renderer> for CardLayer<'_, '_, Message> {
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
        let observed = cursor.observe();
        if let Some(message) = self.on_outside_press
            && is_pointer_press(event)
            && !observed.is_over(self.layout.bounds())
            && !observed.is_over(self.field)
        {
            shell.publish(message.clone());
        }
    }

    /// The card's padding claims the pointer too, so a press there never
    /// reaches the rows beneath it.
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
    use iced::widget::{Column, Space, Widget as _, container, row};
    use iced_runtime::UserInterface;
    use iced_runtime::user_interface::Cache;
    use signex_i18n::UserText;
    use signex_widgets::{button, text};

    const WINDOW: Size = Size::new(360.0, 300.0);
    const FIELD_H: f32 = 24.0;
    const FIELD_W: f32 = 160.0;
    const CARD: Size = Size::new(200.0, 90.0);
    const GAP: f32 = 4.0;

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Hit {
        Field,
        Card,
        Closed,
    }

    /// A row with the field at its right end and the open card.
    fn fixture<'a>() -> Element<'a, Hit> {
        let field = button(Space::new())
            .width(Length::Fixed(FIELD_W))
            .height(Length::Fixed(FIELD_H))
            .on_press(Hit::Field);
        let card = container(
            button(Space::new())
                .width(Length::Fixed(CARD.width / 2.0))
                .height(Length::Fixed(CARD.height / 2.0))
                .on_press(Hit::Card),
        )
        .center_x(Length::Fixed(CARD.width))
        .center_y(Length::Fixed(CARD.height));
        let dropdown =
            FieldDropdown::new(field.boxed(), card.boxed(), GAP).on_outside_press(Hit::Closed);
        row![Space::new().width(Length::Fill), dropdown.boxed()]
            .width(Length::Fill)
            .boxed()
    }

    fn field_bounds() -> Rectangle {
        Rectangle::new(
            Point::new(WINDOW.width - FIELD_W, 0.0),
            Size::new(FIELD_W, FIELD_H),
        )
    }

    fn clicked(at: Point) -> Vec<Hit> {
        clicked_in(fixture(), at)
    }

    /// The messages a left click at `at` publishes from `element`.
    fn clicked_in(element: Element<'_, Hit>, at: Point) -> Vec<Hit> {
        let mut renderer = headless_renderer();
        let mut ui = UserInterface::build(element, WINDOW, Cache::default(), &mut renderer);
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

    /// Mockup row 151: the open menu floats, so the Local nets row under
    /// the Designators field does not move down.
    #[test]
    fn the_open_menu_leaves_the_next_row_where_it_was() {
        let label = |s: &'static str| -> Element<'static, Hit> {
            text(UserText::new(s.to_string())).boxed()
        };
        let plain = Column::new().push(label("field")).push(label("next"));
        let tall = container(Space::new()).height(Length::Fixed(CARD.height));
        let open = Column::new()
            .push(FieldDropdown::new(label("field"), tall.boxed(), GAP).boxed())
            .push(label("next"));
        assert_eq!(y_of(open.boxed(), "next"), y_of(plain.boxed(), "next"));
    }

    /// Mockup row 151: under the field, right-aligned to it.
    #[test]
    fn the_menu_opens_under_the_field_right_aligned_to_it() {
        let field = Rectangle::new(Point::new(150.0, 100.0), Size::new(180.0, FIELD_H));
        let at = menu_origin(field, CARD, GAP, WINDOW);
        assert_eq!(at.x + CARD.width, field.x + field.width);
        assert_eq!(at.y, field.y + field.height + GAP);
    }

    #[test]
    fn the_menu_stays_inside_the_window() {
        let narrow = Rectangle::new(Point::new(0.0, 10.0), Size::new(60.0, FIELD_H));
        assert_eq!(menu_origin(narrow, CARD, GAP, WINDOW).x, 0.0);
        let low = Rectangle::new(Point::new(150.0, 260.0), Size::new(180.0, FIELD_H));
        let at = menu_origin(low, CARD, GAP, WINDOW);
        assert_eq!(at.y + CARD.height + GAP, low.y, "no room below: above it");
        let tiny = Size::new(WINDOW.width, 60.0);
        let at = menu_origin(
            Rectangle::new(Point::new(0.0, 20.0), Size::new(360.0, 20.0)),
            CARD,
            GAP,
            tiny,
        );
        assert_eq!(at.y, 0.0);
    }

    /// Where the field sits, scrolled down the panel past the window's
    /// bottom edge (mockup-audit-5, m151-naming).
    const FIELD_BELOW: f32 = WINDOW.height + 100.0;

    /// #3041: a field the panel has scrolled below the window opens its
    /// menu inside the window, its bottom on the window's.
    #[test]
    fn a_field_below_the_window_opens_its_menu_inside_it() {
        let field = Rectangle::new(Point::new(150.0, FIELD_BELOW), Size::new(180.0, FIELD_H));
        let at = menu_origin(field, CARD, GAP, WINDOW);
        assert_eq!(at.y + CARD.height, WINDOW.height);
        let half_out = Rectangle::new(
            Point::new(150.0, WINDOW.height - FIELD_H / 2.0),
            Size::new(180.0, FIELD_H),
        );
        let at = menu_origin(half_out, CARD, GAP, WINDOW);
        assert_eq!(at.y + CARD.height + GAP, half_out.y, "above it");
    }

    /// The panel's scroller with the field at its right end, below the
    /// window, and the open card.
    fn scrolled_fixture<'a>() -> Element<'a, Hit> {
        let field = button(Space::new())
            .width(Length::Fixed(FIELD_W))
            .height(Length::Fixed(FIELD_H))
            .on_press(Hit::Field);
        let card = container(
            button(Space::new())
                .width(Length::Fixed(CARD.width / 2.0))
                .height(Length::Fixed(CARD.height / 2.0))
                .on_press(Hit::Card),
        )
        .center_x(Length::Fixed(CARD.width))
        .center_y(Length::Fixed(CARD.height));
        let dropdown =
            FieldDropdown::new(field.boxed(), card.boxed(), GAP).on_outside_press(Hit::Closed);
        let line = row![Space::new().width(Length::Fill), dropdown.boxed()].width(Length::Fill);
        iced::widget::scrollable(
            Column::new()
                .push(Space::new().height(Length::Fixed(FIELD_BELOW)).boxed())
                .push(line.boxed())
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
    }

    /// #3041 on the laid-out widget: the card's centre is inside the
    /// window, where a press reaches it.
    #[test]
    fn the_card_of_a_field_below_the_window_takes_a_press_inside_it() {
        let centre = Point::new(
            WINDOW.width - CARD.width / 2.0,
            WINDOW.height - CARD.height / 2.0,
        );
        assert_eq!(clicked_in(scrolled_fixture(), centre), vec![Hit::Card]);
    }

    #[test]
    fn a_press_on_the_card_reaches_the_card_and_keeps_it_open() {
        let origin = menu_origin(field_bounds(), CARD, GAP, WINDOW);
        let centre = Point::new(origin.x + CARD.width / 2.0, origin.y + CARD.height / 2.0);
        assert_eq!(clicked(centre), vec![Hit::Card]);
        let padding = Point::new(origin.x + 2.0, origin.y + 2.0);
        assert_eq!(clicked(padding), Vec::<Hit>::new());
    }

    #[test]
    fn a_press_on_the_field_is_the_fields_own() {
        let on_field = Point::new(WINDOW.width - FIELD_W / 2.0, FIELD_H / 2.0);
        assert_eq!(clicked(on_field), vec![Hit::Field]);
    }

    #[test]
    fn a_press_outside_closes_the_menu() {
        assert_eq!(
            clicked(Point::new(4.0, WINDOW.height - 4.0)),
            vec![Hit::Closed]
        );
    }
}
