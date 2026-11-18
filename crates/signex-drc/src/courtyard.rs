//! Courtyard overlap (#2435): the courtyards of two parts on one side that
//! overlap, or come closer than a rule's minimum. A courtyard is measured
//! as the turned box of its drawings, the body a click selects a part by;
//! a part without courtyard drawings is not checked. Board-wide rules only.

use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, PcbBoard};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use pcb_geom::{ItemRef, Shape, Side, footprint_body};

use crate::near::{gap_under, near_pairs};
use crate::user_rules::LimitKind;
use crate::{DrcRules, Violation};

pub(crate) fn violations(board: &PcbBoard, rules: &DrcRules) -> Vec<Violation> {
    let Some((limit, rule)) = rules.board_limit(LimitKind::MinCourtyardGap) else {
        return Vec::new();
    };
    let parts: Vec<(&Footprint, Side, Shape)> = board
        .footprints
        .iter()
        .filter(|f| has_courtyard(f))
        .filter_map(|f| Some((f, Side::of_layer(&f.layer), footprint_body(f).ok()?)))
        .collect();
    let shapes: Vec<Shape> = parts.iter().map(|p| p.2.clone()).collect();
    let pairs = near_pairs(&shapes, limit, |i, j| parts[i].1 == parts[j].1);
    let mut found = Vec::new();
    for (i, j) in pairs {
        let Some((gap, at)) = gap_under(&shapes[i], &shapes[j], limit) else {
            continue;
        };
        let mut items = vec![
            ItemRef::Footprint(parts[i].0.uuid),
            ItemRef::Footprint(parts[j].0.uuid),
        ];
        items.sort_unstable();
        found.push(Violation {
            kind: LimitKind::MinCourtyardGap.violation(),
            items,
            nets: (0, 0),
            layers: 0,
            at,
            measured_nm: gap,
            required_nm: limit,
            keepout: None,
            rule: Some(rule.clone()),
        });
    }
    found
}

fn has_courtyard(footprint: &Footprint) -> bool {
    footprint.graphics.iter().any(|g| {
        matches!(
            parse_pcb_layer(&g.layer),
            Some(PcbLayer::Technical(
                TechnicalLayer::CourtyardTop | TechnicalLayer::CourtyardBottom
            ))
        )
    })
}

/// The parts with a courtyard, by id.
pub(crate) fn parts(board: &PcbBoard) -> impl Iterator<Item = uuid::Uuid> + '_ {
    board
        .footprints
        .iter()
        .filter(|f| has_courtyard(f))
        .map(|f| f.uuid)
}
