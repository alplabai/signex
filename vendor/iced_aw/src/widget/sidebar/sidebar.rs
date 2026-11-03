//! There are two options available: [`Sidebar`] and [`SidebarWithContent`].
//!
//! [`Sidebar`] is used to displays a sidebar for selecting content to be displayed, and the
//! sidebar is normally to a side of displayed content. You have to manage the logic to show
//! the content by yourself. Mainly used to customize the sidebar, the content, or both.
//!
//! [`SidebarWithContent`] is a single widget containing both the sidebar and content area,
//! and it manages the displaying of the content.

use super::column::FlushColumn;
use crate::{
    ICED_AW_FONT, iced_aw_font,
    style::{
        Status, StyleFn,
        sidebar::{self, Catalog, Style},
    },
};
use iced_core::{
    Alignment, Background, Border, Color, Element, Event, Font, Layout, Length, Padding, Pixels,
    Point, Rectangle, Shadow, Shell, Size, Vector, Widget,
    alignment::{self, Vertical},
    layout::{self, Limits},
    mouse::{self, Cursor},
    overlay, renderer, touch,
    widget::{Meta, Operation, Tree},
};
use iced_widget::{
    Row, Text,
    text::{self, LineHeight, Wrapping},
};
use std::marker::PhantomData;

/// The default icon size.
const DEFAULT_ICON_SIZE: f32 = 16.0;
/// The default text size.
const DEFAULT_TEXT_SIZE: f32 = 16.0;
/// The default size of the close icon.
const DEFAULT_CLOSE_SIZE: f32 = 16.0;
/// The default padding between the tabs.
const DEFAULT_PADDING: Padding = Padding::new(1.0);
/// The default spacing around the tabs.
const DEFAULT_SPACING: Pixels = Pixels::ZERO;

/// A [`TabLabel`] showing an icon and/or a text on a tab
/// on a [`Sidebar`].
#[allow(missing_debug_implementations)]
#[derive(Clone, Hash)]
pub enum TabLabel {
    /// A [`TabLabel`] showing only an icon on the tab.
    Icon(char),

    /// A [`TabLabel`] showing only a text on the tab.
    Text(String),

    /// A [`TabLabel`] showing an icon and a text on the tab.
    IconText(char, String),
}

#[derive(Clone, Copy, Default)]
/// The [`Position`] of the icon relative to text, this enum is only relative if
/// [`TabLabel::IconText`] is used.
pub enum Position {
    /// Icon is placed at the start position.
    #[default]
    Start,
    /// Icon is placed at the end position.
    End,
}

//
//
// ------ Just the sidebar.
//
//

/// A sidebar to show tabs.
///
/// # Example
/// ```ignore
/// # use iced_aw::sidebar::{TabLabel, Sidebar};
/// #
/// #[derive(Debug, Clone)]
/// enum Message {
///     TabSelected(TabId),
/// }
///
/// #[derive(PartialEq, Hash)]
/// enum TabId {
///    One,
///    Two,
///    Three,
/// }
///
/// let sidebar = Sidebar::new(
///     Message::TabSelected,
/// )
/// .push(TabId::One, TabLabel::Text(String::from("One")))
/// .push(TabId::Two, TabLabel::Text(String::from("Two")))
/// .push(TabId::Three, TabLabel::Text(String::from("Three")))
/// .set_active_tab(&TabId::One);
/// ```
#[allow(missing_debug_implementations)]
pub struct Sidebar<'a, Message, TabId, Theme = iced_widget::Theme, Renderer = iced_widget::Renderer>
where
    Renderer: renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog,
    TabId: Eq + Clone,
{
    /// The index of the currently active tab.
    active_tab: usize,
    /// The vector containing the labels of the tabs.
    tab_labels: Vec<TabLabel>,
    /// The vector containing the indices of the tabs.
    tab_indices: Vec<TabId>,
    /// The alignment of the tabs.
    align_tabs: Alignment,
    /// The function that produces the message when a tab is selected.
    on_select: Box<dyn Fn(TabId) -> Message>,
    /// The function that produces the message when the close icon was pressed.
    on_close: Option<Box<dyn Fn(TabId) -> Message>>,
    /// The width of the [`Sidebar`].
    width: Length,
    /// The height of the [`Sidebar`].
    height: Length,
    /// The height of the tabs of the [`Sidebar`].
    tab_height: Length,
    /// The icon size.
    icon_size: f32,
    /// The text size.
    text_size: f32,
    // The size of the close icon.
    close_size: f32,
    /// The padding of the tabs of the [`Sidebar`].
    padding: Padding,
    /// The spacing of the tabs of the [`Sidebar`].
    spacing: Pixels,
    /// The optional icon font of the [`Sidebar`].
    font: Option<Font>,
    /// The optional text font of the [`Sidebar`].
    text_font: Option<Font>,
    /// The style of the [`Sidebar`].
    class: <Theme as Catalog>::Class<'a>,
    /// Where the icon is placed relative to text
    position: Position,
    /// Where to place the close icon on the tab
    close_position: Position,
    #[allow(clippy::missing_docs_in_private_items)]
    _renderer: PhantomData<Renderer>,
}

impl<'a, Message, TabId, Theme, Renderer> Sidebar<'a, Message, TabId, Theme, Renderer>
where
    Renderer: renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog,
    TabId: Eq + Clone,
{
    /// Creates a new [`Sidebar`] with the index of the selected tab and a specified
    /// message which will be send when a tab is selected by the user.
    ///
    /// It expects:
    ///     * the index of the currently active tab.
    ///     * the function that will be called if a tab is selected by the user.
    ///         It takes the index of the selected tab.
    pub fn new<F>(on_select: F) -> Self
    where
        F: 'static + Fn(TabId) -> Message,
    {
        Self::with_tab_labels(Vec::new(), on_select)
    }

    /// Similar to `new` but with a given Vector of the [`TabLabel`]s.
    ///
    /// It expects:
    ///     * the index of the currently active tab.
    ///     * a vector containing the [`TabLabel`]s of the [`Sidebar`].
    ///     * the function that will be called if a tab is selected by the user.
    ///         It takes the index of the selected tab.
    pub fn with_tab_labels<F>(tab_labels: Vec<(TabId, TabLabel)>, on_select: F) -> Self
    where
        F: 'static + Fn(TabId) -> Message,
    {
        Self {
            active_tab: 0,
            tab_indices: tab_labels.iter().map(|(id, _)| id.clone()).collect(),
            tab_labels: tab_labels.into_iter().map(|(_, label)| label).collect(),
            align_tabs: Alignment::Start,
            on_select: Box::new(on_select),
            on_close: None,
            width: Length::Shrink,
            height: Length::Fill,
            tab_height: Length::Shrink,
            icon_size: DEFAULT_ICON_SIZE,
            text_size: DEFAULT_TEXT_SIZE,
            close_size: DEFAULT_CLOSE_SIZE,
            padding: DEFAULT_PADDING,
            spacing: DEFAULT_SPACING,
            font: None,
            text_font: None,
            class: <Theme as Catalog>::default(),
            position: Position::Start,
            close_position: Position::End,
            _renderer: PhantomData,
        }
    }

    /// Sets the alignment of the tabs for the [`Sidebar`].
    #[must_use]
    pub fn align_tabs(mut self, align: Alignment) -> Self {
        self.align_tabs = align;
        self
    }

    /// Sets the size of the close icon of the
    /// [`TabLabel`]s of the [`Sidebar`].
    #[must_use]
    pub fn close_size(mut self, close_size: f32) -> Self {
        self.close_size = close_size;
        self
    }

    /// Gets the id of the currently active tab on the [`Sidebar`].
    #[must_use]
    pub fn get_active_tab_id(&self) -> Option<&TabId> {
        self.tab_indices.get(self.active_tab)
    }

    /// Gets the index of the currently active tab on the [`Sidebar`].
    #[must_use]
    pub fn get_active_tab_idx(&self) -> usize {
        self.active_tab
    }

    /// Gets the width of the [`Sidebar`].
    #[must_use]
    pub fn get_height(&self) -> Length {
        self.height
    }

    /// Gets the width of the [`Sidebar`].
    #[must_use]
    pub fn get_width(&self) -> Length {
        self.width
    }

    /// Sets the height of the [`Sidebar`].
    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the font of the icons of the
    /// [`TabLabel`]s of the [`Sidebar`].
    #[must_use]
    pub fn icon_font(mut self, font: Font) -> Self {
        self.font = Some(font);
        self
    }

    /// Sets the icon size of the [`TabLabel`]s of the [`Sidebar`].
    #[must_use]
    pub fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = icon_size;
        self
    }

    /// Sets the message that will be produced when the close icon of a tab
    /// on the [`Sidebar`] is pressed.
    ///
    /// Setting this enables the drawing of a close icon on the tabs.
    #[must_use]
    pub fn on_close<F>(mut self, on_close: F) -> Self
    where
        F: 'static + Fn(TabId) -> Message,
    {
        self.on_close = Some(Box::new(on_close));
        self
    }

    /// Sets the padding of the tabs of the [`Sidebar`].
    #[must_use]
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Pushes a [`TabLabel`] to the [`Sidebar`].
    #[must_use]
    pub fn push(mut self, id: TabId, tab_label: TabLabel) -> Self {
        self.tab_labels.push(tab_label);
        self.tab_indices.push(id);
        self
    }

    /// Gets the amount of tabs on the [`Sidebar`].
    #[must_use]
    pub fn size(&self) -> usize {
        self.tab_indices.len()
    }

    /// Sets the spacing between the tabs of the [`Sidebar`].
    #[must_use]
    pub fn spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.spacing = spacing.into();
        self
    }

    /// Sets the font of the text of the
    /// [`TabLabel`]s of the [`Sidebar`].
    #[must_use]
    pub fn text_font(mut self, text_font: Font) -> Self {
        self.text_font = Some(text_font);
        self
    }

    /// Sets the text size of the [`TabLabel`]s of the [`Sidebar`].
    #[must_use]
    pub fn text_size(mut self, text_size: f32) -> Self {
        self.text_size = text_size;
        self
    }

    /// Sets the height of a tab on the [`Sidebar`].
    #[must_use]
    pub fn tab_height(mut self, height: Length) -> Self {
        self.tab_height = height;
        self
    }

    /// Sets up the active tab on the [`Sidebar`].
    #[must_use]
    pub fn set_active_tab(mut self, active_tab: &TabId) -> Self {
        self.active_tab = self
            .tab_indices
            .iter()
            .position(|id| id == active_tab)
            .map_or(0, |a| a);
        self
    }

    #[must_use]
    /// Sets the [`Position`] of the close icon on the tab.
    /// Only used when [`Sidebar::on_close()`] is used.
    pub fn set_close_position(mut self, position: Position) -> Self {
        self.close_position = position;
        self
    }

    #[must_use]
    /// Sets the [`Position`] of the Icon next to Text. Only used in [`TabLabel::IconText`].
    pub fn set_position(mut self, position: Position) -> Self {
        self.position = position;
        self
    }

    /// Sets the style of the [`Sidebar`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme, Style>).into();
        self
    }

    /// Sets the class of the input of the [`Sidebar`].
    #[must_use]
    pub fn class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    /// Sets the width of the [`Sidebar`].
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }
}

impl<Message, TabId, Theme, Renderer> Meta for Sidebar<'_, Message, TabId, Theme, Renderer>
where
    Renderer: renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog,
    TabId: Eq + Clone,
{
}

impl<Message, TabId, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Sidebar<'_, Message, TabId, Theme, Renderer>
where
    Renderer: renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog + text::Catalog,
    TabId: Eq + Clone,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn diff(&mut self, _tree: &mut Tree) {
        // The child tree (the column of tabs) is reconciled in `layout`
        // and `operate`, where the column is rebuilt from the tab labels.
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        fn layout_icon<'a, Message, Theme, Renderer>(
            icon: &char,
            size: f32,
            font: Option<Font>,
        ) -> Element<'a, Message, Theme, Renderer>
        where
            Message: 'a,
            Renderer: iced_core::text::Renderer + 'a,
            Theme: iced_widget::text::Catalog + 'a,
        {
            Widget::_boxed(
                Text::<Theme>::new(icon.to_string())
                    .size(size)
                    .font(font.unwrap_or_default())
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
                    .shaping(iced_core::text::Shaping::Advanced)
                    .width(Length::Shrink),
            )
        }

        fn layout_text<'a, Message, Theme, Renderer>(
            text: &'a str,
            size: f32,
            font: Option<Font>,
        ) -> Element<'a, Message, Theme, Renderer>
        where
            Message: 'a,
            Renderer: iced_core::text::Renderer + 'a,
            Theme: iced_widget::text::Catalog + 'a,
        {
            Widget::_boxed(
                Text::<Theme>::new(text)
                    .size(size)
                    .font(font.unwrap_or_default())
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
                    .shaping(text::Shaping::Advanced)
                    .width(Length::Shrink),
            )
        }

        let column = self
            .tab_labels
            .iter()
            .fold(
                FlushColumn::<Message, Theme, Renderer>::new(),
                |column, tab_label| {
                    let label: Row<Element<'_, Message, Theme, Renderer>> = match tab_label {
                        TabLabel::Icon(icon) => Row::new()
                            .align_y(Alignment::Center)
                            .push(layout_icon(icon, self.icon_size + 1.0, self.font)),
                        TabLabel::Text(text) => Row::new()
                            .padding(5.0)
                            .align_y(Alignment::Center)
                            .push(layout_text(text, self.text_size + 1.0, self.text_font)),
                        TabLabel::IconText(icon, text) => {
                            let mut row = Row::new().align_y(Alignment::Center);
                            match self.position {
                                Position::Start => {
                                    row = row
                                        .push(layout_icon(icon, self.icon_size + 1.0, self.font))
                                        .push(layout_text(
                                            text,
                                            self.text_size + 1.0,
                                            self.text_font,
                                        ));
                                }
                                Position::End => {
                                    row = row
                                        .push(layout_text(
                                            text,
                                            self.text_size + 1.0,
                                            self.text_font,
                                        ))
                                        .push(layout_icon(icon, self.icon_size + 1.0, self.font));
                                }
                            }
                            row
                        }
                    };
                    let label = Widget::_boxed(label);
                    let mut tab: Row<Element<'_, Message, Theme, Renderer>> = Row::new();
                    if self.on_close.is_some() {
                        let close: Row<Element<'_, Message, Theme, Renderer>> = Row::new()
                            .width(Length::Fixed(self.close_size * 1.3 + 1.0))
                            .height(Length::Fixed(self.close_size * 1.3 + 1.0))
                            .align_y(Alignment::Center);
                        let close = Widget::_boxed(close);
                        match self.close_position {
                            Position::Start => tab = tab.push(close).push(label),
                            Position::End => tab = tab.push(label).push(close),
                        }
                    } else {
                        tab = tab.push(label);
                    }
                    tab = tab
                        .align_y(Alignment::Center)
                        .padding(self.padding)
                        .height(self.tab_height)
                        .width(self.width);
                    column.push(tab)
                },
            )
            .width(self.width)
            .height(self.height)
            .spacing(self.spacing)
            .align_x(self.align_tabs);
        let mut column = column;
        if tree.children.is_empty() {
            tree.children.push(Tree::new(&column));
        }
        tree.children[0].diff(&mut column);
        column.layout(&mut tree.children[0], renderer, &limits.loose());
        tree.children[0].translation = Vector::ZERO;
        tree.size = tree.children[0].size;
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
        ) && cursor
            .position()
            .is_some_and(|pos| layout.bounds().contains(pos))
        {
            let Some((column_layout, column_tree)) = layout.iter(&tree.children).next() else {
                return;
            };
            let tabs_map: Vec<bool> = column_layout
                .iter(&column_tree.children)
                .map(|(layout, _)| {
                    cursor
                        .position()
                        .is_some_and(|pos| layout.bounds().contains(pos))
                })
                .collect();

            if let Some(new_selected) = tabs_map.iter().position(|b| *b) {
                shell.publish(
                    self.on_close
                        .as_ref()
                        .filter(|_on_close| {
                            let (tab_layout, tab_tree) = column_layout
                                .iter(&column_tree.children)
                                .nth(new_selected)
                                .expect("widget: Layout should have a tab layout at the selected index");
                            let (cross_layout, _) = tab_layout
                                .iter(&tab_tree.children)
                                .nth(1)
                                .expect("widget: Layout should have a close layout");

                            cursor
                                .position()
                                .is_some_and(|pos| cross_layout.bounds().contains(pos))
                        })
                        .map_or_else(
                            || (self.on_select)(self.tab_indices[new_selected].clone()),
                            |on_close| (on_close)(self.tab_indices[new_selected].clone()),
                        ),
                );
                shell.capture_event();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let mut mouse_interaction = mouse::Interaction::default();
        let Some((column_layout, column_tree)) = layout.iter(&tree.children).next() else {
            return mouse_interaction;
        };
        for (layout, _) in column_layout.iter(&column_tree.children) {
            let is_mouse_over = cursor
                .position()
                .is_some_and(|pos| layout.bounds().contains(pos));
            let new_mouse_interaction = if is_mouse_over {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::default()
            };
            if new_mouse_interaction > mouse_interaction {
                mouse_interaction = new_mouse_interaction;
            }
        }
        mouse_interaction
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
        let is_mouse_over = cursor.position().is_some_and(|pos| bounds.contains(pos));
        let style_sheet = if is_mouse_over {
            sidebar::Catalog::style(theme, &self.class, Status::Hovered)
        } else {
            sidebar::Catalog::style(theme, &self.class, Status::Disabled)
        };
        if bounds.intersects(viewport) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: (0.0).into(),
                        width: style_sheet.border_width,
                        color: style_sheet.border_color.unwrap_or(Color::TRANSPARENT),
                        ..Default::default()
                    },
                    shadow: Shadow::default(),
                    ..Default::default()
                },
                style_sheet
                    .background
                    .unwrap_or_else(|| Color::TRANSPARENT.into()),
            );
        }
        let Some((column_layout, column_tree)) = layout.iter(&tree.children).next() else {
            return;
        };
        for ((i, tab), (layout, tab_tree)) in self
            .tab_labels
            .iter()
            .enumerate()
            .zip(column_layout.iter(&column_tree.children))
        {
            draw_tab(
                renderer,
                tab,
                layout,
                tab_tree,
                self.position,
                theme,
                &self.class,
                i == self.get_active_tab_idx(),
                cursor,
                (self.font.unwrap_or(ICED_AW_FONT), self.icon_size),
                (self.text_font.unwrap_or_default(), self.text_size),
                self.close_size,
                viewport,
                self.on_close.is_some(),
                &self.close_position,
            );
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
        // Recreate the column element structure to expose tab labels for testing
        fn layout_icon<'a, Message, Theme, Renderer>(
            icon: &char,
            size: f32,
            font: Option<Font>,
        ) -> Element<'a, Message, Theme, Renderer>
        where
            Message: 'a,
            Renderer: iced_core::text::Renderer + 'a,
            Theme: iced_widget::text::Catalog + 'a,
        {
            Widget::_boxed(
                Text::<Theme>::new(icon.to_string())
                    .size(size)
                    .font(font.unwrap_or_default())
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
                    .shaping(iced_core::text::Shaping::Advanced)
                    .width(Length::Shrink),
            )
        }

        fn layout_text<'a, Message, Theme, Renderer>(
            text: &'a str,
            size: f32,
            font: Option<Font>,
        ) -> Element<'a, Message, Theme, Renderer>
        where
            Message: 'a,
            Renderer: iced_core::text::Renderer + 'a,
            Theme: iced_widget::text::Catalog + 'a,
        {
            Widget::_boxed(
                Text::<Theme>::new(text)
                    .size(size)
                    .font(font.unwrap_or_default())
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center)
                    .shaping(text::Shaping::Advanced)
                    .width(Length::Shrink),
            )
        }

        // Get the close icon content once before the fold
        let close_icon_data = if self.on_close.is_some() {
            let (close_content, close_font, _close_shaping) = iced_aw_font::advanced_text::cancel();
            Some((close_content, close_font))
        } else {
            None
        };

        let column = self
            .tab_labels
            .iter()
            .fold(
                FlushColumn::<Message, Theme, Renderer>::new(),
                |column, tab_label| {
                    let label: Row<Element<'_, Message, Theme, Renderer>> = match tab_label {
                        TabLabel::Icon(icon) => Row::new()
                            .align_y(Alignment::Center)
                            .push(layout_icon(icon, self.icon_size + 1.0, self.font)),
                        TabLabel::Text(text) => Row::new()
                            .padding(5.0)
                            .align_y(Alignment::Center)
                            .push(layout_text(text, self.text_size + 1.0, self.text_font)),
                        TabLabel::IconText(icon, text) => {
                            let mut row = Row::new().align_y(Alignment::Center);
                            match self.position {
                                Position::Start => {
                                    row = row
                                        .push(layout_icon(icon, self.icon_size + 1.0, self.font))
                                        .push(layout_text(
                                            text,
                                            self.text_size + 1.0,
                                            self.text_font,
                                        ));
                                }
                                Position::End => {
                                    row = row
                                        .push(layout_text(
                                            text,
                                            self.text_size + 1.0,
                                            self.text_font,
                                        ))
                                        .push(layout_icon(icon, self.icon_size + 1.0, self.font));
                                }
                            }
                            row
                        }
                    };
                    let label = Widget::_boxed(label);
                    let mut tab: Row<Element<'_, Message, Theme, Renderer>> = Row::new();
                    if let Some((ref close_content, close_font)) = close_icon_data {
                        let close: Row<Element<'_, Message, Theme, Renderer>> = Row::new()
                            .width(Length::Fixed(self.close_size * 1.3 + 1.0))
                            .height(Length::Fixed(self.close_size * 1.3 + 1.0))
                            .align_y(Alignment::Center)
                            .push(Widget::_boxed(
                                Text::<Theme>::new(close_content.as_str())
                                    .size(self.close_size + 1.0)
                                    .font(close_font)
                                    .align_x(alignment::Horizontal::Center)
                                    .align_y(alignment::Vertical::Center)
                                    .shaping(text::Shaping::Advanced)
                                    .width(Length::Shrink),
                            ));
                        let close = Widget::_boxed(close);
                        match self.close_position {
                            Position::Start => tab = tab.push(close).push(label),
                            Position::End => tab = tab.push(label).push(close),
                        }
                    } else {
                        tab = tab.push(label);
                    }
                    tab = tab
                        .align_y(Alignment::Center)
                        .padding(self.padding)
                        .height(self.tab_height)
                        .width(self.width);
                    column.push(tab)
                },
            )
            .width(self.width)
            .height(self.height)
            .spacing(self.spacing)
            .align_x(self.align_tabs);

        let mut column = column;

        // Get or create the tree for the column
        if tree.children.is_empty() {
            tree.children.push(Tree::new(&column));
        }
        tree.children[0].diff(&mut column);

        // Recursively operate on the column to expose all tab labels
        if let Some((column_layout, column_tree)) = layout.iter_mut(&mut tree.children).next() {
            column.operate(column_tree, column_layout, viewport, renderer, operation);
        }
    }
}

/// Draws a tab.
#[allow(
    clippy::borrowed_box,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]
fn draw_tab<Theme, Renderer>(
    renderer: &mut Renderer,
    tab: &TabLabel,
    layout: Layout,
    tree: &Tree,
    position: Position,
    theme: &Theme,
    class: &<Theme as Catalog>::Class<'_>,
    is_selected: bool,
    cursor: Cursor,
    icon_data: (Font, f32),
    text_data: (Font, f32),
    close_size: f32,
    viewport: &Rectangle,
    on_close: bool,
    close_position: &Position,
) where
    Renderer: renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog + text::Catalog,
{
    fn icon_bound_rectangle(item: Option<Layout>) -> Rectangle {
        item.expect("Graphics: Layout should have an icons layout for an IconText")
            .bounds()
    }

    fn text_bound_rectangle(item: Option<Layout>) -> Rectangle {
        item.expect("Graphics: Layout should have an texts layout for an IconText")
            .bounds()
    }

    fn render_icon_text<Renderer>(
        renderer: &mut Renderer,
        tab: &TabLabel,
        label_layout: Layout,
        label_tree: &Tree,
        icon_data: (Font, f32),
        text_data: (Font, f32),
        style: &Style,
        position: Position,
    ) where
        Renderer: renderer::Renderer + iced_core::text::Renderer,
    {
        let mut label_layout_children = label_layout.iter(&label_tree.children).map(|(l, _)| l);
        match tab {
            TabLabel::Icon(icon) => {
                let icon_bounds = icon_bound_rectangle(label_layout_children.next());
                renderer.fill_text(
                    iced_core::text::Text {
                        content: icon.to_string(),
                        bounds: Size::new(icon_bounds.width, icon_bounds.height),
                        size: Pixels(icon_data.1),
                        font: icon_data.0,
                        align_x: text::Alignment::Center,
                        align_y: Vertical::Center,
                        line_height: LineHeight::Relative(1.3),
                        shaping: iced_core::text::Shaping::Advanced,
                        wrapping: Wrapping::default(),
                        ellipsis: text::Ellipsis::default(),
                        hint_factor: None,
                    },
                    Point::new(icon_bounds.center_x(), icon_bounds.center_y()),
                    style.icon_color,
                    icon_bounds,
                );
            }
            TabLabel::Text(text) => {
                let text_bounds = text_bound_rectangle(label_layout_children.next());
                renderer.fill_text(
                    iced_core::text::Text {
                        content: text.clone(),
                        bounds: Size::new(text_bounds.width, text_bounds.height),
                        size: Pixels(text_data.1),
                        font: text_data.0,
                        align_x: text::Alignment::Center,
                        align_y: Vertical::Center,
                        line_height: LineHeight::Relative(1.3),
                        shaping: iced_core::text::Shaping::Advanced,
                        wrapping: Wrapping::default(),
                        ellipsis: text::Ellipsis::default(),
                        hint_factor: None,
                    },
                    Point::new(text_bounds.center_x(), text_bounds.center_y()),
                    style.text_color,
                    text_bounds,
                );
            }
            TabLabel::IconText(icon, text) => {
                let icon_bounds: Rectangle;
                let text_bounds: Rectangle;
                match position {
                    Position::Start => {
                        icon_bounds = icon_bound_rectangle(label_layout_children.next());
                        text_bounds = text_bound_rectangle(label_layout_children.next());
                    }
                    Position::End => {
                        text_bounds = text_bound_rectangle(label_layout_children.next());
                        icon_bounds = icon_bound_rectangle(label_layout_children.next());
                    }
                }
                renderer.fill_text(
                    iced_core::text::Text {
                        content: icon.to_string(),
                        bounds: Size::new(icon_bounds.width, icon_bounds.height),
                        size: Pixels(icon_data.1),
                        font: icon_data.0,
                        align_x: text::Alignment::Center,
                        align_y: Vertical::Center,
                        line_height: LineHeight::Relative(1.3),
                        shaping: iced_core::text::Shaping::Advanced,
                        wrapping: Wrapping::default(),
                        ellipsis: text::Ellipsis::default(),
                        hint_factor: None,
                    },
                    Point::new(icon_bounds.center_x(), icon_bounds.center_y()),
                    style.icon_color,
                    icon_bounds,
                );
                renderer.fill_text(
                    iced_core::text::Text {
                        content: text.clone(),
                        bounds: Size::new(text_bounds.width, text_bounds.height),
                        size: Pixels(text_data.1),
                        font: text_data.0,
                        align_x: text::Alignment::Center,
                        align_y: Vertical::Center,
                        line_height: LineHeight::Relative(1.3),
                        shaping: iced_core::text::Shaping::Advanced,
                        wrapping: Wrapping::default(),
                        ellipsis: text::Ellipsis::default(),
                        hint_factor: None,
                    },
                    Point::new(text_bounds.center_x(), text_bounds.center_y()),
                    style.text_color,
                    text_bounds,
                );
            }
        }
    }

    fn render_close<Renderer>(
        renderer: &mut Renderer,
        style: &Style,
        cross_layout: Layout,
        cursor: Cursor,
        close_size: f32,
        viewport: &Rectangle,
    ) where
        Renderer: renderer::Renderer + iced_core::text::Renderer,
    {
        let cross_bounds = cross_layout.bounds();
        let is_mouse_over_cross = cursor.is_over(cross_bounds);
        let (content, font, shaping) = iced_aw_font::advanced_text::cancel();
        renderer.fill_text(
            iced_core::text::Text {
                content,
                bounds: Size::new(cross_bounds.width, cross_bounds.height),
                size: Pixels(close_size + if is_mouse_over_cross { 1.0 } else { 0.0 }),
                font,
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: LineHeight::Relative(1.3),
                shaping,
                wrapping: Wrapping::default(),
                ellipsis: text::Ellipsis::default(),
                hint_factor: None,
            },
            Point::new(cross_bounds.center_x(), cross_bounds.center_y()),
            style.text_color,
            cross_bounds,
        );
        if is_mouse_over_cross && cross_bounds.intersects(viewport) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: cross_bounds,
                    border: Border {
                        radius: style.icon_border_radius,
                        width: style.border_width,
                        color: style.border_color.unwrap_or(Color::TRANSPARENT),
                        ..Default::default()
                    },
                    shadow: Shadow::default(),
                    ..Default::default()
                },
                style
                    .icon_background
                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
            );
        }
    }

    let bounds = layout.bounds();
    let is_mouse_over = cursor.position().is_some_and(|pos| bounds.contains(pos));
    let style = if is_mouse_over {
        sidebar::Catalog::style(theme, class, Status::Hovered)
    } else if is_selected {
        sidebar::Catalog::style(theme, class, Status::Active)
    } else {
        sidebar::Catalog::style(theme, class, Status::Disabled)
    };
    if bounds.intersects(viewport) {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: (0.0).into(),
                    width: style.tab_label_border_width,
                    color: style.tab_label_border_color,
                    ..Default::default()
                },
                shadow: Shadow::default(),
                ..Default::default()
            },
            style.tab_label_background,
        );
    }
    let mut children = layout.iter(&tree.children);
    if on_close {
        match close_position {
            Position::Start => {
                let (cross_layout, _) = children
                    .next()
                    .expect("Graphics: Expected close icon layout.");
                render_close(renderer, &style, cross_layout, cursor, close_size, viewport);
                let (label_layout, label_tree) = children
                    .next()
                    .expect("Graphics: Layout should have a label layout");
                render_icon_text(
                    renderer,
                    tab,
                    label_layout,
                    label_tree,
                    icon_data,
                    text_data,
                    &style,
                    position,
                );
            }
            Position::End => {
                let (label_layout, label_tree) = children
                    .next()
                    .expect("Graphics: Layout should have a label layout");
                render_icon_text(
                    renderer,
                    tab,
                    label_layout,
                    label_tree,
                    icon_data,
                    text_data,
                    &style,
                    position,
                );
                let (cross_layout, _) = children
                    .next()
                    .expect("Graphics: Expected close icon layout.");
                render_close(renderer, &style, cross_layout, cursor, close_size, viewport);
            }
        }
    } else {
        let (label_layout, label_tree) = children
            .next()
            .expect("Graphics: Layout should have a label layout");
        render_icon_text(
            renderer,
            tab,
            label_layout,
            label_tree,
            icon_data,
            text_data,
            &style,
            position,
        );
    }
}

impl<'a, Message, TabId, Theme, Renderer> From<Sidebar<'a, Message, TabId, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: 'a + renderer::Renderer + iced_core::text::Renderer,
    Theme: 'a + Catalog + text::Catalog,
    Message: 'a,
    TabId: 'a + Eq + Clone,
{
    fn from(sidebar: Sidebar<'a, Message, TabId, Theme, Renderer>) -> Self {
        Widget::_boxed(sidebar)
    }
}

//
//
// ------ Sidebar with content.
//
//

/// A [`SidebarPosition`] for defining the position of a [`Sidebar`] to the content.
#[derive(Clone, Hash)]
#[allow(missing_debug_implementations)]
pub enum SidebarPosition {
    /// A [`SidebarPosition`] for placing the [`Sidebar`] to the start of its content.
    Start,

    /// A [`SidebarPosition`] for placing the [`Sidebar`] to the end of its content.
    End,
}

/// A [`SidebarWithContent`] widget for showing a [`Sidebar`]
/// along with the tab's content.
///
/// # Example
/// ```ignore
/// # use iced_aw::{TabLabel, tabs::SidebarWithContent};
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
/// let tabs = SidebarWithContent::new(Message::TabSelected)
/// .push(TabId::One, TabLabel::Text(String::from("One")), Text::new(String::from("One")))
/// .push(TabId::Two, TabLabel::Text(String::from("Two")), Text::new(String::from("Two")))
/// .push(TabId::Three, TabLabel::Text(String::from("Three")), Text::new(String::from("Three")))
/// .set_active_tab(&TabId::Two);
/// ```
///
#[allow(missing_debug_implementations)]
pub struct SidebarWithContent<
    'a,
    Message,
    TabId,
    Theme = iced_widget::Theme,
    Renderer = iced_widget::Renderer,
> where
    Renderer: 'a + renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog,
    TabId: Eq + Clone,
{
    /// The [`Sidebar`] of the [`SidebarWithContent`].
    sidebar: Sidebar<'a, Message, TabId, Theme, Renderer>,
    /// The vector containing the content of the tabs.
    tabs: Vec<Element<'a, Message, Theme, Renderer>>,
    /// The vector containing the indices of the tabs.
    indices: Vec<TabId>,
    /// The position of the [`Sidebar`].
    sidebar_position: SidebarPosition,
    /// the width of the [`SidebarWithContent`].
    width: Length,
    /// The height of the [`SidebarWithContent`].
    height: Length,
}

impl<'a, Message, TabId, Theme, Renderer> SidebarWithContent<'a, Message, TabId, Theme, Renderer>
where
    Renderer: 'a + renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog + text::Catalog,
    TabId: Eq + Clone,
{
    /// Creates a new [`SidebarWithContent`] widget with the index of the selected tab and a
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
    ///         [`Element`]s of the [`SidebarWithContent`].
    ///     * the function that will be called if a tab is selected by the user.
    ///         It takes the index of the selected tab.
    pub fn new_with_tabs<F>(
        tabs: Vec<(TabId, TabLabel, Element<'a, Message, Theme, Renderer>)>,
        on_select: F,
    ) -> Self
    where
        F: 'static + Fn(TabId) -> Message,
    {
        let mut tab_labels = Vec::with_capacity(tabs.len());
        let mut elements = Vec::with_capacity(tabs.len());
        let mut indices = Vec::with_capacity(tabs.len());
        for (id, tab_label, element) in tabs {
            tab_labels.push((id.clone(), tab_label));
            indices.push(id);
            elements.push(element);
        }
        SidebarWithContent {
            sidebar: Sidebar::with_tab_labels(tab_labels, on_select),
            tabs: elements,
            indices,
            sidebar_position: SidebarPosition::Start,
            width: Length::Fill,
            height: Length::Shrink,
        }
    }

    /// Sets the size of the close icon of the
    /// [`TabLabel`] of the
    /// [`Sidebar`].
    #[must_use]
    pub fn close_size(mut self, close_size: f32) -> Self {
        self.sidebar = self.sidebar.close_size(close_size);
        self
    }

    /// Sets the alignment of the tabs for the [`Sidebar`].
    #[must_use]
    pub fn align_tabs(mut self, align: Alignment) -> Self {
        self.sidebar = self.sidebar.align_tabs(align);
        self
    }

    /// Sets the Icon render Position for the
    /// [`TabLabel`] of the
    /// [`Sidebar`].
    #[must_use]
    pub fn tab_icon_position(mut self, position: Position) -> Self {
        self.sidebar = self.sidebar.set_position(position);
        self
    }

    /// Sets the close icon render Position for the tab of the
    /// [`Sidebar`].
    #[must_use]
    pub fn close_icon_position(mut self, position: Position) -> Self {
        self.sidebar = self.sidebar.set_close_position(position);
        self
    }

    /// Sets the height of the [`SidebarWithContent`].
    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the font of the icons of the
    /// [`TabLabel`]s of the
    /// [`Sidebar`].
    #[must_use]
    pub fn icon_font(mut self, font: Font) -> Self {
        self.sidebar = self.sidebar.icon_font(font);
        self
    }

    /// Sets the icon size of the [`TabLabel`] of the
    /// [`Sidebar`].
    #[must_use]
    pub fn icon_size(mut self, icon_size: f32) -> Self {
        self.sidebar = self.sidebar.icon_size(icon_size);
        self
    }

    /// Sets the message that will be produced when the close icon of a tab
    /// on the [`Sidebar`] is pressed.
    ///
    /// Setting this enables the drawing of a close icon on the tabs.
    #[must_use]
    pub fn on_close<F>(mut self, on_close: F) -> Self
    where
        F: 'static + Fn(TabId) -> Message,
    {
        self.sidebar = self.sidebar.on_close(on_close);
        self
    }

    /// Pushes a [`TabLabel`] along with the tabs
    /// content to the [`SidebarWithContent`].
    #[must_use]
    pub fn push<E>(mut self, id: TabId, tab_label: TabLabel, element: E) -> Self
    where
        E: Into<Element<'a, Message, Theme, Renderer>>,
    {
        self.sidebar = self.sidebar.push(id.clone(), tab_label);
        self.tabs.push(element.into());
        self.indices.push(id);
        self
    }

    /// Sets the active tab of the [`SidebarWithContent`] using the ``TabId``.
    #[must_use]
    pub fn set_active_tab(mut self, id: &TabId) -> Self {
        self.sidebar = self.sidebar.set_active_tab(id);
        self
    }

    /// Sets the height of the [`Sidebar`] of the [`SidebarWithContent`].
    #[must_use]
    pub fn sidebar_height(mut self, height: Length) -> Self {
        self.sidebar = self.sidebar.height(height);
        self
    }

    /// Sets the width of the [`Sidebar`] of the [`SidebarWithContent`].
    #[must_use]
    pub fn sidebar_width(mut self, width: Length) -> Self {
        self.sidebar = self.sidebar.width(width);
        self
    }

    /// Sets the [`SidebarPosition`] of the [`Sidebar`].
    #[must_use]
    pub fn sidebar_position(mut self, position: SidebarPosition) -> Self {
        self.sidebar_position = position;
        self
    }

    /// Sets the style of the [`Sidebar`].
    #[must_use]
    pub fn sidebar_style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.sidebar = self.sidebar.style(style);
        self
    }

    /// Sets the padding of the tabs of the [`Sidebar`].
    #[must_use]
    pub fn tab_label_padding(mut self, padding: impl Into<Padding>) -> Self {
        self.sidebar = self.sidebar.padding(padding);
        self
    }

    /// Sets the spacing between the tabs of the
    /// [`Sidebar`].
    #[must_use]
    pub fn tab_label_spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.sidebar = self.sidebar.spacing(spacing);
        self
    }

    /// Sets the font of the text of the
    /// [`TabLabel`]s of the
    /// [`Sidebar`].
    #[must_use]
    pub fn text_font(mut self, text_font: Font) -> Self {
        self.sidebar = self.sidebar.text_font(text_font);
        self
    }

    /// Sets the text size of the [`TabLabel`] of the
    /// [`Sidebar`].
    #[must_use]
    pub fn text_size(mut self, text_size: f32) -> Self {
        self.sidebar = self.sidebar.text_size(text_size);
        self
    }

    /// Sets the width of the [`SidebarWithContent`].
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }
}

impl<Message, TabId, Theme, Renderer> Meta
    for SidebarWithContent<'_, Message, TabId, Theme, Renderer>
where
    Renderer: iced_core::text::Renderer,
    Theme: Catalog,
    TabId: Eq + Clone,
{
}

impl<Message, TabId, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SidebarWithContent<'_, Message, TabId, Theme, Renderer>
where
    Renderer: renderer::Renderer + iced_core::text::Renderer,
    Theme: Catalog + text::Catalog,
    TabId: Eq + Clone,
{
    fn diff(&mut self, tree: &mut Tree) {
        // should be 2 elements in the list always if not lets reload them.
        if tree.children.len() != 2 {
            tree.children = vec![Tree::new(&self.sidebar), Tree::empty()];
        }

        // The sidebar manages its own child tree in `layout`/`operate`.
        self.sidebar.diff(&mut tree.children[0]);
        tree.children[1].diff_children(&mut self.tabs);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let idx = self.sidebar.get_active_tab_idx();

        let sidebar_limits = limits.width(Length::Shrink).height(self.height);
        self.sidebar
            .layout(&mut tree.children[0], renderer, &sidebar_limits);
        let sidebar_size = tree.children[0].size;

        let tab_content_limits = limits
            .width(self.width)
            .height(self.height)
            .shrink([sidebar_size.width, 0.0]);

        let tab_content_size = if let (Some(element), Some(child)) = (
            self.tabs.get_mut(idx),
            tree.children[1].children.get_mut(idx),
        ) {
            element.layout(child, renderer, &tab_content_limits);
            child.translation = Vector::ZERO;
            child.size
        } else {
            layout::atomic(&tab_content_limits, Length::Fill, Length::Shrink)
        };

        let (sidebar_x, content_x) = match self.sidebar_position {
            SidebarPosition::Start => (0.0, sidebar_size.width),
            SidebarPosition::End => (tab_content_size.width, 0.0),
        };

        tree.children[0].translation = Vector::new(sidebar_x, 0.0);
        tree.children[1].translation = Vector::new(content_x, 0.0);
        tree.children[1].size = tab_content_size;

        tree.size = Size::new(
            sidebar_size.width + tab_content_size.width,
            tab_content_size.height,
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
        let idx = self.sidebar.get_active_tab_idx();
        let mut children = layout.iter_mut(&mut state.children);
        let (sidebar_layout, sidebar_tree) = children
            .next()
            .expect("Native: Layout should have a Sidebar layout");
        let (tab_content_layout, tab_content_tree) = children
            .next()
            .expect("Native: Layout should have a tab content layout");

        self.sidebar.update(
            sidebar_tree,
            event,
            sidebar_layout,
            cursor,
            renderer,
            shell,
            viewport,
        );

        if let Some(element) = self.tabs.get_mut(idx)
            && let Some((layout, tree)) = tab_content_layout
                .iter_mut(&mut tab_content_tree.children)
                .nth(idx)
        {
            element.update(tree, event, layout, cursor, renderer, shell, viewport);
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
        let idx = self.sidebar.get_active_tab_idx();
        let mut children = layout.iter(&state.children);
        let (sidebar_layout, sidebar_tree) = children
            .next()
            .expect("Native: There should be a Sidebar layout");
        let (tab_content_layout, tab_content_tree) = children
            .next()
            .expect("Native: There should be a tab content layout");

        // Sidebar
        let mut mouse_interaction = mouse::Interaction::default();
        let new_mouse_interaction =
            self.sidebar
                .mouse_interaction(sidebar_tree, sidebar_layout, cursor, viewport, renderer);
        if new_mouse_interaction > mouse_interaction {
            mouse_interaction = new_mouse_interaction;
        }

        // Tab content
        if let Some(element) = self.tabs.get(idx)
            && let Some((layout, tree)) = tab_content_layout
                .iter(&tab_content_tree.children)
                .nth(idx)
        {
            let new_mouse_interaction =
                element.mouse_interaction(tree, layout, cursor, viewport, renderer);

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
        let idx = self.sidebar.get_active_tab_idx();
        let mut children = layout.iter(&state.children);
        let (sidebar_layout, sidebar_tree) = children
            .next()
            .expect("Native: There should be a Sidebar layout");
        let (tab_content_layout, tab_content_tree) = children
            .next()
            .expect("Native: There should be a tab content layout");

        self.sidebar.draw(
            sidebar_tree,
            renderer,
            theme,
            style,
            sidebar_layout,
            cursor,
            viewport,
        );

        if let Some(element) = self.tabs.get(idx)
            && let Some((layout, tree)) = tab_content_layout
                .iter(&tab_content_tree.children)
                .nth(idx)
        {
            element.draw(tree, renderer, theme, style, layout, cursor, viewport);
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
        let idx = self.sidebar.get_active_tab_idx();

        let Some(element) = self.tabs.get_mut(idx) else {
            return Vec::new();
        };
        let Some((tab_content_layout, tab_content_tree)) = layout.iter_mut(&mut state.children).nth(1)
        else {
            return Vec::new();
        };
        let Some((layout, tree)) = tab_content_layout
            .iter_mut(&mut tab_content_tree.children)
            .nth(idx)
        else {
            return Vec::new();
        };

        element.overlay(tree, layout, renderer, viewport, translation, window)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let active_tab = self.sidebar.get_active_tab_idx();

        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            let mut children = layout.iter_mut(&mut tree.children);

            // Operate on the sidebar to expose tab labels
            if let Some((sidebar_layout, sidebar_tree)) = children.next() {
                self.sidebar
                    .operate(sidebar_tree, sidebar_layout, viewport, renderer, operation);
            }

            // Operate on the active tab content
            if let Some((tab_content_layout, tab_content_tree)) = children.next()
                && let Some(element) = self.tabs.get_mut(active_tab)
                && let Some((layout, tree)) = tab_content_layout
                    .iter_mut(&mut tab_content_tree.children)
                    .nth(active_tab)
            {
                element.operate(tree, layout, viewport, renderer, operation);
            }
        });
    }
}

impl<'a, Message, TabId, Theme, Renderer>
    From<SidebarWithContent<'a, Message, TabId, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: 'a + renderer::Renderer + iced_core::text::Renderer,
    Theme: 'a + Catalog + text::Catalog,
    Message: 'a,
    TabId: 'a + Eq + Clone,
{
    fn from(content: SidebarWithContent<'a, Message, TabId, Theme, Renderer>) -> Self {
        Widget::_boxed(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    enum TestTabId {
        One,
        Two,
        Three,
    }

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    enum TestMessage {
        TabSelected(TestTabId),
        TabClosed(TestTabId),
    }

    type TestSidebar<'a> = Sidebar<'a, TestMessage, TestTabId, iced_widget::Theme>;
    type TestSidebarWithContent<'a> =
        SidebarWithContent<'a, TestMessage, TestTabId, iced_widget::Theme>;

    #[test]
    fn sidebar_new_creates_instance() {
        let sidebar = TestSidebar::new(TestMessage::TabSelected);

        assert_eq!(sidebar.get_active_tab_idx(), 0);
        assert_eq!(sidebar.size(), 0);
        assert_eq!(sidebar.get_width(), Length::Shrink);
        assert_eq!(sidebar.get_height(), Length::Fill);
    }

    #[test]
    fn sidebar_push_adds_tabs() {
        let sidebar = TestSidebar::new(TestMessage::TabSelected)
            .push(TestTabId::One, TabLabel::Text("Tab One".into()))
            .push(TestTabId::Two, TabLabel::Text("Tab Two".into()))
            .push(TestTabId::Three, TabLabel::Text("Tab Three".into()));

        assert_eq!(sidebar.size(), 3);
    }

    #[test]
    fn sidebar_set_active_tab_sets_correct_index() {
        let sidebar = TestSidebar::new(TestMessage::TabSelected)
            .push(TestTabId::One, TabLabel::Text("Tab One".into()))
            .push(TestTabId::Two, TabLabel::Text("Tab Two".into()))
            .push(TestTabId::Three, TabLabel::Text("Tab Three".into()))
            .set_active_tab(&TestTabId::Two);

        assert_eq!(sidebar.get_active_tab_idx(), 1);
        assert_eq!(sidebar.get_active_tab_id(), Some(&TestTabId::Two));
    }

    #[test]
    fn sidebar_set_active_tab_defaults_to_zero_if_not_found() {
        let sidebar = TestSidebar::new(TestMessage::TabSelected)
            .push(TestTabId::One, TabLabel::Text("Tab One".into()))
            .push(TestTabId::Two, TabLabel::Text("Tab Two".into()));

        // Set to a tab that doesn't exist
        let sidebar = sidebar.set_active_tab(&TestTabId::Three);

        assert_eq!(sidebar.get_active_tab_idx(), 0);
    }

    #[test]
    fn sidebar_with_icon_label() {
        let sidebar = TestSidebar::new(TestMessage::TabSelected)
            .push(TestTabId::One, TabLabel::Icon('A'))
            .push(TestTabId::Two, TabLabel::Icon('B'));

        assert_eq!(sidebar.size(), 2);
    }

    #[test]
    fn sidebar_with_icon_text_label() {
        let sidebar = TestSidebar::new(TestMessage::TabSelected)
            .push(TestTabId::One, TabLabel::IconText('A', "Tab One".into()))
            .push(TestTabId::Two, TabLabel::IconText('B', "Tab Two".into()));

        assert_eq!(sidebar.size(), 2);
    }

    #[test]
    fn sidebar_with_close_callback() {
        let sidebar = TestSidebar::new(TestMessage::TabSelected)
            .push(TestTabId::One, TabLabel::Text("Tab One".into()))
            .on_close(TestMessage::TabClosed);

        assert_eq!(sidebar.size(), 1);
    }

    #[test]
    fn sidebar_builder_methods() {
        let sidebar = TestSidebar::new(TestMessage::TabSelected)
            .push(TestTabId::One, TabLabel::Text("Tab One".into()))
            .width(200)
            .height(Length::Fill)
            .tab_height(Length::Fixed(50.0))
            .icon_size(20.0)
            .text_size(14.0)
            .close_size(16.0)
            .padding(5.0)
            .spacing(2.0)
            .align_tabs(Alignment::Center)
            .set_position(Position::End)
            .set_close_position(Position::Start);

        assert_eq!(sidebar.size(), 1);
        assert_eq!(sidebar.get_width(), Length::Fixed(200.0));
        assert_eq!(sidebar.get_height(), Length::Fill);
    }

    #[test]
    fn sidebar_with_content_new_creates_instance() {
        let sidebar = TestSidebarWithContent::new(TestMessage::TabSelected);

        assert_eq!(sidebar.sidebar.get_active_tab_idx(), 0);
        assert_eq!(sidebar.sidebar.size(), 0);
    }

    #[test]
    fn sidebar_with_content_push_adds_tabs_and_content() {
        use iced_widget::text::Text;

        let sidebar = TestSidebarWithContent::new(TestMessage::TabSelected)
            .push(
                TestTabId::One,
                TabLabel::Text("Tab One".into()),
                Text::new("Content One"),
            )
            .push(
                TestTabId::Two,
                TabLabel::Text("Tab Two".into()),
                Text::new("Content Two"),
            );

        assert_eq!(sidebar.sidebar.size(), 2);
        assert_eq!(sidebar.tabs.len(), 2);
        assert_eq!(sidebar.indices.len(), 2);
    }

    #[test]
    fn sidebar_with_content_set_active_tab() {
        use iced_widget::text::Text;

        let sidebar = TestSidebarWithContent::new(TestMessage::TabSelected)
            .push(
                TestTabId::One,
                TabLabel::Text("Tab One".into()),
                Text::new("Content One"),
            )
            .push(
                TestTabId::Two,
                TabLabel::Text("Tab Two".into()),
                Text::new("Content Two"),
            )
            .set_active_tab(&TestTabId::Two);

        assert_eq!(sidebar.sidebar.get_active_tab_idx(), 1);
    }

    #[test]
    fn sidebar_with_content_builder_methods() {
        use iced_widget::text::Text;

        let sidebar = TestSidebarWithContent::new(TestMessage::TabSelected)
            .push(
                TestTabId::One,
                TabLabel::Text("Tab One".into()),
                Text::new("Content One"),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .sidebar_width(Length::Fixed(150.0))
            .sidebar_height(Length::Fill)
            .sidebar_position(SidebarPosition::End)
            .align_tabs(Alignment::End)
            .tab_label_padding(10.0)
            .tab_label_spacing(5.0)
            .icon_size(18.0)
            .text_size(12.0)
            .close_size(14.0)
            .tab_icon_position(Position::End)
            .close_icon_position(Position::Start);

        assert_eq!(sidebar.sidebar.size(), 1);
    }

    #[test]
    fn position_enum_variants_default() {
        assert!(matches!(Position::default(), Position::Start));
    }

    #[test]
    fn sidebar_get_active_tab_id_returns_none_when_empty() {
        let sidebar = TestSidebar::new(TestMessage::TabSelected);

        assert_eq!(sidebar.get_active_tab_id(), None);
    }

    #[test]
    fn sidebar_get_active_tab_id_returns_correct_id() {
        let sidebar = TestSidebar::new(TestMessage::TabSelected)
            .push(TestTabId::One, TabLabel::Text("Tab One".into()))
            .push(TestTabId::Two, TabLabel::Text("Tab Two".into()))
            .set_active_tab(&TestTabId::Two);

        assert_eq!(sidebar.get_active_tab_id(), Some(&TestTabId::Two));
    }

    #[test]
    fn sidebar_widget_trait_implementation() {
        // This test verifies that the Sidebar implements the Widget trait
        // which includes the operate() method
        let sidebar = TestSidebar::new(TestMessage::TabSelected)
            .push(TestTabId::One, TabLabel::Text("Tab One".into()))
            .push(TestTabId::Two, TabLabel::Text("Tab Two".into()));

        // Verify we can get the size
        assert_eq!(sidebar.size(), 2);

        // Verify widget trait methods are available by checking the type
        let _element: Element<TestMessage, iced_widget::Theme, iced_widget::Renderer> =
            sidebar.into();
    }

    #[test]
    fn sidebar_with_content_widget_trait_implementation() {
        // This test verifies that the SidebarWithContent implements the Widget trait
        // which includes the operate() method
        use iced_widget::text::Text;

        let sidebar = TestSidebarWithContent::new(TestMessage::TabSelected)
            .push(
                TestTabId::One,
                TabLabel::Text("Tab One".into()),
                Text::new("Content One"),
            )
            .push(
                TestTabId::Two,
                TabLabel::Text("Tab Two".into()),
                Text::new("Content Two"),
            );

        assert_eq!(sidebar.sidebar.size(), 2);

        // Verify widget trait methods are available by checking the type
        let _element: Element<TestMessage, iced_widget::Theme, iced_widget::Renderer> =
            sidebar.into();
    }
}
