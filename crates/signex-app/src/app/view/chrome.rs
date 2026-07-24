//! Window chrome and layout scaffolding — the borderless main-window
//! chrome strip, the Preferences body, the detached-modal frame, and the
//! dock-panel / resize-handle helpers.
//!
//! Extracted verbatim from `view/mod.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;

impl Signex {
    /// Custom chrome for the borderless main window. Replaces the OS
    /// title bar with a 36 px strip:
    ///
    /// `[wordmark + menus] [drag] [drag] [min│max│×]`
    ///
    /// with the search bar stacked on top of it, centred on the window
    /// (see `chrome_search_bar_geometry`).
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
            chrome_btn(
                h_min.clone(),
                Message::Window(WindowMsg::MinimizeMainWindow),
                hover_c,
                text_c
            ),
            chrome_btn(
                h_max.clone(),
                Message::Window(WindowMsg::ToggleMaximizeMainWindow),
                hover_c,
                text_c,
            ),
            chrome_btn(
                h_close.clone(),
                Message::Window(WindowMsg::CloseMainWindow),
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

        // Shrinks with the window; the palette dropdown reads the same
        // geometry so it stays glued to the input.
        let (search_bar_x, search_bar_w) =
            crate::app::view::chrome_search_bar_geometry(self.ui_state.window_size.0);

        // Chrome-strip command palette input — VS Code-style fuzzy
        // search over commands, placed symbols, and project files. The
        // text_input is always rendered; the dropdown overlay is gated
        // on `command_palette.open` and rendered by `collect_overlays`.
        let search_icon =
            svg(h_search.clone())
                .width(12)
                .height(12)
                .style(move |_: &iced::Theme, _| svg::Style {
                    color: Some(muted_c),
                });
        let palette_input = text_input(
            "Search files, symbols, commands…",
            &self.ui_state.command_palette.query,
        )
        .id(crate::app::command_palette::COMMAND_PALETTE_INPUT_ID.clone())
        .on_input(|q| Message::CommandPalette(CommandPaletteMsg::QueryChanged(q)))
        .on_submit(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected))
        .padding(iced::Padding::ZERO)
        .size(11)
        .width(Length::Fill)
        .style(
            move |_: &iced::Theme, _status: text_input::Status| text_input::Style {
                // Outer container owns the chrome border + bg, so the
                // input itself is transparent. Without this the input's
                // default frame paints on top of the container's
                // rounded rect and the corners look doubled.
                background: Background::Color(Color::TRANSPARENT),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                icon: text_c,
                placeholder: muted_c,
                value: text_c,
                selection: Color { a: 0.4, ..text_c },
            },
        );
        let search_bar: Element<'_, Message> = container(
            row![search_icon, palette_input]
                .spacing(8)
                .align_y(Alignment::Center),
        )
        .padding(iced::Padding {
            top: 0.0,
            right: 10.0,
            bottom: 0.0,
            left: 10.0,
        })
        .width(search_bar_w)
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
            .on_press(Message::Window(WindowMsg::StartMainWindowDrag))
            .on_double_click(Message::Window(WindowMsg::ToggleMaximizeMainWindow))
            .into()
        };

        // Two layers. Underneath: menus, controls, and two Fill drag
        // zones covering everything between them. On top: the search
        // bar, centred on the window. Putting the bar in the row
        // instead would centre it in the *gap*, which sits right of
        // window centre because the menu section is far wider than the
        // three control buttons.
        //
        // The top layer is a bare container — outside the bar's own
        // bounds it captures nothing, so presses fall straight through
        // to the drag zones below and the whole strip stays draggable.
        let strip = row![menu_padded, drag_zone(), drag_zone(), controls]
            .width(Length::Fill)
            .align_y(Alignment::Center);
        // Positioned by left padding rather than centre alignment so it
        // lands on exactly the `x` the geometry helper reports — the
        // dropdown anchors to that same `x`, and the helper stops
        // centring on very narrow windows.
        let centered_search = container(search_bar)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: search_bar_x,
            })
            .align_y(iced::alignment::Vertical::Center);
        let inner = iced::widget::Stack::new().push(strip).push(centered_search);

        container(inner)
            .width(Length::Fill)
            .height(btn_h)
            .style(crate::styles::toolbar_strip(tokens))
            .into()
    }

    fn view_preferences_body(&self) -> Element<'_, Message> {
        use crate::app::view::dialogs::{MODAL_CLOSE_X_HIT_W, MODAL_HEADER_HEIGHT};
        use iced::widget::{Space, Stack, column as col_widget, mouse_area, row};

        let ui = &self.ui_state;
        let dialog: Element<'_, Message> = crate::preferences::view_body(
            ui.preferences_nav,
            ui.preferences_draft_theme,
            ui.theme_id,
            &ui.preferences_draft_font,
            ui.preferences_draft_power_port_style,
            ui.preferences_draft_label_style,
            ui.preferences_draft_multisheet_style,
            ui.preferences_draft_grid_style,
            ui.preferences_draft_pcb_gpu_render,
            ui.preferences_draft_symbol_grid_size_mm,
            ui.preferences_draft_symbol_grid_style,
            ui.preferences_draft_symbol_pin_selection,
            ui.custom_theme.as_ref().map(|c| c.name.as_str()),
            ui.preferences_dirty,
            &ui.erc_severity_override,
            &self.library.settings,
            &self.document_state.panel_ctx.tokens,
            &ui.preferences_draft_component_classes,
            &ui.preferences_keymap_editor,
            &ui.preferences_keymap_status,
            &ui.preferences_keymap_search,
            ui.preferences_keymap_recorder.as_ref(),
            ui.theme_id,
        )
        .map(|m| Message::Preferences(PreferencesMsg::Inner(m)));

        // OS-level drag handle covering the header strip, minus the
        // close-X hit zone on the right. Press anywhere on the title
        // bar → SC_MOVE drag the borderless OS window. Without this,
        // `decorations: false` strips the OS title bar so there's
        // nothing else for the user to grab.
        let modal = super::state::ModalId::Preferences;
        let drag_layer: Element<'_, Message> = col_widget![
            row![
                mouse_area(
                    Space::new()
                        .width(Length::Fill)
                        .height(Length::Fixed(MODAL_HEADER_HEIGHT))
                )
                .on_press(Message::Window(WindowMsg::StartDetachedWindowDrag(modal)))
                .interaction(iced::mouse::Interaction::Grab),
                Space::new()
                    .width(Length::Fixed(MODAL_CLOSE_X_HIT_W))
                    .height(Length::Fixed(MODAL_HEADER_HEIGHT)),
            ]
            .width(Length::Fill),
            Space::new().width(Length::Fill).height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        Stack::new().push(dialog).push(drag_layer).into()
    }

    pub(super) fn view_detached_modal(&self, modal: super::state::ModalId) -> Element<'_, Message> {
        use super::state::ModalId;
        match modal {
            ModalId::AnnotateDialog => self.view_annotate_dialog_body(),
            ModalId::ErcDialog => self.view_erc_dialog_body(),
            ModalId::AnnotateResetConfirm => self.view_annotate_reset_confirm_body(),
            // Stubs — these modals don't yet have extractable bodies; fall
            // back to a placeholder so the window is non-empty until their
            // body helpers land.
            ModalId::MoveSelection => self.view_move_selection_body(),
            ModalId::NetColorPalette => self.view_net_color_palette_body(),
            ModalId::ParameterManager => self.view_parameter_manager_body(),
            ModalId::PrintPreview => self.view_print_preview_body(),
            ModalId::BomPreview => {
                // Stack the body underneath a 6 px edge-resize
                // overlay so the borderless OS window can be
                // resized by dragging its edges. Without this,
                // `decorations: false` strips the OS frame and
                // there's nothing to grab.
                let body = self.view_bom_preview_body();
                let resize_active = self
                    .document_state
                    .bom_preview
                    .as_ref()
                    .map(|p| p.column_resize.is_some())
                    .unwrap_or(false);
                let mut stack = iced::widget::Stack::new()
                    .push(body)
                    .push(Self::detached_modal_resize_overlay(modal));
                // While a column-resize drag is in flight, lay an
                // invisible mouse_area over the whole modal that
                // pins the cursor to ResizingHorizontally. Without
                // this, the cursor reverts to default the moment
                // it leaves the 4 px handle's hit zone — which
                // happens immediately on horizontal drag.
                if resize_active {
                    let overlay: Element<'_, Message> = iced::widget::mouse_area(
                        iced::widget::Space::new()
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .on_release(Message::BomPreview(BomPreviewMsg::ColumnResizeEnd))
                    .interaction(iced::mouse::Interaction::ResizingHorizontally)
                    .into();
                    stack = stack.push(overlay);
                }
                stack.into()
            }
            ModalId::Preferences => self.view_preferences_body(),
            ModalId::FindReplace
            | ModalId::RenameDialog
            | ModalId::RemoveDialog
            | ModalId::ProjectOptions
            | ModalId::EnableVersionControl
            | ModalId::GridProperties
            | ModalId::SelectionFilterCustom => {
                iced::widget::container(iced::widget::text("Detached modal"))
                    .padding(20)
                    .into()
            }
        }
    }

    /// Same 6 px edge-resize overlay as the main window's, but
    /// emitting `StartDetachedModalResize { modal, direction }`
    /// so it dispatches to the right OS window. Used as a stack
    /// layer above the modal's body in `view_detached_modal`.
    fn detached_modal_resize_overlay<'a>(modal: super::state::ModalId) -> Element<'a, Message> {
        use iced::mouse::Interaction;
        use iced::widget::{Space, column, mouse_area, row};
        use iced::window::Direction;

        const EDGE: f32 = 6.0;

        let straight = move |direction: Direction,
                             cursor: Interaction,
                             horizontal: bool|
              -> Element<'a, Message> {
            let (w, h) = if horizontal {
                (Length::Fill, Length::Fixed(EDGE))
            } else {
                (Length::Fixed(EDGE), Length::Fill)
            };
            mouse_area(Space::new().width(w).height(h))
                .on_press(Message::Window(WindowMsg::StartDetachedModalResize {
                    modal,
                    direction,
                }))
                .interaction(cursor)
                .into()
        };
        let corner = move |direction: Direction, cursor: Interaction| -> Element<'a, Message> {
            mouse_area(
                Space::new()
                    .width(Length::Fixed(EDGE))
                    .height(Length::Fixed(EDGE)),
            )
            .on_press(Message::Window(WindowMsg::StartDetachedModalResize {
                modal,
                direction,
            }))
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
    pub(super) fn resize_edges_overlay<'a>() -> Element<'a, Message> {
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
                    .on_press(Message::Window(WindowMsg::StartMainWindowResize(direction)))
                    .interaction(cursor)
                    .into()
            };

        let corner = |direction: Direction, cursor: Interaction| -> Element<'a, Message> {
            mouse_area(
                Space::new()
                    .width(Length::Fixed(EDGE))
                    .height(Length::Fixed(EDGE)),
            )
            .on_press(Message::Window(WindowMsg::StartMainWindowResize(direction)))
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

    pub(super) fn view_dock_panel(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self
            .document_state
            .dock
            .view_region(pos, &self.document_state.panel_ctx, &self.library)
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

    pub(super) fn view_dock_panel_h(
        &self,
        pos: PanelPosition,
        has_panels: bool,
        collapsed: bool,
        size: f32,
    ) -> Element<'_, Message> {
        let panel = self
            .document_state
            .dock
            .view_region(pos, &self.document_state.panel_ctx, &self.library)
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

    pub(super) fn view_resize_handle(
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
            .on_press(Message::Ui(UiMsg::DragStart(target)))
            .into()
    }
}
