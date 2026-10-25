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
//! The functions work on plain geometry ([`BusGeometry`]) so `signex-erc`,
//! which holds its own snapshot types, applies the *same* analysis instead of
//! re-deriving it. [`sheet_buses`] adds the netlist view for a whole sheet.

use std::collections::HashSet;

use signex_types::net::{NetId, Netlist};
use signex_types::schematic::{BusNameError, Point, SchematicSheet, parse_bus_name};
use uuid::Uuid;

use crate::{SheetConnectivity, point_on_segment, pt_key};

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

/// A connected run of bus segments and what it is called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusGroup {
    /// Indices into [`BusGeometry::buses`], ascending.
    pub segments: Vec<usize>,
    /// Indices into [`BusGeometry::labels`] of the labels that name this bus
    /// (they sit on a segment and parse as a bus name), ascending.
    pub names: Vec<usize>,
    /// Labels sitting on this bus whose text is *not* a bus name, with why.
    pub rejected: Vec<(usize, BusNameError)>,
    /// The member net names the bus carries: the union over `names`,
    /// de-duplicated, in first-seen order.
    pub members: Vec<String>,
}

impl BusGroup {
    /// True when `net_name` is carried by this bus.
    pub fn carries(&self, net_name: &str) -> bool {
        self.members.iter().any(|m| m == net_name)
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

fn on_any_segment(p: &Point, segments: &[(Point, Point)]) -> bool {
    let pk = pt_key(p);
    segments
        .iter()
        .any(|(a, b)| point_on_segment(pk, pt_key(a), pt_key(b)))
}

/// Group bus segments into runs, name each run from its labels, and attach
/// every bus entry to the run it touches and to the labels on its wire.
///
/// Segments join a run only by a shared endpoint (bus-local connectivity, as
/// `bus_bit_width_mismatch` has always grouped them). A label names a run when
/// it sits on one of its segments, at an endpoint **or mid-span**, and its text
/// parses as a bus name. Nothing here touches wire/net connectivity beyond
/// reading it to find an entry wire's labels.
pub fn analyse_buses(geo: &BusGeometry<'_>) -> BusAnalysis {
    let mut bus_conn = SheetConnectivity::from_segments(geo.buses, &[]);
    // Anchor first, read roots after: a union re-points a representative.
    for (pos, _) in geo.labels {
        bus_conn.root_of_anchored(pos, geo.buses);
    }
    let seg_roots: Vec<_> = geo.buses.iter().map(|(a, _)| bus_conn.root_of(a)).collect();

    let mut groups: Vec<BusGroup> = Vec::new();
    let mut group_roots = Vec::new();
    for (i, root) in seg_roots.iter().enumerate() {
        match group_roots.iter().position(|r| r == root) {
            Some(g) => groups[g].segments.push(i),
            None => {
                group_roots.push(*root);
                groups.push(BusGroup {
                    segments: vec![i],
                    names: Vec::new(),
                    rejected: Vec::new(),
                    members: Vec::new(),
                });
            }
        }
    }

    let mut bus_name_labels: HashSet<usize> = HashSet::new();
    for (li, (pos, text)) in geo.labels.iter().enumerate() {
        if !on_any_segment(pos, geo.buses) {
            continue;
        }
        let root = bus_conn.root_of(pos);
        let Some(g) = group_roots.iter().position(|r| *r == root) else {
            continue;
        };
        match parse_bus_name(text) {
            Ok(name) => {
                bus_name_labels.insert(li);
                groups[g].names.push(li);
                for m in name.members() {
                    if !groups[g].members.contains(&m) {
                        groups[g].members.push(m);
                    }
                }
            }
            Err(e) => groups[g].rejected.push((li, e)),
        }
    }

    let entries = attach_entries(geo, &groups, &bus_name_labels);
    BusAnalysis { groups, entries }
}

fn attach_entries(
    geo: &BusGeometry<'_>,
    groups: &[BusGroup],
    bus_name_labels: &HashSet<usize>,
) -> Vec<EntryAttachment> {
    let mut wire_conn = SheetConnectivity::from_segments(geo.wires, geo.junctions);

    // Resolve each entry's ends first, so every point that needs a net root is
    // anchored before any root is read.
    struct Ends {
        entry: usize,
        group: usize,
        wire_end: Point,
    }
    let mut ends = Vec::new();
    for (ei, (pos, size)) in geo.entries.iter().enumerate() {
        let far = Point::new(pos.x + size.0, pos.y + size.1);
        let group_at = |p: &Point| {
            let pk = pt_key(p);
            groups.iter().position(|g| {
                g.segments.iter().any(|&s| {
                    let (a, b) = geo.buses[s];
                    point_on_segment(pk, pt_key(&a), pt_key(&b))
                })
            })
        };
        // The end on the bus is the bus end. Both on a bus is ambiguous; the
        // entry's own position wins.
        let (group, wire_end) = match (group_at(pos), group_at(&far)) {
            (Some(g), _) => (g, far),
            (None, Some(g)) => (g, *pos),
            (None, None) => continue,
        };
        wire_conn.root_of_anchored(&wire_end, geo.wires);
        ends.push(Ends {
            entry: ei,
            group,
            wire_end,
        });
    }
    for (pos, _) in geo.labels {
        wire_conn.root_of_anchored(pos, geo.wires);
    }

    let label_roots: Vec<_> = geo
        .labels
        .iter()
        .map(|(pos, _)| wire_conn.root_of(pos))
        .collect();
    ends.into_iter()
        .map(|e| {
            let root = wire_conn.root_of(&e.wire_end);
            let member_labels = label_roots
                .iter()
                .enumerate()
                .filter(|(li, r)| **r == root && !bus_name_labels.contains(li))
                .map(|(li, _)| li)
                .collect();
            EntryAttachment {
                entry: e.entry,
                group: e.group,
                wire_end: e.wire_end,
                member_labels,
            }
        })
        .collect()
}

/// One member of a bus and the net it names, when that net exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusMember {
    pub name: String,
    /// The net named exactly `name` on this sheet, if the netlist has one.
    pub net: Option<NetId>,
}

/// A bus run as the netlist sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetBus {
    /// Uuids of the bus segments in the run.
    pub segments: Vec<Uuid>,
    /// The bus-name label texts, as written.
    pub names: Vec<String>,
    pub members: Vec<BusMember>,
}

/// Every bus run on `sheet` with its member set, resolved against `netlist`.
///
/// Read-only over the netlist: members are looked up by net name and never
/// merged with each other, and a member with no net of that name stays
/// `net: None` rather than getting one minted.
pub fn sheet_buses(sheet: &SchematicSheet, netlist: &Netlist) -> Vec<SheetBus> {
    let buses: Vec<(Point, Point)> = sheet.buses.iter().map(|b| (b.start, b.end)).collect();
    let labels: Vec<(Point, &str)> = sheet
        .labels
        .iter()
        .map(|l| (l.position, l.text.as_str()))
        .collect();
    let geo = BusGeometry {
        buses: &buses,
        labels: &labels,
        ..Default::default()
    };
    analyse_buses(&geo)
        .groups
        .into_iter()
        .map(|g| SheetBus {
            segments: g.segments.iter().map(|&i| sheet.buses[i].uuid).collect(),
            names: g.names.iter().map(|&i| labels[i].1.to_string()).collect(),
            members: g
                .members
                .into_iter()
                .map(|name| {
                    let net = netlist.nets.iter().find(|n| n.name == name).map(|n| n.id);
                    BusMember { name, net }
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
