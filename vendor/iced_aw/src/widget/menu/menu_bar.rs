//! [`MenuBar`]

#![allow(clippy::unwrap_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::enum_glob_use)]

use iced_core::{
    Alignment, Element, Event, Layout, Length, Padding, Pixels, Rectangle, Shell, Size, Vector,
    Widget,
    layout::{Limits, flex as core_flex},
    mouse, overlay, renderer,
    widget::{Meta, Operation, Tree, tree},
    window,
};

use super::{common::*, menu_bar_overlay::MenuBarOverlay, menu_tree::*};
use crate::style::menu_bar::*;
pub use crate::style::status::{Status, StyleFn};

#[cfg(feature = "debug_log")]
use log::debug;

#[derive(Debug, Clone, Copy)]
pub(super) enum MenuBarTask {
    OpenOnClick,
    CloseOnClick,
}

#[derive(Default, Debug)]
pub(super) struct GlobalState {
    pub(super) open: bool,
    pub(super) pressed: bool,
    task: Option<MenuBarTask>,
    /// Width of the (unclipped) slice content, computed in `layout`.
    ///
    /// Layout nodes no longer exist, so the old `slice_layout.bounds().width`
    /// is stored here instead.
    pub(super) items_width: f32,
}
impl GlobalState {
    pub(super) fn schedule(&mut self, task: MenuBarTask) {
        self.task = Some(task);
    }

    pub(super) fn task(&self) -> Option<MenuBarTask> {
        self.task
    }

    pub(super) fn clear_task(&mut self) {
        self.task = None;
    }
}

#[derive(Default, Debug)]
pub(super) struct MenuBarState {
    pub(super) global_state: GlobalState,
    pub(super) menu_state: MenuState,
}
impl MenuBarState {
    pub(super) fn open<'a, Message, Theme: Catalog, Renderer: renderer::Renderer>(
        &mut self,
        roots: &mut [Item<'a, Message, Theme, Renderer>],
        item_trees: &mut [Tree],
        item_layouts: impl Iterator<Item = Layout>,
        cursor: mouse::Cursor,
        shell: &mut Shell<'_, Message>,
    ) {
        if !self.global_state.open {
            self.global_state.open = true;
            self.menu_state.active = None;
        }

        try_open_menu(
            roots,
            &mut self.menu_state,
            item_trees,
            item_layouts,
            cursor,
            shell,
        );

        self.global_state.task = None;
    }

    pub(super) fn close<Message>(
        &mut self,
        item_trees: &mut [Tree],
        shell: &mut Shell<'_, Message>,
    ) {
        if self.global_state.pressed {
            return;
        }

        for item_tree in item_trees.iter_mut() {
            if item_tree.children.len() == 2 {
                let _ = item_tree.children.pop();
                shell.invalidate_layout();
            }
        }
        self.global_state.pressed = false;
        self.global_state.task = None;
        self.global_state.open = false;
        self.menu_state.active = None;
        shell.request_redraw();
    }
}

/// menu bar
#[must_use]
pub struct MenuBar<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    pub(super) roots: Vec<Item<'a, Message, Theme, Renderer>>,
    spacing: Pixels,
    padding: Padding,
    width: Length,
    height: Length,
    close_on_item_click: Option<bool>,
    close_on_background_click: Option<bool>,
    pub(super) global_parameters: GlobalParameters<'a, Theme>,
}
impl<'a, Message, Theme, Renderer> MenuBar<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    /// Creates a [`MenuBar`] with the given root items.
    pub fn new(mut roots: Vec<Item<'a, Message, Theme, Renderer>>) -> Self {
        for i in &mut roots {
            if let Some(m) = i.menu.as_mut() {
                m.axis = Axis::Vertical;
            }
        }

        Self {
            roots,
            spacing: Pixels::ZERO,
            padding: Padding::ZERO,
            width: Length::Shrink,
            height: Length::Shrink,
            close_on_item_click: None,
            close_on_background_click: None,
            global_parameters: GlobalParameters {
                safe_bounds_margin: 50.0,
                draw_path: DrawPath::FakeHovering,
                scroll_speed: ScrollSpeed {
                    line: 60.0,
                    pixel: 1.0,
                },
                close_on_item_click: false,
                close_on_background_click: false,
                class: Theme::default(),
            },
        }
    }

    /// Sets the width of the [`MenuBar`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`MenuBar`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the spacing of the [`MenuBar`].
    pub fn spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.spacing = spacing.into();
        self
    }

    /// Sets the margin of the safe bounds of the [`Menu`]s in the [`MenuBar`].
    ///
    /// Defines a rectangular safe area that extends each menu's background bounds by a margin.
    /// If the cursor moves outside this area, the menu will be closed.
    pub fn safe_bounds_margin(mut self, margin: f32) -> Self {
        self.global_parameters.safe_bounds_margin = margin;
        self
    }

    /// Sets the draw path option of the [`MenuBar`]
    pub fn draw_path(mut self, draw_path: DrawPath) -> Self {
        self.global_parameters.draw_path = draw_path;
        self
    }

    /// Sets the scroll speed of the [`Menu`]s in the [`MenuBar`]
    pub fn scroll_speed(mut self, scroll_speed: ScrollSpeed) -> Self {
        self.global_parameters.scroll_speed = scroll_speed;
        self
    }

    /// Sets the close on item click option of the [`MenuBar`]
    pub fn close_on_item_click(mut self, value: bool) -> Self {
        self.close_on_item_click = Some(value);
        self
    }

    /// Sets the close on background click option of the [`MenuBar`]
    pub fn close_on_background_click(mut self, value: bool) -> Self {
        self.close_on_background_click = Some(value);
        self
    }

    /// Sets the global default close on item click option
    pub fn close_on_item_click_global(mut self, value: bool) -> Self {
        self.global_parameters.close_on_item_click = value;
        self
    }

    /// Sets the global default close on background click option
    pub fn close_on_background_click_global(mut self, value: bool) -> Self {
        self.global_parameters.close_on_background_click = value;
        self
    }

    /// Sets the padding of the [`MenuBar`].
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the style of the [`MenuBar`].
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.global_parameters.class = (Box::new(style) as StyleFn<'a, Theme, Style>).into();
        self
    }

    /// Sets the class of the input of the [`MenuBar`].
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.global_parameters.class = class.into();
        self
    }
}
impl<Message, Theme, Renderer> Meta for MenuBar<'_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for MenuBar<'_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<MenuBarState>()
    }

    fn state(&self) -> tree::State {
        tree::State::Some(Box::<MenuBarState>::default())
    }

    /// tree: Tree{bar, \[item_tree...]}
    ///
    /// The children of the tree are created here (the tree starts empty).
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children_custom(&mut self.roots, |tree, item| item.diff(tree), Item::tree);
    }

    /// tree: Tree{bar, \[item_tree{stateless, \[widget_tree]}...]}
    ///
    /// Layout is stored in the tree: `tree.size` is the bar size, and each
    /// item tree holds the (parent relative) translation and (possibly
    /// clipped) size of its widget, while the widget tree itself sits at the
    /// item origin with the same size. Hence the layout of an item is also
    /// the layout of its widget.
    ///
    /// The items outside of the visible slice are still laid out, use
    /// `MenuBarState::menu_state.slice` to find the visible ones.
    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        // TODO: unify layout code with Menu

        let scroll_offset = tree
            .state
            .downcast_ref::<MenuBarState>()
            .menu_state
            .scroll_offset;

        // `flex::resolve` needs a contiguous slice of widget trees.
        let mut widget_trees: Vec<Tree> = tree
            .children
            .iter_mut()
            .map(|item_tree| std::mem::replace(&mut item_tree.children[0], Tree::empty()))
            .collect();

        let items_size = core_flex::resolve(
            core_flex::Axis::Horizontal,
            renderer,
            &Limits::new(
                Size {
                    width: 0.0,
                    height: limits.min.height,
                },
                Size {
                    width: f32::INFINITY,
                    height: limits.max.height,
                },
            ),
            Length::Shrink,
            // self.width,
            self.height,
            self.padding,
            self.spacing.0,
            Alignment::Center,
            &mut widget_trees,
            &mut self
                .roots
                .iter_mut()
                .map(|item| &mut item.item)
                .collect::<Vec<_>>(),
            &mut core_flex::Cache::default(),
        );

        // Hoist the widget layout into the item trees.
        for (item_tree, mut widget_tree) in tree.children.iter_mut().zip(widget_trees) {
            item_tree.size = widget_tree.size;
            item_tree.translation = widget_tree.translation;
            widget_tree.translation = Vector::ZERO;
            item_tree.children[0] = widget_tree;
        }

        #[cfg(feature = "debug_log")]
        debug!("menu::MenuBar::layout | items_size: {items_size:?}");

        let resolved_width = match self.width {
            Length::Fill
            | Length::FillPortion(_)
            | Length::Bounded { .. }
            | Length::Fluid(_) => items_size
                .width
                .min(limits.max.width)
                .max(limits.min.width),
            Length::Fixed(amount) => amount.min(limits.max.width).max(limits.min.width),
            Length::Shrink | Length::Fit => items_size.width,
        };

        if tree.children.is_empty() {
            tree.size = Size::new(resolved_width, items_size.height);
            return;
        }

        let lower_bound_rel = self.padding.left - scroll_offset;
        let upper_bound_rel = lower_bound_rel + resolved_width - self.padding.x();

        let slice = MenuSlice::from_bounds_rel(lower_bound_rel, upper_bound_rel, tree, |n| {
            n.translation.x
        });
        #[cfg(feature = "debug_log")]
        debug!("menu::MenuBar::layout | slice: {slice:?}");

        let items_width = if slice.start_index == slice.end_index {
            let item = &mut tree.children[slice.start_index];
            let start_offset = slice.lower_bound_rel - item.translation.x;
            let width = slice.upper_bound_rel - slice.lower_bound_rel;
            clip_item_x(item, width, start_offset);

            width
        } else {
            {
                let item = &mut tree.children[slice.start_index];
                let start_offset = slice.lower_bound_rel - item.translation.x;
                let width = item.size.width - start_offset;
                clip_item_x(item, width, start_offset);
            }

            {
                let item = &mut tree.children[slice.end_index];
                let width = slice.upper_bound_rel - item.translation.x;
                clip_item_x(item, width, 0.0);
            }

            items_size.width
        };

        // The slice is translated by the scroll offset
        for item in &mut tree.children {
            item.translation.x += scroll_offset;
        }

        let bar = tree.state.downcast_mut::<MenuBarState>();
        bar.menu_state.slice = slice;
        bar.global_state.items_width = items_width;

        tree.size = Size {
            width: resolved_width,
            height: items_size.height,
        };
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
        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBar::update", "");

        if tree.children.is_empty() {
            return;
        }

        let slice = tree.state.downcast_ref::<MenuBarState>().menu_state.slice;
        let slice_layouts = slice_layouts(layout, &tree.children, slice);

        let Tree {
            state,
            children: item_trees,
            ..
        } = tree;
        let bar = state.downcast_mut::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
        } = bar;

        let items_width = global_state.items_width;
        itl_iter_slice!(
            slice,
            self.roots;iter_mut,
            item_trees;iter_mut,
            slice_layouts.iter().copied()
        )
        .for_each(|((item, tree), layout)| {
            item.update(tree, event, layout, cursor, renderer, shell, viewport);
        });

        let bar_bounds = layout.bounds();
        // println!("bar_bounds: {:?}", bar_bounds);
        // println!("cursor: {:?}", cursor);
        // println!("cursor in bar_bounds: {:?}", cursor.is_over(bar_bounds));

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if cursor.is_over(bar_bounds) {
                    global_state.pressed = true;
                    if global_state.open {
                        schedule_close_on_click(
                            global_state,
                            &self.global_parameters,
                            slice,
                            &mut self.roots,
                            slice_layouts.iter().copied(),
                            cursor,
                            self.close_on_item_click,
                            self.close_on_background_click,
                        );
                    } else {
                        global_state.schedule(MenuBarTask::OpenOnClick);
                    }
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                global_state.pressed = false;

                if let Some(task) = global_state.task {
                    match task {
                        MenuBarTask::OpenOnClick => {
                            bar.open(
                                &mut self.roots,
                                item_trees,
                                slice_layouts.iter().copied(),
                                cursor,
                                shell,
                            );
                        }
                        MenuBarTask::CloseOnClick => {
                            bar.close(item_trees, shell);
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if global_state.open {
                    if cursor.is_over(bar_bounds) {
                        try_open_menu(
                            &mut self.roots,
                            bar_menu_state,
                            item_trees,
                            slice_layouts.iter().copied(),
                            cursor,
                            shell,
                        );
                        shell.capture_event();
                    } else {
                        bar.close(item_trees, shell);
                    }
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.is_over(bar_bounds) && items_width > layout.bounds().width
                // check if scrolling is on
                {
                    let scroll_speed = self.global_parameters.scroll_speed;
                    let delta_x = match delta {
                        mouse::ScrollDelta::Lines { x, .. } => x * scroll_speed.line,
                        mouse::ScrollDelta::Pixels { x, .. } => x * scroll_speed.pixel,
                    };

                    let min_offset = -(items_width - layout.bounds().width);

                    bar_menu_state.scroll_offset =
                        (bar_menu_state.scroll_offset + delta_x).clamp(min_offset, 0.0);
                    shell.invalidate_layout();
                    shell.request_redraw();
                    shell.capture_event();
                }
            }
            Event::Window(window::Event::Resized { .. }) => {
                if items_width > layout.bounds().width {
                    let min_offset = -(items_width - layout.bounds().width);

                    bar_menu_state.scroll_offset =
                        bar_menu_state.scroll_offset.clamp(min_offset, 0.0);
                }
                shell.invalidate_layout();
                shell.request_redraw();
            }
            _ => {}
        }

        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBar::update", "return | bar: {bar:?}");
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if tree.children.is_empty() {
            return;
        }

        let MenuBarState {
            menu_state: bar_menu_state,
            ..
        } = tree.state.downcast_ref::<MenuBarState>();

        let slice = bar_menu_state.slice;
        let slice_layouts = slice_layouts(layout, &tree.children, slice);

        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            itl_iter_slice!(slice, self.roots;iter_mut, tree.children;iter_mut, slice_layouts.iter().copied())
                .for_each(|((child, state), layout)| {
                    child.operate(state, layout, viewport, renderer, operation);
                });
        });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if tree.children.is_empty() {
            return mouse::Interaction::default();
        }

        let MenuBarState {
            menu_state: bar_menu_state,
            ..
        } = tree.state.downcast_ref::<MenuBarState>();

        let slice_layouts = slice_layouts(layout, &tree.children, bar_menu_state.slice);

        itl_iter_slice!(bar_menu_state.slice, self.roots;iter, tree.children;iter, slice_layouts.iter().copied())
            .map(|((item, tree), layout)| item.mouse_interaction(tree, layout, cursor, renderer))
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
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
        } = tree.state.downcast_ref::<MenuBarState>();

        let slice = bar_menu_state.slice;

        let styling = theme.style(&self.global_parameters.class, Status::Active);
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border: styling.bar_border,
                shadow: styling.bar_shadow,
                ..Default::default()
            },
            styling.bar_background,
        );

        if let (DrawPath::Backdrop, true, Some(active)) = (
            &self.global_parameters.draw_path,
            global_state.open,
            bar_menu_state.active,
        ) {
            let active_bounds = layout
                .iter(&tree.children)
                .nth(active)
                .expect(
                    "Index is not within the menu bar layout. \
                    This should not happen, please report this issue",
                )
                .0
                .bounds();

            renderer.fill_quad(
                renderer::Quad {
                    bounds: active_bounds,
                    border: styling.path_border,
                    ..Default::default()
                },
                styling.path,
            );
        }

        if tree.children.is_empty() {
            return;
        }

        let slice_layouts = slice_layouts(layout, &tree.children, slice);

        renderer.with_layer(
            Rectangle {
                x: layout.bounds().x + self.padding.left,
                y: layout.bounds().y + self.padding.top,
                width: layout.bounds().width - self.padding.x(),
                height: layout.bounds().height - self.padding.y(),
            },
            |r| {
                itl_iter_slice!(slice, self.roots;iter, tree.children;iter, slice_layouts.iter().copied())
                .for_each(|((item, tree), layout)| {
                    item.draw(tree, r, theme, style, layout, cursor, viewport);
                });
            },
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
        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBar::overlay", "");
        let bar = tree.state.downcast_mut::<MenuBarState>();

        if bar.global_state.open {
            #[cfg(feature = "debug_log")]
            debug!(target:"menu::MenuBar::overlay", "return | Menu Overlay");
            vec![
                MenuBarOverlay {
                    menu_bar: self,
                    layout,
                    translation,
                    tree,
                }
                .overlay_element(renderer, window),
            ]
        } else {
            #[cfg(feature = "debug_log")]
            debug!(target:"menu::MenuBar::overlay", "state not open | try return root overlays");
            if tree.children.is_empty() {
                return Vec::new();
            }

            let slice = bar.menu_state.slice;
            let slice_layouts = slice_layouts(layout, &tree.children, slice);

            let Tree {
                children: item_trees,
                ..
            } = tree;

            itl_iter_slice!(slice, self.roots;iter_mut, item_trees;iter_mut, slice_layouts.iter().copied())
                .flat_map(|((item, item_tree), item_layout)| {
                    item.item.overlay(
                        &mut item_tree.children[0],
                        item_layout,
                        renderer,
                        viewport,
                        translation,
                        window,
                    )
                })
                .collect()
        }
    }
}

/// Clips the width of an item in place.
///
/// The widget is clipped (as a layout node used to be), and the resulting
/// size and translation are hoisted into the item tree, so the layout of the
/// item is also the layout of its widget.
fn clip_item_x(item: &mut Tree, width: f32, offset: f32) {
    clip_node_x(&mut item.children[0], width, offset);

    let widget = &mut item.children[0];
    item.size = widget.size;
    item.translation = item.translation + widget.translation;
    widget.translation = Vector::ZERO;
}

/// The layouts of the items in the visible `slice` (inclusive).
fn slice_layouts(layout: Layout, item_trees: &[Tree], slice: MenuSlice) -> Vec<Layout> {
    layout
        .iter(item_trees)
        .skip(slice.start_index)
        .take(slice.end_index + 1 - slice.start_index)
        .map(|(layout, _)| layout)
        .collect()
}
impl<'a, Message, Theme, Renderer> From<MenuBar<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + renderer::Renderer,
{
    fn from(value: MenuBar<'a, Message, Theme, Renderer>) -> Self {
        Widget::_boxed(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_widget::text::Text;

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    enum TestMessage {
        Action1,
        Action2,
    }

    type TestMenuBar<'a> = MenuBar<'a, TestMessage, iced_widget::Theme, iced_widget::Renderer>;

    fn text_item(label: &'static str) -> Item<'static, TestMessage, iced_widget::Theme, iced_widget::Renderer> {
        let element: Element<'static, TestMessage, iced_widget::Theme, iced_widget::Renderer> =
            Widget::_boxed(Text::new(label));
        Item::new(element)
    }

    #[test]
    fn menu_bar_new_creates_instance() {
        let items = vec![text_item("File"), text_item("Edit")];

        let menu_bar = TestMenuBar::new(items);
        assert_eq!(menu_bar.roots.len(), 2);
        assert_eq!(menu_bar.spacing, Pixels::ZERO);
        assert_eq!(menu_bar.width, Length::Shrink);
        assert_eq!(menu_bar.height, Length::Shrink);
    }

    #[test]
    fn menu_bar_width_sets_value() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items).width(Length::Fill);
        assert_eq!(menu_bar.width, Length::Fill);
    }

    #[test]
    fn menu_bar_height_sets_value() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items).height(Length::Fixed(50.0));
        assert_eq!(menu_bar.height, Length::Fixed(50.0));
    }

    #[test]
    fn menu_bar_spacing_sets_value() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items).spacing(Pixels(10.0));
        assert_eq!(menu_bar.spacing, Pixels(10.0));
    }

    #[test]
    fn menu_bar_padding_sets_value() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items).padding(Padding::new(5.0));
        assert_eq!(menu_bar.padding, Padding::new(5.0));
    }

    #[test]
    fn menu_bar_safe_bounds_margin_sets_value() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items).safe_bounds_margin(100.0);
        #[allow(clippy::float_cmp)]
        {
            assert_eq!(menu_bar.global_parameters.safe_bounds_margin, 100.0);
        }
    }

    #[test]
    fn menu_bar_close_on_item_click_sets_value() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items).close_on_item_click(true);
        assert_eq!(menu_bar.close_on_item_click, Some(true));
    }

    #[test]
    fn menu_bar_close_on_background_click_sets_value() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items).close_on_background_click(true);
        assert_eq!(menu_bar.close_on_background_click, Some(true));
    }

    #[test]
    fn menu_bar_close_on_item_click_global_sets_value() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items).close_on_item_click_global(true);
        assert!(menu_bar.global_parameters.close_on_item_click);
    }

    #[test]
    fn menu_bar_close_on_background_click_global_sets_value() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items).close_on_background_click_global(true);
        assert!(menu_bar.global_parameters.close_on_background_click);
    }

    #[test]
    fn menu_bar_with_multiple_items() {
        let items = vec![
            text_item("File"),
            text_item("Edit"),
            text_item("View"),
            text_item("Help"),
        ];

        let menu_bar = TestMenuBar::new(items);
        assert_eq!(menu_bar.roots.len(), 4);
    }

    #[test]
    fn menu_bar_chained_configuration() {
        let items = vec![text_item("File")];

        let menu_bar = TestMenuBar::new(items)
            .width(Length::Fill)
            .height(Length::Fixed(40.0))
            .spacing(Pixels(5.0))
            .padding(Padding::new(10.0))
            .safe_bounds_margin(75.0)
            .close_on_item_click(true)
            .close_on_background_click(true);

        assert_eq!(menu_bar.width, Length::Fill);
        assert_eq!(menu_bar.height, Length::Fixed(40.0));
        assert_eq!(menu_bar.spacing, Pixels(5.0));
        assert_eq!(menu_bar.padding, Padding::new(10.0));
        #[allow(clippy::float_cmp)]
        {
            assert_eq!(menu_bar.global_parameters.safe_bounds_margin, 75.0);
        }
        assert_eq!(menu_bar.close_on_item_click, Some(true));
        assert_eq!(menu_bar.close_on_background_click, Some(true));
    }

    #[test]
    fn menu_bar_tag_returns_state_tag() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items);

        let tag = Widget::<TestMessage, iced_widget::Theme, iced_widget::Renderer>::tag(&menu_bar);
        assert_eq!(tag, tree::Tag::of::<MenuBarState>());
    }

    #[test]
    fn menu_bar_children_returns_item_trees() {
        let items = vec![text_item("File"), text_item("Edit")];

        let mut menu_bar = TestMenuBar::new(items);
        let mut tree = Tree::new(&menu_bar);
        Widget::<TestMessage, iced_widget::Theme, iced_widget::Renderer>::diff(
            &mut menu_bar,
            &mut tree,
        );
        assert_eq!(tree.children.len(), 2);
    }

    #[test]
    fn menu_bar_size_returns_configured_size() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items)
            .width(Length::Fill)
            .height(Length::Fixed(40.0));

        let size =
            Widget::<TestMessage, iced_widget::Theme, iced_widget::Renderer>::size(&menu_bar);
        assert_eq!(size.width, Length::Fill);
        assert_eq!(size.height, Length::Fixed(40.0));
    }

    #[test]
    fn menu_bar_converts_to_element() {
        let items = vec![text_item("File")];
        let menu_bar = TestMenuBar::new(items);
        let _element: Element<TestMessage, iced_widget::Theme, iced_widget::Renderer> =
            menu_bar.into();
    }
}
