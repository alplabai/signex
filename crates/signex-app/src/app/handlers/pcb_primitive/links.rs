//! The net and class links and the net pick of the primitive panels: each
//! reads the board in view and selects or edits through the usual paths.

use std::path::Path;

use pcb_geom::ItemRef;
use signex_i18n::{ids, t};
use signex_types::pcb::Point;

use super::PrimitiveEdit;
use crate::app::Signex;

impl Signex {
    /// Select every track, via and pad of `net` on the board at `board`,
    /// when it is the board in view.
    pub(super) fn select_pcb_net(&mut self, board: &Path, net: u32) {
        if self.active_tab_path().as_deref() != Some(board) {
            return;
        }
        let Some(items) = self.active_pcb().map(|b| pcb_geom::net_items(b, net)) else {
            return;
        };
        self.select_pcb_items(&items);
    }

    /// Select the copper of every net of `class` on the board at `board`.
    pub(super) fn select_pcb_net_class(&mut self, board: &Path, class: &str) {
        if self.active_tab_path().as_deref() != Some(board) {
            return;
        }
        let Some(pcb) = self.active_pcb() else {
            return;
        };
        let Some(table) = self.active_net_classes() else {
            return;
        };
        let items: Vec<ItemRef> = pcb
            .nets
            .iter()
            .filter(|n| {
                signex_types::net_class::first_matching_class(table, &n.name)
                    .is_some_and(|c| c.name == class)
            })
            .flat_map(|n| pcb_geom::net_items(pcb, n.number))
            .collect();
        self.select_pcb_items(&items);
    }

    /// A click on the board while the Net pick button waits: the net of the
    /// copper under `at` goes to the waiting items, and the pick ends.
    /// Whether the click was taken.
    pub(crate) fn pcb_net_pick_clicked(&mut self, at: Point, tolerance_mm: f64) -> bool {
        let Some(target) = self.interaction_state.pcb_primitive.net_pick.clone() else {
            return false;
        };
        if self.active_tab_path().as_deref() != Some(target.board.as_path()) {
            return false;
        }
        self.interaction_state.pcb_primitive.net_pick = None;
        let net = self
            .with_pcb_selection(|sel, board| {
                sel.item_at(board, at, tolerance_mm)
                    .and_then(|item| pcb_geom::item_net(board, item))
            })
            .flatten();
        match net {
            Some(net) => {
                self.apply_primitive_edit(&target, PrimitiveEdit::Net(net), ids::PCB_PRIM_NET)
            }
            None => {
                let line = t!(self.ui_state.i18n, ids::PCB_PRIM_NO_NET_THERE);
                crate::diagnostics::log_warning(line.into_string());
            }
        }
        self.sync_pcb_board_summary();
        true
    }
}
