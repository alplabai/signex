//! Mouse bites (#2410): the tabs that hold each copy across its gaps, the
//! perforation holes along them, and the routed edges, which run round each
//! copy and along each rail's inner edge and stay open where a tab crosses.
//! Tabs sit on the sides of the board's bounding box, evenly spaced.

use signex_types::pcb_panel::{MouseBites, PanelError};

use super::layout::{Facing, Grid, PanelLayout, PanelSegment, Side, nm};
use crate::rect::RectNm;
use crate::units::{PointNm, nm_to_mm};

/// A stretch along one axis, `lo < hi`, nm.
type Span = (i64, i64);

pub(super) fn mouse_bites(
    grid: &Grid,
    ring: &[PointNm],
    bites: &MouseBites,
    layout: &mut PanelLayout,
) -> Result<(), PanelError> {
    let (hole, pitch, width) = (
        nm(bites.hole_diameter)?,
        nm(bites.pitch)?,
        nm(bites.tab_width)?,
    );
    let board = grid.board;
    let rails = grid.rails;
    // Only the edges that face something carry tabs; a lone copy with no
    // rail on a side needs none there, however small it is.
    let needs_x = grid.rows > 1 || rails.top > 0 || rails.bottom > 0;
    let needs_y = grid.columns > 1 || rails.left > 0 || rails.right > 0;
    let count = bites.tabs_per_edge;
    let along_x = if needs_x {
        spans(board.min.x, board.width(), count, width)?
    } else {
        Vec::new()
    };
    let along_y = if needs_y {
        spans(board.min.y, board.height(), count, width)?
    } else {
        Vec::new()
    };
    let tabs = Tabs {
        board,
        along_x,
        along_y,
    };
    layout.bite_diameter = hole;
    for row in 0..grid.rows {
        for column in 0..grid.columns {
            let offset = grid.offset(column, row);
            for side in Side::ALL {
                let facing = grid.facing(column, row, side);
                if facing == Facing::Outside {
                    continue;
                }
                let edge = tabs.edge(side);
                for &(lo, hi) in tabs.on(side) {
                    for at in bite_row(lo, hi, pitch) {
                        layout.bite_holes.push(on_side(side, edge, at).plus(offset));
                    }
                    // A gap between two copies is crossed once: count and
                    // close it from the copy before it.
                    if facing == Facing::Rail || matches!(side, Side::Right | Side::Bottom) {
                        layout.tabs += 1;
                        let far = edge + across(side, grid.gap);
                        for at in [lo, hi] {
                            layout.routes.push(PanelSegment::new(
                                on_side(side, edge, at).plus(offset),
                                on_side(side, far, at).plus(offset),
                            ));
                        }
                    }
                }
            }
            route_outline(grid, &tabs, ring, (column, row), &mut layout.routes);
        }
    }
    rail_edges(grid, &tabs, &mut layout.routes);
    Ok(())
}

/// The tab spans of the board's box: along x on the top and bottom sides,
/// along y on the left and right.
struct Tabs {
    board: RectNm,
    along_x: Vec<Span>,
    along_y: Vec<Span>,
}

impl Tabs {
    fn on(&self, side: Side) -> &[Span] {
        match side {
            Side::Top | Side::Bottom => &self.along_x,
            Side::Left | Side::Right => &self.along_y,
        }
    }

    /// The fixed coordinate of a side of the board's box.
    fn edge(&self, side: Side) -> i64 {
        match side {
            Side::Top => self.board.min.y,
            Side::Bottom => self.board.max.y,
            Side::Left => self.board.min.x,
            Side::Right => self.board.max.x,
        }
    }

    /// The side of the box the outline edge `a`-`b` lies on, if any.
    fn side_of(&self, a: PointNm, b: PointNm) -> Option<Side> {
        let r = self.board;
        if a.y == b.y && a.y == r.min.y {
            Some(Side::Top)
        } else if a.y == b.y && a.y == r.max.y {
            Some(Side::Bottom)
        } else if a.x == b.x && a.x == r.min.x {
            Some(Side::Left)
        } else if a.x == b.x && a.x == r.max.x {
            Some(Side::Right)
        } else {
            None
        }
    }
}

/// `count` tabs of `width` spread evenly over `len` from `start`.
fn spans(start: i64, len: i64, count: u32, width: i64) -> Result<Vec<Span>, PanelError> {
    let n = i64::from(count);
    if n * width > len {
        return Err(PanelError::TabsDoNotFit {
            tabs: count,
            tab_width_mm: nm_to_mm(width),
            edge_mm: nm_to_mm(len),
        });
    }
    Ok((0..n)
        .map(|k| {
            let centre = start + len * (2 * k + 1) / (2 * n);
            let lo = centre - width / 2;
            (lo, lo + width)
        })
        .collect())
}

/// The hole centres along a tab: as many as fit `pitch` apart, centred.
/// `BoardPanel::check` keeps the pitch above the smallest bite hole; the
/// floor of 1 nm only guards the division.
fn bite_row(lo: i64, hi: i64, pitch: i64) -> Vec<i64> {
    let pitch = pitch.max(1);
    let width = hi - lo;
    let count = width / pitch + 1;
    let first = lo + (width - (count - 1) * pitch) / 2;
    (0..count).map(|i| first + i * pitch).collect()
}

/// The point `at` along a side whose fixed coordinate is `edge`.
fn on_side(side: Side, edge: i64, at: i64) -> PointNm {
    match side {
        Side::Top | Side::Bottom => PointNm::new(at, edge),
        Side::Left | Side::Right => PointNm::new(edge, at),
    }
}

/// How far, and which way, the gap reaches out of a side.
fn across(side: Side, gap: PointNm) -> i64 {
    match side {
        Side::Top => -gap.y,
        Side::Bottom => gap.y,
        Side::Left => -gap.x,
        Side::Right => gap.x,
    }
}

/// `lo..hi` without the `spans` (sorted), as the stretches left.
fn minus_spans(lo: i64, hi: i64, spans: &[Span]) -> Vec<Span> {
    let mut left = Vec::new();
    let mut at = lo;
    for &(start, end) in spans {
        if end <= at || start >= hi {
            continue;
        }
        if start > at {
            left.push((at, start));
        }
        at = at.max(end);
    }
    if at < hi {
        left.push((at, hi));
    }
    left
}

/// One copy's outline: an edge on a side of the box that faces a gap stays
/// open where its tabs leave; one on the panel's own edge is the frame's.
fn route_outline(
    grid: &Grid,
    tabs: &Tabs,
    ring: &[PointNm],
    (column, row): (i64, i64),
    routes: &mut Vec<PanelSegment>,
) {
    let offset = grid.offset(column, row);
    for (i, &a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        if a == b {
            continue;
        }
        let Some(side) = tabs.side_of(a, b) else {
            routes.push(PanelSegment::new(a.plus(offset), b.plus(offset)));
            continue;
        };
        if grid.facing(column, row, side) == Facing::Outside {
            continue;
        }
        let (u, v) = match side {
            Side::Top | Side::Bottom => (a.x, b.x),
            Side::Left | Side::Right => (a.y, b.y),
        };
        let edge = tabs.edge(side);
        for (lo, hi) in minus_spans(u.min(v), u.max(v), tabs.on(side)) {
            routes.push(PanelSegment::new(
                on_side(side, edge, lo).plus(offset),
                on_side(side, edge, hi).plus(offset),
            ));
        }
    }
}

/// `spans` repeated `count` times, `step` apart, in order.
fn repeated(spans: &[Span], step: i64, count: i64) -> Vec<Span> {
    let mut out = Vec::with_capacity(spans.len() * count.max(0) as usize);
    for k in 0..count {
        out.extend(spans.iter().map(|&(lo, hi)| (lo + k * step, hi + k * step)));
    }
    out
}

/// Each rail's inner edge, across the panel up to the rails beside it, open
/// where the tabs of the copies next to it cross.
fn rail_edges(grid: &Grid, tabs: &Tabs, routes: &mut Vec<PanelSegment>) {
    let copies = grid.copies_box();
    let frame = grid.frame();
    let (rails, gap, pitch) = (grid.rails, grid.gap, grid.pitch());
    let left = if rails.left > 0 {
        copies.min.x - gap.x
    } else {
        frame.min.x
    };
    let right = if rails.right > 0 {
        copies.max.x + gap.x
    } else {
        frame.max.x
    };
    let top = if rails.top > 0 {
        copies.min.y - gap.y
    } else {
        frame.min.y
    };
    let bottom = if rails.bottom > 0 {
        copies.max.y + gap.y
    } else {
        frame.max.y
    };
    let x_tabs = repeated(&tabs.along_x, pitch.x, grid.columns);
    let y_tabs = repeated(&tabs.along_y, pitch.y, grid.rows);
    for (width, y) in [(rails.top, top), (rails.bottom, bottom)] {
        if width > 0 {
            for (lo, hi) in minus_spans(left, right, &x_tabs) {
                routes.push(PanelSegment::new(PointNm::new(lo, y), PointNm::new(hi, y)));
            }
        }
    }
    for (width, x) in [(rails.left, left), (rails.right, right)] {
        if width > 0 {
            for (lo, hi) in minus_spans(top, bottom, &y_tabs) {
                routes.push(PanelSegment::new(PointNm::new(x, lo), PointNm::new(x, hi)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_spread_evenly_and_refuse_to_overflow_an_edge() {
        assert_eq!(spans(0, 100, 1, 10), Ok(vec![(45, 55)]));
        assert_eq!(spans(10, 100, 2, 10), Ok(vec![(30, 40), (80, 90)]));
        assert!(matches!(
            spans(0, 15, 2, 10),
            Err(PanelError::TabsDoNotFit { tabs: 2, .. })
        ));
    }

    #[test]
    fn bites_centre_on_their_tab_and_spans_cut_a_line() {
        assert_eq!(bite_row(0, 10, 4), vec![1, 5, 9]);
        assert_eq!(bite_row(0, 2, 4), vec![1]);
        assert_eq!(
            minus_spans(0, 100, &[(10, 20), (50, 60)]),
            vec![(0, 10), (20, 50), (60, 100)]
        );
        assert_eq!(minus_spans(0, 10, &[(-5, 15)]), Vec::<Span>::new());
        assert_eq!(minus_spans(0, 10, &[(20, 30)]), vec![(0, 10)]);
    }
}
