//! Displays a [`Tabs`] widget to select the content to be displayed.
//!
//! This is a wrapper around the [`TabBar`] widget.
//! Unlike the [`TabBar`] widget it will also handle
//! the content of the tabs.
//!
//! *This API requires the following crate features to be activated: tabs*

pub mod tab_bar_position;
pub use crate::tab_bar::Position;
use crate::{
    TabLabel,
    style::{
        Status, StyleFn,
        tab_bar::{Catalog, Style},
    },
    widget::tab_bar::TabBar,
};

use iced_core::{
    Element, Event, Font, Layout, Length, Padding, Pixels, Rectangle, Shell, Size, Vector, Widget,
    layout,
    mouse::{self, Cursor},
    overlay, renderer,
    widget::{Meta, Operation, Tree},
};
use iced_widget::text;

pub use tab_bar_position::TabBarPosition;

/// A [`Tabs`] widget for showing a [`TabBar`]
/// along with the tab's content.
///
/// # Example
/// ```ignore
/// # use iced_aw::{TabLabel, tabs::Tabs};
/// # use iced_widget::Text;
/// #
/// #[derive(Debug, Clone)]
/// enum Message {
///     TabSelected(TabId),
/// }
///
/// #[derive(Debug, Clone)]
/// enum TabId {
///    One,
///    Two,
///    Three,
/// }
///
/// let tabs = Tabs::new(Message::TabSelected)
/// .push(TabId::One, TabLabel::Text(String::from("One")), Text::new(String::from("One")))
/// .push(TabId::Two, TabLabel::Text(String::from("Two")), Text::new(String::from("Two")))
/// .push(TabId::Three, TabLabel::Text(String::from("Three")), Text::new(String::from("Three")))
/// .set_active_tab(&TabId::Two);Theme
/// ```
///
#[allow(missing_debug_implementations)]
pub struct Tabs<'a, Message, TabId, Theme = iced_widget::Theme, Renderer = iced_widget::Renderer>
where
    Renderer: 'a + renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog,
    TabId: Eq + Clone,
{
    /// The [`TabBar`](crate::widget::TabBar) of the [`Tabs`].
    tab_bar: TabBar<'a, Message, TabId, Theme, Renderer>,
    /// The vector containing the content of the tabs.
    children: Vec<Element<'a, Message, Theme, Renderer>>,
    /// The vector containing the indices of the tabs.
    indices: Vec<TabId>,
    /// The position of the [`TabBar`](crate::widget::TabBar).
    tab_bar_position: TabBarPosition,
    /// The position of the [`TabBar`](crate::widget::TabBar) Icon.
    tab_icon_position: Position,
    /// the width of the [`Tabs`].
    width: Length,
    /// The height of the [`Tabs`].
    height: Length,
}

impl<'a, Message, TabId, Theme, Renderer> Tabs<'a, Message, TabId, Theme, Renderer>
where
    Renderer: 'a + renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog + text::Catalog,
    TabId: Eq + Clone,
{
    /// Creates a new [`Tabs`] widget with the index of the selected tab and a
    /// specified message which will be send when a tab is selected by the user.
    ///
    /// It expects:
    ///     * the index of the currently active tab.
    ///     * the function that will be called if a tab is selected by the user.
    ///         It takes the index of the selected tab.
    pub fn new<F>(on_select: F) -> Self
    where
        F: 'static + Fn(TabId) -> Message,
    {
        Self::new_with_tabs(Vec::new(), on_select)
    }

    /// Similar to `new` but with a given Vector of the
    /// [`TabLabel`] along with the tab's content.
    ///
    /// It expects:
    ///     * the index of the currently active tab.
    ///     * a vector containing the [`TabLabel`]s along with the content
    ///         [`Element`]s of the [`Tabs`].
    ///     * the function that will be called if a tab is selected by the user.
    ///         It takes the index of the selected tab.
    pub fn new_with_tabs<F>(
        tabs: impl IntoIterator<Item = (TabId, TabLabel, Element<'a, Message, Theme, Renderer>)>,
        on_select: F,
    ) -> Self
    where
        F: 'static + Fn(TabId) -> Message,
    {
        let tabs = tabs.into_iter();
        let n_tabs = tabs.size_hint().0;

        let mut tab_labels = Vec::with_capacity(n_tabs);
        let mut elements = Vec::with_capacity(n_tabs);
        let mut indices = Vec::with_capacity(n_tabs);

        for (id, tab_label, element) in tabs {
            tab_labels.push((id.clone(), tab_label));
            indices.push(id);
            elements.push(element);
        }

        Tabs {
            tab_bar: TabBar::with_tab_labels(tab_labels, on_select),
            children: elements,
            indices,
            tab_bar_position: TabBarPosition::Top,
            tab_icon_position: Position::Left,
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    /// Sets the size of the close icon of the
    /// [`TabLabel`] of the
    /// [`TabBar`].
    #[must_use]
    pub fn close_size(mut self, close_size: f32) -> Self {
        self.tab_bar = self.tab_bar.close_size(close_size);
        self
    }

    /// Sets the Tabs Icon render Position
    /// [`TabLabel`] of the
    /// [`TabBar`].
    #[must_use]
    pub fn tab_icon_position(mut self, position: Position) -> Self {
        self.tab_icon_position = position;
        self
    }

    /// Sets the height of the [`Tabs`].
    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the font of the icons of the
    /// [`TabLabel`]s of the
    /// [`TabBar`].
    #[must_use]
    pub fn icon_font(mut self, font: Font) -> Self {
        self.tab_bar = self.tab_bar.icon_font(font);
        self
    }

    /// Sets the icon size of the [`TabLabel`] of the
    /// [`TabBar`].
    #[must_use]
    pub fn icon_size(mut self, icon_size: f32) -> Self {
        self.tab_bar = self.tab_bar.icon_size(icon_size);
        self
    }

    /// Sets the message that will be produced when the close icon of a tab
    /// on the [`TabBar`] is pressed.
    ///
    /// Setting this enables the drawing of a close icon on the tabs.
    #[must_use]
    pub fn on_close<F>(mut self, on_close: F) -> Self
    where
        F: 'static + Fn(TabId) -> Message,
    {
        self.tab_bar = self.tab_bar.on_close(on_close);
        self
    }

    /// Pushes a [`TabLabel`] along with the tabs
    /// content to the [`Tabs`].
    #[must_use]
    pub fn push<E>(mut self, id: TabId, tab_label: TabLabel, element: E) -> Self
    where
        E: Into<Element<'a, Message, Theme, Renderer>>,
    {
        self.tab_bar = self
            .tab_bar
            .push(id.clone(), tab_label)
            .set_position(self.tab_icon_position);
        self.children.push(element.into());
        self.indices.push(id);
        self
    }

    /// Sets the active tab of the [`Tabs`] using the ``TabId``.
    #[must_use]
    pub fn set_active_tab(mut self, id: &TabId) -> Self {
        self.tab_bar = self.tab_bar.set_active_tab(id);
        self
    }

    /// Sets the height of the [`TabBar`] of the [`Tabs`].
    #[must_use]
    pub fn tab_bar_height(mut self, height: Length) -> Self {
        self.tab_bar = self.tab_bar.height(height);
        self
    }

    /// Sets the maximum height of the [`TabBar`] of the [`Tabs`].
    #[must_use]
    pub fn tab_bar_max_height(mut self, max_height: f32) -> Self {
        self.tab_bar = self.tab_bar.max_height(max_height);
        self
    }

    /// Sets the width of the [`TabBar`] of the [`Tabs`].
    #[must_use]
    pub fn tab_bar_width(mut self, width: Length) -> Self {
        self.tab_bar = self.tab_bar.width(width);
        self
    }

    /// Sets the [`TabBarPosition`] of the [`TabBar`].
    #[must_use]
    pub fn tab_bar_position(mut self, position: TabBarPosition) -> Self {
        self.tab_bar_position = position;
        self
    }

    /// Sets the style of the [`TabBar`].
    #[must_use]
    pub fn tab_bar_style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.tab_bar = self.tab_bar.style(style);
        self
    }

    /// Sets the padding of the tabs of the [`TabBar`].
    #[must_use]
    pub fn tab_label_padding(mut self, padding: impl Into<Padding>) -> Self {
        self.tab_bar = self.tab_bar.padding(padding);
        self
    }

    /// Sets the spacing between the tabs of the
    /// [`TabBar`].
    #[must_use]
    pub fn tab_label_spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.tab_bar = self.tab_bar.spacing(spacing);
        self
    }

    /// Sets the font of the text of the
    /// [`TabLabel`]s of the
    /// [`TabBar`].
    #[must_use]
    pub fn text_font(mut self, text_font: Font) -> Self {
        self.tab_bar = self.tab_bar.text_font(text_font);
        self
    }

    /// Sets the text size of the [`TabLabel`] of the
    /// [`TabBar`].
    #[must_use]
    pub fn text_size(mut self, text_size: f32) -> Self {
        self.tab_bar = self.tab_bar.text_size(text_size);
        self
    }

    /// Sets the width of the [`Tabs`].
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }
}

impl<Message, TabId, Theme, Renderer> Meta for Tabs<'_, Message, TabId, Theme, Renderer>
where
    Renderer: renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog,
    TabId: Eq + Clone,
{
}

impl<Message, TabId, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Tabs<'_, Message, TabId, Theme, Renderer>
where
    Renderer: renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog + text::Catalog,
    TabId: Eq + Clone,
{
    fn diff(&mut self, tree: &mut Tree) {
        // Child 0 is the tab bar tree, child 1 holds one tree per tab content.
        if tree.children.len() != 2 {
            tree.children = vec![Tree::new(&self.tab_bar), Tree::empty()];
        }

        tree.children[0].diff(&mut self.tab_bar);
        tree.children[1].diff_children(&mut self.children);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        let tab_bar_limits = limits.width(self.width).height(Length::Fill);
        self.tab_bar
            .layout(&mut tree.children[0], renderer, &tab_bar_limits);
        let tab_bar_size = tree.children[0].size;

        let tab_content_limits = limits
            .width(self.width)
            .height(self.height)
            .shrink([0.0, tab_bar_size.height]);

        let idx = self.tab_bar.get_active_tab_idx();
        let tab_content_size = if let (Some(element), Some(child)) = (
            self.children.get_mut(idx),
            tree.children[1].children.get_mut(idx),
        ) {
            element.layout(child, renderer, &tab_content_limits);
            child.translation = Vector::ZERO;
            child.size
        } else {
            layout::atomic(&tab_content_limits, Length::Fill, Length::Fill)
        };

        let (bar_y, content_y) = match self.tab_bar_position {
            TabBarPosition::Top => (0.0, tab_bar_size.height),
            TabBarPosition::Bottom => (tab_content_size.height, 0.0),
        };

        tree.children[0].translation = Vector::new(0.0, bar_y);
        tree.children[1].translation = Vector::new(0.0, content_y);
        tree.children[1].size = tab_content_size;

        tree.size = Size::new(
            tab_content_size.width,
            tab_bar_size.height + tab_content_size.height,
        );
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
        let mut children = layout.iter_mut(&mut state.children);
        let (Some((tab_bar_layout, bar_tree)), Some((tab_content_layout, content_tree))) =
            (children.next(), children.next())
        else {
            return;
        };

        self.tab_bar.update(
            bar_tree,
            event,
            tab_bar_layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
        let idx = self.tab_bar.get_active_tab_idx();
        if let Some(element) = self.children.get_mut(idx)
            && let Some(child_tree) = content_tree.children.get_mut(idx)
        {
            element.update(
                child_tree,
                event,
                tab_content_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );
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
        let mut children = layout.iter(&state.children);
        let (Some((tab_bar_layout, bar_tree)), Some((tab_content_layout, content_tree))) =
            (children.next(), children.next())
        else {
            return mouse::Interaction::default();
        };

        // Tab bar
        let mut mouse_interaction = mouse::Interaction::default();
        let new_mouse_interaction = self.tab_bar.mouse_interaction(
            bar_tree,
            tab_bar_layout,
            cursor,
            viewport,
            renderer,
        );

        if new_mouse_interaction > mouse_interaction {
            mouse_interaction = new_mouse_interaction;
        }

        // Tab content
        let idx = self.tab_bar.get_active_tab_idx();
        if let Some(element) = self.children.get(idx)
            && let Some(child_tree) = content_tree.children.get(idx)
        {
            let new_mouse_interaction = element.mouse_interaction(
                child_tree,
                tab_content_layout,
                cursor,
                viewport,
                renderer,
            );

            if new_mouse_interaction > mouse_interaction {
                mouse_interaction = new_mouse_interaction;
            }
        }

        mouse_interaction
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
        let mut children = layout.iter(&state.children);
        let (Some((tab_bar_layout, bar_tree)), Some((tab_content_layout, content_tree))) =
            (children.next(), children.next())
        else {
            return;
        };

        self.tab_bar.draw(
            bar_tree,
            renderer,
            theme,
            style,
            tab_bar_layout,
            cursor,
            viewport,
        );

        let idx = self.tab_bar.get_active_tab_idx();
        if let Some(element) = self.children.get(idx)
            && let Some(child_tree) = content_tree.children.get(idx)
        {
            element.draw(
                child_tree,
                renderer,
                theme,
                style,
                tab_content_layout,
                cursor,
                viewport,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        state: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        let idx = self.tab_bar.get_active_tab_idx();

        let mut children = layout.iter_mut(&mut state.children);
        let _ = children.next();
        let Some((content_layout, content_tree)) = children.next() else {
            return Vec::new();
        };

        match (
            self.children.get_mut(idx),
            content_tree.children.get_mut(idx),
        ) {
            (Some(element), Some(child_tree)) => element.overlay(
                child_tree,
                content_layout,
                renderer,
                viewport,
                translation,
                window,
            ),
            _ => Vec::new(),
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let active_tab = self.tab_bar.get_active_tab_idx();
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            let mut children = layout.iter_mut(&mut tree.children);
            let _ = children.next();
            let Some((content_layout, content_tree)) = children.next() else {
                return;
            };

            if let (Some(element), Some(child_tree)) = (
                self.children.get_mut(active_tab),
                content_tree.children.get_mut(active_tab),
            ) {
                element.operate(child_tree, content_layout, viewport, renderer, operation);
            }
        });
    }
}

impl<'a, Message, TabId, Theme, Renderer> From<Tabs<'a, Message, TabId, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: 'a + renderer::Renderer + iced_core::text::Renderer,
    Theme: 'a + Catalog + text::Catalog,
    Message: 'a,
    TabId: 'a + Eq + Clone,
{
    fn from(tabs: Tabs<'a, Message, TabId, Theme, Renderer>) -> Self {
        Widget::_boxed(tabs)
    }
}
