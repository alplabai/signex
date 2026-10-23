//! The connectivity gate: which world points have something electrical on them.
//!
//! A pin is a terminal (and ERC counts it connected) only if something lands on
//! its tip. This is the one definition of that, shared by [`build_netlist`] and
//! by `signex-erc`, so the two can never disagree on which pins are connected.
//!
//! [`build_netlist`]: super::build_netlist

use std::collections::{HashMap, HashSet};

use signex_types::schematic::Point;
use uuid::Uuid;

use super::pt_key;
use crate::uf::Key;

/// The set of 1 µm points that carry a wire endpoint, junction, label,
/// no-connect marker, or a pin tip touched by a pin of a *different* symbol.
///
/// Junctions count: a pin may tap a wire mid-span where a junction sits (D5.3).
/// Buses do **not**: a bus is a member bundle, not a single net, and the
/// union-find never merges buses, so gating on a bus endpoint minted a
/// one-terminal phantom net (D5.4).
///
/// Pin-to-pin contact counts only between two different symbols (issue #675):
/// a power port dropped straight onto a component pin, or two parts butted
/// together, is a visible connection. Two pins of one symbol sharing a point
/// (stacked power pins) is not a connection between parts and stays unconnected.
#[derive(Debug, Default, Clone)]
pub struct ConnectedPoints {
    anchored: HashSet<Key>,
    tip_owner: HashMap<Key, Uuid>,
}

impl ConnectedPoints {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark every point in `points` (wire endpoints, junctions, labels,
    /// no-connect markers) as carrying a connection.
    pub fn add_anchors(&mut self, points: impl IntoIterator<Item = Point>) {
        self.anchored.extend(points.into_iter().map(|p| pt_key(&p)));
    }

    /// Record pin tips as `(world position, owning symbol)`. Two tips from
    /// different symbols on one point mark that point connected, whatever the
    /// order the tips arrive in.
    pub fn add_pin_tips(&mut self, tips: impl IntoIterator<Item = (Point, Uuid)>) {
        for (p, owner) in tips {
            let k = pt_key(&p);
            match self.tip_owner.get(&k) {
                Some(first) if *first != owner => {
                    self.anchored.insert(k);
                }
                Some(_) => {}
                None => {
                    self.tip_owner.insert(k, owner);
                }
            }
        }
    }

    /// True when something electrical lands on `p`.
    pub fn contains(&self, p: &Point) -> bool {
        self.anchored.contains(&pt_key(p))
    }
}
