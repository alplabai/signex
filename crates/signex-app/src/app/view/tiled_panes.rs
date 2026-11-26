//! View ▸ Window ▸ Tile in the main window (#2249): every docked document in a
//! pane, side by side or stacked. The focused pane is the active tab's full
//! editor; every other pane draws its document and takes a press as "focus
//! this one", so its first click never lands on the wrong document. Pane
//! sizes are the split `canvas_bounds` reports, so overlays placed on the
//! focused canvas line up. State: `UiState::document_tiling`,
//! `InteractionState::tile_panes`; see `handlers::document_tiling`.

use iced::widget::{Column, Row, Space, canvas, container, mouse_area, opaque, stack};
use iced::{Border, Element, Length, Widget as _};
use std::path::Path;

use crate::app::{Message, Signex, TileLayout, UiMsg, WindowMsg};

/// Width of the focused pane's ring; the other panes get a hairline.
const FOCUS_RING: f32 = 2.0 * crate::styles::HAIRLINE;
const PANE_EDGE: f32 = crate::styles::HAIRLINE;

impl Signex {
    /// The documents of `window_id`: the active one alone, or in the tiled
    /// main window one pane per docked document.
    pub(in crate::app::view) fn view_documents(
        &self,
        window_id: iced::window::Id,
    ) -> Element<'_, Message> {
        let tiled = if self.ui_state.main_window_id == Some(window_id) {
            self.tiled_panes()
        } else {
            None
        };
        let Some((layout, panes)) = tiled else {
            return self.view_center(window_id);
        };
        let area = self.document_area_in(None);
        let tiles = crate::app::tiles(panes.len(), (area.width, area.height), layout);
        let last = panes.len() - 1;
        let active = self.document_state.active_tab;
        let cells: Vec<Element<'_, Message>> = panes
            .iter()
            .zip(tiles)
            .enumerate()
            .map(|(n, (&tab, tile))| {
                let focused = tab == active;
                let content = if focused {
                    self.view_center(window_id)
                } else {
                    self.view_unfocused_pane(tab, window_id)
                };
                // The last pane fills the rest, as the last tile takes the
                // rounding remainder.
                let along = |extent: f32| {
                    if n == last {
                        Length::Fill
                    } else {
                        Length::Fixed(extent)
                    }
                };
                let (width, height) = match layout {
                    TileLayout::SideBySide => (along(tile.size.0), Length::Fill),
                    TileLayout::Stacked => (Length::Fill, along(tile.size.1)),
                };
                container(self.framed_pane(content, focused, tab))
                    .width(width)
                    .height(height)
                    .boxed()
            })
            .collect();
        match layout {
            TileLayout::SideBySide => Row::with_children(cells)
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
            TileLayout::Stacked => Column::with_children(cells)
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
        }
    }

    /// `content` with its pane edge: the accent ring on the focused pane, the
    /// border hairline on the others, which also take a press to focus.
    fn framed_pane<'a>(
        &'a self,
        content: Element<'a, Message>,
        focused: bool,
        tab: usize,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let (color, width) = if focused {
            (crate::styles::ti(tokens.accent), FOCUS_RING)
        } else {
            (crate::styles::ti(tokens.border), PANE_EDGE)
        };
        // Drawn over the content and takes no input.
        let edge = container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                border: Border {
                    color,
                    width,
                    ..Border::default()
                },
                ..container::Style::default()
            });
        if focused {
            return stack![content, edge].boxed();
        }
        let press = opaque(
            mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                .on_press(Message::Window(WindowMsg::TilePanePressed(tab))),
        );
        stack![content, press, edge].boxed()
    }

    /// The document of tab `index` as an unfocused pane shows it. Its own
    /// messages are dropped: they would act on the active document.
    fn view_unfocused_pane(
        &self,
        index: usize,
        window_id: iced::window::Id,
    ) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let panes = &self.interaction_state.tile_panes;
        let tab = self.document_state.tabs.get(index);
        let content = match tab {
            Some(tab) if tab.kind.as_project().is_some() => {
                tab.kind.as_project().map(|id| self.view_project_tab(id))
            }
            Some(tab) if tab.kind.as_rules().is_some() => {
                tab.kind.as_rules().map(|id| self.view_rules_tab(id))
            }
            Some(tab) if tab.kind.as_variant_manager().is_some() => tab
                .kind
                .as_variant_manager()
                .map(|id| self.view_variant_manager_tab(id)),
            Some(tab) if tab.kind.is_layer_stack() => Some(self.view_layer_stack_tab()),
            Some(tab) if tab.kind.is_polygon_manager() => Some(self.view_polygon_manager_tab()),
            Some(tab) if tab.kind.is_pin_table() => Some(self.view_pin_table_tab(&tab.path)),
            Some(tab) => self
                .view_library_tab(tab, window_id, true)
                .or_else(|| {
                    let slot = panes.schematic.get(&tab.path)?;
                    Some(self.pane_schematic(slot, &tab.path, window_id))
                })
                .or_else(|| {
                    let board = panes.board.get(&tab.path)?;
                    Some(self.pane_board(board, window_id))
                }),
            None => None,
        };
        content
            .unwrap_or_else(|| {
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(crate::styles::panel_region(tokens))
                    .boxed()
            })
            .map(|_| Message::Ui(UiMsg::Noop))
            .boxed()
    }

    /// The sheet at `path` on its pane canvas: the CPU surface, since a
    /// shader surface keeps one pipeline slot per surface kind.
    fn pane_schematic<'a>(
        &'a self,
        slot: &'a crate::canvas::CanvasSlot,
        path: &Path,
        window_id: iced::window::Id,
    ) -> Element<'a, Message> {
        // The main window's prefs are the active sheet's; the per-sheet ones
        // are this pane's.
        let prefs = self.canvas_view_prefs(window_id);
        let shown = self.shown_placement(path);
        let class_styles = &self.ui_state.net_class_styles;
        let prefs = crate::canvas::CanvasViewPrefs {
            schematic_defaults: self.document_state.schematic_defaults_for(path),
            designator_standard: self.document_state.designator_standard_for(path),
            wire_color_overrides: self
                .document_state
                .engines
                .get(path)
                .map_or(prefs.wire_color_overrides, |engine| {
                    &engine.document().net_colors
                }),
            wire_class_styles: class_styles.styles_in(shown.as_ref()),
            wire_class_revision: class_styles.revision_in(shown.as_ref()),
            wire_tool_active: false,
            placement: None,
            unions: self.union_view(Some(path)),
            notebook_badges: self.notebook_badge_view(Some(path)),
            ..prefs
        };
        crate::canvas::SchematicSurface::new(crate::canvas::SchematicCanvas::new(slot, prefs))
            .boxed()
    }

    /// The board on its pane canvas: background and grid, the board drawn on
    /// the CPU (the GPU board shader is the focused canvas's), its texts.
    fn pane_board<'a>(
        &'a self,
        board: &'a crate::pcb_canvas::PcbCanvas,
        window_id: iced::window::Id,
    ) -> Element<'a, Message> {
        let base = canvas(board).width(Length::Fill).height(Length::Fill);
        let content = crate::pcb_canvas::BoardContentLayer::new(
            board,
            self.ui_state.device_px_per_logical_px(window_id),
        );
        let texts = canvas(board.text_layer())
            .width(Length::Fill)
            .height(Length::Fill);
        stack![base, content, texts].boxed()
    }
}
