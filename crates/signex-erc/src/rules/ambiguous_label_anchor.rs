//! Rule: AmbiguousLabelAnchor.
//!
//! `build_netlist`'s `anchor_point` rule 2 resolves a label that sits on the
//! interior of *several* wires by picking exactly one — the segment with the
//! smallest normalised endpoint-key pair. That tiebreak is deterministic (it
//! has to be: issue #402 made the whole partition independent of document
//! order), but it is still an arbitrary **electrical** decision. Two wires
//! merely crossing at a point are two separate nets; the label names one of
//! them, and nothing on screen tells the user which.
//!
//! Determinism without disclosure is the worse half of the fix, so the netlist
//! keeps the tiebreak and ERC says out loud that a tiebreak happened.
//!
//! Not flagged:
//! - a label on a wire **endpoint** — `anchor_point` rule 1 returns early, the
//!   label is already a node of that wire's class, no choice is made;
//! - a label where a junction sits — the crossing wires are one net there, so
//!   whichever segment wins names the same net.

use signex_net::{point_on_segment, pt_key};
use signex_types::schematic::SelectedKind;

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

use super::wire_pairs;

pub(crate) fn ambiguous_label_anchor(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let wires = wire_pairs(ctx);

    for label in &ctx.labels {
        let pk = pt_key(&label.position);

        // Rule 1 of `anchor_point`: sitting on any endpoint settles it.
        if wires
            .iter()
            .any(|(a, b)| pk == pt_key(a) || pk == pt_key(b))
        {
            continue;
        }
        // A dot merges the crossing wires, so the choice is not electrical.
        if ctx.junctions.iter().any(|j| pt_key(&j.position) == pk) {
            continue;
        }

        let touched = wires
            .iter()
            .filter(|(a, b)| point_on_segment(pk, pt_key(a), pt_key(b)))
            .count();
        if touched < 2 {
            continue;
        }

        out.push(
            Diagnostic::new(
                RuleKind::AmbiguousLabelAnchor,
                format!(
                    "Label '{}' sits where {touched} wires cross with no junction — \
                     its net was picked by tiebreak, not by geometry",
                    label.text,
                ),
                label.position,
            )
            .with_primary(sel(label.uuid, SelectedKind::Label)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ErcJunction, ErcLabel, ErcWire, PaperSize};
    use signex_types::schematic::{LabelType, Point};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn wire(a: Point, b: Point) -> ErcWire {
        ErcWire {
            uuid: Uuid::new_v4(),
            start: a,
            end: b,
        }
    }

    fn net_label(text: &str, pos: Point) -> ErcLabel {
        ErcLabel {
            uuid: Uuid::nil(),
            text: text.into(),
            position: pos,
            label_type: LabelType::Net,
        }
    }

    fn ctx(wires: Vec<ErcWire>, junctions: Vec<ErcJunction>, labels: Vec<ErcLabel>) -> ErcContext {
        ErcContext {
            paper_size: PaperSize::A4,
            symbols: Vec::new(),
            wires,
            buses: Vec::new(),
            labels,
            junctions,
            no_connects: Vec::new(),
            bus_entries: Vec::new(),
            child_sheets: Vec::new(),
            nets: Vec::new(),
            children: HashMap::new(),
        }
    }

    /// The X crossing the netlist resolves by tiebreak: two wires cross at
    /// (5,0), no dot, a label on the crossing. The user cannot predict which
    /// of the two nets `NET` names, so ERC has to say so.
    #[test]
    fn a_label_on_an_undotted_crossing_is_flagged() {
        let c = ctx(
            vec![
                wire(pt(0.0, 0.0), pt(10.0, 0.0)),
                wire(pt(5.0, -5.0), pt(5.0, 5.0)),
            ],
            Vec::new(),
            vec![net_label("NET", pt(5.0, 0.0))],
        );
        let mut out = Vec::new();
        ambiguous_label_anchor(&c, &mut out);
        assert_eq!(out.len(), 1, "{out:?}");
    }

    /// A dot at the crossing makes the two wires one net, so whichever segment
    /// the tiebreak picks names the same thing — nothing ambiguous left.
    #[test]
    fn a_junction_at_the_crossing_clears_the_ambiguity() {
        let c = ctx(
            vec![
                wire(pt(0.0, 0.0), pt(10.0, 0.0)),
                wire(pt(5.0, -5.0), pt(5.0, 5.0)),
            ],
            vec![ErcJunction {
                position: pt(5.0, 0.0),
            }],
            vec![net_label("NET", pt(5.0, 0.0))],
        );
        let mut out = Vec::new();
        ambiguous_label_anchor(&c, &mut out);
        assert!(out.is_empty(), "{out:?}");
    }

    /// The ordinary case: one wire under the label. No choice, no diagnostic.
    #[test]
    fn a_label_on_a_single_wires_interior_is_not_flagged() {
        let c = ctx(
            vec![wire(pt(0.0, 0.0), pt(10.0, 0.0))],
            Vec::new(),
            vec![net_label("NET", pt(5.0, 0.0))],
        );
        let mut out = Vec::new();
        ambiguous_label_anchor(&c, &mut out);
        assert!(out.is_empty(), "{out:?}");
    }

    /// A label at the shared endpoint of two wires: `anchor_point` rule 1
    /// short-circuits, the label is already in that class, nothing is chosen.
    #[test]
    fn a_label_on_a_shared_endpoint_is_not_flagged() {
        let c = ctx(
            vec![
                wire(pt(0.0, 0.0), pt(5.0, 0.0)),
                wire(pt(5.0, 0.0), pt(5.0, 5.0)),
            ],
            Vec::new(),
            vec![net_label("NET", pt(5.0, 0.0))],
        );
        let mut out = Vec::new();
        ambiguous_label_anchor(&c, &mut out);
        assert!(out.is_empty(), "{out:?}");
    }
}
