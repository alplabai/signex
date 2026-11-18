//! Solder mask and paste checks (#2435): a strip of mask between two
//! openings narrower than a rule's minimum (a sliver), and manual mask and
//! paste expansions outside a rule's range. The openings are the exposed
//! pads (a pad has no expansion of its own), the vias not tented on that
//! side and the tracks with a manual mask expansion, each grown by its
//! manual expansion. An item that follows the rule has no expansion to
//! check. Board-wide rules only.

use std::collections::HashMap;

use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::BoardSide;
use signex_types::pcb_route_attrs::ViaMaskSide;

use pcb_geom::{ItemRef, PointNm, Shape, layer_mask, mm_to_nm, track_shape, via_layers, via_shape};

use crate::limits::Object;
use crate::near::near_pairs;
use crate::silk::{Piece, exposed_pads};
use crate::user_rules::LimitKind;
use crate::{DrcRules, Violation};

pub(crate) fn violations(board: &PcbBoard, rules: &DrcRules) -> Vec<Violation> {
    let mut found = Vec::new();
    if let Some(limit) = rules.board_limit(LimitKind::MinMaskSliver) {
        found.extend(slivers(&openings(board), &limit));
    }
    expansions(board, rules, &mut found);
    found
}

/// Each pair of openings on one side whose mask strip between them is
/// wider than nothing and narrower than `limit`; openings that touch or
/// overlap leave no strip.
fn slivers(openings: &[Piece], (limit, rule): &(i64, String)) -> Vec<Violation> {
    let shapes: Vec<Shape> = openings.iter().map(|p| p.shape.clone()).collect();
    let pairs = near_pairs(&shapes, *limit, |i, j| {
        openings[i].side == openings[j].side && openings[i].owner != openings[j].owner
    });
    let mut worst: HashMap<Vec<ItemRef>, Violation> = HashMap::new();
    for (i, j) in pairs {
        if shapes[i].overlaps(&shapes[j]) {
            continue;
        }
        let gap = shapes[i].separation(&shapes[j]);
        let width = gap.distance_nm();
        if width <= 0 || !gap.is_under(*limit) {
            continue;
        }
        let width = width.min(limit - 1);
        let (a, b) = (&openings[i], &openings[j]);
        let mut items = vec![a.owner, b.owner];
        items.sort_unstable();
        if worst.get(&items).is_some_and(|v| v.measured_nm <= width) {
            continue;
        }
        let nets = if a.owner <= b.owner {
            (a.net, b.net)
        } else {
            (b.net, a.net)
        };
        worst.insert(
            items.clone(),
            Violation {
                kind: LimitKind::MinMaskSliver.violation(),
                items,
                nets,
                layers: 0,
                at: gap.midpoint(),
                measured_nm: width,
                required_nm: *limit,
                keepout: None,
                rule: Some(rule.clone()),
            },
        );
    }
    worst.into_values().collect()
}

pub(crate) fn openings(board: &PcbBoard) -> Vec<Piece> {
    let mut out = exposed_pads(board);
    for via in &board.vias {
        let Ok(Shape::Disc { center, radius }) = via_shape(via) else {
            continue;
        };
        let sides = [
            (via.attributes.top, BoardSide::Top),
            (via.attributes.bottom, BoardSide::Bottom),
        ];
        for (mask, side) in sides {
            if mask.tented {
                continue;
            }
            let grow = manual_nm(mask.expansion.manual_mm()).unwrap_or(0);
            let drill = mm_to_nm(via.drill, "via drill").unwrap_or(0) / 2;
            let base = if via.attributes.from_hole_edge {
                drill
            } else {
                radius
            };
            out.push(Piece {
                owner: ItemRef::Via(via.uuid),
                net: via.net,
                side,
                shape: Shape::Disc {
                    center,
                    radius: (base + grow).max(0),
                },
            });
        }
    }
    for segment in &board.segments {
        let Some(grow) = manual_nm(segment.attributes.solder_mask.manual_mm()) else {
            continue;
        };
        let side =
            match signex_types::format::parse_pcb_layer(&segment.layer).and_then(|l| l.copper()) {
                Some(signex_types::pcb_layer::CopperLayer::Top) => BoardSide::Top,
                Some(signex_types::pcb_layer::CopperLayer::Bottom) => BoardSide::Bottom,
                _ => continue,
            };
        if let Ok(Shape::Capsule { a, b, radius }) = track_shape(segment) {
            out.push(Piece {
                owner: ItemRef::Track(segment.uuid),
                net: segment.net,
                side,
                shape: Shape::Capsule {
                    a,
                    b,
                    radius: (radius + grow).max(0),
                },
            });
        }
    }
    out
}

fn manual_nm(mm: Option<f64>) -> Option<i64> {
    mm.and_then(|mm| mm_to_nm(mm, "mask expansion").ok())
}

/// Manual mask expansions of vias (each side not tented) and tracks, and
/// manual paste expansions of tracks, against the rules' range.
fn expansions(board: &PcbBoard, rules: &DrcRules, found: &mut Vec<Violation>) {
    let mask = [LimitKind::MinMaskExpansion, LimitKind::MaxMaskExpansion];
    let paste = [LimitKind::MinPasteExpansion, LimitKind::MaxPasteExpansion];
    let checks_mask = mask.iter().any(|&k| rules.sets(k));
    let checks_paste = paste.iter().any(|&k| rules.sets(k));
    if !checks_mask && !checks_paste {
        return;
    }
    for via in &board.vias {
        let Ok(at) = PointNm::from_mm(via.position, "via position") else {
            continue;
        };
        let object = Object {
            item: ItemRef::Via(via.uuid),
            net: via.net,
            layers: via_layers(via),
            at,
        };
        if !checks_mask {
            continue;
        }
        // One finding per value: both sides alike say it once.
        let mut values: Vec<i64> = [via.attributes.top, via.attributes.bottom]
            .into_iter()
            .filter_map(untented_manual)
            .collect();
        values.dedup();
        for nm in values {
            for kind in mask {
                object.check(rules, kind, nm, found);
            }
        }
    }
    for segment in &board.segments {
        let (Ok(a), Ok(b)) = (
            PointNm::from_mm(segment.start, "track start"),
            PointNm::from_mm(segment.end, "track end"),
        ) else {
            continue;
        };
        let object = Object {
            item: ItemRef::Track(segment.uuid),
            net: segment.net,
            layers: layer_mask(std::iter::once(segment.layer.as_str())),
            at: PointNm::new(a.x + (b.x - a.x) / 2, a.y + (b.y - a.y) / 2),
        };
        let attributes = segment.attributes;
        let values = [
            (checks_mask, attributes.solder_mask.manual_mm(), mask),
            (checks_paste, attributes.paste_mask.manual_mm(), paste),
        ];
        for (checks, mm, kinds) in values {
            let Some(nm) = manual_nm(mm).filter(|_| checks) else {
                continue;
            };
            for kind in kinds {
                object.check(rules, kind, nm, found);
            }
        }
    }
}

fn untented_manual(side: ViaMaskSide) -> Option<i64> {
    if side.tented {
        return None;
    }
    manual_nm(side.expansion.manual_mm())
}
