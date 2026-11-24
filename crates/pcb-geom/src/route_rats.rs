//! The ratsnest while a route is drawn (Caner, 2026-11-21): the line of
//! the net being routed leaves from the route's end, not from the pad it
//! started on, and points to the nearest pad of that net the start's copper
//! does not reach yet. Other nets keep their lines.
//!
//! [`route_rats`] reads the board once (the island pass a ratsnest refresh
//! makes) when a route starts or the board changes under it;
//! [`RouteRats::lines`] is then a scan of the net's pads per pointer move.

use std::collections::BTreeSet;

use signex_types::pcb::PcbBoard;

use crate::ratsnest::{RatLine, copper_islands};
use crate::units::PointNm;
use crate::zone_fill::ZoneFills;

/// The pads of one net split by whether the copper a route starts on
/// already reaches them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RouteRats {
    pub net: u32,
    /// Pad centres on the start's copper: their lines are the route's.
    joined: BTreeSet<PointNm>,
    /// The net's other pad centres, which the route may still reach.
    open: Vec<PointNm>,
}

/// The pads of net `net` on `board`, split by whether the copper of that net
/// under `start` reaches them. A start on no copper of the net (bare board,
/// or a pour, whose pieces are not read here) reaches none. `fills` must be
/// [`crate::zone_fills`] of this board.
pub fn route_rats(board: &PcbBoard, fills: &ZoneFills, net: u32, start: PointNm) -> RouteRats {
    let mut out = RouteRats {
        net,
        ..RouteRats::default()
    };
    if net == 0 {
        return out;
    }
    let Some((nodes, mut islands)) = copper_islands(board, fills) else {
        return out;
    };
    let on_start: Vec<usize> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.net == net && n.shape.contains(start, 0))
        .map(|(i, _)| i)
        .collect();
    let roots: BTreeSet<usize> = on_start.into_iter().map(|i| islands.find(i)).collect();
    for (i, node) in nodes.iter().enumerate() {
        let Some(center) = node.pad_center.filter(|_| node.net == net) else {
            continue;
        };
        if roots.contains(&islands.find(i)) {
            out.joined.insert(center);
        } else {
            out.open.push(center);
        }
    }
    out
}

impl RouteRats {
    /// `base` without the routed net's lines that leave the start's copper:
    /// what the board draws while the route is drawn.
    pub fn without_start(&self, base: &[RatLine]) -> Vec<RatLine> {
        base.iter()
            .filter(|line| !self.leaves_start(line))
            .copied()
            .collect()
    }

    /// The routed net's line from the route's `end` to the nearest pad the
    /// start does not reach (ties to the lower `(x, y)`); `None` when every
    /// pad is reached or `end` is on that pad. `from` is the route's end, so
    /// unlike a board line it need not be the lower point.
    pub fn line_from(&self, end: PointNm) -> Option<RatLine> {
        let pad = self
            .open
            .iter()
            .copied()
            .min_by_key(|p| (p.distance_sq(end), *p))?;
        (pad != end).then_some(RatLine {
            net: self.net,
            from: end,
            to: pad,
        })
    }

    /// The ratsnest while the route ends at `end`: [`Self::without_start`]
    /// of `base` with [`Self::line_from`] `end`.
    pub fn lines(&self, base: &[RatLine], end: PointNm) -> Vec<RatLine> {
        let mut lines = self.without_start(base);
        lines.extend(self.line_from(end));
        lines
    }

    fn leaves_start(&self, line: &RatLine) -> bool {
        line.net == self.net && (self.joined.contains(&line.from) || self.joined.contains(&line.to))
    }
}

#[cfg(test)]
#[path = "route_rats_tests.rs"]
mod tests;
