//! Who handles each old point of a move with "keep wires", decided once,
//! before either pass runs, so the wire-carry pass
//! (`transform::rubber_band::wire_contacts`) and the guarded pin pass
//! (`transform::rubber_band`) never both act on one point.
//!
//! A selected wire's point (an end, or a dot or sheet pin on its span, as
//! `moved_wire_contacts` lists them) is the carry pass's; a moved pin's old
//! tip is the pin pass's. Where they meet:
//!
//! * a pin moving by the wire's own delta rides on the wire: the carry pass
//!   has the point and the pin pass leaves the pin out;
//! * a pin moving by another delta keeps its point: the carry pass leaves it;
//! * a wire the carry pass cannot carry whole (a point whose edit would dot a
//!   contact that had no dot) moves plainly at every point, and the pins that
//!   rode on it go back to the pin pass.

use std::collections::HashSet;

use signex_net::{pt_key, uf::Key};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

/// A move's delta, in the netlist's 1 um key space.
type Delta = Key;

/// The moved pins and the selected wires' points of one move, read before it.
pub(super) struct Ownership {
    tips: Vec<(crate::PinTip, Delta)>,
    wire_points: Vec<(Uuid, Key, Delta)>,
}

impl Ownership {
    pub(super) fn of(document: &SchematicSheet, moves: &[(SelectedItem, f64, f64)]) -> Self {
        let mut out = Self {
            tips: Vec::new(),
            wire_points: Vec::new(),
        };
        for (item, dx, dy) in moves {
            let delta = pt_key(&Point::new(*dx, *dy));
            if item.kind != SelectedKind::Wire {
                let own = crate::moved_pin_tips(document, std::slice::from_ref(item));
                out.tips.extend(own.into_iter().map(|tip| (tip, delta)));
                continue;
            }
            // The points the carry pass carries: one source for both passes.
            let points = crate::moved_wire_contacts(document, std::slice::from_ref(item));
            out.wire_points
                .extend(points.iter().map(|c| (item.uuid, c.old_key(), delta)));
        }
        out
    }

    /// The old points the carry pass leaves to the pin pass: where a pin
    /// moving by another delta than the wire's sat.
    pub(super) fn pin_owned(&self) -> HashSet<Key> {
        (self.tips.iter())
            .filter(|(tip, delta)| {
                let k = tip.old_key();
                (self.wire_points.iter()).any(|(_, at, d)| *at == k && d != delta)
            })
            .map(|(tip, _)| tip.old_key())
            .collect()
    }

    /// The pins for the pin pass, once the carry pass said which wires it
    /// moved plainly (`bailed`): all but those riding on a carried wire.
    pub(super) fn pin_tips(self, bailed: &[Uuid]) -> Vec<crate::PinTip> {
        let Self { tips, wire_points } = self;
        tips.into_iter()
            .filter(|(tip, delta)| {
                let k = tip.old_key();
                !(wire_points.iter())
                    .any(|(wire, at, d)| *at == k && d == delta && !bailed.contains(wire))
            })
            .map(|(tip, _)| tip)
            .collect()
    }
}
