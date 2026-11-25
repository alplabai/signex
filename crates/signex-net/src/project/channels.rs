//! Repeated sheets in the project walk (#1279): a sheet symbol whose
//! designator reads `Repeat(CH, 1, 4)` is walked once per channel, and each
//! channel occurrence renames its designators and its channel-local nets
//! through the project's [`ChannelNaming`]. The rules are in
//! `docs/MULTI_CHANNEL.md`, "Stitching".

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};

use signex_types::project::ChannelNaming;
use signex_types::schematic::{
    ChannelOccurrence, ChannelRepeat, ChildSheet, InstancePath, LabelType, Point, SchematicSheet,
    Symbol, normalize_port_name, parse_bus_name, repeated_entry_name,
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
    let number_bytes = number.to_le_bytes();
    let mut bytes = [0u8; 16];
    for (half, seed) in SEEDS.iter().enumerate() {
        let mut h = *seed;
        let input = sheet_symbol
            .as_bytes()
            .iter()
            .chain(number_bytes.iter())
            .chain(b"signex-channel".iter());
        for b in input {
            h ^= u64::from(*b);
            h = h.wrapping_mul(PRIME);
        }
        bytes[half * 8..half * 8 + 8].copy_from_slice(&h.to_be_bytes());
    }
    uuid::Builder::from_custom_bytes(bytes).into_uuid()
}

/// The version [`channel_uuid`] stamps on every channel placement.
const CHANNEL_UUID_VERSION: usize = 8;

/// The repeated sheet symbol in `placed` (uuid, designator) whose channel is
/// the placement `step`, with that channel.
fn channel_at_step<'a>(
    step: Uuid,
    mut placed: impl Iterator<Item = (Uuid, &'a str)>,
) -> Option<(Uuid, ChannelOccurrence)> {
    placed.find_map(|(symbol, designator)| {
        let channel = ChannelRepeat::parse(designator)?
            .occurrences()
            .into_iter()
            .find(|c| channel_uuid(symbol, c.number) == step)?;
        Some((symbol, channel))
    })
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
    let named: HashMap<Uuid, Cow<'_, str>> = occ
        .sheet
        .symbols
        .iter()
        .filter(|s| !s.is_power)
        .map(|s| {
            let base = s.designator_in(&occ.logical_path);
            (s.uuid, channel_designator(base, &occ.channels, naming))
        })
        .collect();
    for terminals in analysis.terminals.values_mut() {
        for t in terminals.iter_mut() {
            if let Some(name) = named.get(&t.symbol) {
                t.reference = name.to_string();
            }
        }
    }
}

/// The one answer to "what is this symbol called in this placement" for a
/// project with repeated sheets. Every reader outside the stitch (BOM,
/// netlist file, PDF, Update PCB, Annotate) asks here. A placement in no
/// channel is not listed and reads as [`Symbol::designator_in`].
#[derive(Debug, Clone, Default)]
pub struct ChannelDesignators {
    naming: ChannelNaming,
    placements: HashMap<InstancePath, ChannelPlacement>,
}

#[derive(Debug, Clone)]
struct ChannelPlacement {
    /// The placement all channels of the repeat share.
    logical: InstancePath,
    /// The channels it sits in, outermost first.
    channels: Vec<ChannelOccurrence>,
}

impl ChannelDesignators {
    /// The channel placements of the project walk, named through `naming`.
    pub fn of_project(graph: &ProjectGraph, naming: &ChannelNaming) -> Self {
        let mut issues = Vec::new();
        let (occs, _) = walk_occurrences(graph, &mut issues);
        let placements = occs
            .into_iter()
            .filter(|o| !o.channels.is_empty())
            .map(|o| {
                let placement = ChannelPlacement {
                    logical: o.logical_path,
                    channels: o.channels,
                };
                (o.path, placement)
            })
            .collect();
        Self {
            naming: naming.clone(),
            placements,
        }
    }

    /// The one placement `path` reads as, from the sheet symbols in `placed`
    /// (any sheets holding the chain's symbols). For a reader with a stored
    /// placement and no project walk; a step no symbol explains stays as is.
    pub fn of_path<'a>(
        path: &InstancePath,
        placed: impl Iterator<Item = &'a ChildSheet> + Clone,
        naming: &ChannelNaming,
    ) -> Self {
        Self::of_placements(path, placed.map(|cs| (cs.uuid, cs.name.as_str())), naming)
    }

    /// Whether a step of `path` has a channel uuid's form and no sheet symbol
    /// in `placed` (uuid, designator) explains it: the part's channel name
    /// cannot be told yet.
    pub fn has_unknown_channel<'a>(
        path: &InstancePath,
        placed: impl Iterator<Item = (Uuid, &'a str)> + Clone,
    ) -> bool {
        path.chain().iter().any(|&step| {
            step.get_version_num() == CHANNEL_UUID_VERSION
                && channel_at_step(step, placed.clone()).is_none()
        })
    }

    /// [`Self::of_path`] from each sheet symbol's uuid and designator, for a
    /// reader whose parent sheets are not open.
    pub fn of_placements<'a>(
        path: &InstancePath,
        placed: impl Iterator<Item = (Uuid, &'a str)> + Clone,
        naming: &ChannelNaming,
    ) -> Self {
        let mut logical = Vec::with_capacity(path.chain().len());
        let mut channels = Vec::new();
        for &step in path.chain() {
            match channel_at_step(step, placed.clone()) {
                Some((symbol, channel)) => {
                    logical.push(symbol);
                    channels.push(channel);
                }
                None => logical.push(step),
            }
        }
        let mut placements = HashMap::new();
        if !channels.is_empty() {
            placements.insert(
                path.clone(),
                ChannelPlacement {
                    logical: InstancePath::from_chain(logical),
                    channels,
                },
            );
        }
        Self {
            naming: naming.clone(),
            placements,
        }
    }

    /// Whether any placement is a channel of a repeated sheet.
    pub fn has_channels(&self) -> bool {
        !self.placements.is_empty()
    }

    /// Whether `path` is a channel of a repeated sheet.
    pub fn is_channel(&self, path: &InstancePath) -> bool {
        self.placements.contains_key(path)
    }

    /// Every channel placement: its path, the repeated placement its
    /// channels share and the channels it sits in, outermost first.
    pub fn channel_placements(
        &self,
    ) -> impl Iterator<Item = (&InstancePath, &InstancePath, &[ChannelOccurrence])> {
        self.placements
            .iter()
            .map(|(path, p)| (path, &p.logical, p.channels.as_slice()))
    }

    /// The placement a designator of `path` is stored at: the repeated
    /// placement for a channel, else `path` itself.
    pub fn logical_path<'a>(&'a self, path: &'a InstancePath) -> &'a InstancePath {
        self.placements.get(path).map_or(path, |p| &p.logical)
    }

    /// The designator `symbol` carries at `path`. A power symbol keeps its
    /// own, as in the netlist.
    pub fn designator_of<'s>(&self, symbol: &'s Symbol, path: &InstancePath) -> Cow<'s, str> {
        let Some(placement) = self.placements.get(path) else {
            return Cow::Borrowed(symbol.designator_in(path));
        };
        let base = symbol.designator_in(&placement.logical);
        if symbol.is_power {
            return Cow::Borrowed(base);
        }
        channel_designator(base, &placement.channels, &self.naming)
    }

    /// Give a throwaway copy of a sheet its channel designators at `path` as
    /// instance entries, so a reader of [`Symbol::designator_in`] prints them.
    /// Never call this on a document: nothing of it may reach a file.
    pub fn stamp(&self, sheet: &mut SchematicSheet, path: &InstancePath) {
        if !self.is_channel(path) {
            return;
        }
        for symbol in sheet.symbols.iter_mut().filter(|s| !s.is_power) {
            let name = self.designator_of(symbol, path).into_owned();
            if !name.trim().is_empty() {
                symbol.set_instance_designator(path, name);
            }
        }
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
        Some(scope) => (
            &chain[..chain.len().saturating_sub(1)],
            channel_net_text(scope, text, naming),
        ),
        None => (chain.as_slice(), Cow::Borrowed(text)),
    };
    if chain.is_empty() {
        text.into_owned()
    } else {
        format!("{}/{}", chain.join("/"), text)
    }
}

/// A Hierarchical or local label's text in channel `scope`: a shared entry's
/// name stays, any other takes the channel net format.
fn channel_net_text<'t>(
    scope: &ChannelScope,
    text: &'t str,
    naming: &ChannelNaming,
) -> Cow<'t, str> {
    if scope.shared.contains(text) {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(naming.net(text, &scope.channel))
    }
}

/// The channel each directly repeated placement of a project walk is, for a
/// reader that holds a label's placement and not its occurrence.
pub(super) struct ChannelNetNames<'n> {
    scopes: HashMap<InstancePath, ChannelScope>,
    naming: &'n ChannelNaming,
}

impl<'n> ChannelNetNames<'n> {
    pub(super) fn of_project(graph: &ProjectGraph, naming: &'n ChannelNaming) -> Self {
        let mut issues = Vec::new();
        let (occs, _) = walk_occurrences(graph, &mut issues);
        let scopes = occs
            .into_iter()
            .filter_map(|o| Some((o.path, o.channel?)))
            .collect();
        Self { scopes, naming }
    }

    /// Every label reads as written: for a naming that could not be read.
    pub(super) fn as_written(naming: &'n ChannelNaming) -> Self {
        Self {
            scopes: HashMap::new(),
            naming,
        }
    }

    /// A Hierarchical or local label's text at placement `path`; off a
    /// channel it is the text as written.
    pub(super) fn label_text(&self, path: &InstancePath, text: &str) -> String {
        match self.scopes.get(path) {
            Some(scope) => channel_net_text(scope, text, self.naming).into_owned(),
            None => text.to_string(),
        }
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
