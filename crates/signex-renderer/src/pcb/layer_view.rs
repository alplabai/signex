//! What the board view shows of a board's layers (#2223): which layers are
//! hidden, which copper layer is active, and whether the ratsnest is drawn.
//!
//! Session state only. It decides what is drawn and what a pointer can hit,
//! and is never written to the `.snxpcb`. Names and colours of the layers are
//! the app's: this module only says which layers there are.

use std::collections::HashSet;

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, LayerDef, Pad, PcbBoard};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::PcbSnapshot;
use super::emit::{
    layer_rank_index, pad_from_footprint, sort_zone_stack, trace_from_segment, via_from_board_via,
    zone_from_board_zone,
};

/// Alpha factor for copper that is not on the active layer, and for a hidden
/// layer's swatch in the Layers panel.
pub const DIMMED_ALPHA: f32 = 0.35;

/// How an item stands against the active copper layer. Drawn in this order,
/// so the active layer ends up on top.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Emphasis {
    Dimmed,
    Normal,
    Active,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcbLayerView {
    hidden: HashSet<PcbLayer>,
    active: CopperLayer,
    ratsnest_visible: bool,
}

impl Default for PcbLayerView {
    fn default() -> Self {
        Self {
            hidden: HashSet::new(),
            active: CopperLayer::Top,
            ratsnest_visible: true,
        }
    }
}

/// The layer names a pad is on: its own list, else its footprint's layer.
fn pad_layer_names<'a>(footprint: &'a Footprint, pad: &'a Pad) -> &'a [String] {
    if pad.layers.is_empty() {
        std::slice::from_ref(&footprint.layer)
    } else {
        &pad.layers
    }
}

fn copper_of(names: &[String]) -> impl Iterator<Item = CopperLayer> + '_ {
    names
        .iter()
        .filter_map(|name| parse_pcb_layer(name)?.copper())
}

impl PcbLayerView {
    /// The active copper layer. It is hidden only while every copper layer
    /// of the board is (see [`Self::settle_active`]).
    pub fn active(&self) -> CopperLayer {
        self.active
    }

    /// The active layer as drawn: `None` when it is hidden, and then no
    /// copper is dimmed.
    pub fn shown_active(&self) -> Option<CopperLayer> {
        self.is_visible(PcbLayer::Copper(self.active))
            .then_some(self.active)
    }

    pub fn set_active(&mut self, layer: CopperLayer) {
        self.active = layer;
    }

    pub fn is_visible(&self, layer: PcbLayer) -> bool {
        !self.hidden.contains(&layer)
    }

    /// Hide `layer`, or show it again.
    pub fn toggle_visibility(&mut self, layer: PcbLayer) {
        if !self.hidden.remove(&layer) {
            self.hidden.insert(layer);
        }
    }

    pub fn show_all(&mut self) {
        self.hidden.clear();
    }

    pub fn has_hidden(&self) -> bool {
        !self.hidden.is_empty()
    }

    pub fn ratsnest_visible(&self) -> bool {
        self.ratsnest_visible
    }

    pub fn toggle_ratsnest(&mut self) {
        self.ratsnest_visible = !self.ratsnest_visible;
    }

    /// Whether an item on `names` is drawn and hit. A name this build does
    /// not know cannot be hidden, and an item with no layer is always shown.
    pub fn shows_any(&self, names: &[String]) -> bool {
        names.is_empty()
            || names
                .iter()
                .any(|name| parse_pcb_layer(name).is_none_or(|layer| self.is_visible(layer)))
    }

    /// Whether a pad is drawn and hit. A pad is drawn as copper, so it follows
    /// its copper layers: an SMD pad on Top Copper, Top Solder Mask and Top
    /// Paste goes with Top Copper. A pad on no copper layer follows its names.
    pub fn shows_pad(&self, footprint: &Footprint, pad: &Pad) -> bool {
        let names = pad_layer_names(footprint, pad);
        let mut copper = copper_of(names).peekable();
        if copper.peek().is_none() {
            return self.shows_any(names);
        }
        copper.any(|layer| self.is_visible(PcbLayer::Copper(layer)))
    }

    /// Where a pad stands against the active copper layer.
    pub fn pad_emphasis(&self, footprint: &Footprint, pad: &Pad) -> Emphasis {
        self.emphasis(pad_layer_names(footprint, pad))
    }

    /// Where an item on `names` stands against the active copper layer. An
    /// item on no copper layer, or drawn while the active layer is hidden, is
    /// [`Emphasis::Normal`]. An item spanning the active layer (a through via
    /// listed as Top and Bottom while an inner layer is active) is on it.
    pub fn emphasis(&self, names: &[String]) -> Emphasis {
        let Some(active) = self.shown_active() else {
            return Emphasis::Normal;
        };
        let mut span: Option<(CopperLayer, CopperLayer)> = None;
        for layer in copper_of(names) {
            span = Some(span.map_or((layer, layer), |(top, bottom)| {
                (top.min(layer), bottom.max(layer))
            }));
        }
        match span {
            Some((top, bottom)) if (top..=bottom).contains(&active) => Emphasis::Active,
            Some(_) => Emphasis::Dimmed,
            None => Emphasis::Normal,
        }
    }

    /// Step the active layer through the shown layers of `copper` (top to
    /// bottom), wrapping. Hidden layers are skipped; with none shown the
    /// active layer stays. An active layer the board lacks starts from the
    /// first or last.
    pub fn step_active(&mut self, copper: &[CopperLayer], forward: bool) {
        if let Some(next) = self.next_shown(copper, forward) {
            self.active = next;
        }
    }

    /// After a layer was hidden: a hidden active layer moves to the next
    /// shown copper layer, so the pill and the dimming never name a layer
    /// that is not drawn. With every copper layer hidden it stays.
    pub fn settle_active(&mut self, copper: &[CopperLayer]) {
        if self.shown_active().is_none() {
            self.step_active(copper, true);
        }
    }

    fn next_shown(&self, copper: &[CopperLayer], forward: bool) -> Option<CopperLayer> {
        let n = copper.len();
        let start = copper.iter().position(|&c| c == self.active);
        (1..=n)
            .map(|step| match (start, forward) {
                (Some(i), true) => copper[(i + step) % n],
                (Some(i), false) => copper[(i + n - step) % n],
                (None, true) => copper[step - 1],
                (None, false) => copper[n - step],
            })
            .find(|&c| self.is_visible(PcbLayer::Copper(c)))
    }
}

/// The copper layers a board defines, top to bottom; Top and Bottom when
/// it lists none.
pub fn board_copper_layers(layers: &[LayerDef]) -> Vec<CopperLayer> {
    let mut copper: Vec<CopperLayer> = layers
        .iter()
        .filter_map(|def| parse_pcb_layer(&def.name)?.copper())
        .collect();
    copper.sort();
    copper.dedup();
    if copper.is_empty() {
        copper = vec![CopperLayer::Top, CopperLayer::Bottom];
    }
    copper
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerGroup {
    Copper,
    Technical,
    Mechanical,
}

/// One row of the Layers panel. Its name is the app's to give (a catalog
/// message), except for a mechanical layer the user named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerRow {
    pub layer: PcbLayer,
    pub group: LayerGroup,
    /// The board's own name for a mechanical layer; `None` for every other row.
    pub user_name: Option<String>,
}

/// The rows to list for a board: its copper layers, every technical layer,
/// and the mechanical layers its stackup lists, by index.
pub fn layer_rows(layers: &[LayerDef]) -> Vec<LayerRow> {
    let row = |layer: PcbLayer, group| LayerRow {
        layer,
        group,
        user_name: None,
    };
    let mut rows: Vec<LayerRow> = board_copper_layers(layers)
        .into_iter()
        .map(|c| row(PcbLayer::Copper(c), LayerGroup::Copper))
        .collect();
    rows.extend(
        TechnicalLayer::ALL
            .into_iter()
            .map(|t| row(PcbLayer::Technical(t), LayerGroup::Technical)),
    );
    let mut mechanical: Vec<(u8, LayerRow)> = layers
        .iter()
        .filter_map(|def| match parse_pcb_layer(&def.name)? {
            PcbLayer::Mechanical(m) => Some((
                m.index(),
                LayerRow {
                    layer: PcbLayer::Mechanical(m),
                    group: LayerGroup::Mechanical,
                    user_name: (!def.display_name.is_empty()).then(|| def.display_name.clone()),
                },
            )),
            _ => None,
        })
        .collect();
    mechanical.sort_by_key(|(index, _)| *index);
    mechanical.dedup_by_key(|(index, _)| *index);
    rows.extend(mechanical.into_iter().map(|(_, row)| row));
    rows
}

fn active_last<T>(mut items: Vec<(Emphasis, T)>) -> Vec<(Emphasis, T)> {
    items.sort_by_key(|(emphasis, _)| *emphasis);
    items
}

impl PcbSnapshot {
    /// The snapshot of `board` as `view` shows it: hidden layers left out,
    /// copper off the active layer dimmed, the active layer drawn last.
    pub fn from_board_with_view(board: &PcbBoard, view: &PcbLayerView) -> Self {
        Self::from_board_filtered(board, Some(view))
    }

    pub(super) fn from_board_filtered(board: &PcbBoard, view: Option<&PcbLayerView>) -> Self {
        let shown = |names: &[String]| view.is_none_or(|v| v.shows_any(names));
        let emphasis = |names: &[String]| view.map_or(Emphasis::Normal, |v| v.emphasis(names));

        let mut traces = Vec::new();
        for segment in &board.segments {
            let names = std::slice::from_ref(&segment.layer);
            if shown(names) {
                let how = emphasis(names);
                let mut trace = trace_from_segment(segment);
                trace.dimmed = how == Emphasis::Dimmed;
                traces.push((how, trace));
            }
        }
        let mut vias = Vec::new();
        for via in &board.vias {
            if shown(&via.layers) {
                let how = emphasis(&via.layers);
                let mut input = via_from_board_via(via);
                input.dimmed = how == Emphasis::Dimmed;
                vias.push((how, input));
            }
        }
        let mut pads = Vec::new();
        for footprint in &board.footprints {
            for pad in &footprint.pads {
                if view.is_some_and(|v| !v.shows_pad(footprint, pad)) {
                    continue;
                }
                let how = view.map_or(Emphasis::Normal, |v| v.pad_emphasis(footprint, pad));
                if let Some(mut input) = pad_from_footprint(footprint, pad) {
                    input.dimmed = how == Emphasis::Dimmed;
                    pads.push((how, input));
                }
            }
        }

        let layer_ranks = layer_rank_index(board);
        let mut zones = Vec::new();
        let mut rule_areas = Vec::new();
        let mut zone_emphasis = vec![Emphasis::Normal; board.zones.len()];
        for (source_order, zone) in board.zones.iter().enumerate() {
            let names = std::slice::from_ref(&zone.layer);
            if !shown(names) {
                continue;
            }
            let Some(mut input) = zone_from_board_zone(zone, source_order, &layer_ranks) else {
                continue;
            };
            if input.rule_area {
                rule_areas.push(input);
            } else {
                zone_emphasis[source_order] = emphasis(names);
                input.dimmed = zone_emphasis[source_order] == Emphasis::Dimmed;
                zones.push(input);
            }
        }
        // The layer stack order first, then the active layer on top; the
        // second sort is stable, so zone priority inside one layer survives.
        sort_zone_stack(&mut zones);
        sort_zone_stack(&mut rule_areas);
        let zones = zones
            .into_iter()
            .map(|zone| (zone_emphasis[zone.source_order], zone))
            .collect();

        Self {
            traces: strip(active_last(traces)),
            vias: strip(active_last(vias)),
            pads: strip(active_last(pads)),
            zones: strip(active_last(zones)),
            rule_areas,
            ratsnest_lines: Vec::new(),
            drc_markers: Vec::new(),
            board_features: super::BoardFeaturesInput::from_board(board),
        }
    }
}

fn strip<T>(items: Vec<(Emphasis, T)>) -> Vec<T> {
    items.into_iter().map(|(_, item)| item).collect()
}

#[cfg(test)]
#[path = "layer_view_tests.rs"]
mod tests;
