//! The netted copper of a board with the copper layers each item reaches:
//! the one list the ratsnest and the design-rule check both read.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard, Via, ViaType};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use crate::index::{ItemRef, track_shape, via_shape};
use crate::shape::Shape;
use crate::transform::{Side, pad_shape, pad_world_center};
use crate::units::PointNm;

/// A copper item of a numbered net, with where it touches other copper.
pub struct CopperItem {
    pub item: ItemRef,
    pub net: u32,
    /// Bit `n` set: the copper layer with [`layer_bit`] `n`.
    pub layers: u32,
    /// The item's copper; it joins any same-net copper it overlaps.
    pub shape: Shape,
    /// Set for a pad: its centre, where a ratsnest line ends.
    pub pad_center: Option<PointNm>,
}

pub fn collect_copper(board: &PcbBoard) -> Vec<CopperItem> {
    let mut nodes = Vec::new();
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            let Some(net) = pad.net.as_ref().map(|n| n.number).filter(|&n| n != 0) else {
                continue;
            };
            let (Ok(center), Ok(shape)) =
                (pad_world_center(footprint, pad), pad_shape(footprint, pad))
            else {
                continue;
            };
            nodes.push(CopperItem {
                item: ItemRef::Pad {
                    footprint: footprint.uuid,
                    pad: pad.uuid,
                },
                net,
                layers: pad_layers(footprint, pad),
                shape,
                pad_center: Some(center),
            });
        }
    }
    for segment in &board.segments {
        let layers = layer_mask(std::iter::once(segment.layer.as_str()));
        if segment.net == 0 || layers == 0 {
            continue;
        }
        if let Ok(shape) = track_shape(segment) {
            nodes.push(CopperItem {
                item: ItemRef::Track(segment.uuid),
                net: segment.net,
                layers,
                shape,
                pad_center: None,
            });
        }
    }
    for via in &board.vias {
        if via.net == 0 {
            continue;
        }
        let layers = via_layers(via);
        if let (Ok(shape), true) = (via_shape(via), layers != 0) {
            nodes.push(CopperItem {
                item: ItemRef::Via(via.uuid),
                net: via.net,
                layers,
                shape,
                pad_center: None,
            });
        }
    }
    nodes
}

/// A through via reaches every copper layer; a blind or micro via the span
/// between the outermost layers it names.
pub fn via_layers(via: &Via) -> u32 {
    match via.via_type {
        ViaType::Through => u32::MAX,
        ViaType::Blind | ViaType::Micro => span_mask(via.layers.iter().map(String::as_str)),
    }
}

/// A through-hole pad reaches every copper layer; a surface pad the copper
/// layers it lists, or its footprint's side when it lists none.
pub fn pad_layers(footprint: &Footprint, pad: &Pad) -> u32 {
    if pad.pad_type == PadType::Thru {
        return u32::MAX;
    }
    match layer_mask(pad.layers.iter().map(String::as_str)) {
        0 => match Side::of_layer(&footprint.layer) {
            Side::Top => 1 << layer_bit(CopperLayer::Top),
            Side::Bottom => 1 << layer_bit(CopperLayer::Bottom),
        },
        mask => mask,
    }
}

/// Bit position of a copper layer: top 0, inner `n` at `n`, bottom 31.
pub fn layer_bit(layer: CopperLayer) -> u32 {
    match layer {
        CopperLayer::Top => 0,
        CopperLayer::Inner(inner) => u32::from(inner.index()),
        CopperLayer::Bottom => 31,
    }
}

fn copper_bits<'a>(names: impl Iterator<Item = &'a str>) -> impl Iterator<Item = u32> {
    names
        .filter_map(parse_pcb_layer)
        .filter_map(PcbLayer::copper)
        .map(layer_bit)
}

pub fn layer_mask<'a>(names: impl Iterator<Item = &'a str>) -> u32 {
    copper_bits(names).fold(0, |mask, bit| mask | 1 << bit)
}

/// Every copper layer from the topmost to the bottommost one named.
pub(crate) fn span_mask<'a>(names: impl Iterator<Item = &'a str>) -> u32 {
    let bits: Vec<u32> = copper_bits(names).collect();
    match (bits.iter().min(), bits.iter().max()) {
        (Some(&lo), Some(&hi)) => (lo..=hi).fold(0, |mask, bit| mask | 1 << bit),
        _ => 0,
    }
}

/// The copper layers of a bit mask such as `Violation::layers`, top to
/// bottom.
pub fn copper_layers(mask: u32) -> Vec<CopperLayer> {
    CopperLayer::all()
        .filter(|&layer| mask & (1 << layer_bit(layer)) != 0)
        .collect()
}

/// The net of the netted copper under `at` on any copper layer in `layers`
/// (a [`layer_bit`] mask): the net a new track or via there joins. `None`
/// over bare board or copper of no net. Where copper of two nets overlaps,
/// the first one listed by [`collect_copper`] wins: pads, then tracks, then
/// vias.
pub fn net_at(board: &PcbBoard, at: PointNm, layers: u32) -> Option<u32> {
    collect_copper(board)
        .into_iter()
        .find(|item| item.layers & layers != 0 && item.shape.contains(at, 0))
        .map(|item| item.net)
}

#[cfg(test)]
mod tests {
    use signex_types::pcb::{Point, Segment};
    use uuid::Uuid;

    use super::*;

    fn board() -> PcbBoard {
        let track = Segment {
            attributes: Default::default(),
            uuid: Uuid::from_u128(2),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            width: 0.5,
            layer: "TopCopper".into(),
            net: 3,
            unknown_columns: Default::default(),
        };
        PcbBoard {
            sketch: None,
            uuid: Uuid::nil(),
            version: 1,
            generator: String::new(),
            thickness: 1.6,
            outline: Vec::new(),
            layers: Vec::new(),
            setup: None,
            nets: Vec::new(),
            footprints: Vec::new(),
            segments: vec![track],
            vias: Vec::new(),
            zones: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            options: Default::default(),
            unknown: Default::default(),
        }
    }

    fn at(x: f64, y: f64) -> PointNm {
        PointNm::from_mm(Point::new(x, y), "at").unwrap()
    }

    #[test]
    fn net_at_finds_the_copper_under_the_point_on_the_layers_asked() {
        let board = board();
        let top = 1 << layer_bit(CopperLayer::Top);
        let bottom = 1 << layer_bit(CopperLayer::Bottom);
        assert_eq!(net_at(&board, at(5.0, 0.2), top), Some(3));
        assert_eq!(net_at(&board, at(5.0, 0.2), bottom), None, "other layer");
        assert_eq!(net_at(&board, at(5.0, 1.0), top), None, "bare board");
    }
}
