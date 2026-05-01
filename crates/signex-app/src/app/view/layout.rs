use iced::widget::{canvas, column, container, row};
use iced::{Element, Length};

use super::*;

impl Signex {
    /// Custom chrome for the borderless main window. Replaces the OS
    /// title bar with a 36 px strip:
    ///
    /// `[wordmark + menus] [drag] [search bar] [drag] [min│max│×]`
    ///
    /// The drag zones are the only mouse-area clickable regions — menu
    /// buttons, search, and window controls keep their own click
    /// handlers. Double-click on a drag zone toggles maximize.
    pub(super) fn view_main_window_chrome<'a>(
        &self,
        menu_row: Element<'a, Message>,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'a, Message> {
        use iced::widget::{Space, button, container, mouse_area, row, svg, text};
        use iced::{Alignment, Background, Border, Color, Length};

        // Window-control SVG icons resolved through `crate::icons` so the
        // accent sentinel in each SVG tints to the active theme at
        // fetch time.
        let theme_id = self.ui_state.theme_id;
        let h_min = crate::icons::icon_chrome_window_min(theme_id);
        let h_max = crate::icons::icon_chrome_window_max(theme_id);
        let h_close = crate::icons::icon_chrome_window_close(theme_id);
        let h_search = crate::icons::icon_chrome_search(theme_id);

        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        let hover_c = crate::styles::ti(tokens.hover);
        let search_bg = crate::styles::ti(tokens.panel_bg);
        let search_border = crate::styles::ti(tokens.border);
        // Windows-native destructive red for the close hover — overrides
        // the theme hover so close reads as destructive at a glance.
        let close_hover = Color::from_rgba(0.78, 0.22, 0.22, 1.0);
        let btn_h = crate::menu_bar::MENU_BAR_HEIGHT;

        let chrome_btn = |handle: svg::Handle,
                          msg: Message,
                          hover_bg: Color,
                          hover_icon: Color|
         -> Element<'static, Message> {
            // 14×14 brings the X / – / □ glyphs up to native-Windows
            // chrome scale; the prior 10×10 left them visibly smaller
            // than the surrounding menu-bar text.
            let icon = svg(handle)
                .width(14)
                .height(14)
                .style(move |_: &iced::Theme, _| svg::Style {
                    color: Some(text_c),
                });
            button(
                container(icon)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .width(46)
            .height(btn_h)
            .padding(0)
            .on_press(msg)
            .style(move |_: &iced::Theme, status: button::Status| {
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: if hovered {
                        Some(Background::Color(hover_bg))
                    } else {
                        None
                    },
                    text_color: if hovered { hover_icon } else { text_c },
                    border: Border::default(),
                    ..Default::default()
                }
            })
            .into()
        };

        let controls = row![
            chrome_btn(h_min.clone(), Message::MinimizeMainWindow, hover_c, text_c),
            chrome_btn(
                h_max.clone(),
                Message::ToggleMaximizeMainWindow,
                hover_c,
                text_c,
            ),
            chrome_btn(
                h_close.clone(),
                Message::CloseMainWindow,
                close_hover,
                Color::WHITE,
            ),
        ];

        // Left-pad the menu row so the wordmark doesn't sit flush against
        // the window edge; controls stay flush-right so their hover boxes
        // touch the corner like in Windows' native chrome.
        let menu_padded = container(menu_row).padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 8.0,
        });

        // Search bar placeholder — visual only for now. Matches VS Code's
        // central command palette peek: rounded rect with search icon
        // and muted prompt text.
        let search_icon =
            svg(h_search.clone())
                .width(12)
                .height(12)
                .style(move |_: &iced::Theme, _| svg::Style {
                    color: Some(muted_c),
                });
        let search_bar: Element<'_, Message> = container(
            row![
                search_icon,
                text("Search files, symbols, commands…")
                    .size(11)
                    .color(muted_c),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding(iced::Padding {
            top: 0.0,
            right: 10.0,
            bottom: 0.0,
            left: 10.0,
        })
        .width(440)
        .height(28)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(search_bg)),
            border: Border {
                color: search_border,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .into();

        // Drag zones on either side of the search bar. Double-click
        // toggles maximize (Windows title-bar convention).
        let drag_zone = || -> Element<'static, Message> {
            mouse_area(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::StartMainWindowDrag)
            .on_double_click(Message::ToggleMaximizeMainWindow)
            .into()
        };

        // `width(Length::Fill)` on the row is load-bearing: without it, the
        // drag zones' Fill-width collapses to 0 because their parent (this
        // row) is Shrink, and the chrome loses all its draggable real
        // estate the moment menus + search + controls consume their
        // natural widths.
        let inner = row![menu_padded, drag_zone(), search_bar, drag_zone(), controls,]
            .width(Length::Fill)
            .align_y(Alignment::Center);

        container(inner)
            .width(Length::Fill)
            .height(btn_h)
            .style(crate::styles::toolbar_strip(tokens))
            .into()
    }

    /// Cursor-following translucent preview of a tab being dragged.
    /// Shape matches the real tab bar entry — rounded container with
    /// the title text, the ↗ undock indicator, and the × close icon —
    /// so it reads as "the tab itself is moving". The ghost is
    /// non-interactive; it just shows what the user is carrying.
    fn view_tab_drag_ghost(&self, title: &str) -> Element<'_, Message> {
        use iced::widget::{container, row, text};
        use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let active_bg = crate::styles::ti(tokens.hover);
        let accent = crate::styles::ti(tokens.accent);
        // Match the live tab look: same TabPill widget, accent
        // stripe at the bottom, top-rounded corners. The previous
        // ghost showed an old style with inline ↗ undock + × close
        // glyphs that were removed when the tab right-click menu
        // landed.
        let pill_style = TabPillStyle {
            fill: iced::Color { a: 0.88, ..active_bg },
            border: crate::styles::ti(tokens.border),
            accent,
            is_active: true,
            is_last: true,
            accent_position: AccentPosition::Bottom,
        };
        let inner = container(row![text(title.to_string()).size(11).color(text_c)])
            .padding([4, 10]);
        let pill = TabPill::new(inner, pill_style);
        // Anchor near the cursor (right + below) so the pointer
        // remains visible while the ghost trails it.
        let (cx, cy) = self.interaction_state.last_mouse_pos;
        super::view::translate::Translate::new(pill, (cx + 10.0, cy + 6.0)).into()
    }

    pub(super) fn view_main_for(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        // Context-aware menu: each leaf gates on whether its action
        // makes sense in the current app state. `has_schematic` /
        // `has_selection` drive most entries; undo / redo consult
        // the engine's history so they grey out when empty.
        let menu_ctx = crate::menu_bar::MenuContext {
            has_schematic: self.has_active_schematic(),
            has_pcb: self.has_active_pcb(),
            has_project: document.active_project.is_some(),
            has_selection: !interaction.canvas_for_window(window_id).selected.is_empty(),
            can_undo: document
                .engine_for_window(window_id, ui)
                .map(|e| e.can_undo())
                .unwrap_or(false),
            can_redo: document
                .engine_for_window(window_id, ui)
                .map(|e| e.can_redo())
                .unwrap_or(false),
            // Secondary windows (detached modal, undocked tab) borrow
            // the main window's scale. Good enough until per-window
            // scale tracking lands — it's only wrong if the user drags
            // a secondary window onto a monitor with a different DPI.
            scale_factor: ui.main_window_scale,
        };
        let menu_row = menu_bar::view(&document.panel_ctx.tokens, menu_ctx).map(Message::Menu);

        let left_has_panels = document.dock.has_panels(PanelPosition::Left);
        let right_has_panels = document.dock.has_panels(PanelPosition::Right);
        let bottom_has_panels = document.dock.has_panels(PanelPosition::Bottom);
        let left_collapsed = document.dock.is_collapsed(PanelPosition::Left);
        let right_collapsed = document.dock.is_collapsed(PanelPosition::Right);
        let bottom_collapsed = document.dock.is_collapsed(PanelPosition::Bottom);

        let left = self.view_dock_panel(
            PanelPosition::Left,
            left_has_panels,
            left_collapsed,
            ui.left_width,
        );
        let left_handle = self.view_resize_handle(
            DragTarget::LeftPanel,
            left_has_panels && !left_collapsed,
            true,
        );
        let center = self.view_center(window_id);
        let right_handle = self.view_resize_handle(
            DragTarget::RightPanel,
            right_has_panels && !right_collapsed,
            true,
        );
        let right = self.view_dock_panel(
            PanelPosition::Right,
            right_has_panels,
            right_collapsed,
            ui.right_width,
        );

        let center_row = row![left, left_handle, center, right_handle, right];
        let bottom_handle = self.view_resize_handle(
            DragTarget::BottomPanel,
            bottom_has_panels && !bottom_collapsed,
            false,
        );
        let bottom = self.view_dock_panel_h(
            PanelPosition::Bottom,
            bottom_has_panels,
            bottom_collapsed,
            ui.bottom_height,
        );

        // v0.9.1 status bar: show "Saving…" while the off-thread
        // disk write is in flight, fall through to a stale
        // `save_error` message for ~3 s otherwise. `save_message` is
        // borrowed; status_bar::view treats `None` as "render the
        // normal bar".
        let save_message: Option<&str> = if !ui.saving_paths.is_empty() {
            Some("Saving…")
        } else if let Some((msg, t)) = ui.save_error.as_ref() {
            if t.elapsed() < std::time::Duration::from_secs(3) {
                Some(msg.as_str())
            } else {
                None
            }
        } else {
            None
        };

        let status = status_bar::view(
            ui.cursor_x,
            ui.cursor_y,
            ui.grid_visible,
            ui.snap_enabled,
            ui.zoom,
            ui.unit,
            &interaction.current_tool,
            ui.grid_size_mm,
            &document.panel_ctx.tokens,
            save_message,
        )
        .map(Message::StatusBar);

        // Partition tabs across windows: main owns every tab that isn't
        // currently rendered by an undocked-tab window; each undocked
        // window owns exactly its one tab. Closing a tab in one window
        // can no longer reach tabs that belong to the other.
        let all_undocked_paths: std::collections::HashSet<std::path::PathBuf> = ui
            .windows
            .values()
            .filter_map(|kind| match kind {
                super::states::WindowKind::UndockedTab { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        let is_main_window = ui.main_window_id == Some(window_id);

        // Main window is borderless: wordmark + menus + drag + search +
        // min/max/close in a single 36 px row. Undocked tab windows keep
        // their OS chrome and use the plain styled strip.
        let top_chrome: Element<'_, Message> = if is_main_window {
            self.view_main_window_chrome(menu_row, &document.panel_ctx.tokens)
        } else {
            menu_bar::wrap_plain(menu_row, &document.panel_ctx.tokens)
        };
        let mut main = column![top_chrome];
        let visible_paths: std::collections::HashSet<std::path::PathBuf> = if is_main_window {
            document
                .tabs
                .iter()
                .map(|t| t.path.clone())
                .filter(|p| !all_undocked_paths.contains(p))
                .collect()
        } else {
            match ui.windows.get(&window_id) {
                Some(super::states::WindowKind::UndockedTab { path, .. }) => {
                    std::iter::once(path.clone()).collect()
                }
                _ => std::collections::HashSet::new(),
            }
        };
        // Reserve the tab strip's vertical footprint regardless of
        // whether any document is open — opening the first document
        // would otherwise shift the entire chrome down by ~24 px,
        // which feels jarring. The 1 px chrome separator stays
        // visible too so the menu row always reads as a distinct
        // band above the tab strip.
        main = main.push(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(1)
                .style(crate::styles::chrome_separator(
                    &document.panel_ctx.tokens,
                )),
        );
        if !document.tabs.is_empty() && !visible_paths.is_empty() {
            // Resolve "really dragging" — Some only after the
            // cursor has travelled past a 6 px threshold from
            // the press origin. Without this, every click-to-
            // switch armed the drag state and flipped the
            // cursor to Grabbing instantaneously, plus flashed
            // the drag ghost.
            const DRAG_THRESHOLD_PX: f32 = 6.0;
            let dragging = ui.tab_dragging.and_then(|(idx, ox, oy)| {
                let (mx, my) = interaction.last_mouse_pos;
                let dx = mx - ox;
                let dy = my - oy;
                if dx * dx + dy * dy > DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX {
                    Some(idx)
                } else {
                    None
                }
            });
            main = main.push(
                tab_bar::view(
                    &document.tabs,
                    document.active_tab,
                    dragging,
                    &visible_paths,
                    &document.panel_ctx.tokens,
                )
                .map(move |msg| Message::Tab { window_id, msg }),
            );
        } else {
            // Empty placeholder strip with the same metrics as
            // tab_bar::view: 2 px outer padding + 22 px tall inner
            // pill = 26 px total. Without this the chrome jumps
            // when the first tab opens.
            let placeholder = container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(26)
                .style(crate::styles::toolbar_strip(
                    &document.panel_ctx.tokens,
                ));
            main = main.push(placeholder);
        }
        let main = main
            .push(center_row)
            .push(bottom_handle)
            .push(bottom)
            .push(status);

        // Borderless window needs its own edge-resize hit zones — the OS
        // frame would normally handle this, but `decorations: false`
        // removes WS_THICKFRAME on Windows. Tab windows keep OS
        // decorations so they skip the overlay entirely. The overlay is
        // applied later as a Stack layer over `main` so the content
        // keeps its natural origin and overlay y-coordinates stay
        // correct.
        let main: Element<'_, Message> = main.into();

        let has_active_bar = self.has_active_schematic();
        let dragging_tab = ui.tab_dragging.is_some();
        let needs_overlay = has_active_bar
            || interaction.editing_text.is_some()
            || interaction.context_menu.is_some()
            || interaction.project_tree_context_menu.is_some()
            || interaction.tab_context_menu.is_some()
            || interaction.active_bar_menu.is_some()
            || interaction.canvas.placement_paused
            || ui.panel_list_open
            || ui.find_replace.open
            || ui.preferences_open
            || ui.rename_dialog.is_some()
            || ui.remove_dialog.is_some()
            || ui.project_close_confirm.is_some()
            || document.bom_preview.is_some()
            || ui.annotate.dialog_open
            || ui.annotate.reset_confirm
            || ui.erc.dialog_open
            || !document.dock.floating.is_empty()
            || dragging_tab
            || ui.net_color.custom.show;

        if needs_overlay {
            let mut overlays = self.collect_overlays();
            // Tab drag ghost: only renders once the cursor has
            // travelled past the same 6 px threshold the cursor
            // gating uses (`tab_bar::view`). Mirrors that gate
            // here so press-without-move keeps the ghost off.
            if let Some((tab_idx, ox, oy)) = ui.tab_dragging
                && let Some(tab) = document.tabs.get(tab_idx)
            {
                const DRAG_GHOST_THRESHOLD_PX: f32 = 6.0;
                let (mx, my) = interaction.last_mouse_pos;
                let dx = mx - ox;
                let dy = my - oy;
                if dx * dx + dy * dy
                    > DRAG_GHOST_THRESHOLD_PX * DRAG_GHOST_THRESHOLD_PX
                {
                    overlays.push(self.view_tab_drag_ghost(&tab.title));
                }
            }
            let mut stack = iced::widget::Stack::new().push(main);
            // Resize edges sit above the content but below functional
            // overlays (Active Bar, menus, modals) so the 6 px border
            // strip doesn't eat clicks on those.
            if is_main_window {
                stack = stack.push(Self::resize_edges_overlay());
            }
            for overlay in overlays {
                stack = stack.push(overlay);
            }
            stack.into()
        } else if is_main_window {
            iced::widget::Stack::new()
                .push(main)
                .push(Self::resize_edges_overlay())
                .into()
        } else {
            main.into()
        }
    }

    /// Full-window-sized Stack overlay that anchors 6 px resize hit
    /// zones at the borderless main window's edges and corners. Clicks
    /// on the edges call `iced::window::drag_resize` via
    /// `StartMainWindowResize`; anywhere in the middle is an empty
    /// `Space` so events fall through to the content layer below.
    ///
    /// Used as a stack layer over `main` rather than as a structural
    /// wrapper, so the content keeps its natural y-origin and overlay
    /// coordinates (Active Bar, text edit, net-colour picker) stay
    /// correct without a +EDGE correction everywhere.
    fn resize_edges_overlay<'a>() -> Element<'a, Message> {
        use iced::mouse::Interaction;
        use iced::widget::{Space, column, mouse_area, row};
        use iced::window::Direction;

        const EDGE: f32 = 6.0;

        let straight =
            |direction: Direction, cursor: Interaction, horizontal: bool| -> Element<'a, Message> {
                let (w, h) = if horizontal {
                    (Length::Fill, Length::Fixed(EDGE))
                } else {
                    (Length::Fixed(EDGE), Length::Fill)
                };
                mouse_area(Space::new().width(w).height(h))
                    .on_press(Message::StartMainWindowResize(direction))
                    .interaction(cursor)
                    .into()
            };

        let corner = |direction: Direction, cursor: Interaction| -> Element<'a, Message> {
            mouse_area(
                Space::new()
                    .width(Length::Fixed(EDGE))
                    .height(Length::Fixed(EDGE)),
            )
            .on_press(Message::StartMainWindowResize(direction))
            .interaction(cursor)
            .into()
        };

        let top = straight(Direction::North, Interaction::ResizingVertically, true);
        let bottom = straight(Direction::South, Interaction::ResizingVertically, true);
        let left = straight(Direction::West, Interaction::ResizingHorizontally, false);
        let right = straight(Direction::East, Interaction::ResizingHorizontally, false);
        let nw = corner(Direction::NorthWest, Interaction::ResizingDiagonallyDown);
        let ne = corner(Direction::NorthEast, Interaction::ResizingDiagonallyUp);
        let sw = corner(Direction::SouthWest, Interaction::ResizingDiagonallyUp);
        let se = corner(Direction::SouthEast, Interaction::ResizingDiagonallyDown);

        // Middle row: left/right edges frame a Fill/Fill empty Space so
        // the whole overlay is window-sized and the centre passes
        // clicks through.
        let middle = row![
            left,
            Space::new().width(Length::Fill).height(Length::Fill),
            right
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        column![
            row![nw, top, ne]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
            middle,
            row![sw, bottom, se]
                .width(Length::Fill)
                .height(Length::Fixed(EDGE)),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_dock_panel(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self
            .document_state
            .dock
            .view_region(pos, &self.document_state.panel_ctx)
            .map(Message::Dock);
        let width = if !has_panels {
            0.0
        } else if collapsed {
            28.0
        } else {
            size
        };
        container(panel)
            .width(width)
            .height(Length::Fill)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
    }

    fn view_dock_panel_h(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self
            .document_state
            .dock
            .view_region(pos, &self.document_state.panel_ctx)
            .map(Message::Dock);
        let height = if !has_panels {
            0.0
        } else if collapsed {
            28.0
        } else {
            size
        };
        container(panel)
            .width(Length::Fill)
            .height(height)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
    }

    fn view_resize_handle(
        &self,
        target: DragTarget,
        visible: bool,
        horizontal: bool,
    ) -> Element<'_, Message> {
        let size = if visible { 5 } else { 0 };
        let handle_container = if horizontal {
            container(iced::widget::Space::new())
                .width(size)
                .height(Length::Fill)
                .style(crate::styles::resize_handle(
                    &self.document_state.panel_ctx.tokens,
                ))
        } else {
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(size)
                .style(crate::styles::resize_handle(
                    &self.document_state.panel_ctx.tokens,
                ))
        };
        let interaction = if horizontal {
            iced::mouse::Interaction::ResizingHorizontally
        } else {
            iced::mouse::Interaction::ResizingVertically
        };
        iced::widget::mouse_area(handle_container)
            .interaction(interaction)
            .on_press(Message::DragStart(target))
            .into()
    }

    fn view_center(&self, window_id: iced::window::Id) -> Element<'_, Message> {
        let is_main = self.ui_state.main_window_id == Some(window_id);
        let has_schematic = if is_main {
            self.has_active_schematic()
        } else {
            // An undocked tab window renders if its path still has a
            // live engine in the HashMap. Falls back to the main
            // predicate when the window has already been dropped from
            // the windows map (mid-close frame).
            self.document_state
                .engine_for_window(window_id, &self.ui_state)
                .is_some()
        };
        if has_schematic {
            // Canvas events from non-main windows need to carry the
            // window_id through to the dispatch layer so the right
            // per-window canvas receives the mutation. Keyboard
            // shortcuts that synthesize `Message::CanvasEvent` keep
            // targeting the main canvas unchanged.
            let base: Element<'_, Message> =
                canvas(self.interaction_state.canvas_for_window(window_id))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();
            if is_main {
                base
            } else {
                base.map(move |msg| match msg {
                    Message::CanvasEvent(event) => {
                        Message::CanvasEventInWindow { window_id, event }
                    }
                    other => other,
                })
            }
        } else if self.has_active_pcb() {
            canvas(&self.interaction_state.pcb_canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if self.has_active_library() {
            self.view_library_browser()
        } else {
            // Distinguish "nothing loaded at all" from "project loaded,
            // but no document picked yet" — the second case is what
            // the user sees right after opening a .snxprj before
            // clicking any node in the project tree.
            let (title, hint) = if self.document_state.active_project.is_some() {
                (
                    "No document selected",
                    "Choose a schematic or PCB from the project tree",
                )
            } else {
                (
                    "No document open",
                    "Open a project with File > Open or Ctrl+O",
                )
            };
            container(
                column![
                    iced::widget::text(title).size(14).color(crate::styles::ti(
                        self.document_state.panel_ctx.tokens.text_secondary
                    )),
                    iced::widget::text(hint).size(11).color(crate::styles::ti(
                        self.document_state.panel_ctx.tokens.text_secondary
                    )),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .center(Length::Fill)
            .style(crate::styles::panel_region(
                &self.document_state.panel_ctx.tokens,
            ))
            .into()
        }
    }

    /// v0.10.0 — read-only Library Browser tab body.
    ///
    /// Layout: header strip (library name + component count) + a
    /// virtualised-style table of components. Columns: Name, Value,
    /// Footprint, Description. v0.10.1 adds a side preview pane;
    /// v0.10.2 adds a filter bar above the table.
    fn view_library_browser(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let Some(library) = self.active_library() else {
            return container(iced::widget::text("Library not loaded").size(13))
                .center(Length::Fill)
                .style(crate::styles::panel_region(tokens))
                .into();
        };

        let header = {
            let title_text = if library.name.is_empty() {
                "Library".to_string()
            } else {
                library.name.clone()
            };
            let count_text = format!("{} component(s)", library.components.len());
            let mut title_row = column![
                iced::widget::text(title_text)
                    .size(15)
                    .color(crate::styles::ti(tokens.text)),
                iced::widget::text(count_text)
                    .size(11)
                    .color(crate::styles::ti(tokens.text_secondary)),
            ]
            .spacing(2);
            if !library.description.is_empty() {
                title_row = title_row.push(
                    iced::widget::text(library.description.clone())
                        .size(11)
                        .color(crate::styles::ti(tokens.text_secondary)),
                );
            }
            container(title_row)
                .padding([10, 14])
                .width(Length::Fill)
                .style(crate::styles::panel_region(tokens))
        };

        // Column widths chosen empirically for the v0.10.0 scaffold;
        // v0.10.2 will swap the static layout for resizable columns
        // backed by the filter UI.
        const NAME_WIDTH: f32 = 220.0;
        const VALUE_WIDTH: f32 = 140.0;
        const FOOTPRINT_WIDTH: f32 = 180.0;

        let header_row = container(
            row![
                iced::widget::text("Name")
                    .size(12)
                    .width(Length::Fixed(NAME_WIDTH))
                    .color(crate::styles::ti(tokens.text_secondary)),
                iced::widget::text("Value")
                    .size(12)
                    .width(Length::Fixed(VALUE_WIDTH))
                    .color(crate::styles::ti(tokens.text_secondary)),
                iced::widget::text("Footprint")
                    .size(12)
                    .width(Length::Fixed(FOOTPRINT_WIDTH))
                    .color(crate::styles::ti(tokens.text_secondary)),
                iced::widget::text("Description")
                    .size(12)
                    .width(Length::Fill)
                    .color(crate::styles::ti(tokens.text_secondary)),
            ]
            .spacing(8),
        )
        .padding([8, 14])
        .width(Length::Fill)
        .style(crate::styles::panel_region(tokens));

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(library.components.len());
        for component in &library.components {
            let name = if component.name.is_empty() {
                "(unnamed)".to_string()
            } else {
                component.name.clone()
            };
            rows.push(
                container(
                    row![
                        iced::widget::text(name)
                            .size(12)
                            .width(Length::Fixed(NAME_WIDTH))
                            .color(crate::styles::ti(tokens.text)),
                        iced::widget::text(component.value.clone())
                            .size(12)
                            .width(Length::Fixed(VALUE_WIDTH))
                            .color(crate::styles::ti(tokens.text)),
                        iced::widget::text(component.footprint_name.clone())
                            .size(12)
                            .width(Length::Fixed(FOOTPRINT_WIDTH))
                            .color(crate::styles::ti(tokens.text)),
                        iced::widget::text(component.description.clone())
                            .size(12)
                            .width(Length::Fill)
                            .color(crate::styles::ti(tokens.text_secondary)),
                    ]
                    .spacing(8),
                )
                .padding([6, 14])
                .width(Length::Fill)
                .into(),
            );
        }

        let body: Element<'_, Message> = if rows.is_empty() {
            container(
                iced::widget::text("No components in this library.")
                    .size(12)
                    .color(crate::styles::ti(tokens.text_secondary)),
            )
            .center(Length::Fill)
            .into()
        } else {
            iced::widget::scrollable(column(rows).spacing(0))
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        };

        container(column![header, header_row, body].spacing(0))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(crate::styles::panel_region(tokens))
            .into()
    }

}
