//! Silkscreen checks (#2435): silkscreen over or too near an exposed pad,
//! and the silkscreen of two owners (two parts, or a part and a board
//! drawing or string) too near each other. A part's silkscreen is measured
//! by the strokes of its drawings; a board string by the box its glyphs
//! fill. A part's own texts are not measured yet. Board-wide rules only.

use std::collections::HashMap;

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_free::free_pad_frame;
use signex_types::pcb_layer::{BoardSide, CopperLayer, PcbLayer};

use pcb_geom::{
    ItemRef, PickKind, Shape, board_graphic_placement, footprint_graphic_placement,
    graphic_strokes, layer_bit, pad_layers, pad_shape, text_shape,
};

use crate::near::{gap_under, near_pairs};
use crate::user_rules::LimitKind;
use crate::{DrcRules, Violation};

/// One piece of silkscreen or one exposed pad, on one side.
pub(crate) struct Piece {
    pub(crate) owner: ItemRef,
    pub(crate) net: u32,
    pub(crate) side: BoardSide,
    pub(crate) shape: Shape,
}

pub(crate) fn violations(board: &PcbBoard, rules: &DrcRules) -> Vec<Violation> {
    let to_pad = rules.board_limit(LimitKind::MinSilkToPad);
    let to_silk = rules.board_limit(LimitKind::MinSilkToSilk);
    if to_pad.is_none() && to_silk.is_none() {
        return Vec::new();
    }
    let silk = silk_pieces(board);
    let mut found = Vec::new();
    if let Some(limit) = to_pad {
        let pads = exposed_pads(board);
        found.extend(between(&silk, &pads, LimitKind::MinSilkToPad, &limit));
    }
    if let Some(limit) = to_silk {
        found.extend(between(&silk, &silk, LimitKind::MinSilkToSilk, &limit));
    }
    found
}

/// The closest approach of each pair of owners, `a` against `b` (the same
/// list for silk against silk), on one side, under `limit`.
fn between(
    a: &[Piece],
    b: &[Piece],
    kind: LimitKind,
    (limit, rule): &(i64, String),
) -> Vec<Violation> {
    let same = std::ptr::eq(a, b);
    let mut shapes: Vec<Shape> = a.iter().map(|p| p.shape.clone()).collect();
    if !same {
        shapes.extend(b.iter().map(|p| p.shape.clone()));
    }
    let n = a.len();
    let piece = |i: usize| if same || i < n { &a[i] } else { &b[i - n] };
    let pairs = near_pairs(&shapes, *limit, |i, j| {
        let (p, q) = (piece(i), piece(j));
        let sides = same || (i < n && j >= n);
        // Board drawings and strings are not checked against each other:
        // a frame drawn line by line touches itself at every corner.
        let part = |o: ItemRef| matches!(o, ItemRef::Footprint(_));
        let owners = !same || part(p.owner) || part(q.owner);
        sides && owners && p.side == q.side && p.owner != q.owner
    });
    let mut worst: HashMap<Vec<ItemRef>, Violation> = HashMap::new();
    for (i, j) in pairs {
        let Some((gap, at)) = gap_under(&shapes[i], &shapes[j], *limit) else {
            continue;
        };
        let (p, q) = (piece(i), piece(j));
        let mut items = vec![p.owner, q.owner];
        items.sort_unstable();
        if worst.get(&items).is_some_and(|v| v.measured_nm <= gap) {
            continue;
        }
        let net = if same { 0 } else { q.net };
        worst.insert(
            items.clone(),
            Violation {
                kind: kind.violation(),
                items,
                nets: (net, net),
                layers: 0,
                at,
                measured_nm: gap,
                required_nm: *limit,
                keepout: None,
                rule: Some(rule.clone()),
            },
        );
    }
    worst.into_values().collect()
}

/// The overlay side `layer` names; `None` for any other layer.
fn overlay_side(layer: &str) -> Option<BoardSide> {
    parse_pcb_layer(layer)
        .filter(|l| l.is_overlay())
        .and_then(PcbLayer::side)
}

pub(crate) fn silk_pieces(board: &PcbBoard) -> Vec<Piece> {
    let mut out = Vec::new();
    for footprint in &board.footprints {
        for graphic in &footprint.graphics {
            let Some(side) = overlay_side(&graphic.layer) else {
                continue;
            };
            let Ok(Some(placed)) = footprint_graphic_placement(footprint, graphic) else {
                continue;
            };
            out.extend(graphic_strokes(&placed).into_iter().map(|shape| Piece {
                owner: ItemRef::Footprint(footprint.uuid),
                net: 0,
                side,
                shape,
            }));
        }
    }
    for graphic in &board.graphics {
        let (Some(side), Some(kind)) = (overlay_side(&graphic.layer), graphic.kind()) else {
            continue;
        };
        let Ok(Some(placed)) = board_graphic_placement(graphic) else {
            continue;
        };
        let owner = ItemRef::Graphic {
            graphic: graphic.uuid,
            kind: PickKind::of_graphic(kind),
        };
        out.extend(graphic_strokes(&placed).into_iter().map(|shape| Piece {
            owner,
            net: 0,
            side,
            shape,
        }));
    }
    for text in &board.texts {
        let (Some(side), Ok(shape)) = (overlay_side(&text.layer), text_shape(text)) else {
            continue;
        };
        out.push(Piece {
            owner: ItemRef::Text(text.uuid),
            net: 0,
            side,
            shape,
        });
    }
    out
}

/// Every pad with copper on an outer layer, on that side: what a mask
/// opening exposes.
pub(crate) fn exposed_pads(board: &PcbBoard) -> Vec<Piece> {
    let top = 1 << layer_bit(CopperLayer::Top);
    let bottom = 1 << layer_bit(CopperLayer::Bottom);
    let mut out = Vec::new();
    let mut add =
        |footprint: &signex_types::pcb::Footprint, pad: &signex_types::pcb::Pad, owner: ItemRef| {
            let Ok(shape) = pad_shape(footprint, pad) else {
                return;
            };
            let layers = pad_layers(footprint, pad);
            let net = pad.net.as_ref().map_or(0, |n| n.number);
            for (bit, side) in [(top, BoardSide::Top), (bottom, BoardSide::Bottom)] {
                if layers & bit != 0 {
                    out.push(Piece {
                        owner,
                        net,
                        side,
                        shape: shape.clone(),
                    });
                }
            }
        };
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            let owner = ItemRef::Pad {
                footprint: footprint.uuid,
                pad: pad.uuid,
            };
            add(footprint, pad, owner);
        }
    }
    let frame = free_pad_frame();
    for pad in &board.free.pads {
        add(&frame, pad, ItemRef::FreePad(pad.uuid));
    }
    out
}
