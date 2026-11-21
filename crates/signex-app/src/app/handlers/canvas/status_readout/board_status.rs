//! What the board counters of a window's status bar show, and the popover
//! one of them opened (#2460). Every number is read from state `update`
//! keeps: the connection counts cached with the ratsnest, the last DRC run,
//! the window's selection and the clock the last tick stored.

use std::path::{Path, PathBuf};

use iced::Element;
use iced::widget::Widget as _;
use signex_renderer::pcb::board_layers;
use signex_types::pcb::PcbBoard;

use super::super::super::super::*;
use crate::status_bar::{
    AnchorBar, BoardPopoverKind, BoardPopoverModel, BoardReadout, ConnectionTally, DrcAge, DrcList,
    DrcTally, GridChoice, LayerChoice, LayerDropdownGroup, LayerList, NetRow, SelectedRow,
    SelectedWhat, SelectionList, UnroutedList, group_layers,
};

/// Rows the Selection and DRC popovers list; the rest are counted.
const POPOVER_ROWS: usize = 50;

/// The keymap commands whose keys the popovers name.
const NEXT_LAYER: &str = "next_layer";
const PREV_LAYER: &str = "prev_layer";
const NEXT_GRID: &str = "next_grid";
const ROUTE: &str = "place_board_route";

impl Signex {
    /// The board `window` shows as a board tab, with its path: the main
    /// window's active board, or an undocked window's own.
    pub(crate) fn status_board_target(
        &self,
        window: iced::window::Id,
    ) -> Option<(PathBuf, &PcbBoard)> {
        if self.ui_state.main_window_id == Some(window) {
            return Some((self.active_tab_path()?, self.active_pcb()?));
        }
        let path = self.undocked_pcb_path(window)?;
        let board = self.document_state.pcb_board(&path)?;
        Some((path, board))
    }

    /// The board selection of `window`, if it is of the board at `path`.
    fn status_board_selection(
        &self,
        window: iced::window::Id,
        path: &Path,
    ) -> Option<&pcb_geom::Selection> {
        let interaction = &self.interaction_state;
        let state = if self.ui_state.main_window_id == Some(window) {
            &interaction.pcb_selection
        } else {
            interaction.pcb_selections.get(&window)?
        };
        (state.document() == Some(path)).then(|| state.selection())
    }

    /// The board counters of `window`'s bar; `None` unless it shows a board.
    pub(crate) fn status_board(&self, window: iced::window::Id) -> Option<BoardReadout> {
        let (path, _) = self.status_board_target(window)?;
        let revision = self.board_revision(&path);
        let connections = self
            .interaction_state
            .pcb_ratsnest
            .counts_for(&path, revision)
            .map(|counts| ConnectionTally {
                routed: counts.routed(),
                total: counts.total,
                unrouted: counts.unrouted,
            });
        let drc = match self.interaction_state.pcb_drc.done_for(&path) {
            None => DrcTally::NotRun,
            Some(done) => DrcTally::Ran {
                violations: done.report.violations.len(),
                age: DrcAge::between(
                    done.finished_at,
                    self.ui_state.board_status.now.unwrap_or(done.finished_at),
                ),
            },
        };
        let selected = self
            .status_board_selection(window, &path)
            .map_or(0, pcb_geom::Selection::len);
        let open = self
            .ui_state
            .board_status
            .popover
            .as_ref()
            .filter(|p| p.window == window)
            .map(|p| p.kind);
        Some(BoardReadout {
            connections,
            drc,
            selected,
            open,
        })
    }

    /// What the open popover of `window` lists, if one is open there over a
    /// board.
    pub(crate) fn board_popover_model(
        &self,
        window: iced::window::Id,
    ) -> Option<BoardPopoverModel> {
        let popover = self.ui_state.board_status.popover.as_ref()?;
        if popover.window != window {
            return None;
        }
        let (path, board) = self.status_board_target(window)?;
        let keymap = &self.ui_state.active_keymap;
        Some(match popover.kind {
            BoardPopoverKind::Layer if !self.shows_pcb_layer_strip() => {
                BoardPopoverModel::LayerDropdown(self.layer_dropdown(&path, board))
            }
            BoardPopoverKind::Layer | BoardPopoverKind::ActiveLayer => {
                BoardPopoverModel::Layer(self.layer_list(&path, board))
            }
            BoardPopoverKind::Grid => BoardPopoverModel::Grid(GridChoice {
                current_mm: signex_engine::pcb::placement::grid_step_mm(board) as f32,
                custom: popover.custom_grid.clone(),
                refused: popover.custom_refused,
                snap: self.ui_state.snap_enabled,
                cycle_key: keymap.shortcut_label_for_id(NEXT_GRID),
            }),
            BoardPopoverKind::Unrouted => {
                BoardPopoverModel::Unrouted(self.unrouted_list(&path, board))
            }
            BoardPopoverKind::Drc => BoardPopoverModel::Drc(self.drc_list(&path, board)),
            BoardPopoverKind::Selection => {
                BoardPopoverModel::Selection(self.selection_list(window, &path, board))
            }
        })
    }

    /// The layers the next-layer key steps through: the board's, less the
    /// hidden ones, named and coloured as the Layers panel shows them.
    fn layer_list(&self, path: &Path, board: &PcbBoard) -> LayerList {
        let view = self.pcb_layer_view(path);
        let i18n = &self.ui_state.i18n;
        let rows = board_layers(&board.layers)
            .into_iter()
            .filter(|layer| !view.is_hidden(*layer))
            .map(|layer| LayerChoice {
                layer,
                name: crate::pcb_canvas::layer_style::layer_name(i18n, layer),
                colour: crate::pcb_canvas::layer_style::painted_layer_colour(view.paint(), layer),
                active: view.active() == layer,
            })
            .collect();
        let keymap = &self.ui_state.active_keymap;
        LayerList {
            rows,
            next_key: keymap.shortcut_label_for_id(NEXT_LAYER),
            prev_key: keymap.shortcut_label_for_id(PREV_LAYER),
        }
    }

    /// The Layer pill's dropdown: the layers the strip would list, by group.
    fn layer_dropdown(&self, path: &Path, board: &PcbBoard) -> Vec<LayerDropdownGroup> {
        let view = self.pcb_layer_view(path);
        let i18n = &self.ui_state.i18n;
        let active = view.shown_active();
        let rows = crate::app::view::pcb_strip::strip_layers(board, &view)
            .into_iter()
            .map(|layer| LayerChoice {
                layer,
                name: crate::pcb_canvas::layer_style::layer_name(i18n, layer),
                colour: crate::pcb_canvas::layer_style::painted_layer_colour(view.paint(), layer),
                active: active == Some(layer),
            })
            .collect();
        group_layers(rows)
    }

    /// The unrouted connections of the board, most unrouted net first.
    fn unrouted_list(&self, path: &Path, board: &PcbBoard) -> UnroutedList {
        let route_key = self.ui_state.active_keymap.shortcut_label_for_id(ROUTE);
        let revision = self.board_revision(path);
        let Some(counts) = self
            .interaction_state
            .pcb_ratsnest
            .counts_for(path, revision)
        else {
            return UnroutedList {
                unrouted: 0,
                nets: Vec::new(),
                route_key,
            };
        };
        let name_of = |net: u32| {
            board
                .nets
                .iter()
                .find(|def| def.number == net)
                .map(|def| def.name.clone())
                .unwrap_or_else(|| format!("#{net}"))
        };
        let nets = counts
            .nets_by_unrouted()
            .into_iter()
            .filter_map(|net| {
                let unrouted = counts.per_net.get(&net)?;
                Some(NetRow {
                    net,
                    name: name_of(net),
                    pairs: unrouted.pads.clone(),
                    pour_will_connect: unrouted.pour_will_connect,
                })
            })
            .collect();
        UnroutedList {
            unrouted: counts.unrouted,
            nets,
            route_key,
        }
    }

    /// The last DRC run of the board: when, the edits since, its violations.
    fn drc_list(&self, path: &Path, board: &PcbBoard) -> DrcList {
        let drc = &self.interaction_state.pcb_drc;
        let running = drc.is_running_for(path);
        let Some(done) = drc.done_for(path) else {
            return DrcList {
                last_run: None,
                running,
                rows: Vec::new(),
                more: 0,
            };
        };
        let time = chrono::DateTime::<chrono::Local>::from(done.finished_at)
            .format("%H:%M:%S")
            .to_string();
        let edits = done.edits_since(self.board_revision(path));
        let mut rows = self.pcb_drc_panel_for(path, board).rows;
        let total = done.report.violations.len();
        rows.truncate(POPOVER_ROWS);
        DrcList {
            last_run: Some((time, edits)),
            running,
            more: total.saturating_sub(rows.len()),
            rows,
        }
    }

    /// The objects selected on the board in `window`.
    fn selection_list(
        &self,
        window: iced::window::Id,
        path: &Path,
        board: &PcbBoard,
    ) -> SelectionList {
        let selection = self.status_board_selection(window, path);
        let count = selection.map_or(0, pcb_geom::Selection::len);
        let rows = selection
            .into_iter()
            .flat_map(pcb_geom::Selection::items)
            .take(POPOVER_ROWS)
            .map(|item| SelectedRow {
                item,
                what: selected_what(board, item),
            })
            .collect();
        SelectionList {
            count,
            rows,
            linked: self.document_state.board_project(path, None).is_some(),
        }
    }

    /// The open board popover of `window`, placed above the segment that
    /// opened it; `None` when none is open there.
    pub(crate) fn board_popover_card(
        &self,
        window: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        let model = self.board_popover_model(window)?;
        let board = self.status_board(window)?;
        let ui = &self.ui_state;
        let tokens = &self.document_state.panel_ctx.tokens;
        let left = self.status_popover_left(window, model.kind(), &board);
        let card = crate::status_bar::board_popover_view(&model, tokens, &ui.i18n)
            .map(|event| Message::Ui(UiMsg::StatusBar(StatusBarRequest::BoardPopover(event))))
            .boxed();
        // The strip and its set button are hidden under Dropdown, so the
        // layer list carries the layer sets.
        let card: Element<'_, Message> = if matches!(model, BoardPopoverModel::LayerDropdown(_)) {
            iced::widget::column![self.layer_dropdown_sets(window), card]
                .spacing(crate::status_bar::POPUP_GAP)
                .boxed()
        } else {
            card
        };
        // The card claims the pointer over itself, so a press on its padding
        // does not reach the click-outside layer.
        let card = iced::widget::mouse_area(card).interaction(iced::mouse::Interaction::Idle);
        let status_h = crate::status_bar::status_bar_height(tokens);
        Some(
            iced::widget::container(card)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .padding(iced::Padding {
                    top: 0.0,
                    right: 0.0,
                    bottom: status_h + crate::status_bar::POPUP_GAP,
                    left,
                })
                .align_x(iced::alignment::Horizontal::Left)
                .align_y(iced::alignment::Vertical::Bottom)
                .boxed(),
        )
    }

    /// Where a layer set card opened from the layer dropdown goes: its left
    /// edge at the dropdown's, its bottom just above the status bar.
    pub(crate) fn layer_set_card_anchor(&self, window: iced::window::Id) -> (f32, f32) {
        let left = self.status_board(window).map_or(0.0, |board| {
            self.status_popover_left(window, BoardPopoverKind::Layer, &board)
        });
        let status_h = crate::status_bar::status_bar_height(&self.document_state.panel_ctx.tokens);
        let (_, height) = self.ui_state.size_of_window(window);
        (left, height - status_h - crate::status_bar::POPUP_GAP)
    }

    /// The left edge of the `kind` card in `window`'s bar.
    fn status_popover_left(
        &self,
        window: iced::window::Id,
        kind: BoardPopoverKind,
        board: &BoardReadout,
    ) -> f32 {
        let ui = &self.ui_state;
        let grid = self.status_grid(window);
        let angle = self.status_view_angle(window);
        let window_width = if ui.main_window_id == Some(window) {
            ui.window_size.0
        } else {
            ui.undocked_window_sizes
                .get(&window)
                .map_or(ui.window_size.0, |size| size.0)
        };
        let bar = AnchorBar {
            grid_value: crate::status_bar::board_grid_value(
                &ui.i18n,
                grid.visible,
                grid.size_mm,
                grid.unit,
            ),
            snap_enabled: ui.snap_enabled,
            zoom: self.status_zoom(window),
            view_angle: angle.as_ref(),
            board,
            window_width,
        };
        crate::status_bar::board_popover_left(kind, &bar, &ui.i18n)
    }
}

/// What a selected object is: a part by its designator, value and
/// footprint, anything else by its kind.
fn selected_what(board: &PcbBoard, item: pcb_geom::ItemRef) -> SelectedWhat {
    if let pcb_geom::ItemRef::Footprint(id) = item
        && let Some(part) = board.footprints.iter().find(|fp| fp.uuid == id)
    {
        return SelectedWhat::Part {
            designator: part.reference.clone(),
            value: part.value.clone(),
            footprint: part.footprint_id.clone(),
        };
    }
    SelectedWhat::Kind(pcb_geom::PickKind::of(item))
}
