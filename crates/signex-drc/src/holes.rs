//! Hole checks (#2435): hole size, annular ring and hole-to-hole spacing of
//! every via and pad hole, and vias in (or too near) surface-mount pads.
//! Only a limit a written rule sets is checked. See `docs/DRC_RULES.md`,
//! "The base rules".

use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard};
use signex_types::pcb_free::free_pad_frame;
use signex_types::pcb_stack::board_copper_layers;

use pcb_geom::{
    ItemRef, PointNm, Shape, mm_to_nm, pad_layers, pad_shape, pad_world_center, via_layers,
    via_shape,
};

use crate::limits::Object;
use crate::near::{gap_under, near_pairs};
use crate::user_rules::LimitKind;
use crate::{DrcRules, Violation};

/// One drilled hole.
struct Hole {
    object: Object,
    /// The drill diameter, nm.
    drill: i64,
    /// The smallest copper size around it, nm; `None` for a hole without
    /// plated copper.
    copper: Option<i64>,
}

impl Hole {
    fn disc(&self) -> Shape {
        Shape::Disc {
            center: self.object.at,
            radius: self.drill / 2,
        }
    }
}

/// Every hole finding of `board`, unsorted.
pub(crate) fn violations(board: &PcbBoard, rules: &DrcRules) -> Vec<Violation> {
    let mut found = Vec::new();
    let sizes = [
        LimitKind::MinHoleSize,
        LimitKind::MaxHoleSize,
        LimitKind::MinAnnularRing,
        LimitKind::MinHoleToHole,
    ];
    if sizes.iter().any(|&k| rules.sets(k)) {
        let holes = holes(board);
        for hole in &holes {
            for kind in [LimitKind::MinHoleSize, LimitKind::MaxHoleSize] {
                hole.object.check(rules, kind, hole.drill, &mut found);
            }
            if let Some(copper) = hole.copper {
                let ring = (copper - hole.drill) / 2;
                hole.object
                    .check(rules, LimitKind::MinAnnularRing, ring, &mut found);
            }
        }
        if rules.sets(LimitKind::MinHoleToHole) {
            hole_to_hole(&holes, rules, &mut found);
        }
    }
    if rules.sets(LimitKind::MinViaPadGap) {
        vias_in_pads(board, rules, &mut found);
    }
    found
}

fn holes(board: &PcbBoard) -> Vec<Hole> {
    let copper = board_copper_layers(&board.layers);
    let mut out = Vec::new();
    for via in &board.vias {
        let (Ok(at), Ok(drill), Ok(ring)) = (
            PointNm::from_mm(via.position, "via position"),
            mm_to_nm(via.drill, "via drill"),
            mm_to_nm(via.min_diameter(&copper), "via diameter"),
        ) else {
            continue;
        };
        out.push(Hole {
            object: Object {
                item: ItemRef::Via(via.uuid),
                net: via.net,
                layers: via_layers(via),
                at,
            },
            drill,
            copper: Some(ring),
        });
    }
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            let item = ItemRef::Pad {
                footprint: footprint.uuid,
                pad: pad.uuid,
            };
            out.extend(pad_hole(footprint, pad, item));
        }
    }
    let frame = free_pad_frame();
    for pad in &board.free.pads {
        out.extend(pad_hole(&frame, pad, ItemRef::FreePad(pad.uuid)));
    }
    out
}

fn pad_hole(footprint: &Footprint, pad: &Pad, item: ItemRef) -> Option<Hole> {
    let drill = pad.drill.as_ref().filter(|d| d.diameter > 0.0)?;
    let drill = mm_to_nm(drill.diameter, "pad drill").ok()?;
    let at = pad_world_center(footprint, pad).ok()?;
    let plated = pad.pad_type == PadType::Thru;
    let copper = mm_to_nm(pad.size.x.min(pad.size.y), "pad size").ok()?;
    Some(Hole {
        object: Object {
            item,
            net: pad.net.as_ref().map_or(0, |n| n.number),
            // A hole goes through the board: an unplated one is read on
            // every layer, so a board-wide rule applies to it.
            layers: match pad_layers(footprint, pad) {
                0 => u32::MAX,
                layers => layers,
            },
            at,
        },
        drill,
        copper: plated.then_some(copper),
    })
}

/// Two holes closer, edge to edge, than the larger of their two limits.
fn hole_to_hole(holes: &[Hole], rules: &DrcRules, found: &mut Vec<Violation>) {
    let limit = |hole: &Hole| {
        rules.object_limit(
            LimitKind::MinHoleToHole,
            hole.object.net,
            hole.object.layers,
        )
    };
    let limits: Vec<_> = holes.iter().map(limit).collect();
    let discs: Vec<Shape> = holes.iter().map(Hole::disc).collect();
    let reach = rules.widest_limit(LimitKind::MinHoleToHole);
    for (i, j) in near_pairs(&discs, reach, |i, j| {
        holes[i].object.item != holes[j].object.item
    }) {
        let side = match (&limits[i], &limits[j]) {
            (Some(a), Some(b)) if b.0 > a.0 => b,
            (Some(a), _) => a,
            (None, Some(b)) => b,
            (None, None) => continue,
        };
        let Some((gap, at)) = gap_under(&discs[i], &discs[j], side.0) else {
            continue;
        };
        let (a, b) = (&holes[i].object, &holes[j].object);
        let ((first, second), nets) = if a.item <= b.item {
            ((a, b), (a.net, b.net))
        } else {
            ((b, a), (b.net, a.net))
        };
        found.push(Violation {
            kind: LimitKind::MinHoleToHole.violation(),
            items: vec![first.item, second.item],
            nets,
            layers: shared(a.layers, b.layers),
            at,
            measured_nm: gap,
            required_nm: side.0,
            keepout: None,
            rule: Some(side.1.clone()),
        });
    }
}

/// The layers two items share, or either's when they share none.
fn shared(a: u32, b: u32) -> u32 {
    match a & b {
        0 => a | b,
        both => both,
    }
}

/// A via whose copper meets, or comes closer than its limit to, a
/// surface-mount pad of its own net (or of no net) on a shared layer.
fn vias_in_pads(board: &PcbBoard, rules: &DrcRules, found: &mut Vec<Violation>) {
    let mut pads: Vec<(ItemRef, u32, u32, Shape)> = Vec::new();
    let mut add = |footprint: &Footprint, pad: &Pad, item: ItemRef| {
        if pad.pad_type != PadType::Smd {
            return;
        }
        if let Ok(shape) = pad_shape(footprint, pad) {
            let net = pad.net.as_ref().map_or(0, |n| n.number);
            pads.push((item, net, pad_layers(footprint, pad), shape));
        }
    };
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            let item = ItemRef::Pad {
                footprint: footprint.uuid,
                pad: pad.uuid,
            };
            add(footprint, pad, item);
        }
    }
    let frame = free_pad_frame();
    for pad in &board.free.pads {
        add(&frame, pad, ItemRef::FreePad(pad.uuid));
    }
    for via in &board.vias {
        let layers = via_layers(via);
        let Ok(disc) = via_shape(via) else {
            continue;
        };
        let Some((limit, rule)) = rules.object_limit(LimitKind::MinViaPadGap, via.net, layers)
        else {
            continue;
        };
        let near = disc.aabb().inflate(limit);
        for (pad, net, pad_layers, shape) in &pads {
            if layers & pad_layers == 0 || (*net != 0 && *net != via.net) {
                continue;
            }
            if !near.intersects(&shape.aabb()) {
                continue;
            }
            let Some((gap, at)) = gap_under(&disc, shape, limit) else {
                continue;
            };
            let via_item = ItemRef::Via(via.uuid);
            let mut items = vec![via_item, *pad];
            items.sort_unstable();
            found.push(Violation {
                kind: LimitKind::MinViaPadGap.violation(),
                items,
                nets: (via.net, via.net),
                layers: layers & pad_layers,
                at,
                measured_nm: gap,
                required_nm: limit,
                keepout: None,
                rule: Some(rule.clone()),
            });
        }
    }
}
