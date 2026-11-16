//! Select Net and Highlight Net on a board (#2403, #2405): the net named
//! by a Net Inspector row, else the net of the selection, else of what is
//! under the cursor. Select puts its tracks, vias, pads and drawn areas in
//! the selection; Highlight draws its copper bright until it is pressed
//! again for the same net or Esc is pressed. Neither edits the board.

use pcb_geom::{ItemRef, PickKind, Shape};
use signex_i18n::{ids, t};
use signex_types::pcb::PcbBoard;

use crate::app::state::pcb_query::BoardNetHighlight;
use crate::app::{BoardNetMsg, Signex};

/// The net of `item` on `board`; `None` for a footprint body and for an
/// item on no net.
fn net_of(board: &PcbBoard, item: ItemRef) -> Option<u32> {
    match item {
        ItemRef::Zone { zone, .. } => board
            .zones
            .iter()
            .find(|z| z.uuid == zone)
            .map(|z| z.net)
            .filter(|net| *net != 0),
        other => pcb_geom::item_net(board, other),
    }
}

fn net_name(board: &PcbBoard, net: u32) -> String {
    board
        .nets
        .iter()
        .find(|def| def.number == net)
        .map_or_else(|| net.to_string(), |def| def.name.clone())
}

/// Every track, via, pad and selectable drawn area of `net`.
pub(crate) fn net_members(board: &PcbBoard, net: u32) -> Vec<ItemRef> {
    let mut items = pcb_geom::net_items(board, net);
    items.extend(board.zones.iter().filter(|z| z.net == net).filter_map(|z| {
        Some(ItemRef::Zone {
            zone: z.uuid,
            kind: PickKind::of_zone(z)?,
        })
    }));
    items
}

/// The copper of `net`: what Highlight Net draws.
fn net_copper(board: &PcbBoard, net: u32) -> Vec<Shape> {
    let tracks = board
        .segments
        .iter()
        .filter(|s| s.net == net)
        .filter_map(|s| pcb_geom::track_shape(s).ok());
    let vias = board
        .vias
        .iter()
        .filter(|v| v.net == net)
        .filter_map(|v| pcb_geom::via_shape(v).ok());
    let pads = board.footprints.iter().flat_map(|fp| {
        fp.pads
            .iter()
            .filter(|p| p.net.as_ref().is_some_and(|n| n.number == net))
            .filter_map(move |p| pcb_geom::pad_shape(fp, p).ok())
    });
    tracks.chain(vias).chain(pads).collect()
}

impl Signex {
    pub(crate) fn handle_board_net(&mut self, event: BoardNetMsg) {
        match event {
            BoardNetMsg::SelectRequested(name) => self.select_board_net(name.as_deref()),
            BoardNetMsg::HighlightRequested(name) => self.highlight_board_net(name.as_deref()),
        }
    }

    /// The net an action is for, with its name; says why in the Messages
    /// panel when there is none.
    fn board_net_target(&mut self, named: Option<&str>) -> Option<(u32, String)> {
        let i18n = self.ui_state.i18n.clone();
        let found = match named {
            Some(name) => {
                let found = self
                    .active_pcb()
                    .and_then(|board| pcb_geom::net_named(board, name));
                if found.is_none() {
                    let line = t!(i18n, ids::BOARD_NET_UNKNOWN, "net" => name);
                    crate::diagnostics::log_info(line.into_string());
                    return None;
                }
                found
            }
            None => self
                .with_pcb_selection(|sel, board| {
                    sel.selection()
                        .items()
                        .chain(sel.hover())
                        .find_map(|item| net_of(board, item))
                })
                .flatten(),
        };
        let Some(net) = found else {
            crate::diagnostics::log_info(t!(i18n, ids::BOARD_NET_NOTHING).into_string());
            return None;
        };
        let name = self.active_pcb().map(|board| net_name(board, net))?;
        Some((net, name))
    }

    /// Select Net: select every member of the net.
    pub(crate) fn select_board_net(&mut self, named: Option<&str>) {
        let Some((net, name)) = self.board_net_target(named) else {
            return;
        };
        let Some(items) = self.active_pcb().map(|board| net_members(board, net)) else {
            return;
        };
        self.select_pcb_items(&items);
        let line = t!(
            self.ui_state.i18n,
            ids::BOARD_NET_SELECTED,
            "net" => name.as_str(),
            "count" => items.len()
        );
        crate::diagnostics::log_info(line.into_string());
    }

    /// Highlight Net: draw the net bright, or no longer when it already is.
    pub(crate) fn highlight_board_net(&mut self, named: Option<&str>) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return;
        };
        let Some((net, name)) = self.board_net_target(named) else {
            return;
        };
        let query = &mut self.interaction_state.pcb_query;
        let already = query
            .net_highlight
            .as_ref()
            .is_some_and(|h| h.path == path && h.net == net);
        let id = if already {
            query.net_highlight = None;
            ids::BOARD_NET_UNHIGHLIGHTED
        } else {
            query.net_highlight = Some(BoardNetHighlight {
                path,
                net,
                name: name.clone(),
            });
            ids::BOARD_NET_HIGHLIGHTED
        };
        let line = t!(self.ui_state.i18n, id, "net" => name.as_str());
        crate::diagnostics::log_info(line.into_string());
        self.refresh_pcb_highlight();
    }

    /// Esc on a board: drop the net highlight of the board in view. Whether
    /// there was one; the caller repaints the board.
    pub(crate) fn clear_board_net_highlight(&mut self) -> bool {
        let path = self.active_tab_path();
        let query = &mut self.interaction_state.pcb_query;
        let shown = query
            .net_highlight
            .take_if(|h| path.as_deref() == Some(h.path.as_path()));
        let Some(cleared) = shown else {
            return false;
        };
        let line =
            t!(self.ui_state.i18n, ids::BOARD_NET_UNHIGHLIGHTED, "net" => cleared.name.as_str());
        crate::diagnostics::log_info(line.into_string());
        true
    }

    /// The copper the canvas of the board in view draws bright.
    pub(crate) fn board_net_shapes(&self) -> Vec<Shape> {
        let Some(highlight) = self.interaction_state.pcb_query.net_highlight.as_ref() else {
            return Vec::new();
        };
        if self.active_tab_path().as_deref() != Some(highlight.path.as_path()) {
            return Vec::new();
        }
        self.document_state
            .pcb_board(&highlight.path)
            .map(|board| net_copper(board, highlight.net))
            .unwrap_or_default()
    }
}
