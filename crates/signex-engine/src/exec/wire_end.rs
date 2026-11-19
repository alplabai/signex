//! `Command::MoveWireEnd`: drag one end of a wire by its grip.
//!
//! With "keep wires" the grip is checked as it is committed, like a move
//! (`exec::guarded`): the plain end move `P` (the end moved, junctions
//! reconciled, wires folded) is the fallback, and the grip rule (the wires
//! attached at the old end go with it; a pin or label there is left) is the
//! reference. Each try is the full commit; the first whose nets are the
//! reference's is kept: the square neighbours shifted sideways, then every
//! attached wire following the end, then `P` with a notice.

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
        let patch = if keep_wires {
            self.guarded_grip(wire, start, old, to, &pre_wires)
        } else {
            let mut patch = self.reconciled_patch(&[SelectedItem::new(wire, SelectedKind::Wire)]);
            patch |= self.merge_after_move(&pre_wires);
            patch
        };
        let patch_pair = PatchPair {
            semantic: SemanticPatch::SelectionMoved,
            document: patch,
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }

    /// The grip's follow, checked as committed (see the module docs);
    /// `self.document` has the end at `to` already.
    fn guarded_grip(
        &mut self,
        wire: Uuid,
        start: bool,
        old: Point,
        to: Point,
        pre_wires: &crate::PreWires,
    ) -> DocumentPatch {
        let item = SelectedItem::new(wire, SelectedKind::Wire);
        let moved = self.document.clone();
        // P: the plain end move, the fallback.
        let mut plain_patch = self.reconciled_patch(&[item]);
        plain_patch |= self.merge_after_move(pre_wires);
        let plain = std::mem::take(&mut self.document);
        let Some(reference) = grip_reference(&plain, wire, old, to) else {
            // Nothing was attached at the old end: the plain move is the grip.
            self.document = plain;
            return plain_patch;
        };
        let want = signex_net::net_signatures(&reference);
        for squares in [true, false] {
            self.document = moved.clone();
            let follow = crate::follow_wire_grip(&mut self.document, wire, start, old, squares);
            let mut touched = vec![item];
            touched.extend(follow.touched);
            let mut patch = self.reconciled_patch(&touched);
            patch |= self.merge_after_move(pre_wires);
            if signex_net::net_signatures(&self.document) == want {
                return patch;
            }
        }
        self.document = plain;
        if self.preset_sheet.is_none() {
            self.warnings.push(format!(
                "could not keep the wires at ({}, {}) on the dragged end",
                old.x, old.y
            ));
        }
        plain_patch
    }
}

/// The grip rule on the plain end move `plain`: every wire attached at the
/// old end `old` (but the dragged one) ends at `to` instead, and a locked one
/// stays and gets a straight wire from `old` to `to`, as the follow keeps a
/// locked wire with a new path. `None` when nothing is attached there.
fn grip_reference(
    plain: &SchematicSheet,
    wire: Uuid,
    old: Point,
    to: Point,
) -> Option<SchematicSheet> {
    let k = pt_key(&old);
    let mut reference = plain.clone();
    let mut attached = false;
    let mut locked = false;
    for w in reference.wires.iter_mut().filter(|w| w.uuid != wire) {
        let (at_start, at_end) = (pt_key(&w.start) == k, pt_key(&w.end) == k);
        if !(at_start || at_end) {
            continue;
        }
        attached = true;
        if w.locked {
            locked = true;
        } else if at_start {
            w.start = to;
        } else {
            w.end = to;
        }
    }
    if !attached {
        return None;
    }
    if locked {
        reference.wires.push(signex_types::schematic::Wire {
            uuid: Uuid::nil(),
            start: old,
            end: to,
            ..Default::default()
        });
    }
    Some(reference)
}
