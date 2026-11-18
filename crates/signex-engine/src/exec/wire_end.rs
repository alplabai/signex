//! `Command::MoveWireEnd`: drag one end of a wire by its grip.

use crate::*;
use signex_net::pt_key;
use signex_types::schematic::{Point, SelectedItem, SelectedKind};
use uuid::Uuid;

impl Engine {
    /// Put one end of `wire` at `to` (on the wire's own line when it is
    /// horizontal or vertical); with `keep_wires` the wires attached at the
    /// old end follow it (see `transform::rubber_band::wire_contacts`). The
    /// move, the follow and the collinear fold are one history entry.
    pub(super) fn move_wire_end(
        &mut self,
        before: SchematicSheet,
        wire: Uuid,
        start: bool,
        to: Point,
        keep_wires: bool,
    ) -> Result<CommandResult, EngineError> {
        let Some(index) = self.document.wires.iter().position(|w| w.uuid == wire) else {
            return Ok(CommandResult::unchanged());
        };
        let (old, other) = {
            let w = &self.document.wires[index];
            if start {
                (w.start, w.end)
            } else {
                (w.end, w.start)
            }
        };
        // A straight wire stays straight: its end slides along its own line,
        // so the grip makes it longer or shorter.
        let to = match crate::transform::rubber_band::wire_axis(old, other) {
            Some(true) => Point::new(to.x, old.y),
            Some(false) => Point::new(old.x, to.y),
            None => to,
        };
        let target = pt_key(&to);
        if !(to.x.is_finite() && to.y.is_finite())
            || target == pt_key(&old)
            || target == pt_key(&other)
        {
            return Ok(CommandResult::unchanged());
        }
        let pre_wires = self.pre_wires();
        let dragged = &mut self.document.wires[index];
        if start {
            dragged.start = to;
        } else {
            dragged.end = to;
        }
        let mut touched = vec![SelectedItem::new(wire, SelectedKind::Wire)];
        if keep_wires {
            let follow = crate::follow_wire_grip(&mut self.document, wire, start, old);
            touched.extend(follow.touched);
        }
        let mut patch = self.reconciled_patch(&touched);
        patch |= self.merge_after_move(&pre_wires);
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SelectionMoved,
            document: patch,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }
}
