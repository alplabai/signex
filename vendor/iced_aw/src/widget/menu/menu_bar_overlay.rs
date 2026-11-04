#![allow(clippy::unwrap_used)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::enum_glob_use)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::similar_names)]

use iced_core::{
    Event, Layout, Rectangle, Shell, Size, Vector, Widget,
    layout::Limits,
    mouse, overlay, renderer, shell,
    time::Instant,
    widget::{Operation, Tree},
    window,
};

use super::{common::*, menu_bar::*, menu_tree::*};
use crate::style::{Status, menu_bar::*};

#[cfg(feature = "debug_log")]
use log::{debug, trace, warn};

pub(super) struct MenuBarOverlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    pub(super) menu_bar: &'b mut MenuBar<'a, Message, Theme, Renderer>,
    /// Layout of the menu bar itself (without `translation` applied)
    pub(super) layout: Layout,
    pub(super) translation: Vector,
    /// Tree{ bar, [item_tree...] }
    pub(super) tree: &'b mut Tree,
}
impl<'a, 'b, Message, Theme, Renderer> MenuBarOverlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    /// Lays out the menus and turns this into an overlay element.
    ///
    /// Overlays no longer have a separate layout step: the layout has to be
    /// computed up front, when the overlay is created (`Widget::overlay`).
    pub(super) fn overlay_element(
        mut self,
        renderer: &Renderer,
        window: Size,
    ) -> overlay::Element<'b, Message, Theme, Renderer>
    where
        'a: 'b,
        Message: 'b,
        Theme: 'b,
        Renderer: 'b,
    {
        let menu_layouts = self.layout_menus(renderer, window);
        overlay::Element::new(Box::new(LaidOutMenuBarOverlay {
            inner: self,
            menu_layouts,
            window,
        }))
    }

    /// Absolute layouts of the root items of the bar (translation applied),
    /// indexed by absolute item index.
    fn root_layouts(&self) -> Vec<Layout> {
        let translation = self.translation;
        self.layout
            .iter(&self.tree.children)
            .map(|(l, _)| l.move_to(l.position() + translation))
            .collect()
    }

    /// out: [ menu_layout, ...] one absolute layout per open menu, outermost first
    fn layout_menus(&mut self, renderer: &Renderer, bounds: Size) -> Vec<Layout> {
        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBarOverlay::layout", "");

        let mut menu_layouts = vec![];

        let root_layouts = self.root_layouts();

        let bar = self.tree.state.downcast_ref::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
        } = bar;

        if !global_state.open {
            return menu_layouts;
        }

        let Some(active) = bar_menu_state.active else {
            return menu_layouts;
        };

        let active_root = &mut self.menu_bar.roots[active];
        let active_tree = &mut self.tree.children[active]; // item_tree: Tree{ stateless, [ widget_tree, menu_tree ] }
        let parent_bounds = root_layouts[active].bounds();

        fn rec<Message, Theme: Catalog, Renderer: renderer::Renderer>(
            renderer: &Renderer,
            item: &mut Item<'_, Message, Theme, Renderer>,
            tree: &mut Tree,
            menu_layouts: &mut Vec<Layout>,
            parent_bounds: Rectangle,
            parent_direction: (Direction, Direction),
            viewport: &Rectangle,
        ) {
            if let Some(menu) = item.menu.as_mut() {
                let menu_tree = &mut tree.children[1];

                // The menu stores the sizes/translations of its items in `menu_tree`
                // and returns its own absolute layout.
                let (menu_layout, direction) = menu.layout(
                    menu_tree,
                    renderer,
                    &Limits::new(Size::ZERO, Size::INFINITE),
                    parent_bounds,
                    parent_direction,
                    viewport,
                );
                menu_layouts.push(menu_layout);

                let menu_state = menu_tree.state.downcast_ref::<MenuState>();

                if let Some(active) = menu_state.active {
                    let next_item = &mut menu.items[active];
                    let next_parent_bounds = menu_layout
                        .iter(&menu_tree.children)
                        .nth(active)
                        .unwrap()
                        .0
                        .bounds();
                    let next_tree = &mut menu_tree.children[active];
                    rec(
                        renderer,
                        next_item,
                        next_tree,
                        menu_layouts,
                        next_parent_bounds,
                        direction,
                        viewport,
                    );
                }
            }
        }

        let parent_direction = {
            let hcenter = bounds.width / 2.0;
            let vcenter = bounds.height / 2.0;

            let phcenter = parent_bounds.x + parent_bounds.width / 2.0;
            let pvcenter = parent_bounds.y + parent_bounds.height / 2.0;

            (
                if phcenter < hcenter {
                    Direction::Positive
                } else {
                    Direction::Negative
                },
                if pvcenter < vcenter {
                    Direction::Positive
                } else {
                    Direction::Negative
                },
            )
        };

        rec(
            renderer,
            active_root,
            active_tree,
            &mut menu_layouts,
            parent_bounds,
            parent_direction,
            &Rectangle::with_size(bounds),
        );

        menu_layouts
    }
}

/// A [`MenuBarOverlay`] whose menus have been laid out.
pub(super) struct LaidOutMenuBarOverlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    inner: MenuBarOverlay<'a, 'b, Message, Theme, Renderer>,
    /// Absolute layouts of the open menus, outermost first
    menu_layouts: Vec<Layout>,
    window: Size,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for LaidOutMenuBarOverlay<'_, '_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: renderer::Renderer,
{
    #[allow(unused_results)]
    fn update(
        &mut self,
        event: &Event,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBarOverlay::update", "");

        let root_layouts = self.inner.root_layouts();
        let translation = self.inner.translation;
        let viewport = Rectangle::with_size(self.window);
        let bar_bounds = self.inner.layout.bounds() + translation;

        let bar = self.inner.tree.state.downcast_mut::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
        } = bar;
        let slice = bar_menu_state.slice;

        if !global_state.open {
            return;
        }

        let Some(active) = bar_menu_state.active else {
            return;
        };

        let parent_bounds = root_layouts[active].bounds();
        let mut menu_layouts = self.menu_layouts.iter().copied(); // [menu_layout...]

        let active_root = &mut self.inner.menu_bar.roots[active];
        let active_tree = &mut self.inner.tree.children[active];
        let mut prev_bounds_list = vec![bar_bounds];

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                global_state.pressed = true;
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                global_state.pressed = false;
                shell.request_redraw();
            }
            Event::Window(window::Event::Resized { .. }) => {
                bar.close(self.inner.tree.children.as_mut_slice(), shell);
                return;
            }
            _ => {}
        }

        #[rustfmt::skip]
        fn rec<'a, Message, Theme: Catalog, Renderer: renderer::Renderer>(
            global_state: &mut GlobalState,
            global_parameters: &GlobalParameters<'a, Theme>,
            tree: &mut Tree,
            item: &mut Item<'a, Message, Theme, Renderer>,
            event: &Event,
            layout_iter: &mut impl Iterator<Item = Layout>,
            cursor: mouse::Cursor,
            renderer: &Renderer,
            shell: &mut Shell<'_, Message>,
            parent_bounds: Rectangle,
            viewport: &Rectangle,
            prev_bounds_list: &mut Vec<Rectangle>,
            prev_active: &mut Index,
        ) -> RecEvent {
            #[cfg(feature = "debug_log")]
            debug!(target:"menu::MenuBarOverlay::update", "rec");

            let Some(menu) = item.menu.as_mut() else {
                return RecEvent::Close;
            };
            let menu_tree = &mut tree.children[1];

            let Some(menu_layout) = layout_iter.next() else {
                return RecEvent::Close;
            }; // menu_node: Node{inf, [ slice_node, items_bounds, offset_bounds]}

            // menu_layout: absolute layout of the menu items area; the items'
            // sizes/translations are stored in `menu_tree.children`
            let items_bounds = menu_layout.bounds();
            let background_bounds = pad_rectangle(items_bounds, menu.padding);

            prev_bounds_list.push(background_bounds);

            let menu_state = menu_tree.state.downcast_mut::<MenuState>();

            let rec_event = if let Some(active) = menu_state.active {
                let next_parent_bounds = menu_layout
                    .iter(&menu_tree.children)
                    .nth(active)
                    .map(|(layout, _)| layout)
                    .expect(
                        "Index (in slice space) is not within the slice layout. \
                        This should not happen, please report this issue"
                    )
                    .bounds();
                let next_tree = &mut menu_tree.children[active];
                let next_item = &mut menu.items[active];

                rec(
                    global_state,
                    global_parameters,
                    next_tree,
                    next_item,
                    event,
                    layout_iter,
                    cursor,
                    renderer,
                    shell,
                    next_parent_bounds,
                    viewport,
                    prev_bounds_list,
                    &mut menu_state.active,
                )
            } else if cursor == mouse::Cursor::Unavailable{
                #[cfg(feature = "debug_log")]
                debug!(target:"menu::MenuBarOverlay::update", "rec | cursor is unavailable");
                RecEvent::Event
            } else {
                RecEvent::Close
            };

            prev_bounds_list.pop();

            #[cfg(feature = "debug_log")]
            {
                debug!(target:"menu::MenuBarOverlay::update", "");
                debug!(target:"menu::MenuBarOverlay::update", "menu_state.active: {:?}", menu_state.active);
                debug!(target:"menu::MenuBarOverlay::update", "menu_state.slice: {:?}", menu_state.slice);
                debug!(target:"menu::MenuBarOverlay::update", "rec_event: {rec_event:?}");
                debug!(target:"menu::MenuBarOverlay::update", "event: {event:?}");
                debug!(target:"menu::MenuBarOverlay::update", "cursor: {cursor:?}");
                debug!(target:"menu::MenuBarOverlay::update", "cursor is over background bounds: {:?}", cursor.is_over(background_bounds));
                debug!(target:"menu::MenuBarOverlay::update", "shell.is_event_captured(): {:?}", shell.is_event_captured());
            }

            menu.update(
                global_state,
                global_parameters,
                rec_event,
                menu_tree,
                event,
                menu_layout,
                cursor,
                renderer,
                shell,
                viewport,
                parent_bounds,
                prev_bounds_list,
                prev_active,
            )
        }

        let re = rec(
            global_state,
            &self.inner.menu_bar.global_parameters,
            active_tree,
            active_root,
            event,
            &mut menu_layouts,
            cursor,
            renderer,
            shell,
            parent_bounds,
            &viewport,
            &mut prev_bounds_list,
            &mut bar_menu_state.active,
        );

        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBarOverlay::update", "re: {re:?}");

        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
        ) && matches!(global_state.task(), Some(MenuBarTask::CloseOnClick))
        {
            bar.close(self.inner.tree.children.as_mut_slice(), shell);
            return;
        }

        match re {
            RecEvent::Event => {
                let redraw_event = Event::Window(window::Event::RedrawRequested(Instant::now()));
                let mut fake_messages = shell::Bus::new();
                let mut fake_shell = shell.local(&mut fake_messages);

                let MenuBarOverlay {
                    menu_bar,
                    layout,
                    tree,
                    ..
                } = &mut self.inner;
                let draw_path = menu_bar.global_parameters.draw_path;
                let cursor = match draw_path {
                    DrawPath::FakeHovering => {
                        let center = parent_bounds.center();
                        mouse::Cursor::Available(center - translation)
                    }
                    DrawPath::Backdrop => mouse::Cursor::Unavailable,
                };

                #[cfg(feature = "debug_log")]
                debug!(target:"menu::MenuBarOverlay::update", "calling update_items() on MenuBar");
                let item_layouts: Vec<Layout> =
                    layout.iter(&tree.children).map(|(l, _)| l).collect();
                itl_iter_slice!(
                    slice,
                    menu_bar.roots;iter_mut,
                    tree.children;iter_mut,
                    item_layouts[slice.start_index..=slice.end_index].iter().copied()
                )
                .for_each(|((item, tree), layout)| {
                    item.update(
                        tree,
                        &redraw_event,
                        layout,
                        cursor,
                        renderer,
                        &mut fake_shell,
                        &viewport,
                    );
                });
            }
            RecEvent::Close => {
                if !cursor.is_over(bar_bounds) {
                    bar.close(self.inner.tree.children.as_mut_slice(), shell);
                }

                #[cfg(feature = "debug_log")]
                debug!(target:"menu::MenuBarOverlay::update", "RecEvent::Close | MenuBar should process the event");

                assert!(
                    !shell.is_event_captured(),
                    "MenuBarOverlay::update() | RecEvent::Close | Returning"
                );
                // let the menu bar process the event
            }
            RecEvent::None => {
                #[cfg(feature = "debug_log")]
                debug!(target:"menu::MenuBarOverlay::update", "RecEvent::None | MenuBar should process the event");

                assert!(
                    !shell.is_event_captured(),
                    "MenuBarOverlay::update() | RecEvent::None | Returning"
                );
                // let the menu bar process the event
            }
        }

        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBarOverlay::update", "returning | shell.is_layout_invalid(): {:?}", shell.is_layout_invalid());
    }

    fn mouse_interaction(&self, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBarOverlay::mouse_interaction", "");

        let bar = self.inner.tree.state.downcast_ref::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
        } = bar;

        if !global_state.open {
            return mouse::Interaction::default();
        }

        let Some(active) = bar_menu_state.active else {
            return mouse::Interaction::default();
        };

        let mut menu_layouts = self.menu_layouts.iter().copied(); // [menu_layout...]

        let active_root = &self.inner.menu_bar.roots[active];
        let active_tree = &self.inner.tree.children[active];

        fn rec<'a, Message, Theme: Catalog, Renderer: renderer::Renderer>(
            tree: &Tree,
            item: &Item<'a, Message, Theme, Renderer>,
            layout_iter: &mut impl Iterator<Item = Layout>,
            cursor: mouse::Cursor,
            renderer: &Renderer,
        ) -> mouse::Interaction {
            let Some(menu) = item.menu.as_ref() else {
                return mouse::Interaction::default();
            };
            let menu_tree = &tree.children[1];

            let Some(menu_layout) = layout_iter.next() else {
                #[cfg(feature = "debug_log")]
                warn!(target:"menu::MenuBarOverlay::mouse_interaction", "menu exists, but menu_layout is None");
                return mouse::Interaction::default();
            }; // menu_node: Node{inf, [ slice_node, items_bounds, offset_bounds]}

            let menu_state = menu_tree.state.downcast_ref::<MenuState>();

            let i = menu.mouse_interaction(menu_tree, menu_layout, cursor, renderer);

            menu_state.active.map_or(i, |active| {
                let next_tree = &menu_tree.children[active];
                let next_item = &menu.items[active];
                rec(next_tree, next_item, layout_iter, cursor, renderer).max(i)
            })
        }

        rec(
            active_tree,
            active_root,
            &mut menu_layouts,
            cursor,
            renderer,
        )
    }

    fn operate(&mut self, renderer: &Renderer, operation: &mut dyn Operation) {
        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBarOverlay::operate", "");

        let viewport = Rectangle::with_size(self.window);

        let bar = self.inner.tree.state.downcast_ref::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
        } = bar;

        if !global_state.open {
            return;
        }

        let Some(active) = bar_menu_state.active else {
            return;
        };

        let mut menu_layouts = self.menu_layouts.iter().copied(); // [menu_layout...]

        let active_root = &mut self.inner.menu_bar.roots[active];
        let active_tree = &mut self.inner.tree.children[active];

        fn rec<'a, Message, Theme: Catalog, Renderer: renderer::Renderer>(
            tree: &mut Tree,
            item: &mut Item<'a, Message, Theme, Renderer>,
            layout_iter: &mut impl Iterator<Item = Layout>,
            renderer: &Renderer,
            operation: &mut dyn Operation,
            viewport: &Rectangle,
        ) {
            let Some(menu) = item.menu.as_mut() else {
                return;
            };

            let menu_tree = &mut tree.children[1];

            let Some(menu_layout) = layout_iter.next() else {
                #[cfg(feature = "debug_log")]
                warn!(target:"menu::MenuBarOverlay::operate", "menu exists, but menu_layout is None");
                return;
            };

            menu.operate(menu_tree, menu_layout, renderer, operation);

            operation.container(None, menu_layout.bounds(), viewport);
            operation.traverse(&mut |operation| {
                menu.items
                    .iter_mut() // [Item...]
                    .zip(menu_tree.children.iter_mut()) // [item_tree...] // [widget_node...]
                    .for_each(|(child, state)| {
                        rec(state, child, layout_iter, renderer, operation, viewport);
                    });
            });
        }

        rec(
            active_tree,
            active_root,
            &mut menu_layouts,
            renderer,
            operation,
            &viewport,
        );
    }

    fn overlay<'c>(
        &'c mut self,
        renderer: &Renderer,
    ) -> Vec<overlay::Element<'c, Message, Theme, Renderer>> {
        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBarOverlay::overlay", "");

        let window = self.window;
        let viewport = Rectangle::with_size(window);
        let translation = self.inner.translation;
        let root_layouts_raw: Vec<Layout> = self
            .inner
            .layout
            .iter(&self.inner.tree.children)
            .map(|(l, _)| l)
            .collect();

        let Tree {
            state,
            children: item_trees,
            ..
        } = &mut *self.inner.tree;
        let bar = state.downcast_ref::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
        } = bar;
        let slice = bar_menu_state.slice;

        if !global_state.open {
            return Vec::new();
        }

        let Some(active) = bar_menu_state.active else {
            return Vec::new();
        };

        let mut menu_layouts = self.menu_layouts.iter().copied(); // [menu_layout...]

        fn rec<'a, 'b, Message, Theme: Catalog, Renderer: renderer::Renderer>(
            items: &'b mut [Item<'a, Message, Theme, Renderer>],
            menu_tree: &'b mut Tree, // Tree{ menu_state, [item_tree...] }
            menu_layouts: &mut impl Iterator<Item = Layout>, // [menu_layout...]
            overlays: &mut Vec<overlay::Element<'b, Message, Theme, Renderer>>,
            renderer: &Renderer,
            viewport: &Rectangle,
            window: Size,
        ) {
            #[cfg(feature = "debug_log")]
            debug!(target:"menu::MenuBarOverlay::overlay", "rec");

            let menu_state = menu_tree.state.downcast_mut::<MenuState>();
            let menu_layout = menu_layouts.next().unwrap();
            let slice = menu_state.slice;
            let item_layouts: Vec<Layout> = menu_layout
                .iter(&menu_tree.children)
                .map(|(l, _)| l)
                .collect();
            let item_layouts = &item_layouts[slice.start_index..=slice.end_index];

            if let Some(active) = menu_state.active {
                #[cfg(feature = "debug_log")]
                debug!(target:"menu::MenuBarOverlay::overlay", "rec | active");

                let mut next = None;

                for (i, ((item, item_tree), item_layout)) in itl_iter_slice_enum!(
                    slice,
                    items;iter_mut,
                    menu_tree.children;iter_mut,
                    item_layouts.iter().copied()
                ) {
                    #[cfg(feature = "debug_log")]
                    trace!(target:"menu::MenuBarOverlay::overlay", "rec | i: {i}");

                    let Item {
                        item: item_widget,
                        menu: item_menu,
                        ..
                    } = item;

                    let item_widget_tree = if i == active {
                        #[cfg(feature = "debug_log")]
                        debug!(target:"menu::MenuBarOverlay::overlay", "rec | i == active {active}");
                        let [item_widget_tree, item_menu_tree] = item_tree.children.as_mut_slice()
                        else {
                            #[cfg(feature = "debug_log")]
                            warn!(target:"menu::MenuBarOverlay::overlay", "rec | menu_state.active is Some, but there's no state tree for it");
                            continue;
                        };
                        next = Some((item_menu.as_mut().unwrap(), item_menu_tree));
                        item_widget_tree
                    } else {
                        &mut item_tree.children.as_mut_slice()[0]
                    };

                    let item_overlays = item_widget.overlay(
                        item_widget_tree,
                        item_layout,
                        renderer,
                        viewport,
                        Vector::ZERO,
                        window,
                    );
                    #[cfg(feature = "debug_log")]
                    debug!(target:"menu::MenuBarOverlay::overlay", "rec | active | i: {i} | {} overlays", item_overlays.len());
                    overlays.extend(item_overlays);
                }

                if let Some((next_menu, next_menu_tree)) = next {
                    #[cfg(feature = "debug_log")]
                    debug!(target:"menu::MenuBarOverlay::overlay", "rec | next");
                    rec(
                        &mut next_menu.items,
                        next_menu_tree,
                        menu_layouts,
                        overlays,
                        renderer,
                        viewport,
                        window,
                    );
                }
            } else {
                #[cfg(feature = "debug_log")]
                debug!(target:"menu::MenuBarOverlay::overlay", "rec | no active");

                #[cfg(feature = "debug_log")]
                let mut count = slice.start_index;

                for ((item, item_tree), item_layout) in itl_iter_slice!(
                    slice,
                    items;iter_mut,
                    menu_tree.children;iter_mut,
                    item_layouts.iter().copied()
                ) {
                    #[cfg(feature = "debug_log")]
                    trace!(target:"menu::MenuBarOverlay::overlay", "rec | i: {count}");

                    let Item {
                        item: item_widget, ..
                    } = item;

                    let item_overlays = item_widget.overlay(
                        &mut item_tree.children[0],
                        item_layout,
                        renderer,
                        viewport,
                        Vector::ZERO,
                        window,
                    );
                    #[cfg(feature = "debug_log")]
                    debug!(target:"menu::MenuBarOverlay::overlay", "rec | no active | i: {count} | {} overlays", item_overlays.len());
                    overlays.extend(item_overlays);

                    #[cfg(feature = "debug_log")]
                    {
                        count += 1;
                    }
                }
            }
        }

        let mut overlays = vec![];
        let mut next = None;

        for (i, ((item, item_tree), item_layout)) in itl_iter_slice_enum!(
            slice,
            self.inner.menu_bar.roots;iter_mut,
            item_trees;iter_mut,
            root_layouts_raw[slice.start_index..=slice.end_index].iter().copied()
        ) {
            #[cfg(feature = "debug_log")]
            trace!(target:"menu::MenuBarOverlay::overlay", "root | i: {i}");

            let Item {
                item: item_widget,
                menu: item_menu,
                ..
            } = item;
            let [item_widget_tree, item_menu_tree] = item_tree.children.as_mut_slice() else {
                continue;
            };

            if i == active
                && let Some(menu) = item_menu.as_mut()
            {
                next = Some((menu, item_menu_tree));
            }
            let item_overlays = item_widget.overlay(
                item_widget_tree,
                item_layout,
                renderer,
                &viewport,
                translation,
                window,
            );
            #[cfg(feature = "debug_log")]
            debug!(target:"menu::MenuBarOverlay::overlay", "root | i: {i} | {} overlays", item_overlays.len());
            overlays.extend(item_overlays);
        }

        if let Some((next_menu, next_menu_tree)) = next {
            rec(
                &mut next_menu.items,
                next_menu_tree,
                &mut menu_layouts,
                &mut overlays,
                renderer,
                &viewport,
                window,
            );
        }

        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBarOverlay::overlay", "return | {} overlays", overlays.len());
        overlays
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        cursor: mouse::Cursor,
    ) {
        #[cfg(feature = "debug_log")]
        debug!(target:"menu::MenuBarOverlay::draw", "");

        let bar = self.inner.tree.state.downcast_ref::<MenuBarState>();
        let MenuBarState {
            global_state,
            menu_state: bar_menu_state,
        } = bar;

        if !global_state.open {
            return;
        }

        let Some(active) = bar_menu_state.active else {
            return;
        };

        let viewport = Rectangle::with_size(self.window);
        let mut menu_layouts = self.menu_layouts.iter().copied(); // [menu_layout...]

        let active_root = &self.inner.menu_bar.roots[active];
        let active_tree = &self.inner.tree.children[active];

        fn rec<'a, Message, Theme: Catalog, Renderer: renderer::Renderer>(
            global_parameters: &GlobalParameters<'a, Theme>,
            tree: &Tree,
            item: &Item<'a, Message, Theme, Renderer>,
            layout_iter: &mut impl Iterator<Item = Layout>,
            cursor: mouse::Cursor,
            renderer: &mut Renderer,
            theme: &Theme,
            style: &renderer::Style,
            theme_style: &Style,
            viewport: &Rectangle,
        ) {
            let Some(menu) = item.menu.as_ref() else {
                return;
            };

            let menu_tree = &tree.children[1];

            let Some(menu_layout) = layout_iter.next() else {
                #[cfg(feature = "debug_log")]
                warn!(target:"menu::MenuBarOverlay::draw", "menu exists, but menu_layout is None");
                return;
            }; // menu_node: Node{inf, [ slice_node, items_bounds, offset_bounds]}

            let menu_state = menu_tree.state.downcast_ref::<MenuState>();

            menu.draw(
                &global_parameters.draw_path,
                menu_tree,
                renderer,
                theme,
                style,
                theme_style,
                menu_layout,
                cursor,
                viewport,
            );

            if let Some(active) = menu_state.active {
                let next_tree = &menu_tree.children[active];
                let next_item = &menu.items[active];

                renderer.with_layer(*viewport, |r| {
                    rec(
                        global_parameters,
                        next_tree,
                        next_item,
                        layout_iter,
                        cursor,
                        r,
                        theme,
                        style,
                        theme_style,
                        viewport,
                    );
                });
            }
        }

        let theme_style = theme.style(&self.inner.menu_bar.global_parameters.class, Status::Active);

        rec(
            &self.inner.menu_bar.global_parameters,
            active_tree,
            active_root,
            &mut menu_layouts,
            cursor,
            renderer,
            theme,
            style,
            &theme_style,
            &viewport,
        );
    }
}
