//! Walkaround (#2389): a leg from the anchor to the pointer that keeps the
//! clearance from other nets' copper. The legs the corner rule draws are
//! tried first, as they are, then with the bend flipped; when both are
//! blocked, an A* search on a lattice anchored at the start finds a way
//! round, in the rule's directions (axis-aligned for 90°, axis and 45°
//! otherwise, turning 45° at most in the 45° modes), and joins the target
//! with the rule's legs from the first lattice point that reaches it clear.
//! Any-angle routes are pulled taut.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use signex_types::corner::{CornerMode, CornerRule};

use super::world::{NewCopper, RouteWorld};
use crate::rect::RectNm;
use crate::units::PointNm;

/// How a leg is searched.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Walk {
    pub net: u32,
    /// The leg's copper layer, a `layer_bit` mask.
    pub layers: u32,
    /// Half the track width.
    pub radius: i64,
    pub rule: CornerRule,
    /// The lattice step of the search.
    pub step: i64,
    /// Lattice points the search may expand before it gives up.
    pub budget: usize,
}

/// The corners of the rule's legs from `from` to `to`, `to` included and
/// `from` not; empty for the same point. `None` for a corner the geometry
/// cannot hold.
pub fn rule_legs(rule: CornerRule, from: PointNm, to: PointNm) -> Option<Vec<PointNm>> {
    if from == to {
        return Some(Vec::new());
    }
    let legs = rule.legs(from.to_mm(), to.to_mm());
    if legs.is_empty() {
        // Closer than the rule's tolerance: one straight leg.
        return Some(vec![to]);
    }
    let mut corners = legs
        .into_iter()
        .map(|(_, end)| PointNm::from_mm(end, "route corner"))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    // The last corner is the target itself, to the nm.
    if let Some(last) = corners.last_mut() {
        *last = to;
    }
    Some(corners)
}

/// A path from `from` to `to`, both included, whose legs keep the
/// clearance; `None` when the search finds none within its budget.
pub fn walkaround(
    world: &RouteWorld,
    from: PointNm,
    to: PointNm,
    walk: &Walk,
) -> Option<Vec<PointNm>> {
    if let Some(path) = direct(world, walk, from, to) {
        return Some(path);
    }
    // An end inside another net's clearance has no way to it: say so
    // before the search spends its budget.
    if !is_clear(world, walk, from, from) || !is_clear(world, walk, to, to) {
        return None;
    }
    let path = lattice_search(world, walk, from, to)?;
    let path = merge_straight(path);
    Some(if walk.rule.mode == CornerMode::FreeAngle {
        pull_taut(world, walk, path)
    } else {
        path
    })
}

/// The rule's legs, then the flipped rule's, when they are clear.
fn direct(world: &RouteWorld, walk: &Walk, from: PointNm, to: PointNm) -> Option<Vec<PointNm>> {
    [walk.rule, walk.rule.flipped()]
        .into_iter()
        .find_map(|rule| {
            let corners = rule_legs(rule, from, to)?;
            let mut path = vec![from];
            path.extend(corners);
            path.windows(2)
                .all(|leg| is_clear(world, walk, leg[0], leg[1]))
                .then_some(path)
        })
}

fn is_clear(world: &RouteWorld, walk: &Walk, a: PointNm, b: PointNm) -> bool {
    world.is_clear(&NewCopper::track(a, b, walk.radius, walk.net, walk.layers))
}

/// The lattice moves: axis-aligned first, then the diagonals.
const MOVES: [(i64, i64); 8] = [
    (1, 0),
    (0, 1),
    (-1, 0),
    (0, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
    (1, -1),
];

/// How often the search tries to join the target from a point farther
/// than the nearest tried: one expansion in this many.
const JOIN_EVERY: usize = 16;

/// The direction a lattice point was reached in; `START` for the anchor.
const START: u8 = 8;

/// A lattice point and the move that reached it.
type State = (i64, i64, u8);

fn lattice_search(
    world: &RouteWorld,
    walk: &Walk,
    from: PointNm,
    to: PointNm,
) -> Option<Vec<PointNm>> {
    let step = walk.step.max(1);
    let moves: &[(i64, i64)] = if walk.rule.mode == CornerMode::Ortho90 {
        &MOVES[..4]
    } else {
        &MOVES
    };
    let span = (to.x - from.x).abs().max((to.y - from.y).abs());
    let window = RectNm::new(from, to).inflate(span.max(16 * step));
    let point = |(i, j): (i64, i64)| PointNm::new(from.x + i * step, from.y + j * step);
    let diagonal = (step as f64 * std::f64::consts::SQRT_2).round() as i64;
    let ortho = walk.rule.mode == CornerMode::Ortho90;
    let estimate = |p: PointNm| {
        let (dx, dy) = ((to.x - p.x).abs(), (to.y - p.y).abs());
        if ortho {
            dx + dy
        } else {
            let (long, short) = (dx.max(dy), dx.min(dy));
            long - short + (short as f64 * std::f64::consts::SQRT_2) as i64
        }
    };
    // In a 45° mode a corner turns 45° at most (#2372).
    let max_45 = matches!(
        walk.rule.mode,
        CornerMode::Angle45Start | CornerMode::Angle45End
    );
    let start: State = (0, 0, START);
    let mut best: HashMap<State, i64> = HashMap::from([(start, 0)]);
    let mut parent: HashMap<State, State> = HashMap::new();
    let mut open = BinaryHeap::from([Reverse((estimate(from), 0_i64, start))]);
    let mut expanded = 0_usize;
    let mut nearest = i64::MAX;
    while let Some(Reverse((_, cost, state))) = open.pop() {
        if best.get(&state).is_some_and(|&held| cost > held) {
            continue;
        }
        expanded += 1;
        if expanded > walk.budget {
            return None;
        }
        let here = point((state.0, state.1));
        // Joining the target costs a few clearance checks: try it from a
        // point no farther than any tried yet, and now and then otherwise.
        let left = estimate(here);
        let try_join = left <= nearest || expanded % JOIN_EVERY == 0;
        nearest = nearest.min(left);
        if state != start
            && try_join
            && let Some(tail) = direct(world, walk, here, to)
            && (!max_45 || joins_45_at_most(MOVES[usize::from(state.2)], here, &tail))
        {
            let mut path: Vec<PointNm> = trace(&parent, state).into_iter().map(point).collect();
            path.extend(tail.into_iter().skip(1));
            return Some(path);
        }
        for (index, &(dx, dy)) in moves.iter().enumerate() {
            let next: State = (state.0 + dx, state.1 + dy, index as u8);
            let there = point((next.0, next.1));
            if !window.contains_point(there) {
                continue;
            }
            if max_45
                && state.2 != START
                && !turns_45_at_most(MOVES[usize::from(state.2)], (dx, dy))
            {
                continue;
            }
            let length = if dx != 0 && dy != 0 { diagonal } else { step };
            let turn = if state.2 != START && state.2 != next.2 {
                step / 2
            } else {
                0
            };
            let next_cost = cost + length + turn;
            if best.get(&next).is_some_and(|&held| held <= next_cost) {
                continue;
            }
            if !is_clear(world, walk, here, there) {
                continue;
            }
            best.insert(next, next_cost);
            parent.insert(next, state);
            open.push(Reverse((next_cost + estimate(there), next_cost, next)));
        }
    }
    None
}

/// Whether a run going `along` (a lattice move) may turn into `next`: by
/// 45° at most, which for the lattice's eight directions is a positive dot
/// product.
fn turns_45_at_most(along: (i64, i64), next: (i64, i64)) -> bool {
    along.0 * next.0 + along.1 * next.1 > 0
}

/// Whether the path `tail` (starting at `here`) leaves `here` turning 45°
/// at most from a run going `along`.
fn joins_45_at_most(along: (i64, i64), here: PointNm, tail: &[PointNm]) -> bool {
    tail.get(1).is_none_or(|&next| {
        let d = next.minus(here);
        turns_45_at_most(along, (d.x.signum(), d.y.signum()))
    })
}

/// The lattice points from the anchor to `state`.
fn trace(parent: &HashMap<State, State>, mut state: State) -> Vec<(i64, i64)> {
    let mut cells = vec![(state.0, state.1)];
    while let Some(&previous) = parent.get(&state) {
        cells.push((previous.0, previous.1));
        state = previous;
    }
    cells.reverse();
    cells
}

/// The path without repeated points or corners on a straight run. A run
/// merged this way covers exactly the copper of its parts, so it stays
/// clear.
pub(super) fn merge_straight(points: Vec<PointNm>) -> Vec<PointNm> {
    let mut out: Vec<PointNm> = Vec::with_capacity(points.len());
    for p in points {
        if out.last() == Some(&p) {
            continue;
        }
        if let [.., a, b] = out[..] {
            let (ab, bp) = (b.minus(a), p.minus(b));
            if ab.cross(bp) == 0 && ab.dot(bp) > 0 {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// Each corner joined straight to the farthest later one it reaches clear.
fn pull_taut(world: &RouteWorld, walk: &Walk, points: Vec<PointNm>) -> Vec<PointNm> {
    let mut out = Vec::with_capacity(points.len());
    let mut i = 0;
    out.extend(points.first().copied());
    while i + 1 < points.len() {
        let mut j = points.len() - 1;
        while j > i + 1 && !is_clear(world, walk, points[i], points[j]) {
            j -= 1;
        }
        out.push(points[j]);
        i = j;
    }
    out
}
