//! Window chrome and layout scaffolding — the borderless main-window
//! chrome strip, the Preferences body, the detached-modal frame, and the
//! dock-panel / resize-handle helpers.
//!
//! Extracted verbatim from `view/mod.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;

mod buttons;

impl Signex {
    /// Custom chrome for the borderless main window. Replaces the OS
    /// title bar with a 36 px strip:
    ///
    /// `[leading controls] [wordmark + menus] [drag] [drag] [trailing controls]`
    ///
    /// With the macOS system menu bar installed `menu_row` is `None` and the
    /// wordmark + menus section is absent.
    ///
    /// Which buttons sit where depends on the platform and the Window
    /// controls preference (`Signex::window_controls_layout`). The search
    /// bar is stacked on top of it, centred on the window
    /// (see `chrome_search_bar_geometry`).
    ///
    /// The drag zones are the only mouse-area clickable regions — menu
    /// buttons, search, and window controls keep their own click
    /// handlers. Double-click on a drag zone toggles maximize.
    pub(super) fn view_main_window_chrome<'a>(
        &self,
        menu_row: Option<Element<'a, Message>>,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'a, Message> {
        use iced::widget::{Space, container, mouse_area, row, svg};
        use iced::{Alignment, Background, Border, Color, Length};

        let theme_id = self.ui_state.theme_id;
        let h_search = crate::icons::icon_chrome_search(theme_id);

        let text_c = crate::styles::ti(tokens.text);
        let muted_c = crate::styles::ti(tokens.text_secondary);
        let search_bg = crate::styles::ti(tokens.panel_bg);
        let search_border = crate::styles::ti(tokens.border);
        // Windows-native destructive red for the close hover — overrides
        // the theme hover so close reads as destructive at a glance.
        let palette = buttons::Palette {
            theme_id,
            text: text_c,
            hover: crate::styles::ti(tokens.hover),
            close_hover: Color::from_rgba(0.78, 0.22, 0.22, 1.0),
        };
        let btn_h = crate::menu_bar::MENU_BAR_HEIGHT;
        let layout = self.window_controls_layout();
        let fullscreen = self.ui_state.main_window_is_fullscreen();
        let leading_controls = buttons::leading(&layout, palette, fullscreen);
        let trailing_controls = buttons::trailing(&layout, palette);

        // Left-pad the menu row so the wordmark doesn't sit flush against
        // the window edge; Windows-style controls stay flush so their hover
        // boxes touch the corner like in native chrome.
        let menu_padded = menu_row.map(|row| {
            container(row).padding(iced::Padding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 8.0,
            })
        });

        // Shrinks with the window; the palette dropdown reads the same
        // geometry so it stays glued to the input.
        let (search_bar_x, search_bar_w) = self.chrome_search_bar_geometry();

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
        let strip = row![leading_controls]
            .push(menu_padded)
            .push(drag_zone())
            .push(drag_zone())
            .push(trailing_controls)
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

    /// `(x, width)` of the search bar for the current window width and
    /// control layout; the palette dropdown anchors to the same `x`.
    pub(in crate::app::view) fn chrome_search_bar_geometry(&self) -> (f32, f32) {
        let fullscreen = self.ui_state.main_window_is_fullscreen();
        let (leading_w, trailing_w) = buttons::extents(&self.window_controls_layout(), fullscreen);
        let menu_w = if self.native_menu.hides_in_window_menu() {
            0.0
        } else {
            crate::menu_bar::approx_menu_bar_width()
        };
        crate::app::view::chrome_search_bar_geometry(
            self.ui_state.window_size.0,
            leading_w,
            trailing_w,
            menu_w,
        )
    }

    fn view_preferences_body(&self) -> Element<'_, Message> {
        let ui = &self.ui_state;
        let dialog: Element<'_, Message> =
            crate::preferences::view_body(crate::preferences::PrefsView {
                nav: ui.preferences_nav,
                draft_theme: ui.preferences_draft_theme,
                saved_theme: ui.theme_id,
                draft_font: &ui.preferences_draft_font,
                draft_grid_style: ui.preferences_draft_grid_style,
                draft_pcb_gpu_render: ui.preferences_draft_pcb_gpu_render,
                draft_annotate_undo: ui.preferences_draft_annotate_undo,
                draft_move_keeps_wires: ui.preferences_draft_move_keeps_wires,
                draft_window_controls_placement: ui.preferences_draft_window_controls_placement,
                window_controls_restart_note: self.window_controls_needs_restart(),
                draft_autosave_enabled: ui.preferences_draft_autosave_enabled,
                draft_symbol_grid_size_mm: ui.preferences_draft_symbol_grid_size_mm,
                draft_symbol_grid_style: ui.preferences_draft_symbol_grid_style,
                draft_symbol_pin_selection: ui.preferences_draft_symbol_pin_selection,
                draft_zoom_step_percent: ui.preferences_draft_zoom_step_percent,
                draft_ui_scale_percent: ui.preferences_draft_ui_scale_percent,
                custom_name: ui.custom_theme.as_ref().map(|c| c.name.as_str()),
                theme_status: &ui.preferences_theme_status,
                dirty: ui.preferences_dirty,
                erc_overrides: &ui.preferences_draft_erc_severity,
                distributor_settings: &self.library.settings,
                panel_tokens: &self.document_state.panel_ctx.tokens,
                draft_component_classes: &ui.preferences_draft_component_classes,
                keymap_editor: &ui.preferences_keymap_editor,
                keymap_status: &ui.preferences_keymap_status,
                keymap_load_error: ui.keymap_load_error.as_deref(),
                keymap_backup: ui.keymap_backup.as_deref(),
                prefs_load_error: ui.prefs_load_error.as_deref(),
                prefs_status: &ui.preferences_prefs_status,
                keymap_search: &ui.preferences_keymap_search,
                keymap_recorder: ui.preferences_keymap_recorder.as_ref(),
                theme_id: ui.theme_id,
            })
            .map(|m| Message::Preferences(PreferencesMsg::Inner(m)));

        dialog
    }

    pub(super) fn view_detached_modal(
        &self,
        window: iced::window::Id,
        modal: super::state::ModalId,
    ) -> Element<'_, Message> {
        use super::state::ModalId;
        match modal {
            ModalId::AnnotateDialog => self.view_annotate_dialog_body(),
            ModalId::ErcDialog => self.view_erc_dialog_body(),
            ModalId::AnnotateResetConfirm => self.view_annotate_reset_confirm_body(),
            ModalId::MoveSelection => self.view_move_selection_body(),
            ModalId::NetColorPalette => self.view_net_color_palette_body(),
            ModalId::ParameterManager => self.view_parameter_manager_body(),
            ModalId::PrintPreview => self.view_print_preview_body(),
            ModalId::BomPreview => self.view_bom_preview_body(),
            ModalId::Preferences => self.view_preferences_body(),
            ModalId::FindReplace => crate::find_replace::window_view(
                &self.ui_state.find_replace,
                &self.document_state.panel_ctx.tokens,
            )
            .map(Message::FindReplaceMsg),
            ModalId::RenameDialog => self.view_rename_dialog_body(),
            ModalId::RemoveDialog => self.view_remove_dialog_body(),
            ModalId::EnableVersionControl => self.view_enable_version_control_dialog_body(),
            ModalId::GridProperties => self.view_grid_properties_dialog_body(),
            ModalId::SelectionFilterCustom => self.view_selection_filter_custom_body(),
            ModalId::About => crate::about_modal::window_view(
                &self.document_state.panel_ctx.tokens,
                self.ui_state.theme_id,
            ),
            ModalId::KeyboardShortcuts => self.view_keyboard_shortcuts_window(),
            ModalId::PassiveCalculator => crate::passive_calculator_modal::window_view(
                &self.document_state.panel_ctx.tokens,
                self.ui_state.theme_id,
                &self.ui_state.passive_calculator,
            ),
            ModalId::CreateChildSheet => self.view_create_child_sheet_body(),
            ModalId::LibraryPicker
            | ModalId::LibraryEditRow
            | ModalId::LibraryPrimitivePicker
            | ModalId::LibraryDocumentOptions
            | ModalId::LibraryCreateOptions
            | ModalId::LibraryUpdates
            | ModalId::LibraryCloseConfirm
            | ModalId::LibraryDeleteConfirm
            | ModalId::LibraryRecovery
            | ModalId::NetColorCustom => {
                // The card centred in its own window: no scrim, and for the
                // per-browser dialogs the browser that opened this window.
                let owner = self.ui_state.library_modal_owners.get(&window);
                match self.library_dialog_card(modal, owner.map(std::path::PathBuf::as_path)) {
                    Some(card) => iced::widget::container(card)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                        .style(crate::styles::modal_card(
                            &self.document_state.panel_ctx.tokens,
                        ))
                        .into(),
                    None => iced::widget::Space::new().into(),
                }
            }
            ModalId::ErrorNotice => self.view_error_notice(),
            ModalId::NetlistIncompletePrompt => self.view_netlist_incomplete_prompt(),
            ModalId::AutosaveRecovery => match self.document_state.autosave.current_offer() {
                Some(offer) => self.view_autosave_recovery_window(offer),
                None => iced::widget::Space::new().into(),
            },
            ModalId::ProjectCloseConfirm => self.view_project_close_confirm_body(),
            ModalId::AppQuitConfirm => self.view_app_quit_confirm_body(),
            ModalId::FirstRunTour => {
                crate::first_run_tour::window_view(&self.document_state.panel_ctx.tokens)
            }
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

#[cfg(test)]
mod layout_tests {
    use super::*;
    use crate::layout_probe::{LaidOutText, find, lay_out_texts, natural};

    const RESET: &str = "Start a fresh preferences file";

    /// #623: the #602 banner's one action, laid out in the detached
    /// Preferences window at its opening size (900 × 620). Before #604 the
    /// text column measured against the whole width first and left the
    /// button a ~37 px sliver, its label wrapped onto five lines; every
    /// gate passed, because nothing laid the tree out.
    #[test]
    fn the_preferences_file_reset_button_shows_its_label_on_one_line() {
        let (mut app, _task) = Signex::new();
        app.ui_state.prefs_load_error = Some("could not be read as JSON".into());

        let texts = lay_out_texts(app.view_preferences_body(), iced::Size::new(900.0, 620.0));

        let label = find(&texts, RESET).bounds;
        let one_line = natural::<()>(iced::widget::text(RESET).size(11).into(), RESET);
        assert!(
            label.height <= one_line.height + 0.5,
            "the button label wraps: {label:?}, one line is {one_line:?}"
        );
        assert!(
            label.width + 0.5 >= one_line.width,
            "the button is narrower than its label: {label:?}, natural {one_line:?}"
        );
    }

    fn main_window_app() -> (Signex, iced::window::Id) {
        let window = iced::window::Id::unique();
        let (mut app, _task) = Signex::new();
        app.ui_state.main_window_id = Some(window);
        app.ui_state.first_run_tour_open = false;
        (app, window)
    }

    fn main_window_texts(app: &Signex, window: iced::window::Id) -> Vec<LaidOutText> {
        lay_out_texts(app.view(window), iced::Size::new(1400.0, 900.0))
    }

    /// Without a working system menu bar (any other OS, or a failed install
    /// on macOS) the strip carries the in-window row.
    #[test]
    fn the_in_window_menu_row_shows_when_the_system_menu_bar_is_not_in_use() {
        let (mut app, window) = main_window_app();
        app.native_menu.mark_failed_for_test();
        let texts = main_window_texts(&app, window);
        for root in crate::menu_bar::model::ROOT_TITLES {
            assert!(texts.iter().any(|t| t.text == root), "no {root} root");
        }
    }

    /// On macOS the row is left out from the first frame, before the first
    /// install, so it does not flash.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_menu_row_is_absent_on_macos_from_the_first_frame() {
        let (app, window) = main_window_app();
        let texts = main_window_texts(&app, window);
        for root in crate::menu_bar::model::ROOT_TITLES {
            assert!(!texts.iter().any(|t| t.text == root), "{root} still drawn");
        }
    }
}
