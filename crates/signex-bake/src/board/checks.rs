//! Outline checks of a board sketch (mockup 179d): the outline is one closed
//! loop, it does not cross itself, and every opening lies inside it. The
//! checks read the solve the editor already has (no second solve) and walk
//! the outline with the bake's own profile walk, so they see what Bake
//! writes. Each finding names the sketch entities of its place. Rules:
//! `docs/PCB_SKETCH.md`, "Outline checks".

use std::collections::{HashMap, HashSet};

use signex_sketch::baked_kinds::BakedKind;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::geom::{
    Point2, Segment2, SegmentIntersection, point_in_polygon, point_to_segment_distance,
    segment_segment_intersection,
};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_types::pcb::Point;

use super::geometry::{boundary, circle, point};
use super::{BoardBakeError, Context};
use crate::eval_util::{build_ctx, eval_mm};
use crate::profile::{ARC_SAMPLES, trace_closed_profile_entities};
use crate::traced_loops::TracedLoops;

/// How far inside the outline an opening must stay, in mm: a hole that
/// touches the edge within this is still inside.
const EDGE_TOLERANCE_MM: f64 = 1e-6;

/// The outline checks of one solve, one state per check.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineChecks {
    pub closed: CheckState,
    pub no_crossing: CheckState,
    pub openings_inside: CheckState,
    pub inside_corners: CheckState,
}

/// One outline check, in the order Properties lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineCheck {
    /// The outline is one closed loop.
    Closed,
    /// The outline does not cross itself.
    NoCrossing,
    /// Every cutout, keepout and mounting hole lies inside the outline.
    OpeningsInside,
    /// No inside corner is sharper than the router can cut.
    InsideCorners,
}

impl OutlineCheck {
    pub const ALL: [Self; 4] = [
        Self::Closed,
        Self::NoCrossing,
        Self::OpeningsInside,
        Self::InsideCorners,
    ];
}

/// What one check found.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckState {
    Passed,
    Failed(Vec<Finding>),
    NotChecked(NotChecked),
}

/// Why a check did not run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotChecked {
    /// The sketch draws no outline: the board keeps its own.
    NoOutline,
    /// The check needs one closed outline, and the outline is not one.
    NeedsClosedOutline,
    /// A point of the outline has no solved position yet (the solve is
    /// behind an edit).
    Unsolved,
    /// The check needs the router tool diameter, which nothing gives yet.
    NeedsRouterDiameter,
}

/// A failure and its place.
#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    /// The outline stops at an open end. `edges` are the edges whose ends
    /// do not meet; `gap_mm` the distance to the nearest other open end and
    /// `at` the middle of the gap (the end itself when nothing pairs it).
    OpenEnd {
        edges: Vec<SketchEntityId>,
        gap_mm: Option<f64>,
        at: Option<Point>,
    },
    /// The outline forks: a vertex of the loop through `edge` has more than
    /// two edges.
    Branching { edge: SketchEntityId },
    /// The outline cannot be walked through `entity` (a point tagged as
    /// outline, or broken topology).
    Untraceable { entity: SketchEntityId },
    /// The outline is `count` closed loops; `seeds` holds one edge of each.
    SeveralLoops {
        count: usize,
        seeds: Vec<SketchEntityId>,
    },
    /// Two edges of the outline cross at `at`.
    Crossing {
        edges: [SketchEntityId; 2],
        at: Point,
    },
    /// The opening of `kind` traced from `entity` is not inside the outline.
    OpeningOutside {
        entity: SketchEntityId,
        kind: BakedKind,
    },
}

impl Finding {
    /// The sketch entities this finding is about; the first is the one a
    /// click selects.
    pub fn place(&self) -> &[SketchEntityId] {
        match self {
            Self::OpenEnd { edges, .. } => edges,
            Self::SeveralLoops { seeds, .. } => seeds,
            Self::Crossing { edges, .. } => edges,
            Self::Branching { edge } => std::slice::from_ref(edge),
            Self::Untraceable { entity } | Self::OpeningOutside { entity, .. } => {
                std::slice::from_ref(entity)
            }
        }
    }
}

impl OutlineChecks {
    pub fn state(&self, check: OutlineCheck) -> &CheckState {
        match check {
            OutlineCheck::Closed => &self.closed,
            OutlineCheck::NoCrossing => &self.no_crossing,
            OutlineCheck::OpeningsInside => &self.openings_inside,
            OutlineCheck::InsideCorners => &self.inside_corners,
        }
    }

    /// Every failure, check by check.
    pub fn failures(&self) -> impl Iterator<Item = (OutlineCheck, &Finding)> {
        OutlineCheck::ALL.into_iter().flat_map(move |check| {
            let findings: &[Finding] = match self.state(check) {
                CheckState::Failed(findings) => findings,
                _ => &[],
            };
            findings.iter().map(move |finding| (check, finding))
        })
    }
}

/// The checks after a solve attempt: none when the sketch did not solve.
pub fn outline_checks_after(
    sketch: &SketchData,
    solve: Option<&FullSolveOutput>,
) -> Option<OutlineChecks> {
    solve.map(|solve| outline_checks(sketch, solve, &solve.params))
}

/// Check the outline of `sketch` at the positions of `solve`, resolved
/// with `params`.
pub fn outline_checks(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params: &HashMap<String, f64>,
) -> OutlineChecks {
    let ctx = Context {
        sketch,
        solve,
        params,
    };
    let not_checked = |why| CheckState::NotChecked(why);
    let (closed, no_crossing, openings_inside) = match trace_outline(&ctx) {
        Outline::Missing => (
            not_checked(NotChecked::NoOutline),
            not_checked(NotChecked::NoOutline),
            not_checked(NotChecked::NoOutline),
        ),
        Outline::Unsolved => (
            not_checked(NotChecked::Unsolved),
            not_checked(NotChecked::Unsolved),
            not_checked(NotChecked::Unsolved),
        ),
        Outline::Failed(findings) => (
            CheckState::Failed(findings),
            not_checked(NotChecked::NeedsClosedOutline),
            not_checked(NotChecked::NeedsClosedOutline),
        ),
        Outline::Closed(outline) => (
            CheckState::Passed,
            state_of(crossings(&outline)),
            state_of(openings_outside(&ctx, &outline)),
        ),
    };
    OutlineChecks {
        closed,
        no_crossing,
        openings_inside,
        inside_corners: not_checked(NotChecked::NeedsRouterDiameter),
    }
}

fn state_of(findings: Vec<Finding>) -> CheckState {
    if findings.is_empty() {
        CheckState::Passed
    } else {
        CheckState::Failed(findings)
    }
}

/// The outline as the bake sees it.
enum Outline {
    Missing,
    Unsolved,
    Failed(Vec<Finding>),
    Closed(ClosedOutline),
}

/// The one closed outline: its polygon and, per polygon segment (from
/// vertex `i` to `i + 1`), the sketch edge it lies on.
struct ClosedOutline {
    polygon: Vec<Point2>,
    edge_of_segment: Vec<SketchEntityId>,
}

const KIND: BakedKind = BakedKind::BoardOutline;

/// Walk the edges tagged as outline the way `bake_board` does.
fn trace_outline(ctx: &Context<'_>) -> Outline {
    let mut loops = TracedLoops::default();
    let mut closed: Vec<(SketchEntityId, Vec<Point>)> = Vec::new();
    let mut findings = Vec::new();
    let mut open = false;
    let mut tagged = false;
    for entity in ctx.tagged(|e| e.board_outline) {
        tagged = true;
        match boundary(ctx, &mut loops, "outline", entity, KIND) {
            Ok(Some(polygon)) => closed.push((entity.id, polygon)),
            Ok(None) => {}
            Err(BoardBakeError::OpenProfile { .. }) => open = true,
            Err(BoardBakeError::MissingPoint { .. }) => return Outline::Unsolved,
            Err(BoardBakeError::BranchingProfile { edge, .. }) => {
                // Every tagged edge of a forked loop fails the same way.
                if !findings
                    .iter()
                    .any(|f| matches!(f, Finding::Branching { .. }))
                {
                    findings.push(Finding::Branching { edge });
                }
            }
            Err(_) => findings.push(Finding::Untraceable { entity: entity.id }),
        }
    }
    if !tagged {
        return Outline::Missing;
    }
    if open {
        findings.extend(open_ends(ctx));
    }
    if closed.len() > 1 {
        findings.push(Finding::SeveralLoops {
            count: closed.len(),
            seeds: closed.iter().map(|(seed, _)| *seed).collect(),
        });
    }
    match closed.pop() {
        Some((seed, polygon)) if findings.is_empty() => {
            Outline::Closed(ClosedOutline::new(ctx, seed, polygon))
        }
        _ if findings.is_empty() => Outline::Missing,
        _ => Outline::Failed(findings),
    }
}

/// The open ends of the outline: endpoints of outline edges that no other
/// baking edge shares (the walk stops there), paired nearest first.
fn open_ends(ctx: &Context<'_>) -> Vec<Finding> {
    let mut degree: HashMap<SketchEntityId, usize> = HashMap::new();
    for (a, b) in ctx.tagged(|_| true).filter_map(edge_ends) {
        *degree.entry(a).or_default() += 1;
        *degree.entry(b).or_default() += 1;
    }
    let mut ends: Vec<(SketchEntityId, Point)> = Vec::new();
    for entity in ctx.tagged(|e| e.board_outline) {
        let Some((a, b)) = edge_ends(entity) else {
            continue;
        };
        for end in [a, b] {
            if degree.get(&end) == Some(&1)
                && let Ok(at) = point(ctx, end, KIND)
            {
                ends.push((entity.id, at));
            }
        }
    }
    let mut out = Vec::new();
    while !ends.is_empty() {
        let (edge, at) = ends.remove(0);
        let nearest = ends
            .iter()
            .enumerate()
            .map(|(i, (_, other))| (i, distance(at, *other)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, gap)) = nearest else {
            out.push(Finding::OpenEnd {
                edges: vec![edge],
                gap_mm: None,
                at: Some(at),
            });
            break;
        };
        let (other_edge, other) = ends.remove(i);
        let mut edges = vec![edge];
        if other_edge != edge {
            edges.push(other_edge);
        }
        out.push(Finding::OpenEnd {
            edges,
            gap_mm: Some(gap),
            at: Some(Point {
                x: (at.x + other.x) / 2.0,
                y: (at.y + other.y) / 2.0,
            }),
        });
    }
    out
}

/// The two endpoints the profile walk joins an edge by; `None` for points
/// and circles.
fn edge_ends(entity: &Entity) -> Option<(SketchEntityId, SketchEntityId)> {
    match entity.kind {
        EntityKind::Line { start, end } | EntityKind::Arc { start, end, .. } => Some((start, end)),
        EntityKind::Point { .. } | EntityKind::Circle { .. } => None,
    }
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn to_point2(p: Point) -> Point2 {
    Point2::new(p.x, p.y)
}

impl ClosedOutline {
    /// `polygon` traced from `seed`: a line gives one segment, an arc
    /// `ARC_SAMPLES` (the walk's tessellation), a circle all of them.
    fn new(ctx: &Context<'_>, seed: SketchEntityId, polygon: Vec<Point>) -> Self {
        let polygon: Vec<Point2> = polygon.into_iter().map(to_point2).collect();
        let mut edge_of_segment = Vec::with_capacity(polygon.len());
        if let Ok(profile) = trace_closed_profile_entities(ctx.sketch, seed) {
            for edge in &profile.edges {
                let is_arc = ctx
                    .sketch
                    .entities
                    .iter()
                    .any(|e| e.id == edge.id && matches!(e.kind, EntityKind::Arc { .. }));
                let segments = if is_arc { ARC_SAMPLES } else { 1 };
                edge_of_segment.extend(std::iter::repeat_n(edge.id, segments));
            }
        }
        if edge_of_segment.len() != polygon.len() {
            edge_of_segment = vec![seed; polygon.len()];
        }
        Self {
            polygon,
            edge_of_segment,
        }
    }

    fn segment(&self, i: usize) -> Segment2 {
        let n = self.polygon.len();
        Segment2::new(self.polygon[i], self.polygon[(i + 1) % n])
    }

    fn segments(&self) -> impl Iterator<Item = Segment2> + '_ {
        (0..self.polygon.len()).map(|i| self.segment(i))
    }
}

/// Where two edges of the outline cross: one finding per pair of edges.
/// Segments of one edge, and neighbouring segments, never count.
fn crossings(outline: &ClosedOutline) -> Vec<Finding> {
    let n = outline.polygon.len();
    let mut seen: HashSet<(SketchEntityId, SketchEntityId)> = HashSet::new();
    let mut out = Vec::new();
    for i in 0..n {
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let (a, b) = (outline.edge_of_segment[i], outline.edge_of_segment[j]);
            if a == b {
                continue;
            }
            let (p, q) = (outline.segment(i), outline.segment(j));
            if !boxes_touch(p, q) {
                continue;
            }
            let at = match segment_segment_intersection(p, q) {
                SegmentIntersection::None => continue,
                SegmentIntersection::Point { pt, .. } => pt,
                SegmentIntersection::Overlap { from, .. } => from,
            };
            let key = if a.0 <= b.0 { (a, b) } else { (b, a) };
            if seen.insert(key) {
                out.push(Finding::Crossing {
                    edges: [a, b],
                    at: Point { x: at.x, y: at.y },
                });
            }
        }
    }
    out
}

/// Whether the bounding boxes of `p` and `q` overlap: a cheap reject
/// before the exact intersection.
fn boxes_touch(p: Segment2, q: Segment2) -> bool {
    let tol = EDGE_TOLERANCE_MM;
    p.a.x.min(p.b.x) <= q.a.x.max(q.b.x) + tol
        && q.a.x.min(q.b.x) <= p.a.x.max(p.b.x) + tol
        && p.a.y.min(p.b.y) <= q.a.y.max(q.b.y) + tol
        && q.a.y.min(q.b.y) <= p.a.y.max(p.b.y) + tol
}

/// The cutouts, keepouts and mounting holes that are not inside the
/// outline, each named by the entity its bake item takes its id from. An
/// opening that does not bake is left to the bake's own error.
fn openings_outside(ctx: &Context<'_>, outline: &ClosedOutline) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut cutout_loops = TracedLoops::default();
    for entity in ctx.tagged(|e| e.board_cutout.is_some()) {
        let attribute = format!("{:?}", entity.board_cutout);
        let kind = BakedKind::Cutouts;
        if let Ok(Some(polygon)) = boundary(ctx, &mut cutout_loops, &attribute, entity, kind)
            && !polygon_inside(&polygon, outline)
        {
            out.push(Finding::OpeningOutside {
                entity: entity.id,
                kind,
            });
        }
    }
    let mut keepout_loops = TracedLoops::default();
    for entity in ctx.tagged(|e| e.keepout.is_some()) {
        let attribute = format!("{:?}", entity.keepout);
        let kind = BakedKind::Keepouts;
        if let Ok(Some(polygon)) = boundary(ctx, &mut keepout_loops, &attribute, entity, kind)
            && !polygon_inside(&polygon, outline)
        {
            out.push(Finding::OpeningOutside {
                entity: entity.id,
                kind,
            });
        }
    }
    let eval = build_ctx(ctx.sketch, ctx.params);
    for entity in ctx.tagged(|e| e.mounting_hole.is_some()) {
        let kind = BakedKind::MountingHoles;
        let Ok((centre, radius)) = circle(ctx, entity, kind) else {
            continue;
        };
        // The drill, as the bake reads it; the circle when it has none.
        let drill_radius = entity
            .mounting_hole
            .as_ref()
            .and_then(|hole| hole.drill_expr.as_deref())
            .and_then(|expr| eval_mm(expr, &eval).ok())
            .map_or(radius, |drill| drill / 2.0);
        if !disc_inside(centre, drill_radius, outline) {
            out.push(Finding::OpeningOutside {
                entity: entity.id,
                kind,
            });
        }
    }
    out
}

/// Every corner of `polygon` inside the outline and no edge crossing it.
fn polygon_inside(polygon: &[Point], outline: &ClosedOutline) -> bool {
    let points: Vec<Point2> = polygon.iter().copied().map(to_point2).collect();
    if !points
        .iter()
        .all(|&p| point_in_polygon(p, &outline.polygon))
    {
        return false;
    }
    let n = points.len();
    (0..n).all(|i| {
        let side = Segment2::new(points[i], points[(i + 1) % n]);
        outline.segments().all(|edge| {
            !boxes_touch(side, edge)
                || segment_segment_intersection(side, edge) == SegmentIntersection::None
        })
    })
}

/// A disc of `radius` at `centre` inside the outline (touching allowed).
fn disc_inside(centre: Point, radius: f64, outline: &ClosedOutline) -> bool {
    let c = to_point2(centre);
    point_in_polygon(c, &outline.polygon)
        && outline
            .segments()
            .all(|edge| point_to_segment_distance(c, edge.a, edge.b) + EDGE_TOLERANCE_MM >= radius)
}

#[cfg(test)]
#[path = "checks_tests.rs"]
mod tests;
