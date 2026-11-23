//! The compiled design (#1308): the hierarchy as the stitcher sees it, with
//! every repeated sheet expanded into its channels and part and net counts
//! per sheet occurrence. Read off the same stitch as the project netlist;
//! nothing here re-derives a net.

use std::collections::{HashMap, HashSet};

use signex_types::designator::compare_references;
use signex_types::project::ChannelNaming;
use signex_types::schematic::{
    ChannelOccurrence, ChannelRepeat, InstancePath, LabelType, SchematicSheet,
};

use crate::project::{CompiledOccurrence, compiled_occurrences};
use crate::{NetIndex, NetMemberKind, ProjectGraph, SheetKey, build_project_netlist_indexed_named};

/// The compiled project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompiledDesign {
    /// Depth-first, parent before children. A repeated placement is one row
    /// (`repeat` set) followed by one row per channel (`channel` set) one
    /// level deeper, each followed by its own children.
    pub sheets: Vec<CompiledSheet>,
    /// Every net of the netlist, by name.
    pub nets: Vec<CompiledNet>,
    /// Distinct parts of the whole project.
    pub part_count: usize,
    /// Nets of the netlist.
    pub net_count: usize,
    /// Distinct sheet files the walk reached.
    pub sheet_file_count: usize,
    /// Channel occurrences across every repeated sheet.
    pub channel_count: usize,
}

impl CompiledDesign {
    /// Sheet occurrences: every row but the repeated-placement headers, so a
    /// sheet repeated four times counts four.
    pub fn occurrence_count(&self) -> usize {
        self.sheets.iter().filter(|s| s.repeat.is_none()).count()
    }

    /// Every sheet file the compile reached.
    pub fn sheet_keys(&self) -> impl Iterator<Item = &SheetKey> {
        let mut seen = HashSet::new();
        self.sheets
            .iter()
            .map(|s| &s.key)
            .filter(move |k| seen.insert(*k))
    }
}

/// One row of the compiled hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledSheet {
    pub key: SheetKey,
    /// The sheet symbol's designator; the file stem for a root page and for
    /// a repeated placement, the channel name for a channel.
    pub name: String,
    /// The sheet's file name (the last part of its key).
    pub file: String,
    /// The occurrence's placement path; for a repeated placement, the path of
    /// the sheet symbol itself.
    pub path: InstancePath,
    pub depth: usize,
    /// Set on the row of a repeated placement.
    pub repeat: Option<ChannelRepeat>,
    /// Set on a channel row.
    pub channel: Option<ChannelOccurrence>,
    /// Distinct parts on this occurrence (on a repeated placement: across
    /// its channels).
    pub parts: usize,
    /// Nets touching this occurrence (on a repeated placement: across its
    /// channels).
    pub nets: usize,
    /// The title block of the sheet file as the compile read it.
    pub title: SheetTitle,
}

/// The title block values a contents table lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetTitle {
    pub title: String,
    pub revision: String,
    pub date: String,
}

impl SheetTitle {
    fn of(sheet: &SchematicSheet) -> Self {
        let field = |key: &str| sheet.title_block.get(key).cloned().unwrap_or_default();
        Self {
            title: field("title"),
            revision: field("revision"),
            date: field("date"),
        }
    }
}

/// One net of the compiled design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledNet {
    pub name: String,
    /// Sheet occurrences the net touches.
    pub sheets: usize,
    /// Named by a power port or a Power label.
    pub is_power: bool,
}

/// Compile `graph`: walk it with repeated sheets expanded and naming applied,
/// stitch it, and count parts and nets per occurrence.
pub fn compile_design(graph: &ProjectGraph, naming: &ChannelNaming) -> CompiledDesign {
    let (netlist, index) = build_project_netlist_indexed_named(graph, naming);
    let occurrences = compiled_occurrences(graph);
    let counts = Counts::of_index(&index);

    let mut sheets: Vec<CompiledSheet> = Vec::new();
    let mut headed: HashSet<InstancePath> = HashSet::new();
    for occ in &occurrences {
        let own = (occ.key.clone(), occ.path.clone());
        if let (Some((repeat, placement)), Some(channel)) = (&occ.repeat, &occ.channel) {
            if headed.insert(placement.clone()) {
                sheets.push(repeat_row(
                    graph,
                    occ,
                    repeat,
                    placement,
                    &occurrences,
                    &counts,
                ));
            }
            sheets.push(CompiledSheet {
                name: channel.name.clone(),
                channel: Some(channel.clone()),
                ..row(graph, occ, &counts.of(&[own]))
            });
            continue;
        }
        sheets.push(CompiledSheet {
            name: occ
                .designator
                .clone()
                .unwrap_or_else(|| stem(&file_name(&occ.key))),
            ..row(graph, occ, &counts.of(&[own]))
        });
    }

    let mut nets: Vec<CompiledNet> = netlist
        .netlist
        .nets
        .iter()
        .enumerate()
        .map(|(i, net)| CompiledNet {
            name: net.name.clone(),
            sheets: counts.net_sheets.get(&i).map_or(0, HashSet::len),
            is_power: counts.power.contains(&i),
        })
        .collect();
    nets.sort_by(|a, b| compare_references(&a.name, &b.name));

    let sheet_file_count = occurrences
        .iter()
        .map(|o| &o.key)
        .collect::<HashSet<_>>()
        .len();
    CompiledDesign {
        channel_count: occurrences.iter().filter(|o| o.channel.is_some()).count(),
        part_count: index.components().len(),
        net_count: netlist.netlist.nets.len(),
        sheet_file_count,
        sheets,
        nets,
    }
}

/// The row of a repeated placement: its channels' counts together.
fn repeat_row(
    graph: &ProjectGraph<'_>,
    first: &CompiledOccurrence,
    repeat: &ChannelRepeat,
    placement: &InstancePath,
    occurrences: &[CompiledOccurrence],
    counts: &Counts,
) -> CompiledSheet {
    let channels: Vec<Occurrence> = occurrences
        .iter()
        .filter(|o| o.repeat.as_ref().is_some_and(|(_, p)| p == placement))
        .map(|o| (o.key.clone(), o.path.clone()))
        .collect();
    let file = file_name(&first.key);
    CompiledSheet {
        name: stem(&file),
        path: placement.clone(),
        depth: first.depth.saturating_sub(1),
        repeat: Some(repeat.clone()),
        channel: None,
        ..row(graph, first, &counts.of(&channels))
    }
}

fn row(
    graph: &ProjectGraph<'_>,
    occ: &CompiledOccurrence,
    (parts, nets): &(usize, usize),
) -> CompiledSheet {
    CompiledSheet {
        key: occ.key.clone(),
        name: String::new(),
        file: file_name(&occ.key),
        path: occ.path.clone(),
        depth: occ.depth,
        repeat: None,
        channel: None,
        parts: *parts,
        nets: *nets,
        title: graph
            .sheets
            .get(&occ.key)
            .map(SheetTitle::of)
            .unwrap_or_default(),
    }
}

type Occurrence = (SheetKey, InstancePath);

/// Parts and nets per occurrence, read from the net index.
struct Counts {
    /// Occurrence -> designators of its parts (an unannotated part is its
    /// own entry, keyed by its symbol).
    parts: HashMap<Occurrence, HashSet<String>>,
    /// Occurrence -> indices of the nets touching it.
    nets: HashMap<Occurrence, HashSet<usize>>,
    /// Net index -> occurrences it touches.
    net_sheets: HashMap<usize, HashSet<Occurrence>>,
    /// Nets named by a power port or a Power label.
    power: HashSet<usize>,
}

impl Counts {
    fn of_index(index: &NetIndex) -> Self {
        let mut parts: HashMap<Occurrence, HashSet<String>> = HashMap::new();
        for p in index.placed_symbols() {
            let key = if p.is_annotated() {
                p.designator.clone()
            } else {
                format!("\u{1}{}", p.id)
            };
            parts
                .entry((p.sheet.clone(), p.occurrence.clone()))
                .or_default()
                .insert(key);
        }
        let mut nets: HashMap<Occurrence, HashSet<usize>> = HashMap::new();
        let mut net_sheets: HashMap<usize, HashSet<Occurrence>> = HashMap::new();
        let mut power = HashSet::new();
        for i in 0..index.net_count() {
            let Some(listed) = index.net(i) else {
                continue;
            };
            if listed.net.is_none() {
                continue;
            }
            for m in &listed.members {
                let occ = (m.sheet.clone(), m.occurrence.clone());
                nets.entry(occ.clone()).or_default().insert(i);
                net_sheets.entry(i).or_default().insert(occ);
                if matches!(
                    m.kind,
                    NetMemberKind::PowerPort | NetMemberKind::Label(LabelType::Power)
                ) {
                    power.insert(i);
                }
            }
        }
        Self {
            parts,
            nets,
            net_sheets,
            power,
        }
    }

    /// `(parts, nets)` across `occurrences`, each counted once.
    fn of(&self, occurrences: &[Occurrence]) -> (usize, usize) {
        let mut parts: HashSet<&String> = HashSet::new();
        let mut nets: HashSet<usize> = HashSet::new();
        for occ in occurrences {
            parts.extend(self.parts.get(occ).into_iter().flatten());
            nets.extend(self.nets.get(occ).into_iter().flatten().copied());
        }
        (parts.len(), nets.len())
    }
}

fn file_name(key: &SheetKey) -> String {
    let s = key.as_str();
    s.rsplit(['/', '\\']).next().unwrap_or(s).to_string()
}

fn stem(file: &str) -> String {
    file.rsplit_once('.')
        .map_or(file, |(stem, _)| stem)
        .to_string()
}
