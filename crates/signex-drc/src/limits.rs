//! Track width and via size checks (#2307 step 2b). Only a limit a written
//! rule sets is checked: the board setup's minimum width, via diameter and
//! drill are not enforced yet, so a board with no rule text reports
//! nothing here.

use signex_types::pcb::PcbBoard;

use pcb_geom::{ItemRef, PointNm, layer_mask, mm_to_nm, via_layers};

use super::user_rules::LimitKind;
use super::{DrcRules, Violation, ViolationKind};

/// Every width and via-size violation of `board`, unsorted.
pub(super) fn violations(board: &PcbBoard, rules: &DrcRules) -> Vec<Violation> {
    let mut found = Vec::new();
    if rules.sets(LimitKind::MinTrackWidth) || rules.sets(LimitKind::MaxTrackWidth) {
        track_widths(board, rules, &mut found);
    }
    if rules.sets(LimitKind::MinViaDiameter) || rules.sets(LimitKind::MinViaDrill) {
        via_sizes(board, rules, &mut found);
    }
    found
}

fn track_widths(board: &PcbBoard, rules: &DrcRules, found: &mut Vec<Violation>) {
    for segment in &board.segments {
        let layers = layer_mask(std::iter::once(segment.layer.as_str()));
        let (Ok(width), Ok(a), Ok(b)) = (
            mm_to_nm(segment.width, "track width"),
            PointNm::from_mm(segment.start, "track start"),
            PointNm::from_mm(segment.end, "track end"),
        ) else {
            continue;
        };
        let object = Object {
            item: ItemRef::Track(segment.uuid),
            net: segment.net,
            layers,
            at: PointNm::new(a.x + (b.x - a.x) / 2, a.y + (b.y - a.y) / 2),
        };
        for kind in [LimitKind::MinTrackWidth, LimitKind::MaxTrackWidth] {
            object.check(rules, kind, width, found);
        }
    }
}

fn via_sizes(board: &PcbBoard, rules: &DrcRules, found: &mut Vec<Violation>) {
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
        let sizes = [
            (LimitKind::MinViaDiameter, via.diameter, "via diameter"),
            (LimitKind::MinViaDrill, via.drill, "via drill"),
        ];
        for (kind, mm, field) in sizes {
            if let Ok(nm) = mm_to_nm(mm, field) {
                object.check(rules, kind, nm, found);
            }
        }
    }
}

/// A track or via with what a limit is checked against.
struct Object {
    item: ItemRef,
    net: u32,
    layers: u32,
    at: PointNm,
}

impl Object {
    fn check(&self, rules: &DrcRules, kind: LimitKind, measured: i64, found: &mut Vec<Violation>) {
        if self.layers == 0 {
            return;
        }
        let Some((limit, rule)) = rules.object_limit(kind, self.net, self.layers) else {
            return;
        };
        let (broken, violation_kind) = match kind {
            LimitKind::MinTrackWidth => (measured < limit, ViolationKind::TrackWidth),
            LimitKind::MaxTrackWidth => (measured > limit, ViolationKind::TrackWidth),
            LimitKind::MinViaDiameter => (measured < limit, ViolationKind::ViaDiameter),
            LimitKind::MinViaDrill => (measured < limit, ViolationKind::ViaDrill),
            LimitKind::MinClearance => return,
        };
        if !broken {
            return;
        }
        found.push(Violation {
            kind: violation_kind,
            items: vec![self.item],
            nets: (self.net, self.net),
            layers: self.layers,
            at: self.at,
            measured_nm: measured,
            required_nm: limit,
            keepout: None,
            rule: Some(rule),
        });
    }
}
