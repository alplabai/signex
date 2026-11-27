//! The chrome of the document area's panes (mockup row 207, #1307): a tab
//! strip on each pane with its ⋯ button, the pane menu it opens, and the
//! splitters between the panes. The tab a pane draws carries a 2 px line on
//! top: the accent on the focused pane, grey on the others. A splitter
//! drags, and a double click makes the panes equal. Design:
//! docs/MENU_BAR.md, "Panes".

use iced::widget::{Column, Row, Space, Stack, container, mouse_area, row};
use iced::{Color, Element, Length, Rectangle, Widget as _, mouse};
use signex_i18n::{ids, t};
use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};
use signex_widgets::{button, text};

use crate::app::command::enablement::is_enabled;
use crate::app::handlers::document_panes::TiledPanes;
use crate::app::{Message, PaneMsg, Signex, Tile, WindowMsg};
use crate::document_panes::geometry::SPLITTER_GRIP;
use crate::document_panes::{PANE_STRIP_HEIGHT, splitters};
use crate::keymap::AppCommandId;
use crate::styles::ti;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// Width of the pane menu card.
const PANE_MENU_WIDTH: f32 = 220.0;

/// The pane menu's rows, in order (mockup row 207); Merge All Panes is
/// Untile.
const PANE_MENU_ROWS: [&str; 7] = [
    "split_pane_right",
    "split_pane_down",
    "move_tab_to_other_pane",
    "untile_documents",
    "close_pane",
    "sync_pane_zoom",
    "equalize_panes",
];

fn pane_msg(msg: PaneMsg) -> Message {
    Message::Window(WindowMsg::Pane(msg))
}

/// `element` sized to `bounds` and placed at its top-left, on a layer the
/// size of its parent.
fn placed<'a>(element: Element<'a, Message>, bounds: Rectangle) -> Element<'a, Message> {
    let sized = container(element)
        .width(bounds.width)
        .height(bounds.height)
        .boxed();
    signex_widgets::Translate::new(sized, (bounds.x, bounds.y)).boxed()
}

/// A layer the size of its parent holding `cards`, each at its bounds.
pub(in crate::app::view) fn layer<'a>(
    cards: impl IntoIterator<Item = (Element<'a, Message>, Rectangle)>,
) -> Element<'a, Message> {
    let mut stack = Stack::new().width(Length::Fill).height(Length::Fill);
    for (card, bounds) in cards {
        stack = stack.push(placed(card, bounds));
    }
    stack
        .push(
            Space::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
        )
        .boxed()
}

/// `cells` laid out at `tiles` (relative to the area): rows of tiles that
/// share a top edge, each cell its tile's width, the last cell of a row and
/// the last row filling the rest, as the last tile takes the remainder. The
/// panes and the tile rows' preview (`menu_bar`) are laid out with it.
pub(crate) fn place_cells<'a, M: 'a>(cells: Vec<Element<'a, M>>, tiles: &[Tile]) -> Element<'a, M> {
    // Each row: its height and its cells with their widths.
    type CellRow<'a, M> = (f32, Vec<(f32, Element<'a, M>)>);
    let mut rows: Vec<CellRow<'a, M>> = Vec::new();
    let mut top = None;
    for (cell, tile) in cells.into_iter().zip(tiles) {
        if top != Some(tile.position.1) {
            rows.push((tile.size.1, Vec::new()));
            top = Some(tile.position.1);
        }
        if let Some((_, row)) = rows.last_mut() {
            row.push((tile.size.0, cell));
        }
    }
    let count = rows.len();
    let along = |n: usize, last: usize, extent: f32| {
        if n + 1 == last {
            Length::Fill
        } else {
            Length::Fixed(extent)
        }
    };
    let rows = rows.into_iter().enumerate().map(|(r, (height, row))| {
        let cells = row.len();
        let row = Row::with_children(row.into_iter().enumerate().map(|(c, (width, cell))| {
            container(cell)
                .width(along(c, cells, width))
                .height(Length::Fill)
                .boxed()
        }));
        container(row)
            .width(Length::Fill)
            .height(along(r, count, height))
            .boxed()
    });
    Column::with_children(rows)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

impl Signex {
    /// The tab strip on top of shown pane `pane`: its `tabs`, the one it
    /// draws (`drawn`) marked, and the ⋯ button of the pane menu.
    pub(in crate::app::view) fn pane_strip(
        &self,
        pane: usize,
        tabs: &[usize],
        drawn: usize,
        focused: bool,
    ) -> Element<'_, Message> {
        let tokens = &self.document_view.panel_ctx.tokens;
        let line = if focused {
            ti(tokens.accent)
        } else {
            ti(tokens.text_secondary)
        };
        let last = tabs.len().saturating_sub(1);
        let pills = tabs.iter().enumerate().filter_map(|(n, &tab)| {
            let info = self.document_state.tabs.get(tab)?;
            let is_drawn = tab == drawn;
            let ink = if is_drawn {
                ti(tokens.text)
            } else {
                ti(tokens.text_secondary)
            };
            let label = container(
                text(signex_i18n::UserText::new(info.title.clone()))
                    .size(TS.caption)
                    .color(ink),
            )
            .padding([3.0, 10.0])
            .boxed();
            let style = TabPillStyle {
                fill: if is_drawn {
                    ti(tokens.panel_bg)
                } else {
                    Color::TRANSPARENT
                },
                border: ti(tokens.border),
                accent: line,
                is_active: is_drawn,
                is_last: n == last,
                accent_position: AccentPosition::Top,
            };
            let pill = TabPill::new(label, style).square(true);
            Some(
                mouse_area(pill)
                    .on_press(pane_msg(PaneMsg::TabPressed { pane, tab }))
                    .boxed(),
            )
        });
        let i18n = self.ui_state.i18n.as_ref();
        let menu = button(text(t!(i18n, ids::WORKSPACE_LAYOUTS_PANES_MENU)).size(TS.body))
            .padding([0.0, 8.0])
            .style(crate::styles::ghost_button(tokens))
            .on_press(pane_msg(PaneMsg::MenuToggled(pane)));
        let hint =
            container(text(t!(i18n, ids::WORKSPACE_LAYOUTS_PANES_MENU_TOOLTIP)).size(TS.caption))
                .padding([3.0, 8.0])
                .style(crate::styles::context_menu(tokens));
        let menu = signex_widgets::tooltip(menu, hint, iced::widget::tooltip::Position::Bottom);
        container(
            row![
                Row::with_children(pills).height(Length::Fill),
                Space::new().width(Length::Fill),
                menu,
            ]
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fixed(PANE_STRIP_HEIGHT))
        .style(crate::styles::panel_region(tokens))
        .boxed()
    }

    /// The splitters between the panes of `view` over a document area of
    /// `size`, as a layer the size of that area: each drags, and a double
    /// click makes the panes equal.
    pub(in crate::app::view) fn pane_splitters(
        &self,
        view: &TiledPanes,
        size: (f32, f32),
    ) -> Element<'_, Message> {
        let shares = self.pane_shares_for(view);
        let lines = splitters(view.tabs.len(), size, view.layout, shares);
        layer(lines.into_iter().map(|line| {
            let (bounds, interaction) = if line.vertical {
                (
                    Rectangle {
                        x: line.at - SPLITTER_GRIP / 2.0,
                        y: line.from,
                        width: SPLITTER_GRIP,
                        height: line.length,
                    },
                    mouse::Interaction::ResizingHorizontally,
                )
            } else {
                (
                    Rectangle {
                        x: line.from,
                        y: line.at - SPLITTER_GRIP / 2.0,
                        width: line.length,
                        height: SPLITTER_GRIP,
                    },
                    mouse::Interaction::ResizingVertically,
                )
            };
            let grip = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                .on_press(pane_msg(PaneMsg::SplitterPressed(line.index)))
                .on_double_click(pane_msg(PaneMsg::EqualizeRequested))
                .interaction(interaction)
                .boxed();
            (grip, bounds)
        }))
    }

    /// The pane menu while one is open: a click-outside layer and the card
    /// under its pane's ⋯ button, one row per pane command, greyed when the
    /// command cannot run.
    pub(in crate::app::view) fn pane_menu_overlay(&self) -> Option<Element<'_, Message>> {
        let pane = self.ui_state.workspace_layouts.panes.menu?;
        let view = self.tiled_pane_view()?;
        let area = self.document_area_in(None);
        let tile = *self
            .pane_cells(&view, (area.width, area.height))
            .get(pane)?;
        let tokens = &self.document_view.panel_ctx.tokens;
        let context = self.command_context(None);
        let rows = PANE_MENU_ROWS.iter().filter_map(|id| {
            let command = AppCommandId::new(*id).ok()?;
            let label = crate::menu_bar::cmd_label(&self.ui_state.i18n, id, id);
            let row = button(text(signex_i18n::UserText::new(label)).size(TS.body))
                .width(Length::Fill)
                .padding([4.0, 10.0])
                .style(crate::styles::ghost_button(tokens));
            let row = if is_enabled(&command, &context) {
                row.on_press(pane_msg(PaneMsg::MenuCommandPicked { pane, command }))
            } else {
                row
            };
            Some(row.boxed())
        });
        let card = container(Column::with_children(rows).width(Length::Fill))
            .padding(4.0)
            .style(crate::styles::context_menu(tokens))
            .boxed();
        let right = area.x + tile.position.0 + tile.size.0;
        let bounds = Rectangle {
            x: (right - PANE_MENU_WIDTH).max(area.x),
            y: area.y + tile.position.1 + PANE_STRIP_HEIGHT,
            width: PANE_MENU_WIDTH,
            height: 4.0 * 2.0 + PANE_MENU_ROWS.len() as f32 * 30.0,
        };
        let dismiss =
            Self::dismiss_layer_tinted(pane_msg(PaneMsg::MenuDismissed), Color::TRANSPARENT);
        Some(Stack::with_children([dismiss, layer([(card, bounds)])]).boxed())
    }
}
