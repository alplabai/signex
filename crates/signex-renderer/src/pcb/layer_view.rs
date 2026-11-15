//! What the board view shows of a board's layers (#2223): which layers are
//! hidden, which layer is active, single-layer mode, and whether the
//! ratsnest is drawn.
//!
//! Session state only. It decides what is drawn and what a pointer can hit,
//! and is never written to the `.snxpcb`. Names and colours of the layers are
//! the app's: this module only says which layers there are.

use std::collections::HashSet;

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, LayerDef, Pad, PcbBoard};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::emit::{
    is_through_pad, layer_rank_index, pad_from_footprint, sort_zone_stack, trace_from_segment,
    via_from_board_via, zone_from_board_zone,
};
use super::footprint_text::footprint_texts;
use super::graphics::{Drawings, board_drawings, footprint_drawings};
use super::layer_rows::{LayerGroup, group_layers};
use super::{CopperInk, PadInput, PcbSnapshot};

/// Alpha factor for copper that is not on the active layer, and for a hidden
/// layer's swatch in the Layers panel.
pub const DIMMED_ALPHA: f32 = 0.35;

/// The Board Outline row: the layer whose visibility shows or hides the
/// drawn board edge (`PcbBoard::outline` and the rims of its openings). The
/// `.snxpcb` keeps the `EdgeCuts` token for it.
pub const BOARD_OUTLINE: PcbLayer = PcbLayer::Technical(TechnicalLayer::EdgeCuts);

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
    /// The layers the user hid (their eye in the Layers panel).
    hidden: HashSet<PcbLayer>,
    /// Any layer of the board: a copper layer, an overlay, a mask, the
    /// board outline or a mechanical layer.
    active: PcbLayer,
    ratsnest_visible: bool,
    /// The Multi-Layer row: vias and through-hole pads.
    multi_layer_visible: bool,
    /// The board origin marker (#2351).
    origin_marker_visible: bool,
    /// Single-layer mode: only the active layer is drawn and hit, with the
    /// board outline and the vias and through-hole pads that reach it.
    single_layer: bool,
}

impl Default for PcbLayerView {
    fn default() -> Self {
        Self {
            hidden: HashSet::new(),
            active: PcbLayer::Copper(CopperLayer::Top),
            ratsnest_visible: true,
            multi_layer_visible: true,
            origin_marker_visible: true,
            single_layer: false,
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

/// The topmost and bottommost copper layer of an item.
type CopperSpan = (CopperLayer, CopperLayer);

fn copper_span(names: &[String]) -> Option<CopperSpan> {
    copper_of(names).fold(None, |span, layer| {
        Some(span.map_or((layer, layer), |(top, bottom): CopperSpan| {
            (top.min(layer), bottom.max(layer))
        }))
    })
}

/// How an item on `names` is drawn: its emphasis under `view` (nothing is
/// dimmed without one), and its ink: the Multi-Layer for a via or a
/// through-hole pad (`through`), else its topmost copper layer.
fn classify(view: Option<&PcbLayerView>, names: &[String], through: bool) -> (Emphasis, CopperInk) {
    let span = copper_span(names);
    let emphasis = view.map_or(Emphasis::Normal, |v| v.span_emphasis(span));
    let ink = match span {
        _ if through => CopperInk::MultiLayer,
        Some((top, _)) => CopperInk::Layer(top),
        None => CopperInk::Unknown,
    };
    (emphasis, ink)
}

impl PcbLayerView {
    /// The active layer. It is hidden only while every layer it could move
    /// to is (see [`Self::settle_active`]).
    pub fn active(&self) -> PcbLayer {
        self.active
    }

    /// The active layer as drawn: `None` when it is hidden, and then no
    /// copper is dimmed.
    pub fn shown_active(&self) -> Option<PcbLayer> {
        (!self.is_hidden(self.active)).then_some(self.active)
    }

    /// The active layer when it is a shown copper layer: the copper the
    /// rest of the copper is dimmed against.
    pub fn active_copper(&self) -> Option<CopperLayer> {
        self.shown_active().and_then(PcbLayer::copper)
    }

    pub fn set_active(&mut self, layer: impl Into<PcbLayer>) {
        self.active = layer.into();
    }

    /// Whether the user hid `layer` (its eye), whatever single-layer mode
    /// draws.
    pub fn is_hidden(&self, layer: PcbLayer) -> bool {
        self.hidden.contains(&layer)
    }

    /// Whether `layer` is drawn and hit: not hidden, and in single-layer
    /// mode the active layer or the board outline.
    pub fn is_visible(&self, layer: PcbLayer) -> bool {
        !self.is_hidden(layer)
            && (!self.single_layer || layer == self.active || layer == BOARD_OUTLINE)
    }

    /// Hide `layer`, or show it again.
    pub fn toggle_visibility(&mut self, layer: PcbLayer) {
        if !self.hidden.remove(&layer) {
            self.hidden.insert(layer);
        }
    }

    /// Hide `layer`; a hidden one stays hidden.
    pub fn hide(&mut self, layer: PcbLayer) {
        self.hidden.insert(layer);
    }

    /// Show `layer` as the active one and hide every other layer of
    /// `layers` (#2379). The board outline stays as it is, as single-layer
    /// mode keeps it, and so does the Multi-Layer row.
    pub fn show_only(&mut self, layers: &[PcbLayer], layer: PcbLayer) {
        let others = layers
            .iter()
            .copied()
            .filter(|&l| l != layer && l != BOARD_OUTLINE);
        self.hidden.extend(others);
        self.hidden.remove(&layer);
        self.active = layer;
    }

    pub fn show_all(&mut self) {
        self.hidden.clear();
        self.multi_layer_visible = true;
    }

    pub fn has_hidden(&self) -> bool {
        !self.hidden.is_empty() || !self.multi_layer_visible
    }

    /// Show or hide every layer of one Layers panel group of a board; the
    /// Drill group carries the Multi-Layer row too.
    pub fn set_group_visible(&mut self, layers: &[LayerDef], group: LayerGroup, visible: bool) {
        for layer in group_layers(layers, group) {
            if visible {
                self.hidden.remove(&layer);
            } else {
                self.hidden.insert(layer);
            }
        }
        if group == LayerGroup::Drill {
            self.multi_layer_visible = visible;
        }
    }

    /// Whether only the active layer is drawn.
    pub fn single_layer(&self) -> bool {
        self.single_layer
    }

    pub fn toggle_single_layer(&mut self) {
        self.single_layer = !self.single_layer;
    }

    /// Whether vias and through-hole pads (the Multi-Layer row) are shown.
    pub fn multi_layer_visible(&self) -> bool {
        self.multi_layer_visible
    }

    pub fn toggle_multi_layer(&mut self) {
        self.multi_layer_visible = !self.multi_layer_visible;
    }

    /// Whether a via on `names` is drawn and hit: the Multi-Layer row is
    /// shown and so is one of its layers; in single-layer mode, it reaches
    /// the active copper layer.
    pub fn shows_via(&self, names: &[String]) -> bool {
        self.multi_layer_visible && self.shows_through(names)
    }

    fn shows_through(&self, names: &[String]) -> bool {
        if !self.single_layer {
            return self.shows_any(names);
        }
        match (self.active_copper(), copper_span(names)) {
            (Some(active), Some((top, bottom))) => (top..=bottom).contains(&active),
            _ => false,
        }
    }

    pub fn ratsnest_visible(&self) -> bool {
        self.ratsnest_visible
    }

    pub fn toggle_ratsnest(&mut self) {
        self.ratsnest_visible = !self.ratsnest_visible;
    }

    pub fn origin_marker_visible(&self) -> bool {
        self.origin_marker_visible
    }

    pub fn set_origin_marker_visible(&mut self, visible: bool) {
        self.origin_marker_visible = visible;
    }

    pub fn toggle_origin_marker(&mut self) {
        self.origin_marker_visible = !self.origin_marker_visible;
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
    /// A through-hole pad also needs the Multi-Layer row shown.
    pub fn shows_pad(&self, footprint: &Footprint, pad: &Pad) -> bool {
        let through = is_through_pad(pad);
        if through && !self.multi_layer_visible {
            return false;
        }
        let names = pad_layer_names(footprint, pad);
        let mut copper = copper_of(names).peekable();
        if copper.peek().is_none() {
            return self.shows_any(names);
        }
        if through && self.single_layer {
            return self.shows_through(names);
        }
        copper.any(|layer| self.is_visible(PcbLayer::Copper(layer)))
    }

    /// Where an item on `names` stands against the active copper layer. An
    /// item on no copper layer, or drawn while the active layer is hidden or
    /// not copper, is [`Emphasis::Normal`]. An item spanning the active
    /// layer (a through via listed as Top and Bottom while an inner layer is
    /// active) is on it.
    pub fn emphasis(&self, names: &[String]) -> Emphasis {
        self.span_emphasis(copper_span(names))
    }

    fn span_emphasis(&self, span: Option<CopperSpan>) -> Emphasis {
        let (Some(active), Some((top, bottom))) = (self.active_copper(), span) else {
            return Emphasis::Normal;
        };
        if (top..=bottom).contains(&active) {
            Emphasis::Active
        } else {
            Emphasis::Dimmed
        }
    }

    /// Step the active layer through the shown layers of `layers`, in their
    /// order, wrapping. Hidden layers are skipped; with none shown the
    /// active layer stays. An active layer not in `layers` starts from the
    /// first or last.
    pub fn step_active(&mut self, layers: &[PcbLayer], forward: bool) {
        if let Some(next) = self.next_shown(layers, forward) {
            self.active = next;
        }
    }

    /// After a layer was hidden: a hidden active layer moves to the next
    /// shown one of `layers`, so the pill and the dimming never name a layer
    /// that is not drawn. With every one of them hidden it stays.
    pub fn settle_active(&mut self, layers: &[PcbLayer]) {
        if self.shown_active().is_none() {
            self.step_active(layers, true);
        }
    }

    fn next_shown(&self, layers: &[PcbLayer], forward: bool) -> Option<PcbLayer> {
        let n = layers.len();
        let start = layers.iter().position(|&l| l == self.active);
        (1..=n)
            .map(|step| match (start, forward) {
                (Some(i), true) => layers[(i + step) % n],
                (Some(i), false) => layers[(i + n - step) % n],
                (None, true) => layers[step - 1],
                (None, false) => layers[n - step],
            })
            .find(|&l| !self.is_hidden(l))
    }
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

        let mut traces = Vec::new();
        for segment in &board.segments {
            let names = std::slice::from_ref(&segment.layer);
            if shown(names) {
                let (how, ink) = classify(view, names, false);
                let mut trace = trace_from_segment(segment);
                trace.dimmed = how == Emphasis::Dimmed;
                trace.ink = ink;
                traces.push((how, trace));
            }
        }
        let mut vias = Vec::new();
        for via in &board.vias {
            if view.is_none_or(|v| v.shows_via(&via.layers)) {
                let (how, ink) = classify(view, &via.layers, true);
                let mut input = via_from_board_via(via);
                input.dimmed = how == Emphasis::Dimmed;
                input.ink = ink;
                vias.push((how, input));
            }
        }
        let pads: Vec<_> = board
            .footprints
            .iter()
            .flat_map(|footprint| footprint_pads(footprint, view))
            .collect();
        let mut drawings = Drawings {
            texts: board
                .footprints
                .iter()
                .flat_map(|footprint| footprint_texts(footprint, view))
                .collect(),
            ..Drawings::default()
        };
        board_drawings(board, view, &mut drawings);
        for footprint in &board.footprints {
            footprint_drawings(footprint, view, &mut drawings);
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
                let (how, ink) = classify(view, names, false);
                zone_emphasis[source_order] = how;
                input.ink = ink;
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
            board_features: super::BoardFeaturesInput {
                edge_hidden: view.is_some_and(|v| !v.is_visible(BOARD_OUTLINE)),
                ..super::BoardFeaturesInput::from_board(board)
            },
            texts: drawings.texts,
            graphics: drawings.graphics,
        }
    }
}

impl PcbSnapshot {
    /// The pads of `footprint` alone, drawn as the board draws them with
    /// every layer shown: a library preview (#2348) lowers the footprint
    /// through here, so it cannot look different from the board. Its
    /// drawings are left out: the preview draws them itself, in the same
    /// board palette colours (`ResolvedTheme::board_layer_color`).
    pub fn from_footprint(footprint: &Footprint) -> Self {
        Self {
            pads: strip(footprint_pads(footprint, None)),
            ..Self::default()
        }
    }
}

/// The pads of `footprint` that `view` shows, each with its emphasis and ink.
fn footprint_pads(footprint: &Footprint, view: Option<&PcbLayerView>) -> Vec<(Emphasis, PadInput)> {
    let mut pads = Vec::new();
    for pad in &footprint.pads {
        if view.is_some_and(|v| !v.shows_pad(footprint, pad)) {
            continue;
        }
        let (how, ink) = classify(view, pad_layer_names(footprint, pad), is_through_pad(pad));
        if let Some(mut input) = pad_from_footprint(footprint, pad) {
            input.dimmed = how == Emphasis::Dimmed;
            input.ink = ink;
            pads.push((how, input));
        }
    }
    pads
}

fn strip<T>(items: Vec<(Emphasis, T)>) -> Vec<T> {
    items.into_iter().map(|(_, item)| item).collect()
}

#[cfg(test)]
#[path = "layer_view_tests.rs"]
mod tests;
