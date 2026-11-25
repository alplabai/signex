//! What the board view shows of a board's layers (#2223): which layers are
//! hidden, which layer is active, single-layer mode, and whether the
//! ratsnest is drawn.
//!
//! It decides what is drawn and what a pointer can hit, and is never written
//! to the `.snxpcb`; the app keeps it per board in the user's preferences
//! (#2480, [`PcbLayerView::kept`]). Names and colours of the layers are the
//! app's: this module only says which layers there are.

use std::collections::HashSet;

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard};
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::board_paint::{BoardObject, BoardPaint};
use super::emit::{
    arc_traces, is_through_pad, layer_rank_index, pad_from_footprint, sort_zone_stack,
    trace_from_segment, via_from_board_via, zone_from_board_zone,
};
use super::footprint_text::footprint_texts;
use super::graphics::{Drawings, board_drawings, footprint_drawings};
use super::layer_sets::ShownLayerSet;
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
    /// The orientation tile over the canvas (mockup row 194).
    orientation_tile_visible: bool,
    /// Single-layer mode: only the active layer is drawn and hit, with the
    /// board outline and the vias and through-hole pads that reach it.
    single_layer: bool,
    /// The layer set the strip under the canvas lists (#2394); `None`:
    /// every layer.
    layer_set: Option<ShownLayerSet>,
    /// The highlighted layer: every other layer is drawn dimmed (#2394).
    highlighted: Option<PcbLayer>,
    /// The board seen from below: mirrored, bottom layers on top (#2394).
    flipped: bool,
    /// The view's own layer colours and transparencies (#2363).
    paint: BoardPaint,
    /// The kinds of object the user hid (View Options, #2363).
    hidden_objects: HashSet<BoardObject>,
    /// The Layers panel lists only the layers the board uses (#2363).
    used_only: bool,
    /// Show only unrouted (mockup row 164): the board is dimmed except its
    /// ratsnest lines, until the board closes.
    only_unrouted: bool,
}

impl Default for PcbLayerView {
    fn default() -> Self {
        Self {
            hidden: HashSet::new(),
            active: PcbLayer::Copper(CopperLayer::Top),
            ratsnest_visible: true,
            multi_layer_visible: true,
            origin_marker_visible: true,
            orientation_tile_visible: true,
            single_layer: false,
            layer_set: None,
            highlighted: None,
            flipped: false,
            paint: BoardPaint::default(),
            hidden_objects: HashSet::new(),
            used_only: false,
            only_unrouted: false,
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
    let emphasis = view.map_or(Emphasis::Normal, |v| v.item_emphasis(names, span));
    let ink = match span {
        _ if through => CopperInk::MultiLayer,
        Some((top, _)) => CopperInk::Layer(top),
        None => CopperInk::Unknown,
    };
    (emphasis, ink)
}

/// The active copper layer when a via on `names` has copper there.
fn active_in_span(view: Option<&PcbLayerView>, names: &[String]) -> Option<CopperLayer> {
    let active = view?.active_copper()?;
    let (top, bottom) = copper_span(names)?;
    (top..=bottom).contains(&active).then_some(active)
}

/// The active copper layer for a plated through-hole pad, which has copper on
/// every layer; a non-plated pad has none to paint.
fn active_on_through_pad(view: Option<&PcbLayerView>, pad: &Pad) -> Option<CopperLayer> {
    (pad.pad_type == PadType::Thru)
        .then(|| view.and_then(PcbLayerView::active_copper))
        .flatten()
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

    /// The layers the user hid, in no order.
    pub fn hidden_layers(&self) -> impl Iterator<Item = PcbLayer> + '_ {
        self.hidden.iter().copied()
    }

    /// This view as it is kept across launches (#2480): the highlight and
    /// Show only unrouted end with the board, so they are left out.
    pub fn kept(&self) -> Self {
        Self {
            highlighted: None,
            only_unrouted: false,
            ..self.clone()
        }
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

    /// Show every layer again; the strip lists every layer (the All set).
    pub fn show_all(&mut self) {
        self.hidden.clear();
        self.multi_layer_visible = true;
        self.layer_set = None;
    }

    /// Show every layer of `layers` (#2394), or hide every one.
    pub fn set_layers_visible(&mut self, layers: &[PcbLayer], visible: bool) {
        for &layer in layers {
            if visible {
                self.hidden.remove(&layer);
            } else {
                self.hidden.insert(layer);
            }
        }
    }

    /// Show the layers of `set` and hide every other layer of `listed`
    /// (#2394); the strip then lists only the set. The board outline stays
    /// as it is, as Show only this layer keeps it.
    pub fn show_set(&mut self, listed: &[PcbLayer], set: ShownLayerSet) {
        for &layer in listed {
            if set.layers.contains(&layer) {
                self.hidden.remove(&layer);
            } else if layer != BOARD_OUTLINE {
                self.hidden.insert(layer);
            }
        }
        self.layer_set = Some(set);
    }

    /// The set the strip lists; `None` for every layer.
    pub fn layer_set(&self) -> Option<&ShownLayerSet> {
        self.layer_set.as_ref()
    }

    /// Stop listing a set: the strip lists every layer, the hidden ones
    /// stay hidden.
    pub fn forget_layer_set(&mut self) {
        self.layer_set = None;
    }

    /// List `set` in the strip as it was kept (#2480), hiding nothing: the
    /// layers it hid are kept as hidden layers of their own.
    pub fn set_layer_set(&mut self, set: Option<ShownLayerSet>) {
        self.layer_set = set;
    }

    /// Whether the strip lists `layer`: every layer without a set.
    pub fn lists_in_strip(&self, layer: PcbLayer) -> bool {
        self.layer_set
            .as_ref()
            .is_none_or(|set| set.layers.contains(&layer))
    }

    /// The highlighted layer, if any (#2394).
    pub fn highlighted(&self) -> Option<PcbLayer> {
        self.highlighted
    }

    /// Highlight `layer`, dimming every other one; highlighting it again
    /// ends the highlight.
    pub fn toggle_highlight(&mut self, layer: PcbLayer) {
        self.highlighted = (self.highlighted != Some(layer)).then_some(layer);
    }

    /// Whether the board is seen from below (#2394).
    pub fn flipped(&self) -> bool {
        self.flipped
    }

    pub fn toggle_flipped(&mut self) {
        self.flipped = !self.flipped;
    }

    /// The view's own transparencies (#2363); its layer colours are the
    /// board's, see [`Self::paint_of`].
    pub fn paint(&self) -> &BoardPaint {
        &self.paint
    }

    /// What this view draws `board` with: its transparencies and the
    /// colours the board gives its layers (saved with the board,
    /// 2026-11-18).
    pub fn paint_of(&self, board: &PcbBoard) -> BoardPaint {
        board_paint(Some(self), board)
    }

    /// Draw `kind` this transparent, percent.
    pub fn set_object_transparency(&mut self, kind: BoardObject, percent: u8) {
        self.paint.set_transparency(kind, percent);
    }

    /// Whether objects of `kind` are drawn and hit.
    pub fn shows_object(&self, kind: BoardObject) -> bool {
        !self.hidden_objects.contains(&kind)
    }

    /// Whether some kind of object is hidden.
    pub fn hides_objects(&self) -> bool {
        !self.hidden_objects.is_empty()
    }

    /// Show or hide every object of `kind`.
    pub fn set_object_visible(&mut self, kind: BoardObject, visible: bool) {
        if visible {
            self.hidden_objects.remove(&kind);
        } else {
            self.hidden_objects.insert(kind);
        }
    }

    /// Whether the Layers panel lists only the layers the board uses.
    pub fn used_only(&self) -> bool {
        self.used_only
    }

    pub fn set_used_only(&mut self, on: bool) {
        self.used_only = on;
    }

    pub fn has_hidden(&self) -> bool {
        !self.hidden.is_empty() || !self.multi_layer_visible
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

    /// Whether the ratsnest is drawn: Show only unrouted draws it whatever
    /// the stored visibility, which it leaves alone.
    pub fn ratsnest_drawn(&self) -> bool {
        self.ratsnest_visible || self.only_unrouted
    }

    /// Whether the board is dimmed except its unrouted ratsnest lines.
    pub fn only_unrouted(&self) -> bool {
        self.only_unrouted
    }

    pub fn set_only_unrouted(&mut self, on: bool) {
        self.only_unrouted = on;
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

    /// Whether the orientation tile shows over the board canvas (mockup
    /// row 194); its eye is in View Options.
    pub fn orientation_tile_visible(&self) -> bool {
        self.orientation_tile_visible
    }

    pub fn toggle_orientation_tile(&mut self) {
        self.orientation_tile_visible = !self.orientation_tile_visible;
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
        self.item_emphasis(names, copper_span(names))
    }

    /// [`Self::emphasis`] of an item on `names` whose copper spans `span`.
    /// A highlighted layer (#2394) decides alone: an item on it (spanning
    /// it, for copper) is on top, every other item is dimmed.
    fn item_emphasis(&self, names: &[String], span: Option<CopperSpan>) -> Emphasis {
        let Some(highlighted) = self.highlighted else {
            return self.span_emphasis(span);
        };
        let on = match (highlighted.copper(), span) {
            (Some(copper), Some((top, bottom))) => (top..=bottom).contains(&copper),
            _ => names
                .iter()
                .any(|name| parse_pcb_layer(name) == Some(highlighted)),
        };
        if on {
            Emphasis::Active
        } else {
            Emphasis::Dimmed
        }
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

/// Items in draw order: by emphasis, so the active layer ends up on top.
/// Seen from below (#2394), within one emphasis copper nearer the bottom is
/// drawn later, so the bottom layer lies over the top one.
fn in_draw_order<T>(
    mut items: Vec<(Emphasis, T)>,
    flipped: bool,
    ink: impl Fn(&T) -> CopperInk,
) -> Vec<(Emphasis, T)> {
    if flipped {
        items.sort_by_key(|(emphasis, item)| (*emphasis, stack_depth(ink(item))));
    } else {
        items.sort_by_key(|(emphasis, _)| *emphasis);
    }
    items
}

/// Where an ink sits in the board-from-below stack: unknown copper first,
/// then the copper layers top to bottom, then vias and through-hole pads.
fn stack_depth(ink: CopperInk) -> (u8, Option<CopperLayer>) {
    match ink {
        CopperInk::Unknown => (0, None),
        CopperInk::Layer(layer) => (1, Some(layer)),
        CopperInk::MultiLayer => (2, None),
    }
}

impl PcbSnapshot {
    /// The snapshot of `board` as `view` shows it: hidden layers left out,
    /// copper off the active layer dimmed, the active layer drawn last.
    pub fn from_board_with_view(board: &PcbBoard, view: &PcbLayerView) -> Self {
        Self::from_board_filtered(board, Some(view))
    }

    pub(super) fn from_board_filtered(board: &PcbBoard, view: Option<&PcbLayerView>) -> Self {
        let shown = |names: &[String]| view.is_none_or(|v| v.shows_any(names));
        let object = |kind| view.is_none_or(|v| v.shows_object(kind));
        let flipped = view.is_some_and(PcbLayerView::flipped);

        let mut traces = Vec::new();
        for segment in &board.segments {
            let names = std::slice::from_ref(&segment.layer);
            if object(BoardObject::Tracks) && shown(names) {
                let (how, ink) = classify(view, names, false);
                let mut trace = trace_from_segment(segment);
                trace.dimmed = how == Emphasis::Dimmed;
                trace.ink = ink;
                traces.push((how, trace));
            }
        }
        // A copper arc is drawn as copper along its chords (#2373).
        for arc in board
            .free
            .arcs
            .iter()
            .filter(|a| pcb_geom::arc_is_copper(a))
        {
            let names = std::slice::from_ref(&arc.layer);
            if object(BoardObject::Arcs) && shown(names) {
                let (how, ink) = classify(view, names, false);
                for mut trace in arc_traces(arc) {
                    trace.dimmed = how == Emphasis::Dimmed;
                    trace.ink = ink;
                    traces.push((how, trace));
                }
            }
        }
        let mut vias = Vec::new();
        for via in &board.vias {
            if view.is_none_or(|v| v.shows_object(BoardObject::Vias) && v.shows_via(&via.layers)) {
                let (how, ink) = classify(view, &via.layers, true);
                let reached = active_in_span(view, &via.layers);
                let mut input = via_from_board_via(via, reached);
                input.dimmed = how == Emphasis::Dimmed;
                input.ink = ink;
                input.shown_on = reached;
                vias.push((how, input));
            }
        }
        let mut pads: Vec<_> = board
            .footprints
            .iter()
            .filter(|_| object(BoardObject::Pads))
            .flat_map(|footprint| footprint_pads(footprint, view))
            .collect();
        // Free pads are drawn as pads of the board's own frame (#2373).
        if object(BoardObject::Pads) && !board.free.pads.is_empty() {
            let frame = Footprint {
                pads: board.free.pads.clone(),
                ..signex_types::pcb_free::free_pad_frame()
            };
            pads.extend(footprint_pads(&frame, view));
        }
        let mut drawings = Drawings {
            texts: board
                .footprints
                .iter()
                .filter(|_| object(BoardObject::Texts))
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
            let kind = zone_object(zone);
            if !object(kind) || !shown(names) {
                continue;
            }
            let Some(mut input) = zone_from_board_zone(zone, source_order, &layer_ranks) else {
                continue;
            };
            input.object = kind;
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

        let mut board_features = super::BoardFeaturesInput {
            edge_hidden: view.is_some_and(|v| !v.is_visible(BOARD_OUTLINE)),
            ..super::BoardFeaturesInput::from_board(board)
        };
        if !object(BoardObject::Keepouts) {
            board_features.keepouts.clear();
        }

        Self {
            traces: strip(in_draw_order(traces, flipped, |t| t.ink)),
            vias: strip(in_draw_order(vias, flipped, |v| v.ink)),
            pads: strip(in_draw_order(pads, flipped, |p| p.ink)),
            zones: strip(in_draw_order(zones, flipped, |z| z.ink)),
            rule_areas,
            ratsnest_lines: Vec::new(),
            drc_markers: Vec::new(),
            board_features,
            texts: drawings.texts,
            graphics: drawings.graphics,
            paint: board_paint(view, board)
                .with_board_dimmed(view.is_some_and(PcbLayerView::only_unrouted)),
        }
    }
}

/// The View Options row a zone belongs to (#2363): a keepout, a fill, a
/// region or a pour, as the selection filter sorts them.
pub fn zone_object(zone: &signex_types::pcb::Zone) -> BoardObject {
    match pcb_geom::PickKind::of_zone(zone) {
        Some(pcb_geom::PickKind::Keepouts) => BoardObject::Keepouts,
        Some(pcb_geom::PickKind::Fills) => BoardObject::Fills,
        Some(pcb_geom::PickKind::Regions) => BoardObject::Regions,
        _ => BoardObject::Polygons,
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
        let names = pad_layer_names(footprint, pad);
        let (how, ink) = classify(view, names, is_through_pad(pad));
        let shown_on = active_on_through_pad(view, pad);
        // Another pad shows a per-layer size only on a layer it reaches.
        let size_layer = shown_on.or_else(|| active_in_span(view, names));
        if let Some(mut input) = pad_from_footprint(footprint, pad, size_layer) {
            input.dimmed = how == Emphasis::Dimmed;
            input.ink = ink;
            input.shown_on = shown_on;
            pads.push((how, input));
        }
    }
    pads
}

/// The paint `board` is drawn with under `view`: the view's transparencies
/// (none without a view) and the colours the board gives its layers.
pub fn board_paint(view: Option<&PcbLayerView>, board: &PcbBoard) -> BoardPaint {
    let mut paint = view.map(|v| v.paint.clone()).unwrap_or_default();
    for c in &board.options.layer_colors {
        paint.set_colour(c.layer, Some([c.color.r, c.color.g, c.color.b]));
    }
    paint
}

fn strip<T>(items: Vec<(Emphasis, T)>) -> Vec<T> {
    items.into_iter().map(|(_, item)| item).collect()
}

#[cfg(test)]
#[path = "layer_view_tests.rs"]
mod tests;
