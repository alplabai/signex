//! Writing a cross selection into a sheet or board canvas, and framing it.

use std::path::Path;

use pcb_geom::ItemRef;
use signex_types::pcb::{Footprint, Pad, PcbBoard, Point};
use signex_types::schematic::SelectedItem;

use crate::app::Signex;
use crate::app::handlers::canvas::fit_view::fit_rect;
use crate::app::state::cross_select::{CrossSelection, SelectSurface};

use super::surfaces::board_items_for;

impl Signex {
    /// Passive cross select: the selection on `source` changed, so every
    /// surface on the other side selects what it stands for. An emptied
    /// selection empties them; one that stands for nothing leaves them be.
    pub(super) fn mirror_selection_from(&mut self, source: SelectSurface, source_empty: bool) {
        let what = self.cross_selection_of(source);
        if what.is_empty() && !source_empty {
            return;
        }
        if source.is_schematic() {
            for (window, path) in self.boards_shown() {
                let items = self.board_items(&path, &what);
                self.set_board_selection(window, &path, &items);
            }
        } else {
            for (window, path) in self.sheets_shown() {
                let items = self.sheet_items_for(&path, &what);
                self.set_sheet_selection(window, items);
            }
        }
    }

    /// The items on the board at `path` that `what` names.
    pub(super) fn board_items(&self, path: &Path, what: &CrossSelection) -> Vec<ItemRef> {
        self.document_state
            .pcb_board(path)
            .map(|board| board_items_for(board, what))
            .unwrap_or_default()
    }

    /// Select exactly `items` on the board canvas of `window` showing `path`
    /// (nothing, when empty) and repaint its highlight.
    pub(super) fn set_board_selection(
        &mut self,
        window: Option<iced::window::Id>,
        path: &Path,
        items: &[ItemRef],
    ) {
        let revision = self
            .document_state
            .pcb_revisions
            .get(path)
            .copied()
            .unwrap_or(0);
        let Some(board) = self.document_state.pcb_board(path) else {
            return;
        };
        let interaction = &mut self.interaction_state;
        let state = match window {
            None => &mut interaction.pcb_selection,
            Some(id) => interaction.pcb_selections.entry(id).or_default(),
        };
        state.sync(Some(path), revision);
        state.build_index(board);
        let changed = if items.is_empty() {
            state.clear_selection()
        } else {
            state.select_items(items)
        };
        for line in state.take_report() {
            crate::diagnostics::log_warning(line);
        }
        if !changed {
            return;
        }
        match window {
            None => self.refresh_pcb_highlight(),
            Some(_) => self.refresh_undocked_pcb_highlights(path),
        }
    }

    /// Select exactly `items` on the sheet canvas of `window`.
    pub(super) fn set_sheet_selection(
        &mut self,
        window: Option<iced::window::Id>,
        items: Vec<SelectedItem>,
    ) {
        match window {
            None => {
                if self.interaction_state.canvas.selected != items {
                    self.replace_selection(items);
                }
            }
            Some(id) => {
                if let Some(slot) = self.interaction_state.canvases.get_mut(&id)
                    && slot.selected != items
                {
                    slot.selected = items;
                    slot.clear_overlay_cache();
                }
            }
        }
    }

    /// Frame `items` on the board canvas of `window` showing `path`.
    pub(super) fn fit_board_items(
        &mut self,
        window: Option<iced::window::Id>,
        path: &Path,
        items: &[ItemRef],
    ) {
        let Some(bounds) = self
            .document_state
            .pcb_board(path)
            .and_then(|board| board_items_extent(board, items))
        else {
            return;
        };
        let rect = fit_rect(bounds);
        let interaction = &self.interaction_state;
        let canvas = match window {
            None => Some(&interaction.pcb_canvas),
            Some(id) => interaction.pcb_canvases.get(&id),
        };
        if let Some(canvas) = canvas {
            canvas.pending_fit.set(Some(rect));
        }
    }

    /// Frame the selection of the sheet canvas of `window`.
    pub(super) fn fit_sheet_selection(&mut self, window: Option<iced::window::Id>) {
        let Some(id) = window else {
            self.fit_selection();
            return;
        };
        let widths = self.active_wire_widths();
        let Some(slot) = self.interaction_state.canvases.get(&id) else {
            return;
        };
        let bounds = slot.active_snapshot().and_then(|snapshot| {
            crate::schematic_runtime::selection::selection_bounds(
                snapshot,
                &slot.selected,
                &slot.designators,
                widths,
            )
        });
        if let Some(b) = bounds {
            slot.pending_fit
                .set(Some(fit_rect([b.min_x, b.min_y, b.max_x, b.max_y])));
        }
    }
}

/// `[min_x, min_y, max_x, max_y]` in board mm over `items`: a footprint's
/// origin and pads, a pad's centre, a track's ends, a via's centre.
fn board_items_extent(board: &PcbBoard, items: &[ItemRef]) -> Option<[f64; 4]> {
    let mut points: Vec<Point> = Vec::new();
    for item in items {
        match *item {
            ItemRef::Footprint(id) => {
                if let Some(fp) = board.footprints.iter().find(|f| f.uuid == id) {
                    points.push(fp.position);
                    points.extend(fp.pads.iter().filter_map(|pad| pad_centre(fp, pad)));
                }
            }
            ItemRef::Pad { footprint, pad } => {
                let found = board.footprints.iter().find(|f| f.uuid == footprint);
                let centre = found.and_then(|fp| {
                    let pad = fp.pads.iter().find(|p| p.uuid == pad)?;
                    pad_centre(fp, pad)
                });
                points.extend(centre);
            }
            ItemRef::Track(id) => {
                for s in board.segments.iter().filter(|s| s.uuid == id) {
                    points.extend([s.start, s.end]);
                }
            }
            ItemRef::Via(id) => {
                points.extend(
                    board
                        .vias
                        .iter()
                        .filter(|v| v.uuid == id)
                        .map(|v| v.position),
                );
            }
        }
    }
    let first = points.first()?;
    let start = [first.x, first.y, first.x, first.y];
    Some(points.iter().fold(start, |[x0, y0, x1, y1], p| {
        [x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)]
    }))
}

/// Where `pad` of `footprint` sits on the board, in mm.
fn pad_centre(footprint: &Footprint, pad: &Pad) -> Option<Point> {
    pcb_geom::pad_world_center(footprint, pad)
        .ok()
        .map(pcb_geom::PointNm::to_mm)
}
