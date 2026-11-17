//! The Net Inspector with a board in view (#2403): the board's nets, the
//! panel's filter applied, with what `pcb_geom::net_reports` reads for each.

use signex_types::pcb::PcbBoard;

use crate::panels::NetInspectorState;
use crate::panels::net_inspector::{BoardNetRow, BoardNetView};

/// The rows the panel lists for `board`: the nets whose name holds the
/// filter text (any case), by name.
pub(crate) fn board_net_rows(board: &PcbBoard, state: &NetInspectorState) -> BoardNetView {
    let needle = state.filter.trim().to_lowercase();
    let reports = pcb_geom::net_reports(board);
    let total = reports.len();
    let nets = reports
        .into_iter()
        .filter(|r| needle.is_empty() || r.name.to_lowercase().contains(&needle))
        .map(|r| BoardNetRow {
            name: r.name,
            routed_length_mm: r.routed_length_mm,
            vias: r.vias,
            pads: r.pads,
            unrouted: r.unrouted,
        })
        .collect();
    BoardNetView { nets, total }
}
