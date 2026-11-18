//! Track width and via size checks (#2307 step 2b). Only a limit a written
//! rule sets is checked: the board setup's minimum width, via diameter and
//! drill are not enforced yet, so a board with no rule text reports
//! nothing here.

use signex_types::pcb::PcbBoard;
use signex_types::pcb_stack::board_copper_layers;

use pcb_geom::{ItemRef, PointNm, layer_mask, mm_to_nm, via_layers};

use super::user_rules::LimitKind;
use super::{DrcRules, Violation};

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
    // A copper arc is a track of its net (#2373), checked at its middle.
    for arc in board
        .free
        .arcs
        .iter()
        .filter(|a| pcb_geom::arc_is_copper(a))
    {
        let layers = layer_mask(std::iter::once(arc.layer.as_str()));
        let (Ok(width), Ok(at)) = (
            mm_to_nm(arc.width, "arc width"),
            PointNm::from_mm(arc.mid, "arc middle"),
        ) else {
            continue;
        };
        let object = Object {
            item: ItemRef::Arc(arc.uuid),
            net: arc.net,
            layers,
            at,
        };
        for kind in [LimitKind::MinTrackWidth, LimitKind::MaxTrackWidth] {
            object.check(rules, kind, width, found);
        }
    }
}

fn via_sizes(board: &PcbBoard, rules: &DrcRules, found: &mut Vec<Violation>) {
    let copper = board_copper_layers(&board.layers);
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
            // Its smallest copper ring on a layer it passes (#2401).
            (
                LimitKind::MinViaDiameter,
                via.min_diameter(&copper),
                "via diameter",
            ),
            (LimitKind::MinViaDrill, via.drill, "via drill"),
        ];
        for (kind, mm, field) in sizes {
            if let Ok(nm) = mm_to_nm(mm, field) {
                object.check(rules, kind, nm, found);
            }
        }
    }
}

/// One object with what a limit is checked against: a track, via or pad
/// hole for a size, or any item a value of its own is read from.
pub(crate) struct Object {
    pub(crate) item: ItemRef,
    pub(crate) net: u32,
    /// Copper layers it reaches; for an object off copper (a mask
    /// opening's item) every layer, so a board-wide rule applies.
    pub(crate) layers: u32,
    pub(crate) at: PointNm,
}

impl Object {
    /// Push a finding when `measured` breaks the limit of `kind` the
    /// written rules set for this object.
    pub(crate) fn check(
        &self,
        rules: &DrcRules,
        kind: LimitKind,
        measured: i64,
        found: &mut Vec<Violation>,
    ) {
        if self.layers == 0 {
            return;
        }
        let Some((limit, rule)) = rules.object_limit(kind, self.net, self.layers) else {
            return;
        };
        if !kind.is_broken(measured, limit) {
            return;
        }
        found.push(Violation {
            kind: kind.violation(),
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
