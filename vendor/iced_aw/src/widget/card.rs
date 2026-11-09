//! Displays a [`Card`].
//!
//! *This API requires the following crate features to be activated: card*

use crate::iced_aw_font::advanced_text::cancel;
pub use crate::style::{
    card::{Catalog, Style},
    status::{Status, StyleFn},
};
use iced_core::{
    Alignment, Border, Color, Element, Event, Layout, Length, Padding, Rectangle, Shadow, Shell,
    Size, Vector, Widget,
    layout::Limits,
    mouse::{self, Cursor},
    overlay, renderer,
    widget::{Meta, Operation, Tree, tree},
};
use iced_widget::button;

/// The default padding of a [`Card`].
const DEFAULT_PADDING: Padding = Padding::new(10.0);

/// The default size of the close button icon
const DEFAULT_CLOSE_SIZE: f32 = 16.0;

/// Extra space around the close button for click area
const CLOSE_BUTTON_SPACING: f32 = 1.0;

/// A card consisting of a head, body and optional foot.
///
/// # Example
/// ```ignore
/// # use iced_widget::Text;
/// # use iced_aw::Card;
/// #
/// #[derive(Debug, Clone)]
/// enum Message {
///     ClosingCard,
/// }
///
/// let card = Card::new(
///     Text::new("Head"),
///     Text::new("Body")
/// )
/// .foot(Text::new("Foot"))
/// .on_close(Message::ClosingCard);
///
/// ```
#[allow(missing_debug_implementations)]
pub struct Card<'a, Message, Theme = iced_widget::Theme, Renderer = iced_widget::Renderer>
where
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
    /// The width of the [`Card`].
    width: Length,
    /// The height of the [`Card`].
    height: Length,
    /// The maximum width of the [`Card`].
    max_width: f32,
    /// The maximum height of the [`Card`].
    max_height: f32,
    /// The padding of the head of the [`Card`].
    padding_head: Padding,
    /// The padding of the body of the [`Card`].
    padding_body: Padding,
    /// The padding of the foot of the [`Card`].
    padding_foot: Padding,
    /// The optional size of the close icon of the [`Card`].
    close_size: Option<f32>,
    /// The optional message that is send if the close icon of the [`Card`] is pressed.
    on_close: Option<Message>,
    /// The head [`Element`] of the [`Card`].
    head: Element<'a, Message, Theme, Renderer>,
    /// The body [`Element`] of the [`Card`].
    body: Element<'a, Message, Theme, Renderer>,
    /// The optional foot [`Element`] of the [`Card`].
    foot: Option<Element<'a, Message, Theme, Renderer>>,
    /// The optional close button [`Element`] of the [`Card`].
    close_button: Option<Element<'a, Message, Theme, Renderer>>,
    /// The style of the [`Card`].
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Card<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
    /// Creates a new [`Card`] containing the given head and body.
    ///
    /// It expects:
    ///     * the head [`Element`] to display at the top of the [`Card`].
    ///     * the body [`Element`] to display at the middle of the [`Card`].
    pub fn new<H, B>(head: H, body: B) -> Self
    where
        H: Widget<Message, Theme, Renderer> + 'a,
        B: Widget<Message, Theme, Renderer> + 'a,
    {
        Card {
            width: Length::Fill,
            height: Length::Shrink,
            max_width: u32::MAX as f32,
            max_height: u32::MAX as f32,
            padding_head: DEFAULT_PADDING,
            padding_body: DEFAULT_PADDING,
            padding_foot: DEFAULT_PADDING,
            close_size: None,
            on_close: None,
            head: Widget::_boxed(head),
            body: Widget::_boxed(body),
            foot: None,
            close_button: None,
            class: Theme::default(),
        }
    }

    /// Sets the [`Element`] of the foot of the [`Card`].
    #[must_use]
    pub fn foot<F>(mut self, foot: F) -> Self
    where
        F: Widget<Message, Theme, Renderer> + 'a,
    {
        self.foot = Some(Widget::_boxed(foot));
        self
    }

    /// Sets the size of the close icon of the [`Card`].
    #[must_use]
    pub fn close_size(mut self, size: f32) -> Self {
        self.close_size = Some(size);
        self
    }

    /// Sets the height of the [`Card`].
    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the maximum height of the [`Card`].
    #[must_use]
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = height;
        self
    }

    /// Sets the maximum width of the [`Card`].
    #[must_use]
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    /// Sets the message that will be produced when the close icon of the
    /// [`Card`] is pressed.
    ///
    /// Setting this enables the drawing of a close icon on the [`Card`].
    #[must_use]
    pub fn on_close(mut self, msg: Message) -> Self
    where
        Message: Clone + 'a,
        Renderer: iced_core::text::Renderer + 'a,
        Theme: iced_widget::text::Catalog + iced_widget::button::Catalog + 'a,
        <Theme as iced_widget::button::Catalog>::Class<'a>:
            From<iced_widget::button::StyleFn<'a, Theme>>,
    {
        self.on_close = Some(msg.clone());
        self.close_button = Some(self.create_close_button(msg));
        self
    }

    /// Creates a close button element.
    fn create_close_button(&self, msg: Message) -> Element<'a, Message, Theme, Renderer>
    where
        Message: Clone + 'a,
        Renderer: iced_core::text::Renderer + 'a,
        Theme: iced_widget::text::Catalog + iced_widget::button::Catalog + 'a,
        <Theme as iced_widget::button::Catalog>::Class<'a>:
            From<iced_widget::button::StyleFn<'a, Theme>>,
    {
        let (content, font, shaping) = cancel();
        let size = self.close_size.unwrap_or(DEFAULT_CLOSE_SIZE);

        let text_widget = iced_widget::text(content)
            .font(font)
            .size(size)
            .shaping(shaping);

        let button = button(text_widget)
            .padding(0)
            .style(|theme: &Theme, _status| {
                let card_style = <Theme as Catalog>::style(
                    theme,
                    &<Theme as Catalog>::default(),
                    crate::style::status::Status::Active,
                );
                iced_widget::button::Style {
                    background: None,
                    text_color: card_style.close_color,
                    border: iced_core::Border::default(),
                    shadow: iced_core::Shadow::default(),
                    snap: false,
                    focus_ring: None,
                }
            })
            .on_press(msg);

        Widget::_boxed(button)
    }

    /// Sets the padding of the [`Card`].
    ///
    /// This will set the padding of the head, body and foot to the
    /// same value.
    #[must_use]
    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding_head = padding;
        self.padding_body = padding;
        self.padding_foot = padding;
        self
    }

    /// Sets the padding of the head of the [`Card`].
    #[must_use]
    pub fn padding_head(mut self, padding: Padding) -> Self {
        self.padding_head = padding;
        self
    }

    /// Sets the padding of the body of the [`Card`].
    #[must_use]
    pub fn padding_body(mut self, padding: Padding) -> Self {
        self.padding_body = padding;
        self
    }

    /// Sets the padding of the foot of the [`Card`].
    #[must_use]
    pub fn padding_foot(mut self, padding: Padding) -> Self {
        self.padding_foot = padding;
        self
    }

    /// Sets the style of the [`Card`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme, Style>).into();
        self
    }

    /// Sets the class of the input of the [`Card`].
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    /// Sets the width of the [`Card`].
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }
}

/// The measured sizes of the sections of a [`Card`], stored in its widget
/// state. Sizes include the padding of each section.
#[derive(Debug, Clone, Copy, Default)]
struct Sections {
    head: Size,
    body: Size,
    foot: Size,
}

/// The result of laying out a section: its size (padding included) and the
/// translation of its content relative to the origin of the section.
#[derive(Debug, Clone, Copy, Default)]
struct Section {
    size: Size,
    content: Vector,
}

/// Applies a maximum width and height to the given [`Limits`].
fn bounded(limits: &Limits, max_width: f32, max_height: f32) -> Limits {
    let max = Size::new(
        limits.max.width.min(max_width),
        limits.max.height.min(max_height),
    );
    let min = Size::new(
        limits.min.width.min(max.width),
        limits.min.height.min(max.height),
    );

    Limits::with_flags(min, max, limits.compression, limits.infinite)
}

impl<'a, Message, Theme, Renderer> Meta for Card<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Card<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Sections>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Sections::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        let mut children: Vec<&mut Element<'a, Message, Theme, Renderer>> =
            vec![&mut self.head, &mut self.body];

        if let Some(foot) = &mut self.foot {
            children.push(foot);
        }

        if let Some(close_button) = &mut self.close_button {
            children.push(close_button);
        }

        tree.diff_children_custom(
            &mut children,
            |tree, child| tree.diff(&mut **child),
            |child| Tree::new(&**child),
        );
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let limits = bounded(limits, self.max_width, self.max_height);

        let close_button_tree_index = 2 + usize::from(self.foot.is_some());

        let head = head_node(
            renderer,
            &limits,
            &mut self.head,
            self.padding_head,
            self.width,
            self.close_button.as_mut(),
            self.close_size,
            tree,
            close_button_tree_index,
        );

        let limits = limits.shrink(Size::new(0.0, head.section.size.height));

        let foot = self.foot.as_mut().map_or_else(Section::default, |foot| {
            foot_node(renderer, &limits, foot, self.padding_foot, self.width, tree)
        });
        let limits = limits.shrink(Size::new(0.0, foot.size.height));
        let body = body_node(
            renderer,
            &limits,
            &mut self.body,
            self.padding_body,
            self.width,
            tree,
        );

        let body_y = head.section.size.height;
        let foot_y = body_y + body.size.height;

        tree.children[0].translation = head.section.content;
        if let Some(close) = head.close {
            tree.children[close_button_tree_index].translation = close;
        }
        tree.children[1].translation = Vector::new(0.0, body_y) + body.content;
        if self.foot.is_some() {
            tree.children[2].translation = Vector::new(0.0, foot_y) + foot.content;
        }

        tree.size = Size::new(
            body.size.width,
            head.section.size.height + body.size.height + foot.size.height,
        );
        *tree.state.downcast_mut::<Sections>() = Sections {
            head: head.section.size,
            body: body.size,
            foot: foot.size,
        };
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let close_button_tree_index = 2 + usize::from(self.foot.is_some());
        let mut children: Vec<_> = layout.iter_mut(&mut state.children).collect();

        {
            let (layout, tree) = &mut children[0];
            self.head
                .update(tree, event, *layout, cursor, renderer, shell, viewport);
        }

        // Update close button if present
        if let Some(close_button) = self.close_button.as_mut() {
            let (layout, tree) = &mut children[close_button_tree_index];
            close_button.update(tree, event, *layout, cursor, renderer, shell, viewport);
        }

        {
            let (layout, tree) = &mut children[1];
            self.body
                .update(tree, event, *layout, cursor, renderer, shell, viewport);
        }

        if let Some(foot) = self.foot.as_mut() {
            let (layout, tree) = &mut children[2];
            foot.update(tree, event, *layout, cursor, renderer, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let close_button_tree_index = 2 + usize::from(self.foot.is_some());
        let children: Vec<_> = layout.iter(&state.children).collect();

        let is_mouse_over_close = self.close_button.is_some()
            && children
                .get(close_button_tree_index)
                .is_some_and(|(layout, _)| {
                    layout
                        .bounds()
                        .contains(cursor.position().unwrap_or_default())
                });

        let mouse_interaction = if is_mouse_over_close {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        };

        let (head_layout, head_tree) = &children[0];
        let (body_layout, body_tree) = &children[1];

        mouse_interaction
            .max(
                self.head
                    .mouse_interaction(head_tree, *head_layout, cursor, viewport, renderer),
            )
            .max(
                self.body
                    .mouse_interaction(body_tree, *body_layout, cursor, viewport, renderer),
            )
            .max(
                self.foot
                    .as_ref()
                    .map_or_else(mouse::Interaction::default, |foot| {
                        let (foot_layout, foot_tree) = &children[2];
                        foot.mouse_interaction(foot_tree, *foot_layout, cursor, viewport, renderer)
                    }),
            )
    }

    fn operate(
        &mut self,
        state: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            let close_button_tree_index = 2 + usize::from(self.foot.is_some());
            let mut children: Vec<_> = layout.iter_mut(&mut state.children).collect();

            // Operate on head
            {
                let (layout, tree) = &mut children[0];
                self.head
                    .operate(tree, *layout, viewport, renderer, operation);
            }

            // Operate on close button if present
            if let Some(close_button) = self.close_button.as_mut() {
                let (layout, tree) = &mut children[close_button_tree_index];
                close_button.operate(tree, *layout, viewport, renderer, operation);
            }

            // Operate on body
            {
                let (layout, tree) = &mut children[1];
                self.body
                    .operate(tree, *layout, viewport, renderer, operation);
            }

            // Operate on footer if present
            if let Some(footer) = self.foot.as_mut() {
                let (layout, tree) = &mut children[2];
                footer.operate(tree, *layout, viewport, renderer, operation);
            }
        });
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let style_sheet = theme.style(&self.class, Status::Active);
        let sections = *state.state.downcast_ref::<Sections>();
        let children: Vec<_> = layout.iter(&state.children).collect();

        if bounds.intersects(viewport) {
            // Background
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: style_sheet.border_radius.into(),
                        width: style_sheet.border_width,
                        color: style_sheet.border_color,
                        ..Border::default()
                    },
                    shadow: Shadow::default(),
                    snap: false,
                },
                style_sheet.background,
            );

            // Border
            renderer.fill_quad(
                // TODO: fill not necessary
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: style_sheet.border_radius.into(),
                        width: style_sheet.border_width,
                        color: style_sheet.border_color,
                        ..Border::default()
                    },
                    shadow: Shadow::default(),
                    snap: false,
                },
                Color::TRANSPARENT,
            );
        }

        let origin = bounds.position();
        let head_bounds = Rectangle::new(origin, sections.head);
        let body_bounds = Rectangle::new(
            origin + Vector::new(0.0, sections.head.height),
            sections.body,
        );
        let foot_bounds = Rectangle::new(
            origin + Vector::new(0.0, sections.head.height + sections.body.height),
            sections.foot,
        );

        // ----------- Head ----------------------
        let close_button_tree_index = 2 + usize::from(self.foot.is_some());
        draw_head(
            renderer,
            head_bounds,
            &self.head,
            children[0],
            cursor,
            viewport,
            theme,
            &style_sheet,
            self.close_button
                .as_ref()
                .zip(children.get(close_button_tree_index).copied()),
        );

        // ----------- Body ----------------------
        draw_body(
            renderer,
            body_bounds,
            &self.body,
            children[1],
            cursor,
            viewport,
            theme,
            &style_sheet,
        );

        // ----------- Foot ----------------------
        draw_foot(
            renderer,
            foot_bounds,
            self.foot.as_ref().zip(children.get(2).copied()),
            cursor,
            viewport,
            theme,
            &style_sheet,
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
        let mut children = vec![&mut self.head, &mut self.body];
        if let Some(foot) = &mut self.foot {
            children.push(foot);
        }

        children
            .into_iter()
            .zip(layout.iter_mut(&mut tree.children))
            .flat_map(|(child, (layout, state))| {
                child.overlay(state, layout, renderer, viewport, translation, window)
            })
            .collect()
    }
}

/// The layout of the head: the head section and, if present, the translation
/// of the close button relative to the origin of the card.
struct HeadLayout {
    section: Section,
    close: Option<Vector>,
}

/// Calculates the layout of the head.
#[allow(clippy::too_many_arguments)]
fn head_node<Message, Theme, Renderer>(
    renderer: &Renderer,
    limits: &Limits,
    head: &mut Element<'_, Message, Theme, Renderer>,
    padding: Padding,
    width: Length,
    close_button: Option<&mut Element<'_, Message, Theme, Renderer>>,
    close_size: Option<f32>,
    tree: &mut Tree,
    close_button_tree_index: usize,
) -> HeadLayout
where
    Renderer: renderer::Renderer + iced_core::text::Renderer,
{
    let header_size = head.size();

    let mut limits = limits
        .loose()
        .width(width)
        .height(header_size.height)
        .shrink(padding);

    let close_size = close_size.unwrap_or_else(|| renderer.text_size().0);

    if close_button.is_some() {
        limits = limits.shrink(Size::new(close_size, 0.0));
    }

    head.layout(&mut tree.children[0], renderer, &limits);
    let head_content_size = tree.children[0].size;
    let mut size = limits.resolve(width, header_size.height, head_content_size);

    let close = if let Some(close_btn) = close_button {
        let button_size = close_size + CLOSE_BUTTON_SPACING;
        let button_limits = limits
            .loose()
            .width(Length::Fixed(button_size))
            .height(Length::Fixed(button_size));
        close_btn.layout(
            &mut tree.children[close_button_tree_index],
            renderer,
            &button_limits,
        );

        size = Size::new(size.width + close_size, size.height);

        Some(Vector::new(size.width - padding.right, padding.top))
    } else {
        None
    };

    HeadLayout {
        section: Section {
            size: size.expand(padding),
            content: Vector::new(padding.left, padding.top),
        },
        close,
    }
}

/// Calculates the layout of the body.
fn body_node<Message, Theme, Renderer>(
    renderer: &Renderer,
    limits: &Limits,
    body: &mut Element<'_, Message, Theme, Renderer>,
    padding: Padding,
    width: Length,
    tree: &mut Tree,
) -> Section
where
    Renderer: renderer::Renderer,
{
    let body_size = body.size();

    let limits = limits
        .loose()
        .width(width)
        .height(body_size.height)
        .shrink(padding);

    body.layout(&mut tree.children[1], renderer, &limits);
    let size = limits.resolve(width, body_size.height, tree.children[1].size);

    Section {
        size: size.expand(padding),
        content: Vector::new(padding.left, padding.top),
    }
}

/// Calculates the layout of the foot.
fn foot_node<Message, Theme, Renderer>(
    renderer: &Renderer,
    limits: &Limits,
    foot: &mut Element<'_, Message, Theme, Renderer>,
    padding: Padding,
    width: Length,
    tree: &mut Tree,
) -> Section
where
    Renderer: renderer::Renderer,
{
    let foot_size = foot.size();

    let limits = limits
        .loose()
        .width(width)
        .height(foot_size.height)
        .shrink(padding);

    foot.layout(&mut tree.children[2], renderer, &limits);
    let content_size = tree.children[2].size;
    let size = limits.resolve(width, foot_size.height, content_size);

    Section {
        size: size.expand(padding),
        content: Vector::new(padding.left, padding.right)
            + content_size.align(size, Alignment::Start, Alignment::Center),
    }
}

/// Draws the head of the card.
#[allow(clippy::too_many_arguments)]
fn draw_head<Message, Theme, Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    head: &Element<'_, Message, Theme, Renderer>,
    (head_layout, head_state): (Layout, &Tree),
    cursor: Cursor,
    viewport: &Rectangle,
    theme: &Theme,
    style: &Style,
    close_button: Option<(&Element<'_, Message, Theme, Renderer>, (Layout, &Tree))>,
) where
    Renderer: renderer::Renderer,
{
    let border_radius = style.border_radius;

    // Head background
    if bounds.intersects(viewport) {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: border_radius.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                    ..Border::default()
                },
                shadow: Shadow::default(),
                snap: false,
            },
            style.head_background,
        );
    }

    // cover rounded button of header
    let button_bounds = Rectangle {
        x: bounds.x,
        y: bounds.y + bounds.height - border_radius,
        width: bounds.width,
        height: border_radius,
    };
    if button_bounds.intersects(viewport) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: button_bounds,
                border: Border {
                    radius: (0.0).into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                    ..Border::default()
                },
                shadow: Shadow::default(),
                snap: false,
            },
            style.head_background,
        );
    }

    head.draw(
        head_state,
        renderer,
        theme,
        &renderer::Style {
            text_color: style.head_text_color,
        },
        head_layout,
        cursor,
        viewport,
    );

    // Draw close button if present
    if let Some((close_btn, (close_layout, close_state))) = close_button {
        close_btn.draw(
            close_state,
            renderer,
            theme,
            &renderer::Style {
                text_color: style.close_color,
            },
            close_layout,
            cursor,
            viewport,
        );
    }
}

/// Draws the body of the card.
#[allow(clippy::too_many_arguments)]
fn draw_body<Message, Theme, Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    body: &Element<'_, Message, Theme, Renderer>,
    (body_layout, body_state): (Layout, &Tree),
    cursor: Cursor,
    viewport: &Rectangle,
    theme: &Theme,
    style: &Style,
) where
    Renderer: renderer::Renderer,
{
    // Body background
    if bounds.intersects(viewport) {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: (0.0).into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                    ..Border::default()
                },
                shadow: Shadow::default(),
                snap: false,
            },
            style.body_background,
        );
    }

    body.draw(
        body_state,
        renderer,
        theme,
        &renderer::Style {
            text_color: style.body_text_color,
        },
        body_layout,
        cursor,
        viewport,
    );
}

/// Draws the foot of the card.
fn draw_foot<Message, Theme, Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    foot: Option<(&Element<'_, Message, Theme, Renderer>, (Layout, &Tree))>,
    cursor: Cursor,
    viewport: &Rectangle,
    theme: &Theme,
    style: &Style,
) where
    Renderer: renderer::Renderer,
{
    // Foot background
    if bounds.intersects(viewport) {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: style.border_radius.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                    ..Border::default()
                },
                shadow: Shadow::default(),
                snap: false,
            },
            style.foot_background,
        );
    }

    if let Some((foot, (foot_layout, foot_state))) = foot {
        foot.draw(
            foot_state,
            renderer,
            theme,
            &renderer::Style {
                text_color: style.foot_text_color,
            },
            foot_layout,
            cursor,
            viewport,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<Card<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: 'a + renderer::Renderer + iced_core::text::Renderer,
    Theme: 'a + Catalog,
    Message: Clone + 'a,
{
    fn from(card: Card<'a, Message, Theme, Renderer>) -> Self {
        Widget::_boxed(card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    enum TestMessage {
        Close,
    }

    type TestCard<'a> = Card<'a, TestMessage, iced_widget::Theme, iced_widget::Renderer>;

    #[test]
    fn card_new_has_default_values() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        );

        assert_eq!(card.width, Length::Fill);
        assert_eq!(card.height, Length::Shrink);
        assert_eq!(card.max_width, u32::MAX as f32);
        assert_eq!(card.max_height, u32::MAX as f32);
        assert_eq!(card.padding_head, DEFAULT_PADDING);
        assert_eq!(card.padding_body, DEFAULT_PADDING);
        assert_eq!(card.padding_foot, DEFAULT_PADDING);
        assert!(card.close_size.is_none());
        assert!(card.on_close.is_none());
        assert!(card.foot.is_none());
        assert!(card.close_button.is_none());
    }

    #[test]
    fn card_width_sets_value() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .width(400);

        assert_eq!(card.width, Length::Fixed(400.0));
    }

    #[test]
    fn card_height_sets_value() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .height(300);

        assert_eq!(card.height, Length::Fixed(300.0));
    }

    #[test]
    fn card_max_width_sets_value() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .max_width(500.0);

        assert_eq!(card.max_width, 500.0);
    }

    #[test]
    fn card_max_height_sets_value() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .max_height(400.0);

        assert_eq!(card.max_height, 400.0);
    }

    #[test]
    fn card_padding_sets_all_sections() {
        let padding = Padding::new(15.0);
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .padding(padding);

        assert_eq!(card.padding_head, padding);
        assert_eq!(card.padding_body, padding);
        assert_eq!(card.padding_foot, padding);
    }

    #[test]
    fn card_padding_head_sets_value() {
        let padding = Padding::new(20.0);
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .padding_head(padding);

        assert_eq!(card.padding_head, padding);
        assert_eq!(card.padding_body, DEFAULT_PADDING);
        assert_eq!(card.padding_foot, DEFAULT_PADDING);
    }

    #[test]
    fn card_padding_body_sets_value() {
        let padding = Padding::new(25.0);
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .padding_body(padding);

        assert_eq!(card.padding_head, DEFAULT_PADDING);
        assert_eq!(card.padding_body, padding);
        assert_eq!(card.padding_foot, DEFAULT_PADDING);
    }

    #[test]
    fn card_padding_foot_sets_value() {
        let padding = Padding::new(10.0);
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .padding_foot(padding);

        assert_eq!(card.padding_head, DEFAULT_PADDING);
        assert_eq!(card.padding_body, DEFAULT_PADDING);
        assert_eq!(card.padding_foot, padding);
    }

    #[test]
    fn card_foot_adds_footer() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .foot(iced_widget::text::Text::new("Footer"));

        assert!(card.foot.is_some());
    }

    #[test]
    fn card_on_close_sets_message() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .on_close(TestMessage::Close);

        assert!(card.on_close.is_some());
    }

    #[test]
    fn card_close_size_sets_value() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .close_size(24.0);

        assert_eq!(card.close_size, Some(24.0));
    }

    #[test]
    fn card_size_method_returns_correct_size() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .width(500)
        .height(400);

        let size = Widget::<TestMessage, iced_widget::Theme, iced_widget::Renderer>::size(&card);
        assert_eq!(size.width, Length::Fixed(500.0));
        assert_eq!(size.height, Length::Fixed(400.0));
    }

    #[test]
    fn card_chaining_methods() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .foot(iced_widget::text::Text::new("Footer"))
        .width(600)
        .height(450)
        .max_width(800.0)
        .max_height(600.0)
        .padding(Padding::new(15.0))
        .on_close(TestMessage::Close)
        .close_size(20.0);

        assert!(card.foot.is_some());
        assert_eq!(card.width, Length::Fixed(600.0));
        assert_eq!(card.height, Length::Fixed(450.0));
        assert_eq!(card.max_width, 800.0);
        assert_eq!(card.max_height, 600.0);
        assert_eq!(card.padding_head, Padding::new(15.0));
        assert_eq!(card.padding_body, Padding::new(15.0));
        assert_eq!(card.padding_foot, Padding::new(15.0));
        assert!(card.on_close.is_some());
        assert_eq!(card.close_size, Some(20.0));
    }

    #[test]
    fn card_default_padding_value() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        );

        assert_eq!(card.padding_head, Padding::new(10.0));
        assert_eq!(card.padding_body, Padding::new(10.0));
        assert_eq!(card.padding_foot, Padding::new(10.0));
    }

    #[test]
    fn card_individual_padding_overrides() {
        let general_padding = Padding::new(10.0);
        let head_padding = Padding::new(20.0);

        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .padding(general_padding)
        .padding_head(head_padding);

        assert_eq!(card.padding_head, head_padding);
        assert_eq!(card.padding_body, general_padding);
        assert_eq!(card.padding_foot, general_padding);
    }

    #[test]
    fn card_without_footer_has_none() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        );

        assert!(card.foot.is_none());
    }

    #[test]
    fn card_without_on_close_has_none() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        );

        assert!(card.on_close.is_none());
    }

    #[test]
    fn card_close_size_without_on_close() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .close_size(24.0);

        assert_eq!(card.close_size, Some(24.0));
        assert!(card.on_close.is_none());
    }

    #[test]
    fn card_default_max_dimensions() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        );

        assert_eq!(card.max_width, u32::MAX as f32);
        assert_eq!(card.max_height, u32::MAX as f32);
    }

    #[test]
    fn card_length_fill() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        assert_eq!(card.width, Length::Fill);
        assert_eq!(card.height, Length::Fill);
    }

    #[test]
    fn card_length_shrink() {
        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .width(Length::Shrink)
        .height(Length::Shrink);

        assert_eq!(card.width, Length::Shrink);
        assert_eq!(card.height, Length::Shrink);
    }

    #[test]
    fn card_asymmetric_padding() {
        let head_padding = Padding {
            top: 5.0,
            right: 10.0,
            bottom: 15.0,
            left: 20.0,
        };
        let body_padding = Padding {
            top: 10.0,
            right: 20.0,
            bottom: 30.0,
            left: 40.0,
        };
        let foot_padding = Padding {
            top: 2.0,
            right: 4.0,
            bottom: 6.0,
            left: 8.0,
        };

        let card = TestCard::new(
            iced_widget::text::Text::new("Head"),
            iced_widget::text::Text::new("Body"),
        )
        .padding_head(head_padding)
        .padding_body(body_padding)
        .padding_foot(foot_padding);

        assert_eq!(card.padding_head, head_padding);
        assert_eq!(card.padding_body, body_padding);
        assert_eq!(card.padding_foot, foot_padding);
    }
}
