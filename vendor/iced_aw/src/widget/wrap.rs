//! A widget that displays its children in multiple horizontal or vertical runs.
//!
//! *This API requires the following crate features to be activated: `wrap`*
use iced_core::{
    Alignment, Element, Event, Layout, Length, Padding, Pixels, Rectangle, Shell, Size, Vector,
    Widget,
    layout::Limits,
    mouse::{self, Cursor},
    overlay, renderer,
    widget::{Meta, Operation, Tree},
};
use std::marker::PhantomData;

/// A container that distributes its contents horizontally.
#[allow(missing_debug_implementations)]
pub struct Wrap<
    'a,
    Message,
    Direction,
    Theme = iced_widget::Theme,
    Renderer = iced_widget::Renderer,
> {
    /// The elements to distribute.
    pub elements: Vec<Element<'a, Message, Theme, Renderer>>,
    /// The alignment of the [`Wrap`].
    pub alignment: Alignment,
    /// The width of the [`Wrap`].
    pub width: Length,
    /// The height of the [`Wrap`].
    pub height: Length,
    /// The maximum width of the [`Wrap`].
    pub max_width: f32,
    /// The maximum height of the [`Wrap`].
    pub max_height: f32,
    /// The padding of each element of the [`Wrap`].
    pub padding: Padding,
    /// The spacing between each element of the [`Wrap`].
    pub spacing: Pixels,
    /// The spacing between each line of the [`Wrap`].
    pub line_spacing: Pixels,
    /// The minimal length of each line of the [`Wrap`].
    pub line_minimal_length: f32,
    #[allow(clippy::missing_docs_in_private_items)]
    _direction: PhantomData<Direction>,
}

impl<'a, Message, Theme, Renderer> Wrap<'a, Message, direction::Horizontal, Theme, Renderer> {
    /// Creates an empty horizontal [`Wrap`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_elements(Vec::new())
    }

    /// Creates a [`Wrap`] with the given elements.
    ///
    /// It expects:
    ///     * the vector containing the [`Element`]s for this [`Wrap`].
    #[must_use]
    pub fn with_elements(elements: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            elements,
            ..Wrap::default()
        }
    }
}

impl<'a, Message, Theme, Renderer> Wrap<'a, Message, direction::Vertical, Theme, Renderer> {
    /// Creates an empty vertical [`Wrap`].
    #[must_use]
    pub fn new_vertical() -> Self {
        Self::with_elements_vertical(Vec::new())
    }

    /// Creates a [`Wrap`] with the given elements.
    ///
    /// It expects:
    ///     * the vector containing the [`Element`]s for this [`Wrap`].
    #[must_use]
    pub fn with_elements_vertical(elements: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            elements,
            ..Wrap::default()
        }
    }
}

impl<'a, Message, Renderer, Direction, Theme> Wrap<'a, Message, Direction, Theme, Renderer> {
    /// Sets the spacing of the [`Wrap`].
    #[must_use]
    pub fn spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.spacing = spacing.into();
        self
    }

    /// Sets the spacing of the lines of the [`Wrap`].
    #[must_use]
    pub fn line_spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.line_spacing = spacing.into();
        self
    }

    /// Sets the minimal length of the lines of the [`Wrap`].
    #[must_use]
    pub const fn line_minimal_length(mut self, units: f32) -> Self {
        self.line_minimal_length = units;
        self
    }

    /// Sets the padding of the elements in the [`Wrap`].
    #[must_use]
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the width of the [`Wrap`].
    #[must_use]
    pub const fn width_items(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    /// Sets the height of the [`Wrap`].
    #[must_use]
    pub const fn height_items(mut self, height: Length) -> Self {
        self.height = height;
        self
    }

    /// Sets the maximum width of the [`Wrap`].
    #[must_use]
    pub const fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width;
        self
    }

    /// Sets the maximum height of the [`Wrap`].
    #[must_use]
    pub const fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = max_height;
        self
    }

    /// Sets the alignment of the [`Wrap`].
    #[must_use]
    pub const fn align_items(mut self, align: Alignment) -> Self {
        self.alignment = align;
        self
    }

    /// Pushes an [`Element`] to the [`Wrap`].
    #[must_use]
    pub fn push<E>(mut self, element: E) -> Self
    where
        E: Widget<Message, Theme, Renderer> + 'a,
    {
        if !element.is_void() {
            self.elements.push(Widget::_boxed(element));
        }
        self
    }
}

impl<Message, Renderer, Direction, Theme> Meta for Wrap<'_, Message, Direction, Theme, Renderer> {}

impl<Message, Renderer, Direction, Theme> Widget<Message, Theme, Renderer>
    for Wrap<'_, Message, Direction, Theme, Renderer>
where
    Self: WrapLayout<Renderer>,
    Renderer: renderer::Renderer,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.elements);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        self.inner_layout(tree, renderer, limits);
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
        self.elements
            .iter_mut()
            .zip(layout.iter_mut(&mut state.children))
            .for_each(|(child, (layout, state))| {
                child.update(state, event, layout, cursor, renderer, shell, viewport);
            });
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
        overlay::from_children(
            &mut self.elements,
            tree,
            layout,
            renderer,
            viewport,
            translation,
            window,
        )
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.elements
            .iter()
            .zip(layout.iter(&state.children))
            .map(|(child, (layout, state))| {
                child.mouse_interaction(state, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        for (child, (layout, state)) in self.elements.iter().zip(layout.iter(&state.children)) {
            child.draw(state, renderer, theme, style, layout, cursor, viewport);
        }
    }

    fn operate(
        &mut self,
        state: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.traverse(&mut |operation| {
            for (element, (layout, state)) in self
                .elements
                .iter_mut()
                .zip(layout.iter_mut(&mut state.children))
            {
                element.operate(state, layout, viewport, renderer, operation);
            }
        });
    }
}

impl<'a, Message, Theme, Renderer> From<Wrap<'a, Message, direction::Vertical, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: 'a + renderer::Renderer,
    Message: 'a,
    Theme: 'a,
{
    fn from(wrap: Wrap<'a, Message, direction::Vertical, Theme, Renderer>) -> Self {
        Widget::_boxed(wrap)
    }
}

impl<'a, Message, Theme, Renderer> From<Wrap<'a, Message, direction::Horizontal, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: 'a + renderer::Renderer,
    Message: 'a,
    Theme: 'a,
{
    fn from(wrap: Wrap<'a, Message, direction::Horizontal, Theme, Renderer>) -> Self {
        Widget::_boxed(wrap)
    }
}

impl<Message, Renderer, Direction, Theme> Default
    for Wrap<'_, Message, Direction, Theme, Renderer>
{
    fn default() -> Self {
        Self {
            elements: vec![],
            alignment: Alignment::Start,
            width: Length::Shrink,
            height: Length::Shrink,
            max_width: 4_294_967_295.0,
            max_height: 4_294_967_295.0,
            padding: Padding::ZERO,
            spacing: Pixels::ZERO,
            line_spacing: Pixels::ZERO,
            line_minimal_length: 10.0,
            _direction: PhantomData,
        }
    }
}
/// A inner layout of the [`Wrap`].
pub trait WrapLayout<Renderer>
where
    Renderer: renderer::Renderer,
{
    /// A inner layout of the [`Wrap`]. The resulting size and the translation of
    /// every child are stored in the given [`Tree`].
    fn inner_layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits);
}

/// Applies the [`Wrap`] width/height/padding/max constraints to the given limits.
fn wrap_limits<Message, Direction, Theme, Renderer>(
    wrap: &Wrap<'_, Message, Direction, Theme, Renderer>,
    limits: &Limits,
) -> Limits {
    let mut limits = limits
        .shrink(wrap.padding)
        .width(wrap.width)
        .height(wrap.height);

    limits.max.width = limits.max.width.min(wrap.max_width);
    limits.min.width = limits.min.width.min(limits.max.width);
    limits.max.height = limits.max.height.min(wrap.max_height);
    limits.min.height = limits.min.height.min(limits.max.height);

    limits
}

/// The fraction of the free space to skip for the given [`Alignment`].
fn alignment_factor(alignment: Alignment) -> f32 {
    match alignment {
        Alignment::Start => 0.0,
        Alignment::Center => 0.5,
        Alignment::End => 1.0,
    }
}

impl<'a, Message, Theme, Renderer> WrapLayout<Renderer>
    for Wrap<'a, Message, direction::Horizontal, Theme, Renderer>
where
    Renderer: renderer::Renderer + 'a,
{
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn inner_layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let padding = self.padding;
        let spacing = self.spacing;
        let line_spacing = self.line_spacing;
        #[allow(clippy::cast_precision_loss)] // TODO: possible precision loss
        let line_minimal_length = self.line_minimal_length;
        let limits = wrap_limits(self, limits);
        let max_width = limits.bounds().width;

        let mut curse = padding.left;
        let mut deep_curse = padding.left;
        let mut current_line_height = line_minimal_length;
        let mut max_main = curse;
        let mut align = vec![];
        let mut start = 0;
        let mut end = 0;
        for (elem, child) in self.elements.iter_mut().zip(&mut tree.children) {
            let node_limit = Limits::with_flags(
                Size::new(limits.min.width, line_minimal_length),
                limits.max,
                limits.compression,
                limits.infinite,
            );
            elem.layout(child, renderer, &node_limit);

            let size = child.size;

            let offset_init = size.width + spacing.0;
            let offset = curse + offset_init;

            if offset > max_width {
                deep_curse += current_line_height + line_spacing.0;
                align.push((start..end, current_line_height));
                start = end;
                end += 1;
                current_line_height = line_minimal_length;
                child.translation = Vector::new(padding.left, deep_curse);
                curse = offset_init + padding.left;
            } else {
                child.translation = Vector::new(curse, deep_curse);
                curse = offset;
                end += 1;
            }
            current_line_height = current_line_height.max(size.height);
            max_main = max_main.max(curse);
        }
        if end != start {
            align.push((start..end, current_line_height));
        }
        let factor = alignment_factor(self.alignment);
        for (range, max_length) in align {
            for child in &mut tree.children[range] {
                child.translation.y += (max_length - child.size.height) * factor;
            }
        }
        let (width, height) = (
            max_main - padding.left,
            deep_curse - padding.left + current_line_height,
        );
        let size = limits.resolve(self.width, self.height, Size::new(width, height));

        tree.size = size.expand(padding);
    }
}

impl<'a, Message, Theme, Renderer> WrapLayout<Renderer>
    for Wrap<'a, Message, direction::Vertical, Theme, Renderer>
where
    Renderer: renderer::Renderer + 'a,
{
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn inner_layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let padding = self.padding;
        let spacing = self.spacing;
        let line_spacing = self.line_spacing;
        #[allow(clippy::cast_precision_loss)] // TODO: possible precision loss
        let line_minimal_length = self.line_minimal_length;
        let limits = wrap_limits(self, limits);
        let max_height = limits.bounds().height;

        let mut curse = padding.left;
        let mut wide_curse = padding.left;
        let mut current_line_width = line_minimal_length;
        let mut max_main = curse;
        let mut align = vec![];
        let mut start = 0;
        let mut end = 0;
        for (elem, child) in self.elements.iter_mut().zip(&mut tree.children) {
            let node_limit = Limits::with_flags(
                Size::new(line_minimal_length, limits.min.height),
                limits.max,
                limits.compression,
                limits.infinite,
            );
            elem.layout(child, renderer, &node_limit);

            let size = child.size;

            let offset_init = size.height + spacing.0;
            let offset = curse + offset_init;

            if offset > max_height {
                wide_curse += current_line_width + line_spacing.0;
                align.push((start..end, current_line_width));
                start = end;
                end += 1;
                current_line_width = line_minimal_length;
                child.translation = Vector::new(wide_curse, padding.left);
                curse = offset_init + padding.left;
            } else {
                child.translation = Vector::new(wide_curse, curse);
                end += 1;
                curse = offset;
            }
            current_line_width = current_line_width.max(size.width);
            max_main = max_main.max(curse);
        }
        if end != start {
            align.push((start..end, current_line_width));
        }

        let factor = alignment_factor(self.alignment);
        for (range, max_length) in align {
            for child in &mut tree.children[range] {
                child.translation.x += (max_length - child.size.width) * factor;
            }
        }

        let (width, height) = (
            wide_curse - padding.left + current_line_width,
            max_main - padding.left,
        );
        let size = limits.resolve(self.width, self.height, Size::new(width, height));

        tree.size = size.expand(padding);
    }
}

/// An optional directional attribute of the [`Wrap`].
pub mod direction {
    /// An vertical direction of the [`Wrap`](super::Wrap).
    #[derive(Debug)]
    pub struct Vertical;
    /// An horizontal direction of the [`Wrap`](super::Wrap).
    #[derive(Debug)]
    pub struct Horizontal;
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[derive(Clone)]
    enum TestMessage {}

    type TestWrapHorizontal<'a> =
        Wrap<'a, TestMessage, direction::Horizontal, iced_widget::Theme, iced_widget::Renderer>;
    type TestWrapVertical<'a> =
        Wrap<'a, TestMessage, direction::Vertical, iced_widget::Theme, iced_widget::Renderer>;

    // ============================================================================
    // Constructor Tests - Horizontal
    // ============================================================================

    #[test]
    fn wrap_horizontal_new_creates_empty_wrap() {
        let wrap = TestWrapHorizontal::new();
        assert_eq!(wrap.elements.len(), 0);
    }

    #[test]
    fn wrap_horizontal_new_has_default_values() {
        let wrap = TestWrapHorizontal::new();

        assert_eq!(wrap.elements.len(), 0);
        assert_eq!(wrap.alignment, Alignment::Start);
        assert_eq!(wrap.width, Length::Shrink);
        assert_eq!(wrap.height, Length::Shrink);
        assert_eq!(wrap.max_width, 4_294_967_295.0);
        assert_eq!(wrap.max_height, 4_294_967_295.0);
        assert_eq!(wrap.padding, Padding::ZERO);
        assert_eq!(wrap.spacing, Pixels::ZERO);
        assert_eq!(wrap.line_spacing, Pixels::ZERO);
        assert_eq!(wrap.line_minimal_length, 10.0);
    }

    #[test]
    fn wrap_horizontal_with_elements_creates_wrap_with_elements() {
        let elements: Vec<Element<TestMessage, iced_widget::Theme, iced_widget::Renderer>> =
            vec![Widget::_boxed(iced_widget::text::Text::new("Test"))];
        let wrap = TestWrapHorizontal::with_elements(elements);
        assert_eq!(wrap.elements.len(), 1);
    }

    #[test]
    fn wrap_horizontal_with_elements_empty_vec() {
        let elements: Vec<Element<TestMessage, iced_widget::Theme, iced_widget::Renderer>> = vec![];
        let wrap = TestWrapHorizontal::with_elements(elements);
        assert_eq!(wrap.elements.len(), 0);
    }

    // ============================================================================
    // Constructor Tests - Vertical
    // ============================================================================

    #[test]
    fn wrap_vertical_new_creates_empty_wrap() {
        let wrap = TestWrapVertical::new_vertical();
        assert_eq!(wrap.elements.len(), 0);
    }

    #[test]
    fn wrap_vertical_new_has_default_values() {
        let wrap = TestWrapVertical::new_vertical();

        assert_eq!(wrap.elements.len(), 0);
        assert_eq!(wrap.alignment, Alignment::Start);
        assert_eq!(wrap.width, Length::Shrink);
        assert_eq!(wrap.height, Length::Shrink);
        assert_eq!(wrap.max_width, 4_294_967_295.0);
        assert_eq!(wrap.max_height, 4_294_967_295.0);
        assert_eq!(wrap.padding, Padding::ZERO);
        assert_eq!(wrap.spacing, Pixels::ZERO);
        assert_eq!(wrap.line_spacing, Pixels::ZERO);
        assert_eq!(wrap.line_minimal_length, 10.0);
    }

    #[test]
    fn wrap_vertical_with_elements_creates_wrap_with_elements() {
        let elements: Vec<Element<TestMessage, iced_widget::Theme, iced_widget::Renderer>> =
            vec![Widget::_boxed(iced_widget::text::Text::new("Test"))];
        let wrap = TestWrapVertical::with_elements_vertical(elements);
        assert_eq!(wrap.elements.len(), 1);
    }

    #[test]
    fn wrap_vertical_with_elements_empty_vec() {
        let elements: Vec<Element<TestMessage, iced_widget::Theme, iced_widget::Renderer>> = vec![];
        let wrap = TestWrapVertical::with_elements_vertical(elements);
        assert_eq!(wrap.elements.len(), 0);
    }

    // ============================================================================
    // Push Method Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_push_adds_element() {
        let wrap = TestWrapHorizontal::new().push(iced_widget::text::Text::new("Test"));
        assert_eq!(wrap.elements.len(), 1);
    }

    #[test]
    fn wrap_horizontal_push_multiple_elements() {
        let wrap = TestWrapHorizontal::new()
            .push(iced_widget::text::Text::new("First"))
            .push(iced_widget::text::Text::new("Second"))
            .push(iced_widget::text::Text::new("Third"));
        assert_eq!(wrap.elements.len(), 3);
    }

    #[test]
    fn wrap_vertical_push_adds_element() {
        let wrap = TestWrapVertical::new_vertical().push(iced_widget::text::Text::new("Test"));
        assert_eq!(wrap.elements.len(), 1);
    }

    #[test]
    fn wrap_vertical_push_multiple_elements() {
        let wrap = TestWrapVertical::new_vertical()
            .push(iced_widget::text::Text::new("First"))
            .push(iced_widget::text::Text::new("Second"))
            .push(iced_widget::text::Text::new("Third"));
        assert_eq!(wrap.elements.len(), 3);
    }

    // ============================================================================
    // Spacing Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_spacing_sets_value() {
        let wrap = TestWrapHorizontal::new().spacing(10.0);
        assert_eq!(wrap.spacing, Pixels(10.0));
    }

    #[test]
    fn wrap_horizontal_spacing_zero_sets_value() {
        let wrap = TestWrapHorizontal::new().spacing(0.0);
        assert_eq!(wrap.spacing, Pixels::ZERO);
    }

    #[test]
    fn wrap_vertical_spacing_sets_value() {
        let wrap = TestWrapVertical::new_vertical().spacing(15.0);
        assert_eq!(wrap.spacing, Pixels(15.0));
    }

    // ============================================================================
    // Line Spacing Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_line_spacing_sets_value() {
        let wrap = TestWrapHorizontal::new().line_spacing(20.0);
        assert_eq!(wrap.line_spacing, Pixels(20.0));
    }

    #[test]
    fn wrap_horizontal_line_spacing_zero_sets_value() {
        let wrap = TestWrapHorizontal::new().line_spacing(0.0);
        assert_eq!(wrap.line_spacing, Pixels::ZERO);
    }

    #[test]
    fn wrap_vertical_line_spacing_sets_value() {
        let wrap = TestWrapVertical::new_vertical().line_spacing(25.0);
        assert_eq!(wrap.line_spacing, Pixels(25.0));
    }

    // ============================================================================
    // Line Minimal Length Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_line_minimal_length_sets_value() {
        let wrap = TestWrapHorizontal::new().line_minimal_length(50.0);
        assert_eq!(wrap.line_minimal_length, 50.0);
    }

    #[test]
    fn wrap_horizontal_line_minimal_length_default_is_10() {
        let wrap = TestWrapHorizontal::new();
        assert_eq!(wrap.line_minimal_length, 10.0);
    }

    #[test]
    fn wrap_vertical_line_minimal_length_sets_value() {
        let wrap = TestWrapVertical::new_vertical().line_minimal_length(100.0);
        assert_eq!(wrap.line_minimal_length, 100.0);
    }

    // ============================================================================
    // Padding Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_padding_sets_value() {
        let wrap = TestWrapHorizontal::new().padding(10);
        assert_eq!(wrap.padding, Padding::new(10.0));
    }

    #[test]
    fn wrap_horizontal_padding_default_is_zero() {
        let wrap = TestWrapHorizontal::new();
        assert_eq!(wrap.padding, Padding::ZERO);
    }

    #[test]
    fn wrap_horizontal_padding_with_padding_struct() {
        let padding = Padding {
            top: 5.0,
            right: 10.0,
            bottom: 15.0,
            left: 20.0,
        };
        let wrap = TestWrapHorizontal::new().padding(padding);
        assert_eq!(wrap.padding, padding);
    }

    #[test]
    fn wrap_vertical_padding_sets_value() {
        let wrap = TestWrapVertical::new_vertical().padding(12);
        assert_eq!(wrap.padding, Padding::new(12.0));
    }

    // ============================================================================
    // Width Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_width_items_sets_value() {
        let wrap = TestWrapHorizontal::new().width_items(Length::Fill);
        assert_eq!(wrap.width, Length::Fill);
    }

    #[test]
    fn wrap_horizontal_width_items_default_is_shrink() {
        let wrap = TestWrapHorizontal::new();
        assert_eq!(wrap.width, Length::Shrink);
    }

    #[test]
    fn wrap_horizontal_width_items_fixed() {
        let wrap = TestWrapHorizontal::new().width_items(Length::Fixed(200.0));
        assert_eq!(wrap.width, Length::Fixed(200.0));
    }

    #[test]
    fn wrap_vertical_width_items_sets_value() {
        let wrap = TestWrapVertical::new_vertical().width_items(Length::Fill);
        assert_eq!(wrap.width, Length::Fill);
    }

    // ============================================================================
    // Height Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_height_items_sets_value() {
        let wrap = TestWrapHorizontal::new().height_items(Length::Fill);
        assert_eq!(wrap.height, Length::Fill);
    }

    #[test]
    fn wrap_horizontal_height_items_default_is_shrink() {
        let wrap = TestWrapHorizontal::new();
        assert_eq!(wrap.height, Length::Shrink);
    }

    #[test]
    fn wrap_horizontal_height_items_fixed() {
        let wrap = TestWrapHorizontal::new().height_items(Length::Fixed(150.0));
        assert_eq!(wrap.height, Length::Fixed(150.0));
    }

    #[test]
    fn wrap_vertical_height_items_sets_value() {
        let wrap = TestWrapVertical::new_vertical().height_items(Length::Fill);
        assert_eq!(wrap.height, Length::Fill);
    }

    // ============================================================================
    // Max Width Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_max_width_sets_value() {
        let wrap = TestWrapHorizontal::new().max_width(500.0);
        assert_eq!(wrap.max_width, 500.0);
    }

    #[test]
    fn wrap_horizontal_max_width_default() {
        let wrap = TestWrapHorizontal::new();
        assert_eq!(wrap.max_width, 4_294_967_295.0);
    }

    #[test]
    fn wrap_vertical_max_width_sets_value() {
        let wrap = TestWrapVertical::new_vertical().max_width(600.0);
        assert_eq!(wrap.max_width, 600.0);
    }

    // ============================================================================
    // Max Height Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_max_height_sets_value() {
        let wrap = TestWrapHorizontal::new().max_height(400.0);
        assert_eq!(wrap.max_height, 400.0);
    }

    #[test]
    fn wrap_horizontal_max_height_default() {
        let wrap = TestWrapHorizontal::new();
        assert_eq!(wrap.max_height, 4_294_967_295.0);
    }

    #[test]
    fn wrap_vertical_max_height_sets_value() {
        let wrap = TestWrapVertical::new_vertical().max_height(300.0);
        assert_eq!(wrap.max_height, 300.0);
    }

    // ============================================================================
    // Alignment Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_align_items_start() {
        let wrap = TestWrapHorizontal::new().align_items(Alignment::Start);
        assert_eq!(wrap.alignment, Alignment::Start);
    }

    #[test]
    fn wrap_horizontal_align_items_center() {
        let wrap = TestWrapHorizontal::new().align_items(Alignment::Center);
        assert_eq!(wrap.alignment, Alignment::Center);
    }

    #[test]
    fn wrap_horizontal_align_items_end() {
        let wrap = TestWrapHorizontal::new().align_items(Alignment::End);
        assert_eq!(wrap.alignment, Alignment::End);
    }

    #[test]
    fn wrap_horizontal_align_items_default_is_start() {
        let wrap = TestWrapHorizontal::new();
        assert_eq!(wrap.alignment, Alignment::Start);
    }

    #[test]
    fn wrap_vertical_align_items_center() {
        let wrap = TestWrapVertical::new_vertical().align_items(Alignment::Center);
        assert_eq!(wrap.alignment, Alignment::Center);
    }

    // ============================================================================
    // Method Chaining Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_chaining_all_methods() {
        let wrap = TestWrapHorizontal::new()
            .push(iced_widget::text::Text::new("Test"))
            .spacing(5.0)
            .line_spacing(10.0)
            .line_minimal_length(20.0)
            .padding(15)
            .width_items(Length::Fill)
            .height_items(Length::Shrink)
            .max_width(800.0)
            .max_height(600.0)
            .align_items(Alignment::Center);

        assert_eq!(wrap.elements.len(), 1);
        assert_eq!(wrap.spacing, Pixels(5.0));
        assert_eq!(wrap.line_spacing, Pixels(10.0));
        assert_eq!(wrap.line_minimal_length, 20.0);
        assert_eq!(wrap.padding, Padding::new(15.0));
        assert_eq!(wrap.width, Length::Fill);
        assert_eq!(wrap.height, Length::Shrink);
        assert_eq!(wrap.max_width, 800.0);
        assert_eq!(wrap.max_height, 600.0);
        assert_eq!(wrap.alignment, Alignment::Center);
    }

    #[test]
    fn wrap_vertical_chaining_all_methods() {
        let wrap = TestWrapVertical::new_vertical()
            .push(iced_widget::text::Text::new("Test"))
            .spacing(8.0)
            .line_spacing(12.0)
            .line_minimal_length(25.0)
            .padding(20)
            .width_items(Length::Shrink)
            .height_items(Length::Fill)
            .max_width(700.0)
            .max_height(500.0)
            .align_items(Alignment::End);

        assert_eq!(wrap.elements.len(), 1);
        assert_eq!(wrap.spacing, Pixels(8.0));
        assert_eq!(wrap.line_spacing, Pixels(12.0));
        assert_eq!(wrap.line_minimal_length, 25.0);
        assert_eq!(wrap.padding, Padding::new(20.0));
        assert_eq!(wrap.width, Length::Shrink);
        assert_eq!(wrap.height, Length::Fill);
        assert_eq!(wrap.max_width, 700.0);
        assert_eq!(wrap.max_height, 500.0);
        assert_eq!(wrap.alignment, Alignment::End);
    }

    // ============================================================================
    // Widget Size Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_size_method_returns_correct_size() {
        let wrap = TestWrapHorizontal::new()
            .width_items(Length::Fixed(100.0))
            .height_items(Length::Fixed(50.0));

        let size = Widget::<TestMessage, iced_widget::Theme, iced_widget::Renderer>::size(&wrap);
        assert_eq!(size.width, Length::Fixed(100.0));
        assert_eq!(size.height, Length::Fixed(50.0));
    }

    #[test]
    fn wrap_vertical_size_method_returns_correct_size() {
        let wrap = TestWrapVertical::new_vertical()
            .width_items(Length::Fill)
            .height_items(Length::Shrink);

        let size = Widget::<TestMessage, iced_widget::Theme, iced_widget::Renderer>::size(&wrap);
        assert_eq!(size.width, Length::Fill);
        assert_eq!(size.height, Length::Shrink);
    }

    // ============================================================================
    // Default Trait Tests
    // ============================================================================

    #[test]
    fn wrap_horizontal_default_creates_empty_wrap_with_defaults() {
        let wrap: TestWrapHorizontal = Wrap::default();

        assert_eq!(wrap.elements.len(), 0);
        assert_eq!(wrap.alignment, Alignment::Start);
        assert_eq!(wrap.width, Length::Shrink);
        assert_eq!(wrap.height, Length::Shrink);
        assert_eq!(wrap.max_width, 4_294_967_295.0);
        assert_eq!(wrap.max_height, 4_294_967_295.0);
        assert_eq!(wrap.padding, Padding::ZERO);
        assert_eq!(wrap.spacing, Pixels::ZERO);
        assert_eq!(wrap.line_spacing, Pixels::ZERO);
        assert_eq!(wrap.line_minimal_length, 10.0);
    }

    #[test]
    fn wrap_vertical_default_creates_empty_wrap_with_defaults() {
        let wrap: TestWrapVertical = Wrap::default();

        assert_eq!(wrap.elements.len(), 0);
        assert_eq!(wrap.alignment, Alignment::Start);
        assert_eq!(wrap.width, Length::Shrink);
        assert_eq!(wrap.height, Length::Shrink);
        assert_eq!(wrap.max_width, 4_294_967_295.0);
        assert_eq!(wrap.max_height, 4_294_967_295.0);
        assert_eq!(wrap.padding, Padding::ZERO);
        assert_eq!(wrap.spacing, Pixels::ZERO);
        assert_eq!(wrap.line_spacing, Pixels::ZERO);
        assert_eq!(wrap.line_minimal_length, 10.0);
    }
}
