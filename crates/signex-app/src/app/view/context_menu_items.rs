//! Shared context-menu item and helper primitives — menu rows,
//! separators, the icon-slot column, and the keymap shortcut-label
//! lookup — consumed by the menu builders in `context_menus.rs`.
//!
//! Extracted verbatim from `view/context_menus.rs` (ADR-0001, issue
//! #164) as pure code motion — no behaviour change. These are methods
//! of the same `Signex` view impl, split across sibling files.

use super::*;

impl Signex {
    /// Save menu row, active only when a schematic tab is open —
    /// used by the project-tree context menu's project-root variant.
    /// Altium's right-click menus do not surface keyboard shortcuts,
    /// so the shortcut column is intentionally empty here even though
    /// Ctrl+S still fires `MenuMessage::Save` globally.
    pub(super) fn save_menu_item<'a>(&self, enabled: bool) -> Element<'a, Message> {
        if enabled {
            self.ctx_menu_item_msg(
                None,
                "Save",
                "",
                Message::Menu(crate::menu_bar::MenuMessage::Save),
            )
        } else {
            self.ctx_menu_item_disabled(None, "Save", None)
        }
    }

    /// Resolve the on-disk `.snxlib` path for a project-tree library
    /// node click. The tree path under the `Libraries` group is
    /// `[project_idx, libraries_branch_idx, library_idx]` — the
    /// matching project's `LibraryNodeInfo[library_idx].root` carries
    /// the absolute path. Returns `None` when the path doesn't sit at
    /// a library leaf (defensive — `view_project_tree_context_menu`
    /// already gates on `is_library_node`, but the icon-by-depth
    /// detection there isn't strict enough to skip the lookup).
    pub(super) fn library_node_path_from_tree(&self, tree_path: &[usize]) -> Option<std::path::PathBuf> {
        let project_idx = *tree_path.first()?;
        let library_idx = *tree_path.get(2)?;
        self.document_state
            .panel_ctx
            .projects
            .get(project_idx)?
            .libraries
            .get(library_idx)
            .map(|lib| lib.root.clone())
    }

    /// Build the 26-wide icon column for a context-menu row. Mirrors
    /// `dd_item_icon` in `active_bar.rs` so the icons in the right-
    /// click menu visually align with the dropdown menus in the bar.
    /// `None` still reserves the column so labels in icon-less rows
    /// line up with their iconed neighbours.
    fn ctx_menu_icon_slot<'a>(
        icon: Option<iced::widget::svg::Handle>,
        muted: bool,
    ) -> Element<'a, Message> {
        match icon {
            Some(h) => {
                let mut s = iced::widget::svg(h)
                    .width(20)
                    .height(20)
                    .content_fit(iced::ContentFit::Contain);
                if muted {
                    s = s.style(|_: &iced::Theme, _| iced::widget::svg::Style {
                        color: Some(iced::Color::from_rgba8(0x66, 0x6A, 0x7E, 1.0)),
                    });
                }
                container(s)
                    .width(26)
                    .height(20)
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center)
                    .into()
            }
            None => iced::widget::Space::new().width(26).height(20).into(),
        }
    }

    /// Resolve a keymap command id to its display label under the active
    /// profile, falling back to `fallback` when the command is unbound.
    pub(super) fn keymap_shortcut_label(&self, command_id: &str, fallback: &str) -> String {
        crate::keymap::AppCommandId::new(command_id)
            .ok()
            .and_then(|command| self.ui_state.active_keymap.shortcut_label(&command))
            .unwrap_or_else(|| fallback.to_string())
    }

    pub(super) fn ctx_menu_item_kb<'a>(
        &self,
        icon: Option<iced::widget::svg::Handle>,
        label: &str,
        shortcut: &str,
        action: ContextAction,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let hover_c = crate::styles::ti(tokens.hover);
        iced::widget::button(
            iced::widget::row![
                Self::ctx_menu_icon_slot(icon, false),
                iced::widget::text(label.to_string()).size(11).color(text_c),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(shortcut.to_string())
                    .size(10)
                    .color(crate::styles::ti(tokens.text_secondary)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        )
        .width(Self::CONTEXT_MENU_WIDTH)
        .padding([4, 12])
        .on_press(Message::ContextMenu(ContextMenuMsg::Action(action)))
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover_c)),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            },
        )
        .into()
    }

    pub(super) fn ctx_menu_item_msg<'a>(
        &self,
        icon: Option<iced::widget::svg::Handle>,
        label: &str,
        shortcut: &str,
        message: Message,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let hover_c = crate::styles::ti(tokens.hover);
        iced::widget::button(
            iced::widget::row![
                Self::ctx_menu_icon_slot(icon, false),
                iced::widget::text(label.to_string()).size(11).color(text_c),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(shortcut.to_string())
                    .size(10)
                    .color(crate::styles::ti(tokens.text_secondary)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        )
        .width(Self::CONTEXT_MENU_WIDTH)
        .padding([4, 12])
        .on_press(message)
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover_c)),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            },
        )
        .into()
    }

    /// Icon-less variant of [`ctx_menu_item_msg`] for context menus
    /// where no item carries an icon (e.g. the tab right-click menu).
    /// Drops the 26 px icon-slot column so labels start flush with the
    /// menu's left padding.
    pub(super) fn ctx_menu_item_msg_no_icon<'a>(
        &self,
        label: &str,
        shortcut: &str,
        message: Message,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let hover_c = crate::styles::ti(tokens.hover);
        iced::widget::button(
            iced::widget::row![
                iced::widget::text(label.to_string()).size(11).color(text_c),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(shortcut.to_string())
                    .size(10)
                    .color(crate::styles::ti(tokens.text_secondary)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        )
        .width(Self::CONTEXT_MENU_WIDTH)
        .padding([4, 12])
        .on_press(message)
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover_c)),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            },
        )
        .into()
    }

    /// Icon-less, disabled-row counterpart to
    /// [`ctx_menu_item_msg_no_icon`].
    pub(super) fn ctx_menu_item_disabled_no_icon<'a>(&self, label: &str) -> Element<'a, Message> {
        let text_secondary = crate::styles::ti(self.document_state.panel_ctx.tokens.text_secondary);
        container(
            iced::widget::row![
                iced::widget::text(label.to_string())
                    .size(11)
                    .color(text_secondary),
                iced::widget::Space::new().width(Length::Fill),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        )
        .padding([4, 12])
        .width(Self::CONTEXT_MENU_WIDTH)
        .into()
    }

    /// Submenu launcher row — hover to open after a 200 ms delay
    /// (subscription-driven), or click for instant open. Active state
    /// highlights the row so the user can tell which submenu is open.
    pub(super) fn ctx_menu_item_submenu<'a>(
        &self,
        icon: Option<iced::widget::svg::Handle>,
        label: &str,
        kind: ContextSubmenu,
        active: bool,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let hover_c = crate::styles::ti(tokens.hover);
        let active_bg = crate::styles::ti(tokens.selection);
        let arrow_c = crate::styles::ti(tokens.text_secondary);
        let btn = iced::widget::button(
            iced::widget::row![
                Self::ctx_menu_icon_slot(icon, false),
                iced::widget::text(label.to_string()).size(11).color(text_c),
                iced::widget::Space::new().width(Length::Fill),
                iced::widget::text(SUBMENU_ARROW.to_string())
                    .size(SUBMENU_ARROW_SIZE)
                    .color(arrow_c),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        )
        .width(Self::CONTEXT_MENU_WIDTH)
        .padding([4, 12])
        .on_press(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(kind)))
        .style(
            move |_: &iced::Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover_c)),
                    _ if active => Some(iced::Background::Color(active_bg)),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border::default(),
                    text_color: text_c,
                    ..iced::widget::button::Style::default()
                }
            },
        );
        // Wrap in mouse_area for on_enter / on_exit so the hover timer
        // (handled by `ContextMenuMsg::SubmenuTickHover`) can open the
        // submenu after 200 ms without the user clicking.
        iced::widget::mouse_area(btn)
            .on_enter(Message::ContextMenu(ContextMenuMsg::SubmenuHover(kind)))
            .on_exit(Message::ContextMenu(ContextMenuMsg::SubmenuLeave))
            .into()
    }

    pub(super) fn ctx_menu_item_disabled<'a>(
        &self,
        icon: Option<iced::widget::svg::Handle>,
        label: &str,
        right: Option<&str>,
    ) -> Element<'a, Message> {
        let text_secondary = crate::styles::ti(self.document_state.panel_ctx.tokens.text_secondary);
        let mut row = iced::widget::row![
            Self::ctx_menu_icon_slot(icon, true),
            iced::widget::text(label.to_string())
                .size(11)
                .color(text_secondary),
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        if let Some(right_text) = right {
            // Submenu arrow (›) renders at the unified `SUBMENU_ARROW_SIZE`
            // so disabled placeholder rows showing the chevron line up
            // with live submenu launchers and the menu_bar dropdowns.
            // Every other right-column string (keyboard shortcuts,
            // "coming in vX.Y" version badges) renders at 10 to match
            // the shortcut-column sizing used by enabled rows.
            let size = if right_text == SUBMENU_ARROW {
                SUBMENU_ARROW_SIZE
            } else {
                10.0
            };
            row = row.push(
                iced::widget::text(right_text.to_string())
                    .size(size)
                    .color(text_secondary),
            );
        }

        container(row)
            .padding([4, 12])
            .width(Self::CONTEXT_MENU_WIDTH)
            .into()
    }

    pub(super) fn ctx_menu_sep<'a>(&self) -> Element<'a, Message> {
        let border_c = crate::styles::ti(self.document_state.panel_ctx.tokens.border);
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(border_c.into()),
                ..container::Style::default()
            })
            .into()
    }
}
