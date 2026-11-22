//! What the PCB panel shows (mockup row 106): the board's nets with their
//! pads and tracks per layer, and the panel's own mode, filter, chip and
//! selected net. Nothing here edits the board.

use std::collections::{BTreeMap, HashMap};

use pcb_geom::ConnectionCounts;
use signex_i18n::{I18n, MessageId, ids};
use signex_types::format::parse_pcb_layer;
use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::PcbLayer;

/// One pad of a net.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbNetPad {
    /// The designator of its part.
    pub part: String,
    pub pad: String,
    /// Its layers as the board names them.
    pub layers: Vec<String>,
    /// A drilled pad reaches every copper layer.
    pub is_through: bool,
}

/// A net's tracks on one layer.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbNetLayer {
    /// The layer as the board names it.
    pub layer: String,
    pub tracks: usize,
    pub length_mm: f64,
}

/// One net row, all from the board.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbNetRow {
    pub number: u32,
    pub name: String,
    /// Pads on the net.
    pub nodes: usize,
    /// Total length of the net's track segments, in mm.
    pub length_mm: f64,
    /// Connections still to route (ratsnest lines).
    pub unrouted: usize,
    pub pads: Vec<PcbNetPad>,
    /// Copper top to bottom, then any other layer by name.
    pub layers: Vec<PcbNetLayer>,
    pub vias: usize,
}

/// The board's nets and totals.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbNetsSummary {
    pub nets: Vec<PcbNetRow>,
    pub pads: usize,
    /// The connection counts of this revision were read, so `unrouted` holds.
    pub unrouted_known: bool,
}

/// The board's net list, plus any net a pad names that the list lacks.
fn net_names(board: &PcbBoard) -> BTreeMap<u32, String> {
    let mut names = BTreeMap::new();
    for n in &board.nets {
        names.insert(n.number, n.name.clone());
    }
    for pad in board.footprints.iter().flat_map(|fp| fp.pads.iter()) {
        if let Some(net) = &pad.net {
            names.entry(net.number).or_insert_with(|| net.name.clone());
        }
    }
    names
}

/// The pads of each net, and how many pads the board has.
fn pads_by_net(board: &PcbBoard) -> (HashMap<u32, Vec<PcbNetPad>>, usize) {
    let mut by_net = HashMap::<u32, Vec<PcbNetPad>>::new();
    let mut total = 0;
    for fp in &board.footprints {
        for pad in &fp.pads {
            total += 1;
            if let Some(net) = &pad.net {
                by_net.entry(net.number).or_default().push(PcbNetPad {
                    part: fp.reference.clone(),
                    pad: pad.number.clone(),
                    layers: pad.layers.clone(),
                    is_through: pad.drill.is_some(),
                });
            }
        }
    }
    (by_net, total)
}

/// The tracks of each net per layer, copper top to bottom.
fn layers_by_net(board: &PcbBoard) -> HashMap<u32, Vec<PcbNetLayer>> {
    let mut sums = HashMap::<u32, BTreeMap<&str, (usize, f64)>>::new();
    for seg in &board.segments {
        let (dx, dy) = (seg.end.x - seg.start.x, seg.end.y - seg.start.y);
        let sum = sums
            .entry(seg.net)
            .or_default()
            .entry(seg.layer.as_str())
            .or_default();
        sum.0 += 1;
        sum.1 += dx.hypot(dy);
    }
    sums.into_iter()
        .map(|(net, layers)| {
            let mut rows: Vec<PcbNetLayer> = layers
                .into_iter()
                .map(|(layer, (tracks, length_mm))| PcbNetLayer {
                    layer: layer.to_owned(),
                    tracks,
                    length_mm,
                })
                .collect();
            rows.sort_by_cached_key(|row| {
                let copper = parse_pcb_layer(&row.layer).and_then(PcbLayer::copper);
                (copper.is_none(), copper, row.layer.clone())
            });
            (net, rows)
        })
        .collect()
}

impl PcbNetsSummary {
    /// Summarise `board`: pads, tracks per layer and vias of each net.
    pub fn of(board: &PcbBoard) -> Self {
        let (mut pads, total) = pads_by_net(board);
        let mut layers = layers_by_net(board);
        let mut vias = HashMap::<u32, usize>::new();
        for via in &board.vias {
            *vias.entry(via.net).or_default() += 1;
        }
        let nets = net_names(board)
            .into_iter()
            .filter(|(_, name)| !name.is_empty())
            .map(|(number, name)| {
                let pads = pads.remove(&number).unwrap_or_default();
                let layers = layers.remove(&number).unwrap_or_default();
                PcbNetRow {
                    number,
                    name,
                    nodes: pads.len(),
                    length_mm: layers.iter().map(|l| l.length_mm).sum(),
                    unrouted: 0,
                    pads,
                    layers,
                    vias: vias.get(&number).copied().unwrap_or(0),
                }
            })
            .collect();
        Self {
            nets,
            pads: total,
            unrouted_known: false,
        }
    }

    /// The same nets with what `counts` says is still unrouted.
    #[must_use]
    pub fn with_unrouted(mut self, counts: &ConnectionCounts) -> Self {
        for net in &mut self.nets {
            net.unrouted = counts
                .per_net
                .get(&net.number)
                .map_or(0, |unrouted| unrouted.lines.len());
        }
        self.unrouted_known = true;
        self
    }
}

/// The name of the class `net` belongs to: the first that matches.
pub fn class_of<'c>(classes: &'c [NetClass], net: &str) -> Option<&'c str> {
    first_matching_class(classes, net).map(|class| class.name.as_str())
}

/// What the panel lists (the mode dropdown of mockup row 106).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PcbPanelMode {
    #[default]
    Nets,
    NetClasses,
}

impl PcbPanelMode {
    /// The modes the dropdown offers, in its order.
    pub const ALL: [Self; 2] = [Self::Nets, Self::NetClasses];

    pub fn message(self) -> MessageId {
        match self {
            Self::Nets => ids::PANELS_PCB_MODE_NETS,
            Self::NetClasses => ids::PANELS_PCB_MODE_NET_CLASSES,
        }
    }
}

/// A chip over the net list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PcbNetChip {
    #[default]
    All,
    Unrouted,
    Class(String),
}

/// What the user did in the PCB panel.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbPanelMsg {
    ModePicked(PcbPanelMode),
    FilterChanged(String),
    ChipPicked(PcbNetChip),
    /// A net row was clicked: it is selected, or no longer when it was.
    NetClicked(String),
}

/// The panel's own state, kept across context rebuilds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbPanelUi {
    pub mode: PcbPanelMode,
    pub filter: String,
    pub chip: PcbNetChip,
    /// The net whose detail shows, by name.
    pub selected: Option<String>,
}

impl PcbPanelUi {
    pub fn apply(&mut self, msg: PcbPanelMsg) {
        match msg {
            PcbPanelMsg::ModePicked(mode) => self.mode = mode,
            PcbPanelMsg::FilterChanged(filter) => self.filter = filter,
            PcbPanelMsg::ChipPicked(chip) => self.chip = chip,
            PcbPanelMsg::NetClicked(net) => {
                self.selected = (self.selected.as_ref() != Some(&net)).then_some(net);
            }
        }
    }

    /// The picked chip, or All when it no longer stands for anything: a
    /// class that is gone, or Unrouted before the counts are read.
    pub fn chip_in_force(&self, summary: &PcbNetsSummary, classes: &[NetClass]) -> PcbNetChip {
        match &self.chip {
            PcbNetChip::Unrouted if !summary.unrouted_known => PcbNetChip::All,
            PcbNetChip::Class(name) if !classes.iter().any(|c| &c.name == name) => PcbNetChip::All,
            chip => chip.clone(),
        }
    }

    /// The nets the chip and the filter keep, in board order.
    pub fn shown<'s>(
        &self,
        summary: &'s PcbNetsSummary,
        classes: &[NetClass],
        i18n: &I18n,
    ) -> Vec<&'s PcbNetRow> {
        let needle = i18n.fold_for_search(self.filter.trim());
        let chip = self.chip_in_force(summary, classes);
        summary
            .nets
            .iter()
            .filter(|net| match &chip {
                PcbNetChip::All => true,
                PcbNetChip::Unrouted => net.unrouted > 0,
                PcbNetChip::Class(name) => class_of(classes, &net.name) == Some(name.as_str()),
            })
            .filter(|net| {
                needle.is_empty() || i18n.fold_for_search(&net.name).contains(needle.as_str())
            })
            .collect()
    }

    /// The selected net, while the board still has it.
    pub fn selected_net<'s>(&self, summary: &'s PcbNetsSummary) -> Option<&'s PcbNetRow> {
        let name = self.selected.as_deref()?;
        summary.nets.iter().find(|net| net.name == name)
    }
}

/// The chips over the list with their counts: All, Unrouted once the counts
/// are read, and each class with a net on this board.
pub fn net_chips(summary: &PcbNetsSummary, classes: &[NetClass]) -> Vec<(PcbNetChip, usize)> {
    let mut chips = vec![(PcbNetChip::All, summary.nets.len())];
    if summary.unrouted_known {
        let unrouted = summary.nets.iter().filter(|n| n.unrouted > 0).count();
        chips.push((PcbNetChip::Unrouted, unrouted));
    }
    for class in classes {
        let members = summary
            .nets
            .iter()
            .filter(|n| class_of(classes, &n.name) == Some(class.name.as_str()))
            .count();
        if members > 0 {
            chips.push((PcbNetChip::Class(class.name.clone()), members));
        }
    }
    chips
}

/// The net whose board highlight must toggle so the highlight follows the
/// selection: the newly selected net unless it is already bright, or the
/// net just deselected while it is the bright one.
pub fn highlight_toggle(
    shown: Option<&str>,
    previous: Option<&str>,
    selected: Option<&str>,
) -> Option<String> {
    match selected {
        Some(net) => (shown != Some(net)).then(|| net.to_owned()),
        None => previous.filter(|p| shown == Some(*p)).map(str::to_owned),
    }
}
