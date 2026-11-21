//! Vias moved by a drag: a press on a via carries it, the release moves it
//! as one board undo step through `signex_engine::pcb::placement`, which
//! refuses a locked via. The track ends on it follow as they do for a
//! footprint's pads (#2379); with Ctrl held they stay.

use std::collections::BTreeSet;
use std::path::Path;

use signex_engine::pcb::placement;
use signex_types::pcb::Point;
use uuid::Uuid;

use super::pcb_placement::refuse;
use crate::app::Signex;
use crate::pcb_canvas::{CarryKind, PcbCarry, via_carry_delta};

impl Signex {
    /// A press at `from` on a via starts a drag of it. Whether it did. A
    /// locked via refuses at once, and the press then carries nothing.
    pub(super) fn start_via_drag(&mut self, path: &Path, from: Point, reach: f64) -> bool {
        let Some(vias) = self
            .with_pcb_selection(|state, board| state.grab_vias_for_drag(board, from, reach))
            .flatten()
        else {
            return false;
        };
        let refused = self
            .active_pcb()
            .and_then(|board| placement::editable_vias(board, &vias).err());
        let window = self.interaction_state.acting_window;
        let mut carry = PcbCarry::new(CarryKind::Drag, path.to_path_buf(), window, BTreeSet::new());
        carry.anchor = Some(from);
        match refused {
            Some(refusal) => refuse("Move", &refusal),
            None => {
                carry.via_tracks = self
                    .active_pcb()
                    .is_some_and(|board| !pcb_geom::tracks_on_vias(board, &vias).is_empty());
                carry.vias = vias;
            }
        }
        self.interaction_state.pcb_carry = Some(carry);
        self.refresh_pcb_highlight();
        true
    }

    /// The offset of the carried `vias` for a pointer gone from `from` to
    /// `to`, under the Snap options.
    pub(super) fn via_drag_delta(
        &self,
        vias: &BTreeSet<Uuid>,
        from: Point,
        to: Point,
    ) -> Option<Point> {
        let board = self.active_pcb()?;
        let snap = &self.ui_state.pcb_snap;
        Some(via_carry_delta(
            board,
            vias,
            from,
            to,
            snap,
            self.pcb_active_layer(),
        ))
    }

    /// Release a via drag at `to`: move the vias as one step.
    pub(super) fn drop_via_carry(&mut self, carry: &PcbCarry, from: Point, to: Point) {
        let built = self
            .via_drag_delta(&carry.vias, from, to)
            .and_then(|delta| {
                let board = self.active_pcb()?;
                Some(if carry.leaves_tracks {
                    placement::move_vias_only(board, &carry.vias, delta)
                } else {
                    placement::move_vias(board, &carry.vias, delta)
                })
            });
        self.end_pcb_carry();
        if let Some(built) = built {
            self.run_placement("Move", built);
        }
    }
}
