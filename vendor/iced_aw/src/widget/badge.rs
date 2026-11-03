//! Use a badge for color highlighting important information.
//!
//! *This API requires the following crate features to be activated: badge*

use iced_core::{
    Alignment, Border, Color, Element, Event, Layout, Length, Padding, Rectangle, Shadow, Shell,
    Size, Vector, Widget,
    layout::Limits,
    mouse::{self, Cursor},
    renderer,
    widget::{Meta, Operation, Tree},
    window,
};

pub use crate::style::{
    badge::{Catalog, Style},
    status::{Status, StyleFn},
};

/// The ratio of the border radius.
const BORDER_RADIUS_RATIO: f32 = 34.0 / 15.0;

/// A badge for color highlighting small information.
///
/// # Example
/// ```ignore
/// # use iced_widget::Text;
/// # use iced_aw::Badge;
/// #
/// #[derive(Debug, Clone)]
/// enum Message {
/// }
///
/// let badge = Badge::<Message>::new(Text::new("Text"));
/// ```
#[allow(missing_debug_implementations)]
pub struct Badge<'a, Message, Theme = iced_widget::Theme, Renderer = iced_widget::Renderer>
where
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
    /// The padding of the [`Badge`].
    padding: u16,
    /// The width of the [`Badge`].
    width: Length,
    /// The height of the [`Badge`].
    height: Length,
    /// The horizontal alignment of the [`Badge`].
    horizontal_alignment: Alignment,
    /// The vertical alignment of the [`Badge`].
    vertical_alignment: Alignment,
    /// The style of the [`Badge`].
    class: Theme::Class<'a>,
    /// The content [`Element`] of the [`Badge`].
    content: Element<'a, Message, Theme, Renderer>,
    /// The [`Status`] of the [`Badge`]
    status: Option<Status>,
}

impl<'a, Message, Theme, Renderer> Badge<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
    /// Creates a new [`Badge`] with the given content.
    ///
    /// It expects:
    ///     * the content [`Element`] to display in the [`Badge`].
    pub fn new<T>(content: T) -> Self
    where
        T: Into<Element<'a, Message, Theme, Renderer>>,
    {
        Badge {
            padding: 7,
            width: Length::Shrink,
            height: Length::Shrink,
            horizontal_alignment: Alignment::Center,
            vertical_alignment: Alignment::Center,
            class: Theme::default(),
            content: content.into(),
            status: None,
        }
    }

    /// Sets the horizontal alignment of the content of the [`Badge`].
    #[must_use]
    pub fn align_x(mut self, alignment: Alignment) -> Self {
        self.horizontal_alignment = alignment;
        self
    }

    /// Sets the vertical alignment of the content of the [`Badge`].
    #[must_use]
    pub fn align_y(mut self, alignment: Alignment) -> Self {
        self.vertical_alignment = alignment;
        self
    }

    /// Sets the height of the [`Badge`].
    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the padding of the [`Badge`].
    #[must_use]
    pub fn padding(mut self, units: u16) -> Self {
        self.padding = units;
        self
    }

    /// Sets the style of the [`Badge`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme, Style>).into();
        self
    }

    /// Sets the class of the input of the [`Badge`].
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    /// Sets the width of the [`Badge`].
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }
}

impl<Message, Theme, Renderer> Meta for Badge<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
    Theme: Catalog,
{
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Badge<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: Catalog,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let padding: Padding = self.padding.into();
        let limits = limits
            .loose()
            .width(self.width)
            .height(self.height)
            .shrink(padding);

        let child = &mut tree.children[0];
        self.content
            .layout(child, renderer, &limits.loose());
        let content_size = child.size;
        let size = limits.resolve(self.width, self.height, content_size);

        child.translation = Vector::new(padding.left, padding.top)
            + content_size.align(size, self.horizontal_alignment, self.vertical_alignment);

        tree.size = size.expand(padding);
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
        let (child_layout, child_tree) = layout
            .iter_mut(&mut tree.children)
            .next()
            .expect("widget: Layout should have a children layout for a badge.");

        self.content.update(
            child_tree,
            event,
            child_layout,
            cursor,
            renderer,
            shell,
            viewport,
        );

        let current_status = if cursor.is_over(layout.bounds()) {
            Status::Hovered
        } else {
            Status::Active
        };

        if let Event::Window(window::Event::RedrawRequested(_now)) = event {
            self.status = Some(current_status);
        } else if self.status.is_some_and(|status| status != current_status) {
            shell.request_redraw();
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
        let (child_layout, child_tree) = layout
            .iter(&state.children)
            .next()
            .expect("widget: Layout should have a children layout for a badge.");

        self.content.mouse_interaction(
            child_tree,
            child_layout,
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
        _style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let (child_layout, child_tree) = layout
            .iter(&tree.children)
            .next()
            .expect("Graphics: Layout should have a children layout for Badge");

        let style_sheet = theme.style(&self.class, self.status.unwrap_or(Status::Active));

        let border_radius = style_sheet
            .border_radius
            .unwrap_or(bounds.height / BORDER_RADIUS_RATIO);

        if bounds.intersects(viewport) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: border_radius.into(),
                        width: style_sheet.border_width,
                        color: style_sheet.border_color.unwrap_or(Color::BLACK),
                        ..Border::default()
                    },
                    shadow: Shadow::default(),
                    ..renderer::Quad::default()
                },
                style_sheet.background,
            );
        }

        self.content.draw(
            child_tree,
            renderer,
            theme,
            &renderer::Style {
                text_color: style_sheet.text_color,
            },
            child_layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            let (child_layout, child_tree) = layout
                .iter_mut(&mut tree.children)
                .next()
                .expect("Badge layout should have a content child");

            self.content
                .operate(child_tree, child_layout, viewport, renderer, operation);
        });
    }
}

impl<'a, Message, Theme, Renderer> From<Badge<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Renderer: 'a + renderer::Renderer,
    Theme: 'a + Catalog,
{
    fn from(badge: Badge<'a, Message, Theme, Renderer>) -> Self {
        badge._boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    enum TestMessage {}

    type TestBadge<'a> = Badge<'a, TestMessage, iced_widget::Theme, iced_widget::Renderer>;

    #[test]
    fn badge_new_has_default_values() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test"));

        assert_eq!(badge.padding, 7);
        assert_eq!(badge.width, Length::Shrink);
        assert_eq!(badge.height, Length::Shrink);
        assert_eq!(badge.horizontal_alignment, Alignment::Center);
        assert_eq!(badge.vertical_alignment, Alignment::Center);
        assert!(badge.status.is_none());
    }

    #[test]
    fn badge_padding_sets_value() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test")).padding(15);
        assert_eq!(badge.padding, 15);
    }

    #[test]
    fn badge_width_sets_value() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test")).width(200);
        assert_eq!(badge.width, Length::Fixed(200.0));
    }

    #[test]
    fn badge_width_fill_sets_value() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test")).width(Length::Fill);
        assert_eq!(badge.width, Length::Fill);
    }

    #[test]
    fn badge_height_sets_value() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test")).height(50);
        assert_eq!(badge.height, Length::Fixed(50.0));
    }

    #[test]
    fn badge_height_shrink_sets_value() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test")).height(Length::Shrink);
        assert_eq!(badge.height, Length::Shrink);
    }

    #[test]
    fn badge_align_x_sets_value() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test")).align_x(Alignment::Start);
        assert_eq!(badge.horizontal_alignment, Alignment::Start);
    }

    #[test]
    fn badge_align_y_sets_value() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test")).align_y(Alignment::End);
        assert_eq!(badge.vertical_alignment, Alignment::End);
    }

    #[test]
    fn badge_size_method_returns_correct_size() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test"))
            .width(100)
            .height(50);

        let size = Widget::<TestMessage, iced_widget::Theme, iced_widget::Renderer>::size(&badge);
        assert_eq!(size.width, Length::Fixed(100.0));
        assert_eq!(size.height, Length::Fixed(50.0));
    }

    #[test]
    fn badge_chaining_methods() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test"))
            .padding(10)
            .width(150)
            .height(40)
            .align_x(Alignment::Start)
            .align_y(Alignment::End);

        assert_eq!(badge.padding, 10);
        assert_eq!(badge.width, Length::Fixed(150.0));
        assert_eq!(badge.height, Length::Fixed(40.0));
        assert_eq!(badge.horizontal_alignment, Alignment::Start);
        assert_eq!(badge.vertical_alignment, Alignment::End);
    }

    #[test]
    fn badge_default_padding_is_7() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test"));
        assert_eq!(badge.padding, 7);
    }

    #[test]
    fn badge_padding_can_be_zero() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test")).padding(0);
        assert_eq!(badge.padding, 0);
    }

    #[test]
    fn badge_padding_max_value() {
        let badge = TestBadge::new(iced_widget::text::Text::new("Test")).padding(u16::MAX);
        assert_eq!(badge.padding, u16::MAX);
    }

    #[test]
    fn badge_alignment_combinations() {
        let alignments = [Alignment::Start, Alignment::Center, Alignment::End];

        for h_align in alignments {
            for v_align in alignments {
                let badge = TestBadge::new(iced_widget::text::Text::new("Test"))
                    .align_x(h_align)
                    .align_y(v_align);

                assert_eq!(badge.horizontal_alignment, h_align);
                assert_eq!(badge.vertical_alignment, v_align);
            }
        }
    }

    #[test]
    fn badge_length_fillportion() {
        let badge =
            TestBadge::new(iced_widget::text::Text::new("Test")).width(Length::FillPortion(3));

        assert_eq!(badge.width, Length::FillPortion(3));
    }

    #[test]
    fn badge_widget_has_operate_method() {
        // Verify that Badge implements the operate method from Widget trait
        // This is a compile-time check to ensure the signature is correct
        fn assert_has_operate<W, M, T, R>(_widget: &W)
        where
            W: Widget<M, T, R>,
            R: renderer::Renderer,
        {
        }

        // This is a compiler only test. not really a running test.
        let badge = TestBadge::new(iced_widget::text::Text::new("Test"));
        assert_has_operate(&badge);
    }

    #[test]
    fn badge_diff_creates_content_tree() {
        let mut badge = TestBadge::new(iced_widget::text::Text::new("Original"));
        let mut tree = Tree::empty();

        // Diffing an empty tree should create exactly one child (the content)
        Widget::<TestMessage, iced_widget::Theme, iced_widget::Renderer>::diff(
            &mut badge, &mut tree,
        );

        assert_eq!(tree.children.len(), 1);
    }
}
