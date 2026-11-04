//! Distribute content rows vertically while setting the row width to widest row.
//!
//! For alignment [`Alignment::Start`] the last element of the row is flushed to the end.
//! For alignment [`Alignment::End`] the first element of the row is flushed to the start.
//!
//! Future: Idea to implement leaders before/after the flushed element for `Start`/`End`
//! alignments.

use iced_core::{
    Alignment, Element, Layout, Length, Padding, Pixels, Rectangle, Shell, Size, Vector, Widget,
    alignment,
    event::Event,
    layout, mouse, overlay, renderer,
    widget::{
        Meta, Operation,
        tree::{self, Tree},
    },
};
use iced_widget::Row;

/// A container that distributes its contents vertically.
#[allow(missing_debug_implementations)]
pub struct FlushColumn<'a, Message, Theme = iced_widget::Theme, Renderer = iced_widget::Renderer> {
    spacing: Pixels,
    padding: Padding,
    width: Length,
    height: Length,
    max_width: f32,
    align: Alignment,
    clip: bool,
    children: Vec<Row<Element<'a, Message, Theme, Renderer>>>,
    flush: bool,
}

impl<'a, Message: 'a, Theme: 'a, Renderer> FlushColumn<'a, Message, Theme, Renderer>
where
    Renderer: iced_core::Renderer + 'a,
{
    /// Creates an empty [`FlushColumn`].
    #[must_use]
    pub fn new() -> Self {
        Self::from_vec(Vec::new())
    }

    /// Creates a [`FlushColumn`] with the given capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vec(Vec::with_capacity(capacity))
    }

    /// Creates a [`FlushColumn`] with the given elements.
    #[must_use]
    pub fn with_children(
        children: impl IntoIterator<Item = Row<Element<'a, Message, Theme, Renderer>>>,
    ) -> Self {
        let iterator = children.into_iter();
        Self::with_capacity(iterator.size_hint().0).extend(iterator)
    }

    /// Creates a [`FlushColumn`] from an already allocated [`Vec`].
    ///
    /// Keep in mind that the [`FlushColumn`] will not inspect the [`Vec`], which means
    /// it won't automatically adapt to the sizing strategy of its contents.
    ///
    /// If any of the children have a [`Length::Fill`] strategy, you will need to
    /// call [`FlushColumn::width`] or [`FlushColumn::height`] accordingly.
    #[must_use]
    pub fn from_vec(children: Vec<Row<Element<'a, Message, Theme, Renderer>>>) -> Self {
        Self {
            spacing: Pixels::ZERO,
            padding: Padding::ZERO,
            width: Length::Fit,
            height: Length::Fit,
            max_width: f32::INFINITY,
            align: Alignment::Start,
            clip: false,
            children,
            flush: true,
        }
    }

    /// Sets the vertical spacing _between_ elements.
    ///
    /// Custom margins per element do not exist in iced. You should use this
    /// method instead! While less flexible, it helps you keep spacing between
    /// elements consistent.
    #[must_use]
    pub fn spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.spacing = amount.into();
        self
    }

    /// Sets the [`Padding`] of the [`FlushColumn`].
    #[must_use]
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the width of the [`FlushColumn`].
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`FlushColumn`].
    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the maximum width of the [`FlushColumn`].
    #[must_use]
    pub fn max_width(mut self, max_width: impl Into<Pixels>) -> Self {
        self.max_width = max_width.into().0;
        self
    }

    /// Sets the horizontal alignment of the contents of the [`FlushColumn`] .
    #[must_use]
    pub fn align_x(mut self, align: impl Into<alignment::Vertical>) -> Self {
        self.align = Alignment::from(align.into());
        self
    }

    /// Sets whether the contents of the [`FlushColumn`] should be clipped on
    /// overflow.
    #[must_use]
    pub fn clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }

    /// Sets whether the end row element is flushed to the end when the alignment is set to Start,
    /// or the start row element is flushed to the start when the alignment is set to End.
    /// No effect for alignment set to Center.
    #[must_use]
    pub fn flush(mut self, flush: bool) -> Self {
        self.flush = flush;
        self
    }

    /// Adds an element to the [`FlushColumn`].
    #[must_use]
    pub fn push(mut self, child: impl Into<Row<Element<'a, Message, Theme, Renderer>>>) -> Self {
        // Sizing is enclosed from the children during `diff`.
        self.children.push(child.into());
        self
    }

    /// Adds an element to the [`FlushColumn`], if `Some`.
    #[must_use]
    pub fn push_maybe(
        self,
        child: Option<impl Into<Row<Element<'a, Message, Theme, Renderer>>>>,
    ) -> Self {
        if let Some(child) = child {
            self.push(child)
        } else {
            self
        }
    }

    /// Extends the [`FlushColumn`] with the given children.
    #[must_use]
    pub fn extend(
        self,
        children: impl IntoIterator<Item = Row<Element<'a, Message, Theme, Renderer>>>,
    ) -> Self {
        children.into_iter().fold(self, Self::push)
    }
}

#[allow(clippy::mismatching_type_param_order)]
impl<'a, Message: 'a, Renderer> Default for FlushColumn<'a, Message, Renderer>
where
    Renderer: iced_core::Renderer + 'a,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message: 'a, Theme: 'a, Renderer: iced_core::Renderer + 'a>
    FromIterator<Row<Element<'a, Message, Theme, Renderer>>>
    for FlushColumn<'a, Message, Theme, Renderer>
{
    fn from_iter<T: IntoIterator<Item = Row<Element<'a, Message, Theme, Renderer>>>>(
        iter: T,
    ) -> Self {
        Self::with_children(iter)
    }
}

impl<Message, Theme, Renderer> Meta for FlushColumn<'_, Message, Theme, Renderer> {}

impl<'a, Message: 'a, Theme: 'a, Renderer> Widget<Message, Theme, Renderer>
    for FlushColumn<'a, Message, Theme, Renderer>
where
    Renderer: iced_core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<layout::flex::Cache>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(layout::flex::Cache::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.children);

        if self.width.is_fit() || self.height.is_fit() {
            for child in &self.children {
                let size = child.size();

                self.width = self.width.cross(size.width);
                self.height = self.height.stack(size.height);
            }
        }
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        let mut limits = *limits;
        if self.max_width.is_finite() {
            limits.max.width = limits.max.width.min(self.max_width).max(limits.min.width);
            limits.infinite.width = false;
        }

        let cache = tree.state.downcast_mut::<layout::flex::Cache>();

        let size = layout::flex::resolve(
            layout::flex::Axis::Vertical,
            renderer,
            &limits,
            self.width,
            self.height,
            self.padding,
            self.spacing.0,
            self.align,
            &mut tree.children,
            &mut self.children,
            cache,
        );

        let mut container_x = f32::MAX;
        let mut container_width = 0.0f32;
        for row in &tree.children {
            if row.size.width > container_width {
                container_width = row.size.width;
            }
            if row.translation.x < container_x {
                container_x = row.translation.x;
            }
        }
        for row in &mut tree.children {
            let width_diff = container_width - row.size.width;
            if !row.children.is_empty() {
                for element in &mut row.children {
                    element.translation.x += match self.align {
                        Alignment::Start => 0.0,
                        Alignment::Center => width_diff / 2.0,
                        Alignment::End => width_diff,
                    };
                }
                if row.children.len() > 1 {
                    match self.align {
                        Alignment::Start => {
                            let element = row.children.last_mut().expect("Always exists.");
                            element.translation.x += width_diff;
                        }
                        Alignment::Center => {}
                        Alignment::End => {
                            let element = row.children.first_mut().expect("Always exists.");
                            element.translation.x -= width_diff;
                        }
                    }
                }
            }
            row.size.width = container_width;
            row.translation.x = container_x;
        }
        tree.size = size;
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
            self.children
                .iter_mut()
                .zip(layout.iter_mut(&mut tree.children))
                .for_each(|(child, (layout, state))| {
                    child.operate(state, layout, viewport, renderer, operation);
                });
        });
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
        for (child, (layout, state)) in self
            .children
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
        {
            child.update(state, event, layout, cursor, renderer, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.children
            .iter()
            .zip(layout.iter(&tree.children))
            .map(|(child, (layout, state))| {
                child.mouse_interaction(state, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
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
        if let Some(clipped_viewport) = layout.bounds().intersection(viewport) {
            for (child, (layout, state)) in self.children.iter().zip(layout.iter(&tree.children)) {
                child.draw(
                    state,
                    renderer,
                    theme,
                    style,
                    layout,
                    cursor,
                    if self.clip {
                        &clipped_viewport
                    } else {
                        viewport
                    },
                );
            }
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
        overlay::from_children(
            &mut self.children,
            tree,
            layout,
            renderer,
            viewport,
            translation,
            window,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<FlushColumn<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced_core::Renderer + 'a,
{
    fn from(column: FlushColumn<'a, Message, Theme, Renderer>) -> Self {
        Widget::_boxed(column)
    }
}
