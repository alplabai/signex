//! `Command::BreakWire`: split one wire in two at a point (#1254).

use crate::*;
use signex_net::{point_on_segment, pt_key};
use signex_types::schematic::{Point, SelectedItem, SelectedKind};
use uuid::Uuid;

impl Engine {
    /// Cut `wire` at `at`. The first half keeps the wire's id and place in
    /// the sheet, the second is a new wire right after it with the same
    /// width and lock. A point that is not strictly inside the wire changes
    /// nothing and records no history; a spot holding a pin, bus end or
    /// another wire's end is refused with `BreakWireBlocked`. No merge pass runs, so the halves stay
    /// apart; connectivity is unchanged (two wires meeting end to end).
    pub(super) fn break_wire(
        &mut self,
        before: SchematicSheet,
        wire: Uuid,
        at: Point,
    ) -> Result<CommandResult, EngineError> {
        let Some(index) = self.document.wires.iter().position(|w| w.uuid == wire) else {
            return Ok(CommandResult::unchanged());
        };
        let Some(cut) = cut_point(&self.document.wires[index], at) else {
            return Ok(CommandResult::unchanged());
        };

        // A new wire end at a spot that holds a pin or another wire's end
        // would join that element's net, which the wire passing by did not.
        let key = pt_key(&cut);
        let other_end = self
            .document
            .wires
            .iter()
            .enumerate()
            .any(|(i, w)| i != index && (pt_key(&w.start) == key || pt_key(&w.end) == key));
        if other_end || self.connection_points().contains(&key) {
            return Err(EngineError::BreakWireBlocked);
        }

        let first = &mut self.document.wires[index];
        let mut second = first.clone();
        second.uuid = Uuid::new_v4();
        second.start = cut;
        first.end = cut;
        let items = [
            SelectedItem::new(first.uuid, SelectedKind::Wire),
            SelectedItem::new(second.uuid, SelectedKind::Wire),
        ];
        self.document.wires.insert(index + 1, second);

        let patch_pair = PatchPair {
            semantic: SemanticPatch::ObjectPlaced,
            document: self.reconciled_patch(&items),
        };
        self.record_history(before, patch_pair);
        Ok(CommandResult::changed(patch_pair))
    }
}

/// How far from a horizontal or vertical wire a request may land and still
/// cut it, mm: an unsnapped click lands near the wire, not exactly on it.
const BREAK_TOLERANCE_MM: f64 = 0.25;

/// Where the cut falls on `wire` for a request at `at`: `None` unless `at` is
/// on the wire and not at either end. A horizontal or vertical wire is cut at
/// the point of the wire nearest `at`, within [`BREAK_TOLERANCE_MM`]; a
/// diagonal one only where `at` is exactly on it in the netlist's 1 um key
/// space, so labels and pins on it stay on one half.
fn cut_point(wire: &signex_types::schematic::Wire, at: Point) -> Option<Point> {
    let (a, b) = (pt_key(&wire.start), pt_key(&wire.end));
    let (start, end) = (wire.start, wire.end);
    let on_line = if a.1 == b.1 {
        let within = at.x > start.x.min(end.x) && at.x < start.x.max(end.x);
        (within && (at.y - start.y).abs() <= BREAK_TOLERANCE_MM).then(|| Point::new(at.x, start.y))
    } else if a.0 == b.0 {
        let within = at.y > start.y.min(end.y) && at.y < start.y.max(end.y);
        (within && (at.x - start.x).abs() <= BREAK_TOLERANCE_MM).then(|| Point::new(start.x, at.y))
    } else {
        let k = pt_key(&at);
        point_on_segment(k, a, b).then_some(at)
    }?;
    // Too close to an end to leave a wire of its own in the netlist's key space.
    let k = pt_key(&on_line);
    (k != a && k != b).then_some(on_line)
}

impl Engine {
    /// The wire a Break Wire request at `at` would cut, for a caller that has
    /// only a point: the first wire in sheet order that [`Command::BreakWire`]
    /// would split there, `None` when there is none.
    #[must_use]
    pub fn wire_to_break_at(&self, at: Point) -> Option<Uuid> {
        self.document
            .wires
            .iter()
            .find(|w| cut_point(w, at).is_some())
            .map(|w| w.uuid)
    }
}
