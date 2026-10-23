//! The connectivity gate: which world points have something electrical on them.
//!
//! A pin is a terminal (and ERC counts it connected) only if something lands on
//! its tip. This is the one definition of that, shared by [`build_netlist`] and
//! by `signex-erc`, so the two can never disagree on which pins are connected.
//!
//! [`build_netlist`]: super::build_netlist

use std::collections::{HashMap, HashSet};

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

/// Tip keys where power ports of different values share a point with no
/// anchor (wire endpoint, junction or label) on it. Pin-to-pin contact is not a
/// connection (ADR-0002 D5.1), so such a tip joins no net and names none —
/// the shared point key would otherwise short two rails. A wire endpoint,
/// junction or label there is an anchor: the ports are genuinely connected, the
/// short is real, and the nets reflect it. `signex-erc` reports both cases as
/// `PowerPortShort`. No-connect markers are not anchors here.
pub fn touching_port_conflicts(
    ports: &[(Point, &str)],
    segments: &[(Point, Point)],
    junctions: &[Point],
    label_points: &[Point],
) -> HashSet<Key> {
    let mut anchored: HashSet<Key> = HashSet::new();
    anchored.extend(segments.iter().flat_map(|(a, b)| [pt_key(a), pt_key(b)]));
    anchored.extend(junctions.iter().map(pt_key));
    anchored.extend(label_points.iter().map(pt_key));

    let mut values_at: HashMap<Key, HashSet<&str>> = HashMap::new();
    for (pos, value) in ports {
        values_at.entry(pt_key(pos)).or_default().insert(value);
    }
    values_at
        .into_iter()
        .filter(|(key, values)| values.len() > 1 && !anchored.contains(key))
        .map(|(key, _)| key)
        .collect()
}
