//! What the PCB panel shows (mockup row 106): the board's nets with their
//! pads and tracks per layer, its differential pairs and polygon pours, and
//! the panel's own mode, filter, chips and selections. Nothing here edits
//! the board.

use std::collections::{BTreeMap, HashMap, HashSet};

use pcb_geom::ConnectionCounts;
use signex_engine::pcb::route::diff_pair_of;
use signex_i18n::{I18n, MessageId, ids};
use signex_types::format::parse_pcb_layer;
use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

/// One pad of a net.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbNetPad {
    /// The designator of its part.
    pub part: String,
    pub pad: String,
    /// The value of its part (`10k`, `7805`), the pad's descriptor in the
    /// net detail (#3001); empty when the part has none. The board stores
    /// no pin names to describe a pad by.
    pub part_value: String,
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

/// One differential pair of the board, as the router pairs its nets.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbDiffPairRow {
    /// What the two net names share (`USB_D` for `USB_DP` / `USB_DM`).
    pub name: String,
    pub positive: String,
    pub negative: String,
    /// Routed length of each half, in mm.
    pub positive_mm: f64,
    pub negative_mm: f64,
}

impl PcbDiffPairRow {
    /// How much longer one half is than the other, in mm.
    pub fn skew_mm(&self) -> f64 {
        (self.positive_mm - self.negative_mm).abs()
    }
}

/// One polygon pour of the board (the Polygon Manager's rows).
#[derive(Debug, Clone, PartialEq)]
pub struct PcbPourRow {
    pub zone: Uuid,
    /// The net it pours, empty for none.
    pub net: String,
    /// Its layer as the board names it.
    pub layer: String,
    pub priority: u32,
    pub shelved: bool,
}

impl PcbPourRow {
    /// A shelved pour holds no copper, so it needs a look.
    pub fn needs_attention(&self) -> bool {
        self.shelved
    }
}

/// The board's nets and totals.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbNetsSummary {
    pub nets: Vec<PcbNetRow>,
    pub pads: usize,
    /// The connection counts of this revision were read, so `unrouted` holds.
    pub unrouted_known: bool,
    /// The differential pairs, by name.
    pub pairs: Vec<PcbDiffPairRow>,
    /// The polygon pours, in board order.
    pub pours: Vec<PcbPourRow>,
}

/// What two pair nets share, without a trailing `_`: `USB_D` for `USB_DP`
/// and `USB_DM`; both names when they share nothing.
fn pair_name(positive: &str, negative: &str) -> String {
    let shared: String = positive
        .chars()
        .zip(negative.chars())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a)
        .collect();
    let stem = shared.trim_end_matches(['_', '-', '+', '.']);
    if stem.is_empty() {
        format!("{positive} / {negative}")
    } else {
        stem.to_owned()
    }
}

/// The polygon pours of `board`, in board order.
fn pours_of(board: &PcbBoard) -> Vec<PcbPourRow> {
    board
        .zones
        .iter()
        .filter(|zone| zone.is_pour())
        .map(|zone| PcbPourRow {
            zone: zone.uuid,
            net: zone.net_name.clone(),
            layer: zone.layer.clone(),
            priority: zone.priority,
            shelved: zone.shelved,
        })
        .collect()
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
                    part_value: fp.value.trim().to_owned(),
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
            pairs: Vec::new(),
            pours: pours_of(board),
        }
    }

    /// The same nets with the differential pairs the router finds on
    /// `board` under `classes` (`diff_pair_of`: by name ending, else by a
    /// class with a pair gap that holds two nets), each once, by name.
    #[must_use]
    pub fn with_pairs(mut self, board: &PcbBoard, classes: &[NetClass]) -> Self {
        let mut seen = HashSet::new();
        let mut pairs = Vec::new();
        for number in self.nets.iter().map(|n| n.number) {
            let Some(pair) = diff_pair_of(board, classes, number) else {
                continue;
            };
            if !seen.insert(pair.positive) {
                continue;
            }
            let net = |wanted: u32| self.nets.iter().find(|n| n.number == wanted);
            let (Some(p), Some(n)) = (net(pair.positive), net(pair.negative)) else {
                continue;
            };
            pairs.push(PcbDiffPairRow {
                name: pair_name(&p.name, &n.name),
                positive: p.name.clone(),
                negative: n.name.clone(),
                positive_mm: p.length_mm,
                negative_mm: n.length_mm,
            });
        }
        pairs.sort_by(|a, b| a.name.cmp(&b.name));
        self.pairs = pairs;
        self
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
    DiffPairs,
    XSignals,
    /// The board's polygon pours, the compact half of the Polygon Manager
    /// (mockup rows 106 and 178).
    Polygons,
}

impl PcbPanelMode {
    /// The modes the dropdown offers, in its order.
    pub const ALL: [Self; 5] = [
        Self::Nets,
        Self::NetClasses,
        Self::DiffPairs,
        Self::XSignals,
        Self::Polygons,
    ];

    pub fn message(self) -> MessageId {
        match self {
            Self::Nets => ids::PANELS_PCB_MODE_NETS,
            Self::NetClasses => ids::PANELS_PCB_MODE_NET_CLASSES,
            Self::DiffPairs => ids::PANELS_PCB_MODE_DIFF_PAIRS,
            Self::XSignals => ids::PANELS_PCB_MODE_XSIGNALS,
            Self::Polygons => ids::PANELS_PCB_MODE_POLYGONS,
        }
    }

    /// How many items the mode lists, for its dropdown entry. The board
    /// stores no xSignals yet, so that mode lists none.
    pub fn count(self, summary: &PcbNetsSummary, classes: &[NetClass]) -> usize {
        match self {
            Self::Nets => summary.nets.len(),
            Self::NetClasses => classes.len(),
            Self::DiffPairs => summary.pairs.len(),
            Self::XSignals => 0,
            Self::Polygons => summary.pours.len(),
        }
    }
}

/// A chip over the pour list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PcbPourChip {
    #[default]
    All,
    NeedsAttention,
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
    /// A class row of Net Classes, a pair row or a pour row was clicked:
    /// the same toggle as a net row.
    ClassClicked(String),
    PairClicked(String),
    PourClicked(Uuid),
    PourChipPicked(PcbPourChip),
    /// The Polygons mode and the Polygon Manager tab, which share
    /// [`PcbPanelUi::polygons`] (mockup row 178).
    Polygons(crate::panels::PolygonMsg),
}

/// The panel's own state, kept across context rebuilds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbPanelUi {
    pub mode: PcbPanelMode,
    pub filter: String,
    pub chip: PcbNetChip,
    /// The net whose detail shows, by name.
    pub selected: Option<String>,
    /// The class whose rules and members show under the class list.
    pub class: Option<String>,
    /// The pair whose detail shows, by name.
    pub pair: Option<String>,
    pub pour: Option<Uuid>,
    pub pour_chip: PcbPourChip,
    /// The Polygon Manager's view state, read by its tab and by this
    /// panel's Polygons mode.
    pub polygons: crate::panels::PolygonsUi,
}

/// `clicked`, unless it is what `current` holds already.
fn toggled<T: PartialEq>(current: Option<T>, clicked: T) -> Option<T> {
    (current.as_ref() != Some(&clicked)).then_some(clicked)
}

impl PcbPanelUi {
    pub fn apply(&mut self, msg: PcbPanelMsg) {
        match msg {
            PcbPanelMsg::Polygons(event) => self.polygons.apply(&event),
            PcbPanelMsg::ModePicked(mode) => self.mode = mode,
            PcbPanelMsg::FilterChanged(filter) => self.filter = filter,
            PcbPanelMsg::ChipPicked(chip) => self.chip = chip,
            PcbPanelMsg::NetClicked(net) => self.selected = toggled(self.selected.take(), net),
            PcbPanelMsg::ClassClicked(class) => self.class = toggled(self.class.take(), class),
            PcbPanelMsg::PairClicked(pair) => self.pair = toggled(self.pair.take(), pair),
            PcbPanelMsg::PourClicked(pour) => {
                self.pour = toggled(self.pour.take(), pour);
                // One pick for the panel and the Polygon Manager tab
                // (DECISIONS 2026-11-18, "Tab + compact panel pair").
                if self.pour == Some(pour) {
                    self.polygons.current = Some(pour);
                }
            }
            PcbPanelMsg::PourChipPicked(chip) => self.pour_chip = chip,
        }
    }

    /// The selected class, while the table still has it.
    pub fn selected_class<'c>(&self, classes: &'c [NetClass]) -> Option<&'c NetClass> {
        let name = self.class.as_deref()?;
        classes.iter().find(|class| class.name == name)
    }

    /// The selected pair, while the board still has it.
    pub fn selected_pair<'s>(&self, summary: &'s PcbNetsSummary) -> Option<&'s PcbDiffPairRow> {
        let name = self.pair.as_deref()?;
        summary.pairs.iter().find(|pair| pair.name == name)
    }

    /// The pours the chip keeps, in board order.
    pub fn shown_pours<'s>(&self, summary: &'s PcbNetsSummary) -> Vec<&'s PcbPourRow> {
        summary
            .pours
            .iter()
            .filter(|pour| match self.pour_chip {
                PcbPourChip::All => true,
                PcbPourChip::NeedsAttention => pour.needs_attention(),
            })
            .collect()
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
