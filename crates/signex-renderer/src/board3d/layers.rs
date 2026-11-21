//! The areas of the board's real layers, from its data: the copper of each
//! layer, the mask with its openings, the finish inside them and the
//! silkscreen. All in the 3D frame. Rules: `docs/RENDERING_RULES.md`,
//! "board-3d".

use std::collections::BTreeMap;

use pcb_geom::{
    Polygon, Side, difference, intersection, layer_bit, layer_mask, pad_layers, pad_shape, union,
    via_layers, zone_area,
};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard};
use signex_types::pcb_free::free_pad_frame;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::copper::{ROUND_SEGMENTS, capsule, circle};
use super::drawings::{graphic_polygons, text_polygons};
use super::region::{in_view_frame, polygon};
use super::stack::StackHeights;
use super::{Face, board_xy};
use crate::pcb::{BoardObject, CopperInk, PcbLayerView, PcbSnapshot};

/// Points per quarter turn of a round pad part, as the flat view draws it.
const PAD_RING_STEPS: usize = 5;
/// How far mask over copper is lifted toward white, so a track under the
/// mask reads as a lighter, raised line of the mask's own colour.
const MASK_OVER_COPPER_LIFT: f32 = 0.16;

/// The colour of `mask` where copper raises it.
pub(super) fn mask_over_copper(mask: [f32; 4]) -> [f32; 4] {
    let lift = |c: f32| c + (1.0 - c) * MASK_OVER_COPPER_LIFT;
    [lift(mask[0]), lift(mask[1]), lift(mask[2]), mask[3]]
}

/// One face's layers.
#[derive(Clone, Debug, Default)]
pub(super) struct FaceArt {
    /// Copper under the mask.
    pub(super) covered_copper: Vec<Polygon>,
    /// Copper in a mask opening, which takes the surface finish.
    pub(super) finish: Vec<Polygon>,
    /// Mask over bare laminate.
    pub(super) mask: Vec<Polygon>,
    /// Mask raised by the copper or a tented via under it.
    pub(super) mask_over_copper: Vec<Polygon>,
    pub(super) silk: Vec<Polygon>,
}

/// Every layer's areas.
#[derive(Clone, Debug, Default)]
pub(super) struct BoardArt {
    pub(super) top: FaceArt,
    pub(super) bottom: FaceArt,
    /// The inner copper, in the order of [`StackHeights::inner`].
    pub(super) inner: Vec<Vec<Polygon>>,
}

impl BoardArt {
    pub(super) fn face(&self, face: Face) -> &FaceArt {
        match face {
            Face::Top => &self.top,
            Face::Bottom => &self.bottom,
        }
    }
}

/// What the areas are read from.
pub(super) struct ArtInput<'a> {
    pub(super) board: &'a PcbBoard,
    /// Every item, hidden or not: holes and mask openings are the board's.
    pub(super) every: &'a PcbSnapshot,
    /// What the layer eyes show, with the pours' fills.
    pub(super) shown: &'a PcbSnapshot,
    pub(super) view: &'a PcbLayerView,
    /// The board's outline in the 3D frame.
    pub(super) outline: &'a [[f32; 2]],
    pub(super) heights: &'a StackHeights,
}

/// The areas of every layer; empty for a board with no outline to frame.
pub(super) fn board_art(input: &ArtInput<'_>) -> BoardArt {
    let Some(area) = polygon(input.outline, &[]) else {
        return BoardArt::default();
    };
    let area = [area];
    let fixed = fixed_holes(input.every);
    let drills = via_drills(input);
    let every_hole: Vec<Polygon> = fixed
        .iter()
        .cloned()
        .chain(drills.iter().filter_map(|via| via.drill.clone()))
        .collect();
    let mut copper = copper_by_layer(input);
    let mut finished = |layer: CopperLayer| -> Vec<Polygon> {
        let shapes = copper.remove(&layer).unwrap_or_default();
        if shapes.is_empty() {
            return Vec::new();
        }
        difference(&intersection(&union(&shapes, &[]), &area), &every_hole)
    };
    let inner = input
        .heights
        .inner
        .iter()
        .map(|(layer, _)| finished(*layer))
        .collect();
    let top_copper = finished(CopperLayer::Top);
    let bottom_copper = finished(CopperLayer::Bottom);
    let face = |face, copper| face_art(input, face, &area, copper, &fixed, &drills);
    BoardArt {
        top: face(Face::Top, top_copper),
        bottom: face(Face::Bottom, bottom_copper),
        inner,
    }
}

/// `face`'s copper split by the mask, the mask itself and the silkscreen,
/// which stays off every opening and hole.
fn face_art(
    input: &ArtInput<'_>,
    face: Face,
    area: &[Polygon],
    copper: Vec<Polygon>,
    fixed: &[Polygon],
    drills: &[ViaDrill],
) -> FaceArt {
    let openings = union(&mask_openings(input, face), &[]);
    let mut through = fixed.to_vec();
    let mut tents = Vec::new();
    for via in drills {
        if via.tented(face) {
            tents.extend(via.disc.clone());
        } else {
            through.extend(via.drill.clone());
        }
    }
    let cleared = union(&openings, &through);
    let coat = difference(area, &cleared);
    let raised = union(&copper, &tents);
    let silk = difference(
        &intersection(&union(&silk_shapes(input, face), &[]), area),
        &cleared,
    );
    FaceArt {
        finish: intersection(&copper, &openings),
        covered_copper: difference(&copper, &openings),
        mask_over_copper: intersection(&coat, &raised),
        mask: difference(&coat, &raised),
        silk,
    }
}

/// A via's drill and copper disc, and whether each side is tented.
struct ViaDrill {
    drill: Option<Polygon>,
    disc: Option<Polygon>,
    tented: [bool; 2],
}

impl ViaDrill {
    fn tented(&self, face: Face) -> bool {
        match face {
            Face::Top => self.tented[0],
            Face::Bottom => self.tented[1],
        }
    }
}

fn disc(center: [f32; 2], radius: f32) -> Option<Polygon> {
    if !(radius.is_finite() && radius > 0.0) {
        return None;
    }
    polygon(&circle(board_xy(center), radius, ROUND_SEGMENTS), &[])
}

/// Each via of the board. `every.vias` holds them in board order, with the
/// sizes the slab cuts.
fn via_drills(input: &ArtInput<'_>) -> Vec<ViaDrill> {
    input
        .board
        .vias
        .iter()
        .zip(&input.every.vias)
        .map(|(via, drawn)| ViaDrill {
            drill: disc(drawn.center, drawn.drill_mm * 0.5),
            disc: disc(drawn.center, drawn.diameter_mm * 0.5),
            tented: [via.attributes.top.tented, via.attributes.bottom.tented],
        })
        .collect()
}

/// The cutouts, mounting holes and pad drills.
fn fixed_holes(every: &PcbSnapshot) -> Vec<Polygon> {
    let features = &every.board_features;
    let cutouts = features.cutouts.iter().filter_map(|cutout| {
        let ring: Vec<[f32; 2]> = cutout.iter().copied().map(board_xy).collect();
        polygon(&ring, &[])
    });
    let mounting = features
        .holes
        .iter()
        .filter_map(|hole| disc(hole.center, hole.drill_mm * 0.5));
    let pads = every
        .pads
        .iter()
        .filter_map(|pad| disc(pad.center, pad.drill_mm * 0.5));
    cutouts.chain(mounting).chain(pads).collect()
}

/// `f` on every pad of the board, the free pads in their own frame.
fn each_pad(board: &PcbBoard, mut f: impl FnMut(&Footprint, &Pad)) {
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            f(footprint, pad);
        }
    }
    let frame = free_pad_frame();
    for pad in &board.free.pads {
        f(&frame, pad);
    }
}

fn pad_polygon(footprint: &Footprint, pad: &Pad) -> Option<Polygon> {
    let ring = pad_shape(footprint, pad).ok()?.ring(PAD_RING_STEPS);
    (ring.len() >= 3).then(|| in_view_frame(&Polygon::new(ring, Vec::new())))
}

/// The copper layers a pad reaches: a non-plated pad only those it lists.
fn pad_copper(footprint: &Footprint, pad: &Pad) -> u32 {
    match pad.pad_type {
        PadType::NpThru => layer_mask(pad.layers.iter().map(String::as_str)),
        _ => pad_layers(footprint, pad),
    }
}

/// The copper of each layer of the stack, as the layer eyes show it.
fn copper_by_layer(input: &ArtInput<'_>) -> BTreeMap<CopperLayer, Vec<Polygon>> {
    let mut out: BTreeMap<CopperLayer, Vec<Polygon>> = input
        .heights
        .copper_layers()
        .into_iter()
        .map(|layer| (layer, Vec::new()))
        .collect();
    for zone in input.shown.zones.iter().filter(|zone| !zone.shelved) {
        let CopperInk::Layer(layer) = zone.ink else {
            continue;
        };
        let Some(slot) = out.get_mut(&layer) else {
            continue;
        };
        match &zone.fill {
            Some(pieces) => slot.extend(
                pieces
                    .iter()
                    .filter_map(|piece| view_polygon(&piece.vertices, &piece.holes)),
            ),
            None => slot.extend(view_polygon(&zone.vertices, &zone.holes)),
        }
    }
    for trace in &input.shown.traces {
        if let CopperInk::Layer(layer) = trace.ink
            && let Some(slot) = out.get_mut(&layer)
        {
            let ring = capsule(board_xy(trace.p0), board_xy(trace.p1), trace.width_mm * 0.5);
            slot.extend(polygon(&ring, &[]));
        }
    }
    let view = input.view;
    if view.shows_object(BoardObject::Pads) {
        each_pad(input.board, |footprint, pad| {
            if view.shows_pad(footprint, pad)
                && let Some(shape) = pad_polygon(footprint, pad)
            {
                put_on_layers(&mut out, pad_copper(footprint, pad), &shape);
            }
        });
    }
    if view.shows_object(BoardObject::Vias) {
        for (via, drawn) in input.board.vias.iter().zip(&input.every.vias) {
            if view.shows_via(&via.layers)
                && let Some(shape) = disc(drawn.center, drawn.diameter_mm * 0.5)
            {
                put_on_layers(&mut out, via_layers(via), &shape);
            }
        }
    }
    out
}

fn put_on_layers(out: &mut BTreeMap<CopperLayer, Vec<Polygon>>, bits: u32, shape: &Polygon) {
    for (layer, slot) in out.iter_mut() {
        if bits & (1u32 << layer_bit(*layer)) != 0 {
            slot.push(shape.clone());
        }
    }
}

/// Rings in board mm (y down) as a polygon in the 3D frame.
fn view_polygon(outer: &[[f32; 2]], holes: &[Vec<[f32; 2]>]) -> Option<Polygon> {
    let outer: Vec<[f32; 2]> = outer.iter().copied().map(board_xy).collect();
    let holes: Vec<Vec<[f32; 2]>> = holes
        .iter()
        .map(|hole| hole.iter().copied().map(board_xy).collect())
        .collect();
    polygon(&outer, &holes)
}

fn mask_layer(face: Face) -> PcbLayer {
    PcbLayer::Technical(match face {
        Face::Top => TechnicalLayer::SolderMaskTop,
        Face::Bottom => TechnicalLayer::SolderMaskBottom,
    })
}

fn silk_layer(face: Face) -> PcbLayer {
    PcbLayer::Technical(match face {
        Face::Top => TechnicalLayer::SilkscreenTop,
        Face::Bottom => TechnicalLayer::SilkscreenBottom,
    })
}

/// Whether `pad` opens the mask on `face`: the mask layers it lists, or,
/// when it lists no mask or paste layer, its type's default. The rule the
/// fabrication images use (`signex_output::fab::collect::pad_openings`).
fn pad_opens(footprint: &Footprint, pad: &Pad, face: Face) -> bool {
    use TechnicalLayer as T;
    let listed: Vec<TechnicalLayer> = pad
        .layers
        .iter()
        .filter_map(|token| match parse_pcb_layer(token) {
            Some(PcbLayer::Technical(
                t @ (T::SolderMaskTop | T::SolderMaskBottom | T::PasteTop | T::PasteBottom),
            )) => Some(t),
            _ => None,
        })
        .collect();
    if !listed.is_empty() {
        return listed.into_iter().any(|t| PcbLayer::Technical(t) == mask_layer(face));
    }
    match pad.pad_type {
        PadType::Thru | PadType::NpThru => true,
        PadType::Smd | PadType::Connect => matches!(
            (Side::of_layer(&footprint.layer), face),
            (Side::Top, Face::Top) | (Side::Bottom, Face::Bottom)
        ),
    }
}

/// The mask openings of `face`: pads, vias not tented there, tracks with a
/// manual mask expansion and shapes drawn on the mask layer.
fn mask_openings(input: &ArtInput<'_>, face: Face) -> Vec<Polygon> {
    let board = input.board;
    let layer = mask_layer(face);
    let mut out = Vec::new();
    each_pad(board, |footprint, pad| {
        if pad_opens(footprint, pad, face) {
            out.extend(pad_polygon(footprint, pad));
        }
    });
    for (via, drawn) in board.vias.iter().zip(&input.every.vias) {
        let side = match face {
            Face::Top => via.attributes.top,
            Face::Bottom => via.attributes.bottom,
        };
        if side.tented {
            continue;
        }
        let grow = side.expansion.manual_mm().unwrap_or(0.0) as f32;
        let base = if via.attributes.from_hole_edge {
            drawn.drill_mm
        } else {
            drawn.diameter_mm
        };
        out.extend(disc(drawn.center, base * 0.5 + grow));
    }
    let copper = PcbLayer::Copper(match face {
        Face::Top => CopperLayer::Top,
        Face::Bottom => CopperLayer::Bottom,
    });
    for segment in &board.segments {
        let Some(grow) = segment.attributes.solder_mask.manual_mm() else {
            continue;
        };
        let radius = (segment.width * 0.5 + grow) as f32;
        if parse_pcb_layer(&segment.layer) == Some(copper) && radius > 0.0 {
            let a = board_xy([segment.start.x as f32, segment.start.y as f32]);
            let b = board_xy([segment.end.x as f32, segment.end.y as f32]);
            out.extend(polygon(&capsule(a, b, radius), &[]));
        }
    }
    for graphic in input.every.graphics.iter().filter(|g| g.layer == layer) {
        out.extend(graphic_polygons(graphic));
    }
    out.extend(zones_on(board, layer));
    out
}

/// The silkscreen of `face` the layer eyes show.
fn silk_shapes(input: &ArtInput<'_>, face: Face) -> Vec<Polygon> {
    let layer = silk_layer(face);
    let mut out = Vec::new();
    for graphic in input.shown.graphics.iter().filter(|g| g.layer == layer) {
        out.extend(graphic_polygons(graphic));
    }
    for text in input.shown.texts.iter().filter(|t| t.layer == layer) {
        out.extend(text_polygons(text));
    }
    if input.view.is_visible(layer) {
        out.extend(zones_on(input.board, layer));
    }
    out
}

/// The areas of the zones drawn on the technical layer `layer`.
fn zones_on(board: &PcbBoard, layer: PcbLayer) -> impl Iterator<Item = Polygon> + '_ {
    board
        .zones
        .iter()
        .filter(move |zone| !zone.is_rule_area() && parse_pcb_layer(&zone.layer) == Some(layer))
        .filter_map(zone_area)
        .map(|area| in_view_frame(&area))
}
