//! The panel laid out on a board outline: the frame, the copies, the rails,
//! the fiducials and tooling holes, and the V-score lines. Mouse bites are
//! in `tabs`.

use std::collections::BTreeSet;

use signex_types::pcb::Point;
use signex_types::pcb_panel::{BoardPanel, PanelError, PanelSeparation};

use super::tabs;
use crate::rect::RectNm;
use crate::units::{MAX_ABS_NM, PointNm, mm_to_nm, nm_to_mm};

/// A straight line of the panel, nm: a V-score, or a routed edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelSegment {
    pub from: PointNm,
    pub to: PointNm,
}

impl PanelSegment {
    pub(super) fn new(from: PointNm, to: PointNm) -> Self {
        Self { from, to }
    }
}

/// Everything a panel puts round its copies, in board nm (y down).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelLayout {
    /// The whole panel: copies, gaps and rails.
    pub frame: RectNm,
    /// The box round the board's outline, where the first copy sits.
    pub board: RectNm,
    /// Each copy's offset from the board, row by row from the top left;
    /// the first is zero.
    pub copies: Vec<PointNm>,
    /// The rails that are there (top, bottom, left, right).
    pub rails: Vec<RectNm>,
    pub fiducials: Vec<PointNm>,
    pub fiducial_diameter: i64,
    pub tooling_holes: Vec<PointNm>,
    pub tooling_hole_diameter: i64,
    /// The perforation holes of every tab, on the copies' edges.
    pub bite_holes: Vec<PointNm>,
    pub bite_diameter: i64,
    /// Tabs across the gaps, each counted once.
    pub tabs: usize,
    /// Score lines across the whole frame.
    pub v_scores: Vec<PanelSegment>,
    /// The routed edges of a mouse-bite panel: each copy's outline and each
    /// rail's inner edge, open where a tab crosses, and the tabs' sides.
    pub routes: Vec<PanelSegment>,
    /// Whether the outline is its own bounding box. Tabs and V-scores follow
    /// the box; on another shape a tab may not reach the board.
    pub rectangular: bool,
}

impl PanelLayout {
    pub fn width_mm(&self) -> f64 {
        nm_to_mm(self.frame.width())
    }

    pub fn height_mm(&self) -> f64 {
        nm_to_mm(self.frame.height())
    }
}

/// The rail widths, nm.
#[derive(Debug, Clone, Copy)]
pub(super) struct Rails {
    pub top: i64,
    pub bottom: i64,
    pub left: i64,
    pub right: i64,
}

/// A side of a copy, in the board's own frame (top is the smaller y).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];
}

/// What lies past a side of a copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Facing {
    Neighbour,
    Rail,
    /// The edge of the panel itself.
    Outside,
}

/// The array of copies, nm.
#[derive(Debug, Clone, Copy)]
pub(super) struct Grid {
    pub board: RectNm,
    pub columns: i64,
    pub rows: i64,
    /// The gap between copies, and between a copy and a rail.
    pub gap: PointNm,
    pub rails: Rails,
}

impl Grid {
    /// The step from one copy to the next.
    pub fn pitch(&self) -> PointNm {
        PointNm::new(
            self.board.width() + self.gap.x,
            self.board.height() + self.gap.y,
        )
    }

    pub fn offset(&self, column: i64, row: i64) -> PointNm {
        let pitch = self.pitch();
        PointNm::new(column * pitch.x, row * pitch.y)
    }

    /// The box round every copy.
    pub fn copies_box(&self) -> RectNm {
        let last = self.offset(self.columns - 1, self.rows - 1);
        RectNm::new(self.board.min, self.board.max.plus(last))
    }

    /// The copies with a gap and a rail on each side that has a rail.
    pub fn frame(&self) -> RectNm {
        let copies = self.copies_box();
        let out = |rail: i64, gap: i64| if rail > 0 { rail + gap } else { 0 };
        let r = self.rails;
        RectNm::new(
            PointNm::new(
                copies.min.x - out(r.left, self.gap.x),
                copies.min.y - out(r.top, self.gap.y),
            ),
            PointNm::new(
                copies.max.x + out(r.right, self.gap.x),
                copies.max.y + out(r.bottom, self.gap.y),
            ),
        )
    }

    pub fn facing(&self, column: i64, row: i64, side: Side) -> Facing {
        let (inside, rail) = match side {
            Side::Top => (row > 0, self.rails.top),
            Side::Bottom => (row + 1 < self.rows, self.rails.bottom),
            Side::Left => (column > 0, self.rails.left),
            Side::Right => (column + 1 < self.columns, self.rails.right),
        };
        if inside {
            Facing::Neighbour
        } else if rail > 0 {
            Facing::Rail
        } else {
            Facing::Outside
        }
    }
}

pub(super) fn nm(mm: f64) -> Result<i64, PanelError> {
    mm_to_nm(mm, "panel").map_err(|_| PanelError::TooLarge)
}

/// Lay `panel` out on the board `outline`. Refused for a panel that breaks a
/// rule, a board without an outline, tabs that do not fit an edge, and a
/// panel past 1 km.
pub fn layout(outline: &[Point], panel: &BoardPanel) -> Result<PanelLayout, PanelError> {
    panel.check()?;
    let ring = ring(outline)?;
    let board = RectNm::bounding(ring.iter().copied()).ok_or(PanelError::NoOutline)?;
    if board.width() <= 0 || board.height() <= 0 {
        return Err(PanelError::NoOutline);
    }
    let rails = Rails {
        top: nm(panel.rails.top)?,
        bottom: nm(panel.rails.bottom)?,
        left: nm(panel.rails.left)?,
        right: nm(panel.rails.right)?,
    };
    let grid = Grid {
        board,
        columns: i64::from(panel.columns),
        rows: i64::from(panel.rows),
        gap: PointNm::new(nm(panel.spacing_x)?, nm(panel.spacing_y)?),
        rails,
    };
    let frame = grid.frame();
    let reach = [frame.min.x, frame.min.y, frame.max.x, frame.max.y];
    if reach
        .iter()
        .any(|v| v.unsigned_abs() > MAX_ABS_NM.unsigned_abs())
    {
        return Err(PanelError::TooLarge);
    }
    let fiducial_diameter = nm(panel.fiducial_diameter)?;
    let tooling_hole_diameter = nm(panel.tooling_hole_diameter)?;
    let (fiducials, tooling_holes) = marks(
        frame,
        rails,
        fiducial_diameter > 0,
        tooling_hole_diameter > 0,
    );
    let mut layout = PanelLayout {
        frame,
        board,
        copies: (0..grid.rows)
            .flat_map(|row| (0..grid.columns).map(move |column| grid.offset(column, row)))
            .collect(),
        rails: rail_boxes(frame, rails),
        fiducials,
        fiducial_diameter,
        tooling_holes,
        tooling_hole_diameter,
        bite_holes: Vec::new(),
        bite_diameter: 0,
        tabs: 0,
        v_scores: Vec::new(),
        routes: Vec::new(),
        rectangular: is_box(&ring, board),
    };
    match panel.separation {
        PanelSeparation::VScore => layout.v_scores = v_scores(&grid, frame),
        PanelSeparation::MouseBites => tabs::mouse_bites(&grid, &ring, &panel.bites, &mut layout)?,
    }
    Ok(layout)
}

/// The outline in nm, without a repeated closing vertex.
fn ring(outline: &[Point]) -> Result<Vec<PointNm>, PanelError> {
    let mut ring = outline
        .iter()
        .map(|p| PointNm::from_mm(*p, "outline").map_err(|_| PanelError::TooLarge))
        .collect::<Result<Vec<_>, _>>()?;
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    if ring.len() < 3 {
        return Err(PanelError::NoOutline);
    }
    Ok(ring)
}

fn is_box(ring: &[PointNm], board: RectNm) -> bool {
    ring.len() == 4
        && ring.iter().all(|p| {
            (p.x == board.min.x || p.x == board.max.x) && (p.y == board.min.y || p.y == board.max.y)
        })
}

fn rail_boxes(frame: RectNm, rails: Rails) -> Vec<RectNm> {
    let (lo, hi) = (frame.min, frame.max);
    [
        (rails.top, (lo, PointNm::new(hi.x, lo.y + rails.top))),
        (rails.bottom, (PointNm::new(lo.x, hi.y - rails.bottom), hi)),
        (rails.left, (lo, PointNm::new(lo.x + rails.left, hi.y))),
        (rails.right, (PointNm::new(hi.x - rails.right, lo.y), hi)),
    ]
    .into_iter()
    .filter(|(width, _)| *width > 0)
    .map(|(_, (a, b))| RectNm::new(a, b))
    .collect()
}

/// The fiducials and tooling holes on the top and bottom rails, centred
/// across the rail: a tooling hole half a rail width in from each end, a
/// fiducial a rail width and a half in, on the left of the top rail and at
/// both ends of the bottom one, so the three never look the same turned.
fn marks(
    frame: RectNm,
    rails: Rails,
    fiducials: bool,
    tooling: bool,
) -> (Vec<PointNm>, Vec<PointNm>) {
    let mut dots = Vec::new();
    let mut holes = Vec::new();
    let rows = [
        (rails.top, frame.min.y + rails.top / 2, false),
        (rails.bottom, frame.max.y - rails.bottom / 2, true),
    ];
    for (width, y, both_ends) in rows {
        if width <= 0 {
            continue;
        }
        let inset = width / 2;
        if tooling {
            holes.push(PointNm::new(frame.min.x + inset, y));
            holes.push(PointNm::new(frame.max.x - inset, y));
        }
        if fiducials {
            dots.push(PointNm::new(frame.min.x + 3 * inset, y));
            if both_ends {
                dots.push(PointNm::new(frame.max.x - 3 * inset, y));
            }
        }
    }
    (dots, holes)
}

/// A score line along every copy edge that is not the panel's own edge,
/// across the whole frame; two copies that touch share one.
fn v_scores(grid: &Grid, frame: RectNm) -> Vec<PanelSegment> {
    let mut xs = BTreeSet::new();
    let mut ys = BTreeSet::new();
    for column in 0..grid.columns {
        let x = grid.offset(column, 0).x;
        if grid.facing(column, 0, Side::Left) != Facing::Outside {
            xs.insert(grid.board.min.x + x);
        }
        if grid.facing(column, 0, Side::Right) != Facing::Outside {
            xs.insert(grid.board.max.x + x);
        }
    }
    for row in 0..grid.rows {
        let y = grid.offset(0, row).y;
        if grid.facing(0, row, Side::Top) != Facing::Outside {
            ys.insert(grid.board.min.y + y);
        }
        if grid.facing(0, row, Side::Bottom) != Facing::Outside {
            ys.insert(grid.board.max.y + y);
        }
    }
    let vertical = xs
        .into_iter()
        .map(|x| PanelSegment::new(PointNm::new(x, frame.min.y), PointNm::new(x, frame.max.y)));
    let horizontal = ys
        .into_iter()
        .map(|y| PanelSegment::new(PointNm::new(frame.min.x, y), PointNm::new(frame.max.x, y)));
    vertical.chain(horizontal).collect()
}
