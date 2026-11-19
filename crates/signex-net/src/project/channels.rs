//! Repeated sheets in the project walk (#1279): a sheet symbol whose
//! designator reads `Repeat(CH, 1, 4)` is walked once per channel, and each
//! channel occurrence renames its designators and its channel-local nets
//! through the project's [`ChannelNaming`]. The rules are in
//! `docs/MULTI_CHANNEL.md`, "Stitching".

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};

use signex_types::net::Terminal;
use signex_types::project::ChannelNaming;
use signex_types::schematic::{
    ChannelOccurrence, ChannelRepeat, ChildSheet, InstancePath, LabelType, Point, SchematicSheet,
    normalize_port_name, parse_bus_name, repeated_entry_name,
};
use uuid::Uuid;

use super::{Analysis, L2, Occ, ProjectGraph, SheetKey, walk_occurrences};
use crate::build::{point_on_segment, pt_key};
use crate::bus::{BusAnalysis, BusGeometry, analyse_buses};
use crate::uf::{Key, find as uf_find, union as uf_union};

/// What a channel occurrence knows about the repeated placement it came
/// from.
#[derive(Debug, Clone)]
pub(super) struct ChannelScope {
    pub channel: ChannelOccurrence,
    /// The plain (shared) entry names of the repeated sheet symbol.
    pub shared: HashSet<String>,
}

/// The channels a child placement compiles to, or `None` for a plain
/// placement.
pub(super) fn channel_scopes(cs: &ChildSheet) -> Option<Vec<ChannelScope>> {
    let repeat = ChannelRepeat::parse(&cs.name)?;
    let shared: HashSet<String> = cs
        .pins
        .iter()
        .filter(|p| repeated_entry_name(&p.name).is_none())
        .filter_map(|p| normalize_port_name(&p.name).map(str::to_string))
        .collect();
    Some(
        repeat
            .occurrences()
            .into_iter()
            .map(|channel| ChannelScope {
                channel,
                shared: shared.clone(),
            })
            .collect(),
    )
}

/// The placement uuid of channel `number` of the sheet symbol `sheet_symbol`:
/// a version-8 uuid of a fixed 128-bit mix of the two, so every channel has a
/// distinct, stable [`InstancePath`](signex_types::schematic::InstancePath)
/// across runs and machines. See `docs/MULTI_CHANNEL.md`, "Channel identity".
pub fn channel_uuid(sheet_symbol: Uuid, number: u32) -> Uuid {
    const SEEDS: [u64; 2] = [0xcbf2_9ce4_8422_2325, 0x84222325_cbf29ce4];
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut bytes = [0u8; 16];
    for (half, seed) in SEEDS.iter().enumerate() {
        let mut h = *seed;
        let input = sheet_symbol
            .as_bytes()
            .iter()
            .chain(number.to_le_bytes().iter())
            .chain(b"signex-channel".iter());
        for b in input {
            h ^= u64::from(*b);
            h = h.wrapping_mul(PRIME);
        }
        bytes[half * 8..half * 8 + 8].copy_from_slice(&h.to_be_bytes());
    }
    uuid::Builder::from_custom_bytes(bytes).into_uuid()
}

/// `base` as it reads in a placement inside `channels` (outermost first).
/// A blank designator stays blank.
pub(super) fn channel_designator<'s>(
    base: &'s str,
    channels: &[ChannelOccurrence],
    naming: &ChannelNaming,
) -> Cow<'s, str> {
    if channels.is_empty() || base.trim().is_empty() {
        return Cow::Borrowed(base);
    }
    let mut out = base.trim().to_string();
    for ch in channels {
        out = naming.designator(&out, ch);
    }
    Cow::Owned(out)
}

/// Rename every non-power terminal of a channel occurrence to its channel
/// designator; a plain occurrence is left as it is.
pub(super) fn rename_terminals(analysis: &mut Analysis, occ: &Occ, naming: &ChannelNaming) {
    if occ.channels.is_empty() {
        return;
    }
    let power: HashSet<Uuid> = occ
        .sheet
        .symbols
        .iter()
        .filter(|s| s.is_power)
        .map(|s| s.uuid)
        .collect();
    for terminals in analysis.terminals.values_mut() {
        for t in terminals.iter_mut().filter(|t| !power.contains(&t.symbol)) {
            rename_terminal(t, &occ.channels, naming);
        }
    }
}

fn rename_terminal(t: &mut Terminal, channels: &[ChannelOccurrence], naming: &ChannelNaming) {
    if let Cow::Owned(name) = channel_designator(&t.reference, channels, naming) {
        t.reference = name;
    }
}

/// The name a label of `occ` gives its net, as the export writes it.
/// `qualifiable` is true for Hierarchical and local Net labels, which take
/// their sheet chain. In a channel occurrence the channel's own chain entry is
/// left out: a shared entry's name reads as on the parent, any other local
/// name takes the channel net format.
pub(super) fn scoped_name(
    occ: &Occ,
    qualifiable: bool,
    text: &str,
    naming: &ChannelNaming,
) -> String {
    if !qualifiable {
        return text.to_string();
    }
    let chain = &occ.name_chain;
    let (chain, text): (&[String], Cow<str>) = match &occ.channel {
        Some(scope) => {
            let up = &chain[..chain.len().saturating_sub(1)];
            if scope.shared.contains(text) {
                (up, Cow::Borrowed(text))
            } else {
                (up, Cow::Owned(naming.net(text, &scope.channel)))
            }
        }
        None => (chain.as_slice(), Cow::Borrowed(text)),
    };
    if chain.is_empty() {
        text.into_owned()
    } else {
        format!("{}/{}", chain.join("/"), text)
    }
}

/// The bus runs of one sheet as a sheet entry sees them: named by the
/// sheet's labels and by its bus-named sheet entries.
pub struct EntryBuses {
    segments: Vec<(Point, Point)>,
    analysis: BusAnalysis,
}

impl EntryBuses {
    /// Analyse a sheet's bus geometry. `geo.labels` should come from
    /// [`EntryBuses::texts`] so entries name runs the same way everywhere.
    pub fn new(geo: &BusGeometry<'_>) -> Self {
        Self {
            segments: geo.buses.to_vec(),
            analysis: analyse_buses(geo),
        }
    }

    /// The texts that name a sheet's bus runs: every label, then every sheet
    /// entry whose name is a bus name (`VOUT_3V3[1..4]`).
    pub fn texts<'a>(
        labels: impl IntoIterator<Item = (Point, &'a str)>,
        entries: impl IntoIterator<Item = (Point, &'a str)>,
    ) -> Vec<(Point, &'a str)> {
        let bus_entries = entries
            .into_iter()
            .map(|(p, n)| (p, n.trim()))
            .filter(|(_, n)| parse_bus_name(n).is_ok());
        labels.into_iter().chain(bus_entries).collect()
    }

    /// Analyse `sheet`'s buses.
    pub fn of_sheet(sheet: &SchematicSheet) -> Self {
        let pins = sheet
            .child_sheets
            .iter()
            .flat_map(|cs| cs.pins.iter())
            .map(|p| (p.position, p.name.as_str()));
        let labels = sheet.labels.iter().map(|l| (l.position, l.text.as_str()));
        let texts = Self::texts(labels, pins);
        let segments: Vec<(Point, Point)> = sheet.buses.iter().map(|b| (b.start, b.end)).collect();
        let entries: Vec<(Point, (f64, f64))> = sheet
            .bus_entries
            .iter()
            .map(|e| (e.position, e.size))
            .collect();
        let wires: Vec<(Point, Point)> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
        let junctions: Vec<Point> = sheet.junctions.iter().map(|j| j.position).collect();
        Self::new(&BusGeometry {
            buses: &segments,
            entries: &entries,
            wires: &wires,
            junctions: &junctions,
            labels: &texts,
        })
    }

    /// The bus run a point sits on (endpoint or interior), lowest run first.
    pub fn run_at(&self, at: Point) -> Option<usize> {
        let p = pt_key(&at);
        self.analysis.groups.iter().position(|g| {
            g.segments.iter().any(|&s| {
                let (a, b) = self.segments[s];
                point_on_segment(p, pt_key(&a), pt_key(&b))
            })
        })
    }

    /// The member nets run `run` carries, in written order.
    pub fn members(&self, run: usize) -> &[String] {
        self.analysis
            .groups
            .get(run)
            .map_or(&[], |g| g.members.as_slice())
    }
}

/// Join every `Repeat(<net>)` entry across its channels (rule 3c, #1279):
/// channel k's `<net>` port joins the k-th member of the bus run the entry
/// meets on the parent. On such a run, the parent's own nets named by a
/// member and the member nets of every other bus-typed entry on it join the
/// same member, so a bus drawn from entry to entry carries the channels
/// through. Runs that meet no Repeat entry are left to the bus rules above.
pub(super) fn join_repeat_entries(
    l2: &mut HashMap<L2, L2>,
    occs: &[Occ],
    edges: &[(usize, usize, usize)],
    analyses: &[Analysis],
) {
    let parents: BTreeSet<usize> = edges
        .iter()
        .filter(|(_, _, child)| occs[*child].channel.is_some())
        .map(|(parent, ..)| *parent)
        .collect();
    for parent in parents {
        join_on_parent(l2, parent, occs, edges, analyses);
    }
}

fn join_on_parent(
    l2: &mut HashMap<L2, L2>,
    parent: usize,
    occs: &[Occ],
    edges: &[(usize, usize, usize)],
    analyses: &[Analysis],
) {
    let sheet = occs[parent].sheet;
    let buses = EntryBuses::of_sheet(sheet);
    let mut bucket: HashMap<(usize, String), L2> = HashMap::new();
    let children: Vec<(usize, usize)> = edges
        .iter()
        .filter(|(p, ..)| *p == parent)
        .map(|&(_, cs_index, child)| (cs_index, child))
        .collect();

    for &(cs_index, child) in &children {
        let Some(scope) = &occs[child].channel else {
            continue;
        };
        for pin in &sheet.child_sheets[cs_index].pins {
            let Some(net) = repeated_entry_name(&pin.name) else {
                continue;
            };
            let Some(run) = buses.run_at(pin.position) else {
                continue;
            };
            let Some(member) = buses.members(run).get(scope.channel.index as usize - 1) else {
                continue;
            };
            for &root in analyses[child].port_labels.get(net).into_iter().flatten() {
                join(l2, &mut bucket, (run, member.clone()), (child, root));
            }
        }
    }
    let runs: BTreeSet<usize> = bucket.keys().map(|(run, _)| *run).collect();
    if runs.is_empty() {
        return;
    }

    for (root, labels) in &analyses[parent].net_labels {
        for label in labels {
            for &run in &runs {
                if buses.members(run).contains(&label.text) {
                    join(l2, &mut bucket, (run, label.text.clone()), (parent, *root));
                }
            }
        }
    }

    for &(cs_index, child) in &children {
        for pin in &sheet.child_sheets[cs_index].pins {
            let Some(bus) = normalize_port_name(&pin.name).and_then(|n| parse_bus_name(n).ok())
            else {
                continue;
            };
            let Some(run) = buses.run_at(pin.position).filter(|r| runs.contains(r)) else {
                continue;
            };
            let key = bus.join_key();
            for (_, port) in analyses[child]
                .buses
                .ports
                .iter()
                .filter(|(ty, p)| *ty != LabelType::Net && p.key == key)
            {
                for member in buses.members(run) {
                    for &root in port.roots.get(member).into_iter().flatten() {
                        join(l2, &mut bucket, (run, member.clone()), (child, root));
                    }
                }
            }
        }
    }
}

fn join(
    l2: &mut HashMap<L2, L2>,
    bucket: &mut HashMap<(usize, String), L2>,
    slot: (usize, String),
    node: (usize, Key),
) {
    match bucket.get(&slot).copied() {
        Some(rep) => uf_union(l2, node, rep),
        None => {
            bucket.insert(slot, node);
            uf_find(l2, node);
        }
    }
}

/// One occurrence of the walk as the compiled view lists it.
#[derive(Debug, Clone)]
pub(crate) struct CompiledOccurrence {
    pub key: SheetKey,
    pub path: InstancePath,
    /// Rows above it in the compiled tree: 0 for a root page, one per sheet
    /// symbol and one more per repeated placement it sits in.
    pub depth: usize,
    /// The sheet symbol's designator; `None` for a root page.
    pub designator: Option<String>,
    /// Set on a channel occurrence.
    pub channel: Option<ChannelOccurrence>,
    /// Set on a channel occurrence: the repeat and the path of the repeated
    /// placement itself (the sheet symbol, not one of its channels).
    pub repeat: Option<(ChannelRepeat, InstancePath)>,
}

/// Every occurrence of the project walk, in walk order (parent before its
/// children, a repeated placement's channels in channel order).
pub(crate) fn compiled_occurrences(graph: &ProjectGraph) -> Vec<CompiledOccurrence> {
    let mut issues = Vec::new();
    let (occs, edges) = walk_occurrences(graph, &mut issues);
    let mut placed_by: HashMap<usize, (usize, usize)> = HashMap::new();
    for &(parent, cs_index, child) in &edges {
        placed_by.insert(child, (parent, cs_index));
    }
    occs.iter()
        .enumerate()
        .map(|(oid, occ)| {
            let placement = placed_by
                .get(&oid)
                .map(|&(parent, cs)| (&occs[parent], &occs[parent].sheet.child_sheets[cs]));
            let repeat = placement
                .zip(occ.channel.as_ref())
                .and_then(|((parent, cs), _)| {
                    let repeat = ChannelRepeat::parse(&cs.name)?;
                    Some((repeat, parent.path.child(cs.uuid)))
                });
            CompiledOccurrence {
                key: occ.key.clone(),
                path: occ.path.clone(),
                depth: occ.name_chain.len() + occ.channels.len(),
                designator: placement.map(|(_, cs)| cs.name.clone()),
                channel: occ.channel.as_ref().map(|s| s.channel.clone()),
                repeat,
            }
        })
        .collect()
}
