//! Bus membership — which member nets a bus carries.
//!
//! A bus is a bundle, never a net: [`build_netlist`](crate::build_netlist)
//! does not merge bus segments, does not union the members of a bus with each
//! other, and mints no net for a bus (D5.4). What a bus *means* lives here,
//! beside the netlist rather than inside it:
//!
//! * a bus takes its member set from the bus-name label(s) sitting on it
//!   (`D[0..7]` carries `D0`..`D7`; see [`parse_bus_name`] for the syntax);
//! * a [`BusEntry`](signex_types::schematic::BusEntry) is a stub from the bus
//!   to a wire. It joins no net by itself: the wire reaches a member net only
//!   through the member label on that wire, exactly as for any other wire.
//!   The entry's job here is to say *which bus* that wire's label should be a
//!   member of.
//!
//! The analysis works on plain geometry ([`BusGeometry`]) so `signex-erc`,
//! which holds its own snapshot types, applies the *same* analysis instead of
//! re-deriving it. Every lookup goes through a grid index, so the cost is
//! linear in the number of segments, labels and entries.

use std::collections::{HashMap, HashSet};

use signex_types::schematic::{BusNameError, Point, parse_bus_name};

use crate::uf::Key;
use crate::{SheetConnectivity, pt_key};

mod index;
use index::SegmentIndex;

/// The raw geometry a bus analysis reads. All slices are in the caller's own
/// order; every index in the result refers back into them.
#[derive(Debug, Clone, Copy, Default)]
pub struct BusGeometry<'a> {
    /// Bus segments as `(start, end)`.
    pub buses: &'a [(Point, Point)],
    /// Bus entries as `(position, size)`; the stub runs `position` to
    /// `position + size`.
    pub entries: &'a [(Point, (f64, f64))],
    pub wires: &'a [(Point, Point)],
    pub junctions: &'a [Point],
    /// Every label as `(position, text)`, whatever its kind.
    pub labels: &'a [(Point, &'a str)],
}

/// One bus-name label that names a run, expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusLabelName {
    /// Index into [`BusGeometry::labels`].
    pub label: usize,
    /// Signals the name carries, counting repeats.
    pub width: usize,
    /// The distinct member net names the name carries.
    pub members: HashSet<String>,
}

/// A connected run of bus segments and what it is called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusGroup {
    /// Indices into [`BusGeometry::buses`], ascending.
    pub segments: Vec<usize>,
    /// The labels that name this run, ascending by label index.
    pub names: Vec<BusLabelName>,
    /// Labels sitting on this run whose text is *not* a bus name, with why.
    pub rejected: Vec<(usize, BusNameError)>,
    /// The member net names the run carries: the union over `names`,
    /// de-duplicated, in first-seen order.
    pub members: Vec<String>,
    member_set: HashSet<String>,
}

impl BusGroup {
    /// True when `net_name` is carried by this run.
    pub fn carries(&self, net_name: &str) -> bool {
        self.member_set.contains(net_name)
    }
}

/// How one bus entry attaches to a bus and to the wire it feeds.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryAttachment {
    /// Index into [`BusGeometry::entries`].
    pub entry: usize,
    /// The bus run the entry's bus end touches.
    pub group: usize,
    /// The stub end that is not on the bus: where the wire attaches.
    pub wire_end: Point,
    /// Indices into [`BusGeometry::labels`] of the member labels on the net
    /// of `wire_end` (bus-name labels are not member labels), ascending.
    pub member_labels: Vec<usize>,
}

/// Result of [`analyse_buses`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BusAnalysis {
    pub groups: Vec<BusGroup>,
    /// Only entries with an end on a bus; a floating entry is not listed.
    pub entries: Vec<EntryAttachment>,
}

/// Group bus segments into runs, name each run from its labels, and attach
/// every bus entry to the run it touches and to the labels on its wire.
///
/// Two segments are one run when one's endpoint lies on the other (a shared
/// endpoint or a T) or a junction lies on both. A label names a run when it
/// sits on one of its segments and its text parses as a bus name: at an
/// endpoint **or mid-span**. A label whose text is not a bus name touches a
/// bus at a segment endpoint only (D5.4), and is recorded in
/// [`BusGroup::rejected`]. Where several segments contain a point, the lowest
/// index wins, so the answer depends on nothing but the input order.
pub fn analyse_buses(geo: &BusGeometry<'_>) -> BusAnalysis {
    let index = SegmentIndex::new(geo.buses);
    let seg_group = group_segments(geo, &index);
    let group_count = seg_group.iter().max().map_or(0, |m| m + 1);

    let mut groups: Vec<BusGroup> = (0..group_count)
        .map(|_| BusGroup {
            segments: Vec::new(),
            names: Vec::new(),
            rejected: Vec::new(),
            members: Vec::new(),
            member_set: HashSet::new(),
        })
        .collect();
    for (seg, &g) in seg_group.iter().enumerate() {
        groups[g].segments.push(seg);
    }

    let mut bus_name_labels: HashSet<usize> = HashSet::new();
    for (li, (pos, text)) in geo.labels.iter().enumerate() {
        let pk = pt_key(pos);
        match parse_bus_name(text) {
            Ok(name) => {
                let Some(seg) = index.containing(pk).min() else {
                    continue;
                };
                bus_name_labels.insert(li);
                let members = name.members();
                let group = &mut groups[seg_group[seg]];
                for m in &members {
                    if group.member_set.insert(m.clone()) {
                        group.members.push(m.clone());
                    }
                }
                group.names.push(BusLabelName {
                    label: li,
                    width: members.len(),
                    members: members.into_iter().collect(),
                });
            }
            Err(e) => {
                // Not a bus name: only an endpoint contact counts.
                let owner = index
                    .containing(pk)
                    .filter(|&s| index.is_endpoint(s, pk))
                    .min();
                if let Some(seg) = owner {
                    groups[seg_group[seg]].rejected.push((li, e));
                }
            }
        }
    }

    let entries = attach_entries(geo, &index, &seg_group, &bus_name_labels);
    BusAnalysis { groups, entries }
}

/// Group id of every segment; ids number runs in order of first segment.
fn group_segments(geo: &BusGeometry<'_>, index: &SegmentIndex) -> Vec<usize> {
    let n = geo.buses.len();
    let mut dsu = Dsu::new(n);
    for (s, (a, b)) in geo.buses.iter().enumerate() {
        for end in [pt_key(a), pt_key(b)] {
            for other in index.containing(end) {
                dsu.union(s, other);
            }
        }
    }
    for j in geo.junctions {
        let mut on = index.containing(pt_key(j));
        if let Some(first) = on.next() {
            for other in on {
                dsu.union(first, other);
            }
        }
    }
    let mut ids: HashMap<usize, usize> = HashMap::new();
    (0..n)
        .map(|s| {
            let root = dsu.find(s);
            let next = ids.len();
            *ids.entry(root).or_insert(next)
        })
        .collect()
}

struct Dsu(Vec<usize>);

impl Dsu {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.0[x] != x {
            self.0[x] = self.0[self.0[x]];
            x = self.0[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        // Keep the smaller index as root so ids do not depend on call order.
        self.0[ra.max(rb)] = ra.min(rb);
    }
}

/// Wire connectivity with the same anchoring `build_netlist` applies to
/// labels (`anchor_point`): an endpoint is its own node; a point inside
/// several wires takes the one with the smallest normalised endpoint pair.
struct WireNets<'a> {
    conn: SheetConnectivity,
    index: SegmentIndex,
    endpoints: HashSet<Key>,
    wires: &'a [(Point, Point)],
}

impl<'a> WireNets<'a> {
    fn new(wires: &'a [(Point, Point)], junctions: &[Point]) -> Self {
        Self {
            conn: SheetConnectivity::from_segments(wires, junctions),
            index: SegmentIndex::new(wires),
            endpoints: wires
                .iter()
                .flat_map(|(a, b)| [pt_key(a), pt_key(b)])
                .collect(),
            wires,
        }
    }

    fn root_of(&mut self, p: &Point) -> Key {
        let pk = pt_key(p);
        if self.endpoints.contains(&pk) {
            return self.conn.root_of(p);
        }
        let wires = self.wires;
        let chosen = self.index.containing(pk).min_by_key(|&i| {
            let (a, b) = (pt_key(&wires[i].0), pt_key(&wires[i].1));
            (a.min(b), a.max(b))
        });
        match chosen {
            Some(i) => self.conn.root_of(&wires[i].0),
            None => self.conn.root_of(p),
        }
    }
}

fn attach_entries(
    geo: &BusGeometry<'_>,
    index: &SegmentIndex,
    seg_group: &[usize],
    bus_name_labels: &HashSet<usize>,
) -> Vec<EntryAttachment> {
    if geo.entries.is_empty() {
        return Vec::new();
    }
    let group_at = |p: &Point| index.containing(pt_key(p)).min().map(|s| seg_group[s]);

    let mut nets = WireNets::new(geo.wires, geo.junctions);
    let mut labels_by_root: HashMap<Key, Vec<usize>> = HashMap::new();
    for (li, (pos, _)) in geo.labels.iter().enumerate() {
        if !bus_name_labels.contains(&li) {
            labels_by_root
                .entry(nets.root_of(pos))
                .or_default()
                .push(li);
        }
    }

    let mut out = Vec::new();
    for (ei, (pos, size)) in geo.entries.iter().enumerate() {
        let far = Point::new(pos.x + size.0, pos.y + size.1);
        // The end on the bus is the bus end. Both on a bus is ambiguous; the
        // entry's own position wins.
        let (group, wire_end) = match (group_at(pos), group_at(&far)) {
            (Some(g), _) => (g, far),
            (None, Some(g)) => (g, *pos),
            (None, None) => continue,
        };
        let root = nets.root_of(&wire_end);
        out.push(EntryAttachment {
            entry: ei,
            group,
            wire_end,
            member_labels: labels_by_root.get(&root).cloned().unwrap_or_default(),
        });
    }
    out
}

#[cfg(test)]
mod tests;
