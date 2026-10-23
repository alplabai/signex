//! The connectivity gate: which world points have something electrical on them.
//!
//! A pin is a terminal (and ERC counts it connected) only if something lands on
//! its tip. This is the one definition of that, shared by [`build_netlist`] and
//! by `signex-erc`, so the two can never disagree on which pins are connected.
//!
//! [`build_netlist`]: super::build_netlist

use std::collections::HashSet;

use signex_types::schematic::Point;

use super::pt_key;
use crate::uf::Key;

/// The set of 1 µm points that carry a wire endpoint, junction, label,
/// or no-connect marker.
///
/// Junctions count: a pin may tap a wire mid-span where a junction sits (D5.3).
/// Buses do **not**: a bus is a member bundle, not a single net, and the
/// union-find never merges buses, so gating on a bus endpoint minted a
/// one-terminal phantom net (D5.4).
///
/// Pin-to-pin contact is deliberately **not** a connection (ADR-0002 D5.1):
/// two pins sharing a point with no wire, junction or label there are not
/// joined. `signex-erc` reports such a touch (`PinsTouchNotConnected`) so the
/// user adds a wire instead of the netlist guessing.
#[derive(Debug, Default, Clone)]
pub struct ConnectedPoints {
    anchored: HashSet<Key>,
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

    /// True when something electrical lands on `p`.
    pub fn contains(&self, p: &Point) -> bool {
        self.anchored.contains(&pt_key(p))
    }
}
