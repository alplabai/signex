//! The copper of a board with the copper layers each item reaches: the
//! netted list the ratsnest and the design-rule check both read
//! ([`collect_copper`]), and the copper of no net ([`no_net_copper`], #2515)
//! the check and the router measure against every net but never connect.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard, Via, ViaType};
use signex_types::pcb_free::free_pad_frame;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use crate::drawn_copper::drawn_copper;
use crate::free::arc_shapes;
use crate::index::{ItemRef, track_shape, via_shape};
use crate::shape::Shape;
use crate::transform::{Side, pad_shape, pad_world_center};
use crate::units::{GeomError, PointNm, mm_to_nm};

/// A copper item with its net (0: no net) and where it touches other
/// copper.
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

/// Which copper a collection takes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Nets {
    /// Copper of a numbered net.
    Numbered,
    /// Copper of no net.
    NoNet,
}

impl Nets {
    fn takes(self, net: u32) -> bool {
        (net != 0) == (self == Nets::Numbered)
    }
}

/// The copper of a numbered net: what the ratsnest connects.
pub fn collect_copper(board: &PcbBoard) -> Vec<CopperItem> {
    board_copper(board, Nets::Numbered)
}

/// The copper of no net (#2515): pads, free pads, tracks, copper arcs and
/// vias of net 0, then every drawing and string on copper
/// ([`drawn_copper`]). Copper of no net never makes a connection, so the
/// ratsnest does not read this list; the design rule check measures it
/// against every net's copper and the board edge, and the router keeps
/// clear of it. An unplated pad of no net is copper only on the copper
/// layers it lists, as the Gerber flashes it.
pub fn no_net_copper(board: &PcbBoard) -> Vec<CopperItem> {
    let mut items = board_copper(board, Nets::NoNet);
    items.extend(drawn_copper(board));
    items
}

fn pad_net(pad: &Pad) -> u32 {
    pad.net.as_ref().map_or(0, |n| n.number)
}

/// The copper layers a pad of the `nets` collection reaches: for no net,
/// an unplated pad only on the copper layers it lists.
fn pad_reach(footprint: &Footprint, pad: &Pad, nets: Nets) -> u32 {
    match (nets, pad.pad_type) {
        (Nets::NoNet, PadType::NpThru) => layer_mask(pad.layers.iter().map(String::as_str)),
        _ => pad_layers(footprint, pad),
    }
}

fn board_copper(board: &PcbBoard, nets: Nets) -> Vec<CopperItem> {
    let mut nodes = Vec::new();
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            let net = pad_net(pad);
            let layers = pad_reach(footprint, pad, nets);
            if !nets.takes(net) || layers == 0 {
                continue;
            }
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
                layers,
                shape,
                pad_center: Some(center),
            });
        }
    }
    for segment in &board.segments {
        let layers = layer_mask(std::iter::once(segment.layer.as_str()));
        if !nets.takes(segment.net) || layers == 0 {
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
    free_copper(board, nets, &mut nodes);
    for via in &board.vias {
        if !nets.takes(via.net) {
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

/// The free pads and copper arcs (#2373) of the `nets` collection. A free
/// pad is a pad in the board's frame, ending ratsnest lines like a
/// footprint pad; an arc is one item per chord, all with its identity.
fn free_copper(board: &PcbBoard, nets: Nets, nodes: &mut Vec<CopperItem>) {
    let frame = free_pad_frame();
    for pad in &board.free.pads {
        let net = pad_net(pad);
        let layers = pad_reach(&frame, pad, nets);
        if !nets.takes(net) || layers == 0 {
            continue;
        }
        let (Ok(center), Ok(shape)) = (pad_world_center(&frame, pad), pad_shape(&frame, pad))
        else {
            continue;
        };
        nodes.push(CopperItem {
            item: ItemRef::FreePad(pad.uuid),
            net,
            layers,
            shape,
            pad_center: Some(center),
        });
    }
    for arc in &board.free.arcs {
        let layers = layer_mask(std::iter::once(arc.layer.as_str()));
        if !nets.takes(arc.net) || layers == 0 {
            continue;
        }
        for shape in arc_shapes(arc).unwrap_or_default() {
            nodes.push(CopperItem {
                item: ItemRef::Arc(arc.uuid),
                net: arc.net,
                layers,
                shape,
                pad_center: None,
            });
        }
    }
}

/// A through via reaches every copper layer; a blind or micro via the span
/// between the outermost layers it names.
pub fn via_layers(via: &Via) -> u32 {
    match via.via_type {
        ViaType::Through => u32::MAX,
        ViaType::Blind | ViaType::Micro => span_mask(via.layers.iter().map(String::as_str)),
    }
}

/// A via's copper per group of the layers it reaches that share one
/// diameter (#2401): one piece, its [`via_shape`], for a via without
/// per-layer sizes. The layers are a [`layer_bit`] mask.
pub fn via_pieces(via: &Via) -> Result<Vec<(u32, Shape)>, GeomError> {
    let layers = via_layers(via);
    if via.layer_sizes.is_none() {
        return Ok(vec![(layers, via_shape(via)?)]);
    }
    let center = PointNm::from_mm(via.position, "via.position")?;
    let mut pieces: Vec<(u32, Shape)> = Vec::new();
    for layer in copper_layers(layers) {
        let radius = (mm_to_nm(via.diameter_on(layer), "via.diameter")? / 2).abs();
        let bit = 1 << layer_bit(layer);
        match pieces
            .iter_mut()
            .find(|(_, shape)| matches!(shape, Shape::Disc { radius: r, .. } if *r == radius))
        {
            Some((mask, _)) => *mask |= bit,
            None => pieces.push((bit, Shape::Disc { center, radius })),
        }
    }
    Ok(pieces)
}

/// A pad's copper per group of the layers it reaches that share one size
/// (#2401): one piece, its [`pad_shape`], for a pad without per-layer
/// sizes. The layers are a [`layer_bit`] mask.
pub fn pad_pieces(footprint: &Footprint, pad: &Pad) -> Result<Vec<(u32, Shape)>, GeomError> {
    let layers = pad_layers(footprint, pad);
    if pad.layer_sizes.is_none() {
        return Ok(vec![(layers, pad_shape(footprint, pad)?)]);
    }
    let mut pieces: Vec<(u32, Shape)> = Vec::new();
    for layer in copper_layers(layers) {
        let sized = Pad {
            size: pad.size_on(layer),
            ..pad.clone()
        };
        let shape = pad_shape(footprint, &sized)?;
        let bit = 1 << layer_bit(layer);
        match pieces.iter_mut().find(|(_, s)| *s == shape) {
            Some((mask, _)) => *mask |= bit,
            None => pieces.push((bit, shape)),
        }
    }
    Ok(pieces)
}

/// The copper of `pieces` (from [`pad_pieces`] or [`via_pieces`]) on any
/// layer of the [`layer_bit`] mask `layers`.
pub fn pieces_on(pieces: Vec<(u32, Shape)>, layers: u32) -> impl Iterator<Item = Shape> {
    pieces
        .into_iter()
        .filter(move |(mask, _)| mask & layers != 0)
        .map(|(_, shape)| shape)
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
            locked: false,
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
            stackup: None,
            rooms: Vec::new(),
            free: Default::default(),
            options: Default::default(),
            unknown: Default::default(),
        }
    }

    fn at(x: f64, y: f64) -> PointNm {
        PointNm::from_mm(Point::new(x, y), "at").unwrap()
    }

    /// #2515: the copper of no net is its own list; the netted list, which
    /// the ratsnest and `net_at` read, holds none of it.
    #[test]
    fn copper_of_no_net_is_listed_apart_from_the_netted_copper() {
        use crate::route::test_board::{board as route_board, pad_at, track_at};

        let mut unnetted = pad_at(1, 0.0, 5.0, 1.0, 0);
        unnetted.pads[0].net = None;
        let mut hole = pad_at(2, 5.0, 5.0, 1.0, 0);
        hole.pads[0].pad_type = PadType::NpThru;
        hole.pads[0].layers.clear();
        let mut board = route_board(
            vec![unnetted, hole, pad_at(3, 10.0, 5.0, 1.0, 4)],
            vec![
                track_at(10, (0.0, 0.0), (10.0, 0.0), 0),
                track_at(11, (0.0, 2.0), (10.0, 2.0), 4),
            ],
        );
        board.texts.push(signex_types::pcb::BoardText {
            uuid: Uuid::from_u128(20),
            text: "A".into(),
            position: Point::new(5.0, 8.0),
            layer: "TopCopper".into(),
            font_size: 1.0,
            rotation: 0.0,
        });
        let ids = |items: Vec<CopperItem>| {
            let mut ids: Vec<ItemRef> = items.into_iter().map(|c| c.item).collect();
            ids.dedup();
            ids
        };
        let pad = |n: u128| ItemRef::Pad {
            footprint: Uuid::from_u128(4000 + n),
            pad: Uuid::from_u128(5000 + n),
        };
        assert_eq!(
            ids(no_net_copper(&board)),
            [
                pad(1),
                ItemRef::Track(Uuid::from_u128(10)),
                ItemRef::Text(Uuid::from_u128(20)),
            ],
            "an unplated pad listing no copper layer is no copper"
        );
        assert_eq!(
            ids(collect_copper(&board)),
            [pad(3), ItemRef::Track(Uuid::from_u128(11))]
        );
        let top = 1 << layer_bit(CopperLayer::Top);
        assert_eq!(net_at(&board, at(5.0, 0.0), top), None, "no-net track");
        assert_eq!(net_at(&board, at(5.0, 2.0), top), Some(4));
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

    #[test]
    fn a_via_with_a_smaller_inner_size_has_one_piece_per_size() {
        let mut via = signex_types::pcb::Via {
            uuid: Uuid::from_u128(4),
            position: Point::new(0.0, 0.0),
            diameter: 0.6,
            drill: 0.3,
            layers: vec!["TopCopper".into(), "BottomCopper".into()],
            net: 1,
            via_type: ViaType::Through,
            locked: false,
            layer_sizes: None,
            unknown_columns: Default::default(),
            attributes: Default::default(),
        };
        assert_eq!(via_pieces(&via).unwrap().len(), 1);
        via.layer_sizes = Some(signex_types::pcb::LayerSizes::TopMiddleBottom {
            top: 0.6,
            middle: 0.4,
            bottom: 0.6,
        });
        let pieces = via_pieces(&via).unwrap();
        let outer = (1 << layer_bit(CopperLayer::Top)) | (1 << layer_bit(CopperLayer::Bottom));
        let radius = |mask: u32| {
            pieces
                .iter()
                .find(|(m, _)| *m == mask)
                .map(|(_, s)| match s {
                    Shape::Disc { radius, .. } => *radius,
                    _ => 0,
                })
        };
        assert_eq!(pieces.len(), 2);
        assert_eq!(radius(outer), Some(300_000));
        assert_eq!(radius(!outer), Some(200_000), "every inner layer");
    }
}
