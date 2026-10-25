//! A canonical, id-free picture of a sheet's connectivity, for "did this edit
//! change any net?" checks.
//!
//! It reads the same per-sheet derivation the project stitcher starts from -
//! wire/junction union, on-sheet label merge, and child-sheet pins anchored
//! to the wire they sit on (endpoint or interior) - so a caller comparing two
//! signatures sees exactly the differences the netlist and the hierarchy
//! would. Net numbers (`N$3`) are not part of it; only what a net contains is.

use std::collections::{BTreeSet, HashMap};

use signex_types::schematic::{Point, SchematicSheet};
use uuid::Uuid;

use crate::build::{
    anchor_point, best_net_name_of, collect_net_labels, collect_terminals, merged_sheet_parent,
    power_name_carriers, pt_key,
};
use crate::uf::{Key, find as uf_find};

/// What one net contains.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NetSignature {
    /// `(symbol uuid, pin id)` of every component pin landing on the net.
    pub terminals: BTreeSet<(Uuid, String)>,
    /// Labels on the net.
    pub labels: BTreeSet<Uuid>,
    /// Child-sheet pins anchored on the net.
    pub sheet_pins: BTreeSet<Uuid>,
    /// The net's name when a label or power port names it, else empty.
    pub name: String,
}

impl NetSignature {
    /// The same net without `terminals` and `sheet_pins`; `None` when nothing
    /// is left to identify it.
    #[must_use]
    pub fn without(
        &self,
        terminals: &BTreeSet<(Uuid, String)>,
        sheet_pins: &BTreeSet<Uuid>,
    ) -> Option<Self> {
        let projected = Self {
            terminals: self.terminals.difference(terminals).cloned().collect(),
            labels: self.labels.clone(),
            sheet_pins: self.sheet_pins.difference(sheet_pins).copied().collect(),
            name: self.name.clone(),
        };
        let empty = projected.terminals.is_empty()
            && projected.labels.is_empty()
            && projected.sheet_pins.is_empty();
        (!empty).then_some(projected)
    }
}

/// Every net of `sheet` that has a terminal, a label or a child-sheet pin,
/// sorted.
#[must_use]
pub fn net_signatures(sheet: &SchematicSheet) -> Vec<NetSignature> {
    net_signatures_at(sheet, &[]).0
}

/// [`net_signatures`], plus for each point key in `at` the net that point sits
/// on: an opaque id shared by every point on one net, and the net's index in
/// the returned list, or `None` when that net has no terminal, label or
/// child-sheet pin to identify it.
#[must_use]
pub fn net_signatures_at(
    sheet: &SchematicSheet,
    at: &[Key],
) -> (Vec<NetSignature>, Vec<(Key, Option<usize>)>) {
    let mut parent = merged_sheet_parent(sheet);
    let wires: Vec<(Point, Point)> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
    for pin in sheet.child_sheets.iter().flat_map(|c| c.pins.iter()) {
        anchor_point(&mut parent, pt_key(&pin.position), &wires);
    }

    let mut nets: HashMap<Key, NetSignature> = HashMap::new();
    let mut entry = |root: Key| {
        nets.entry(root).or_insert_with(|| NetSignature {
            terminals: BTreeSet::new(),
            labels: BTreeSet::new(),
            sheet_pins: BTreeSet::new(),
            name: String::new(),
        });
        root
    };

    let terminals = collect_terminals(sheet, &mut parent);
    let labels = collect_net_labels(sheet, &mut parent);
    let power = power_name_carriers(sheet, &mut parent);
    let mut pin_roots = Vec::new();
    for pin in sheet.child_sheets.iter().flat_map(|c| c.pins.iter()) {
        pin_roots.push((uf_find(&mut parent, pt_key(&pin.position)), pin.uuid));
    }

    let roots: Vec<Key> = terminals
        .keys()
        .copied()
        .chain(labels.keys().copied())
        .chain(pin_roots.iter().map(|(root, _)| *root))
        .collect();
    for root in roots {
        entry(root);
    }
    for (root, list) in terminals {
        let net = nets.get_mut(&root).expect("entry created");
        net.terminals
            .extend(list.into_iter().map(|t| (t.symbol, t.pin)));
    }
    for (root, list) in &labels {
        let net = nets.get_mut(root).expect("entry created");
        net.labels.extend(list.iter().map(|l| l.uuid));
    }
    for (root, uuid) in pin_roots {
        nets.get_mut(&root)
            .expect("entry created")
            .sheet_pins
            .insert(uuid);
    }
    for (root, net) in &mut nets {
        let named = labels
            .get(root)
            .map(|list| list.iter().map(|l| (l.label_type, l.text.as_str())))
            .into_iter()
            .flatten();
        let carriers: Vec<&str> = power
            .iter()
            .filter(|(r, _)| r == root)
            .map(|(_, value)| value.as_str())
            .collect();
        if let Some((_, name)) = best_net_name_of(named, &carriers) {
            net.name = name;
        }
    }

    // A queried point on a wire's interior is on that wire's net, like a
    // child-sheet pin there.
    for key in at {
        anchor_point(&mut parent, *key, &wires);
    }
    let point_roots: Vec<Key> = at.iter().map(|k| uf_find(&mut parent, *k)).collect();
    let mut ordered: Vec<(Key, NetSignature)> = nets.into_iter().collect();
    ordered.sort_by(|a, b| a.1.cmp(&b.1));
    let found = point_roots
        .iter()
        .map(|root| (*root, ordered.iter().position(|(r, _)| r == root)))
        .collect();
    (ordered.into_iter().map(|(_, net)| net).collect(), found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{ChildSheet, FillType, SheetPin, Wire};

    fn sheet_with_pin_on_wire_interior() -> SchematicSheet {
        let mut sheet = SchematicSheet::default();
        sheet.wires.push(Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(0.0, 0.0),
            end: Point::new(20.0, 0.0),
            ..Default::default()
        });
        sheet.child_sheets.push(ChildSheet {
            uuid: Uuid::new_v4(),
            name: "Child".into(),
            filename: "child.snxsch".into(),
            position: Point::new(10.0, 0.0),
            size: (10.0, 10.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![SheetPin {
                uuid: Uuid::new_v4(),
                name: "IN".into(),
                direction: "input".into(),
                position: Point::new(10.0, 0.0),
                rotation: 0.0,
                auto_generated: false,
                user_moved: false,
            }],
            instances: Vec::new(),
        });
        sheet
    }

    #[test]
    fn a_sheet_pin_on_a_wire_interior_is_part_of_that_net() {
        let sheet = sheet_with_pin_on_wire_interior();
        let sigs = net_signatures(&sheet);
        assert_eq!(sigs.len(), 1, "{sigs:?}");
        assert_eq!(sigs[0].sheet_pins.len(), 1);
    }

    #[test]
    fn a_point_reports_the_net_it_sits_on() {
        let sheet = sheet_with_pin_on_wire_interior();
        let (sigs, found) = net_signatures_at(
            &sheet,
            &[
                pt_key(&Point::new(5.0, 0.0)),
                pt_key(&Point::new(50.0, 50.0)),
            ],
        );
        assert_eq!(found[0].1, Some(0), "{sigs:?}");
        assert_eq!(found[1].1, None);
    }

    #[test]
    fn projecting_a_pin_out_drops_a_net_nothing_else_identifies() {
        let sheet = sheet_with_pin_on_wire_interior();
        let sig = &net_signatures(&sheet)[0];
        let pins = sig.sheet_pins.clone();
        assert!(sig.without(&BTreeSet::new(), &pins).is_none());
        assert!(sig.without(&BTreeSet::new(), &BTreeSet::new()).is_some());
    }
}
