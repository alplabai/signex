//! Floating overlay builders — the hover cards (`symbol_hover`,
//! `net_hover`), the chrome-strip command-palette dropdown, and the shared
//! click-outside-to-dismiss layer.
//!
//! Extracted verbatim from `view/mod.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files. The overlay-assembly
//! entry point `collect_overlays` stays in `view/mod.rs` alongside the
//! other composition-core methods.

use super::canvas_region;
use super::*;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

mod bars;
#[cfg(test)]
mod dismiss_tests;
mod dock_zones;
#[cfg(test)]
mod filter_menu_tests;
mod modals;
mod net_hover;
mod palette;
#[cfg(test)]
mod palette_scrim_tests;
mod panel_list;
mod placement_paused;
pub(super) mod read_only_badge;
mod symbol_hover;
mod text_edit;
#[cfg(test)]
mod window_gate_tests;
mod zoom_pill;
#[cfg(test)]
pub(super) use zoom_pill::pill_origin as zoom_pill_origin;
mod zoom_presets;

impl Signex {
    /// The canvas rectangle in window coordinates, from the live dock sizes.
    pub(in crate::app) fn canvas_bounds(&self) -> iced::Rectangle {
        self.canvas_bounds_in(None)
    }

    /// [`Self::canvas_bounds`] in the undocked window `window`, laid out at
    /// that window's own size (`None`: the main window). In a tiled main
    /// window it is the focused pane (#2249).
    pub(in crate::app) fn canvas_bounds_in(
        &self,
        window: Option<iced::window::Id>,
    ) -> iced::Rectangle {
        let area = self.document_area_in(window);
        match (window, self.tiled_panes()) {
            (None, Some((layout, panes))) => {
                let active = self.document_state.active_tab;
                let index = panes.iter().position(|tab| *tab == active).unwrap_or(0);
                canvas_region::pane_bounds(area, panes.len(), index, layout)
            }
            _ => area,
        }
    }

    /// The region between the docks, chrome and status bar where the
    /// documents show, in `window` (`None`: the main window).
    pub(in crate::app) fn document_area_in(
        &self,
        window: Option<iced::window::Id>,
    ) -> iced::Rectangle {
        use crate::dock::PanelPosition::{Bottom, Left, Right};
        let dock = &self.document_state.dock;
        let extent = |pos, size| canvas_region::DockExtent {
            present: dock.has_panels(pos),
            collapsed: dock.is_collapsed(pos),
            size,
        };
        let ui = &self.ui_state;
        canvas_region::canvas_bounds(
            window.map_or(ui.window_size, |w| ui.size_of_window(w)),
            extent(Left, ui.left_width),
            extent(Right, ui.right_width),
            extent(Bottom, ui.bottom_height),
            &self.document_state.panel_ctx.tokens,
        )
    }

    /// Where a hover card may go in `window_id`: the canvas in the main
    /// window, the whole window elsewhere.
    pub(super) fn card_bounds(&self, window_id: iced::window::Id) -> iced::Rectangle {
        canvas_region::card_bounds(
            self.ui_state.main_window_id == Some(window_id),
            self.canvas_bounds(),
            self.ui_state.window_size,
        )
    }

    pub(super) fn dismiss_layer(on_press: Message) -> Element<'static, Message> {
        Self::dismiss_layer_tinted(
            on_press,
            crate::styles::tokens::scrim(crate::styles::tokens::BACKDROP_ALPHA),
        )
    }

    /// [`Self::dismiss_layer`] painted in `scrim`.
    pub(super) fn dismiss_layer_tinted(
        on_press: Message,
        scrim: iced::Color,
    ) -> Element<'static, Message> {
        // Opaque semi-transparent backdrop that blocks interaction with
        // underlying content. Left-click anywhere on it triggers the
        // dismiss message.
        //
        // Right-click menus use `MenuBackdrop` instead (#2349).
        //
        // We intentionally do *not* wire `on_right_press` — iced's
        // `mouse_area` would `capture_event()` the right-press and
        // prevent the underlying canvas from starting a pan. Instead
        // the canvas itself owns the right-press (its pan gesture) and
        // closes the context menu once the pan actually starts moving
        // (see `canvas/mod.rs`'s `CursorMoved` handler, which fires
        // `ContextMenuMsg::Close` the moment `pan_moved` flips on).
        //
        // The wheel is a separate matter, handled one level up:
        // `collect_overlays` wraps every overlay whose id
        // `overlay_id::swallows_scroll` marks, so scroll cannot reach the
        // canvas through an overlay regardless of whether that overlay
        // built a dismiss layer at all (#562).
        iced::widget::mouse_area(
            container(iced::widget::Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(scrim)),
                    ..container::Style::default()
                }),
        )
        .on_press(on_press)
        .boxed()
    }

    /// The click-outside layer of an Active Bar flyout, on either bar. Unlike
    /// [`Self::dismiss_layer`] it takes a right press too and sends
    /// `on_right_press`, which also holds that press's release off the
    /// canvas, so an armed tool stays armed.
    pub(super) fn flyout_dismiss_layer(
        on_dismiss: Message,
        on_right_press: Message,
        scrim: iced::Color,
    ) -> Element<'static, Message> {
        iced::widget::mouse_area(Self::dismiss_layer_tinted(on_dismiss, scrim))
            .on_right_press(on_right_press)
            .boxed()
    }

    /// Active-bar dropdown menu (Place / Align / filter presets etc.).
    /// Absolute-positioned with `Translate` so the column can auto-size
    /// to its widest label. Pushes the dismiss layer then the dropdown.
    pub(in crate::app) fn active_bar_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let ui = &self.ui_state;
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let Some(ab_menu) = interaction.active_bar_menu else {
            return Vec::new();
        };
        // Only the window the dropdown was opened in paints it (#1910).
        if !self.overlay_belongs_to(interaction.active_bar_menu_window, window_id) {
            return Vec::new();
        }
        let has_selection = !interaction.canvas_for_window(window_id).selected.is_empty();
        let has_net_colors = !ui.net_colors.is_empty();
        let dropdown = crate::active_bar::view_dropdown(
            &self.ui_state.i18n,
            ab_menu,
            &document.panel_ctx.tokens,
            &interaction.selection_filters,
            &interaction.custom_filter_presets,
            self.ui_state.effective_theme_id(),
            has_selection,
            has_net_colors,
            &|id| self.keymap_shortcut_label(id),
        )
        .map(Message::ActiveBar)
        .boxed();
        let metrics = signex_widgets::active_bar::Metrics::GRAPHITE;
        let x_off = crate::active_bar::dropdown_x_offset(ab_menu, &metrics);
        // Bar bottom: MENU_BAR_HEIGHT + tabs + top margin + bar height,
        // plus a small gap so the dropdown visually touches.
        let ab_y: f32 = canvas_region::chrome_top(&document.panel_ctx.tokens)
            + crate::active_bar::bar_top_margin()
            + crate::active_bar::bar_height_for(&metrics)
            + crate::active_bar::BAR_DROPDOWN_GAP_PX;
        let (ww, _) = ui.size_of_window(window_id);
        let adjusted_x =
            x_off + crate::active_bar::bar_left_in_window(interaction.current_tool, ww, &metrics);
        Self::flyout_and_place(
            Message::ActiveBar(crate::active_bar::ActiveBarMsg::CloseMenus),
            Message::ActiveBar(crate::active_bar::ActiveBarMsg::FlyoutRightPressed),
            Message::ActiveBar(crate::active_bar::ActiveBarMsg::RightPressTaken),
            dropdown,
            (adjusted_x, ab_y),
        )
    }

    /// Canvas right-click context menu (+ its Place/Align submenu): the
    /// card opens at the click and the open submenu's flyout sits beside it,
    /// level with its launcher row; both are kept inside the window. Pushes
    /// dismiss, then the placed menu.
    pub(in crate::app) fn context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let interaction = &self.interaction_state;
        let Some(ctx_menu) = interaction.context_menu.as_ref() else {
            return Vec::new();
        };
        // Only the window the menu was opened in paints it (#1910).
        if !self.overlay_belongs_to(ctx_menu.window, window_id) {
            return Vec::new();
        }
        let card = self.view_context_menu(window_id);
        let mut menu =
            super::context_menu::AnchoredMenu::new(card.element, (ctx_menu.x, ctx_menu.y));
        if let (Some(kind), Some(offset)) = (interaction.context_submenu, card.flyout_offset) {
            // A board's menu opens the board's flyouts (#2405).
            let submenu = if self.pcb_targets(Some(window_id)).shown {
                self.view_board_context_submenu(kind, window_id)
            } else {
                self.view_context_submenu(kind, window_id)
            };
            menu = menu.flyout(Self::submenu_panel(submenu), offset);
        }
        Self::anchored_layers(Message::ContextMenu(ContextMenuMsg::Close), menu)
    }

    /// Document-tab right-click menu. Rendered before the project-tree
    /// menu since the two are mutually exclusive. Pushes dismiss then the
    /// placed menu.
    pub(super) fn tab_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let interaction = &self.interaction_state;
        let Some(tab_ctx) = interaction.tab_context_menu.as_ref() else {
            // The tab strip's "+N" menu shares this slot; the menus are
            // exclusive (`close_all_context_menus`).
            let overflow = self.tab_overflow_menu_overlay(window_id);
            if !overflow.is_empty() {
                return overflow;
            }
            return self.dock_tab_context_menu_overlay();
        };
        // Only the main window's tab strip opens this menu, so only the main
        // window paints it (#2037).
        if !self.overlay_belongs_to(None, window_id) {
            return Vec::new();
        }
        Self::menu_overlay(
            Message::ContextMenu(ContextMenuMsg::CloseTab),
            self.view_tab_context_menu(tab_ctx),
            (tab_ctx.x, tab_ctx.y),
        )
    }

    /// Dock panel tab right-click menu: dismiss layer, then the placed menu.
    fn dock_tab_context_menu_overlay(&self) -> Vec<Element<'_, Message>> {
        let Some(menu_ctx) = self.document_state.dock.tab_menu else {
            return Vec::new();
        };
        Self::menu_overlay(
            Message::Dock(crate::dock::DockMessage::TabMenuDismissed),
            self.view_dock_tab_context_menu(),
            (menu_ctx.x, menu_ctx.y),
        )
    }

    /// Projects-panel tree right-click menu (+ its Add New to Project,
    /// Planned or library Add New flyout). Pushes dismiss, then the placed
    /// menu and its flyout.
    pub(super) fn project_tree_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let interaction = &self.interaction_state;
        let Some(tree_ctx) = interaction.project_tree_context_menu.as_ref() else {
            return Vec::new();
        };
        // Only the window whose Projects panel opened the menu paints it.
        if !self.overlay_belongs_to(tree_ctx.window, window_id) {
            return Vec::new();
        }
        if tree_ctx.add_new_only {
            return Self::menu_overlay(
                Message::ContextMenu(ContextMenuMsg::CloseProjectTree),
                self.view_context_submenu(ContextSubmenu::AddNewToProject, window_id),
                (tree_ctx.x, tree_ctx.y),
            );
        }
        let card = self.view_project_tree_context_menu(tree_ctx);
        let mut menu =
            super::context_menu::AnchoredMenu::new(card.element, (tree_ctx.x, tree_ctx.y));
        // Only the tree's own flyouts open from this menu; the offset is
        // there only while one of its launchers is open.
        if let (
            Some(
                kind @ (ContextSubmenu::AddNewToProject
                | ContextSubmenu::ProjectPlanned
                | ContextSubmenu::AddNewToLibrary),
            ),
            Some(offset),
        ) = (interaction.context_submenu, card.flyout_offset)
        {
            let submenu = self.view_context_submenu(kind, window_id);
            menu = menu.flyout(Self::submenu_panel(submenu), offset);
        }
        Self::anchored_layers(Message::ContextMenu(ContextMenuMsg::CloseProjectTree), menu)
    }

    /// v0.18.10 — Altium-style grid picker popup. Floats at the cursor
    /// when `G` is pressed in a footprint editor. Pushes dismiss then the
    /// placed menu, only in the window it opened in.
    pub(super) fn grid_picker_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let Some(picker) = self.interaction_state.grid_picker.as_ref() else {
            return Vec::new();
        };
        if picker.owner != self.overlay_owner(window_id) {
            return Vec::new();
        }
        Self::menu_overlay(
            Message::Ui(UiMsg::GridPickerClose),
            self.view_grid_picker_menu(),
            (picker.x, picker.y),
        )
    }

    /// Floating panels — one `Translate`-positioned widget per floating
    /// panel. No clamp: a panel may be dragged anywhere, even past the
    /// window edge (the OS clips).
    pub(super) fn floating_panels_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let document = &self.document_state;
        let mut out: Vec<Element<'_, Message>> = Vec::new();
        for i in 0..document.dock.floating.len() {
            if let Some(panel_widget) = document.dock.view_floating_panel(
                i,
                self.panel_ctx_for(window_id),
                &self.library,
                self.sheet_grid(),
            ) {
                let fp = &document.dock.floating[i];
                out.push(
                    signex_widgets::Translate::new(
                        panel_widget
                            .map(move |msg| self.scope_dock_message(window_id, Message::Dock(msg)))
                            .boxed(),
                        (fp.x, fp.y),
                    )
                    .boxed(),
                );
            }
        }
        out
    }
}

/// `element` with the mouse wheel stopped at its own bounds, so it cannot
/// reach the canvas underneath (#562). Wrap the painted card, not a
/// window-sized positioner (`Translate`, a `Fill` column): `mouse_area`
/// captures wherever its bounds are, so a positioner would take the wheel
/// over the whole canvas.
pub(super) fn scroll_guard<'a>(element: Element<'a, Message>) -> Element<'a, Message> {
    // `Idle` makes the surface count as a hit for the `Stack`, which hands
    // the layers below a levitating cursor: no hover reaches the canvas.
    iced::widget::mouse_area(element)
        .on_scroll(|_| Message::noop())
        .interaction(iced::mouse::Interaction::Idle)
        .boxed()
}
