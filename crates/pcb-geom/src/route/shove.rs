//! Push and shove (#2389): other nets' tracks in a new leg's way move
//! sideways, square to themselves, just far enough to keep the clearance
//! the rules ask. A moved track's ends carry the tracks joined there with
//! them (their far ends stay put), so the net stays connected. What a
//! moved track then crowds is pushed in turn, a few rounds deep. Nothing
//! moves when a track is locked or ends on a pad or via, or a locked track
//! is joined at its end, when same-net copper joins it
//! other than at a shared end (a T-junction, an overlap), when the leg
//! crosses a track, when something else than a track is in the way, or
//! when the moved copper would break a clearance or enter a keepout: the
//! caller then walks around instead. A via among the legs never pushes
//! (a track it crowds stays), but every moved track keeps clear of it.

use std::collections::HashMap;

use uuid::Uuid;

use super::world::{NewCopper, NewKind, RouteWorld};
use crate::copper::CopperItem;
use crate::index::ItemRef;
use crate::shape::{Shape, segments_intersect};
use crate::units::PointNm;

/// How many times a push may cause another.
const ROUNDS: usize = 4;

/// A track's new ends after a push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pushed {
    pub track: Uuid,
    pub start: PointNm,
    pub end: PointNm,
}

/// What pushes in a round: the new legs, then the tracks moved by the
/// round before (with their index, which never pushes itself).
type Pusher = (Option<usize>, CopperItem);

/// What one shove did: the tracks to move ([`shove`]'s answer) and how
/// many rounds of pushing it ran, whether it cleared the way or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShoveRun {
    pub pushed: Option<Vec<Pushed>>,
    /// Rounds that pushed tracks: 0 when nothing was in the way.
    pub rounds: usize,
}

/// The tracks to move so that `legs` keep the clearance, each with its
/// new ends, sorted by id; empty when nothing is in the way. `None` when
/// the way cannot be cleared by pushing.
pub fn shove(world: &RouteWorld, legs: &[NewCopper]) -> Option<Vec<Pushed>> {
    shove_run(world, legs).pushed
}

/// [`shove`], with the rounds of pushing it took (#2483).
pub fn shove_run(world: &RouteWorld, legs: &[NewCopper]) -> ShoveRun {
    let given_up = |rounds| ShoveRun {
        pushed: None,
        rounds,
    };
    if legs.iter().any(|leg| world.in_keepout(leg)) {
        return given_up(0);
    }
    let mut moved: HashMap<usize, Shape> = HashMap::new();
    let mut pushers: Vec<Pusher> = legs.iter().map(|leg| (None, leg.as_item())).collect();
    for round in 0..=ROUNDS {
        let mut blockers: Vec<usize> = pushers
            .iter()
            .flat_map(|(own, item)| world.conflicts(item, &moved, *own))
            .collect();
        blockers.sort_unstable();
        blockers.dedup();
        if blockers.is_empty() {
            return ShoveRun {
                pushed: finish(world, legs, &moved),
                rounds: round,
            };
        }
        let mut next = Vec::new();
        for index in blockers {
            // A track an earlier push this round stretched already moved;
            // `finish` checks where it went.
            if next.contains(&index) {
                continue;
            }
            match push_one(world, index, &pushers, &mut moved) {
                Some(touched) => next.extend(touched),
                None => return given_up(round + 1),
            }
        }
        pushers = next
            .into_iter()
            .map(|index| (Some(index), with_shape(world, index, moved[&index])))
            .collect();
    }
    given_up(ROUNDS + 1)
}

/// Move track `index` clear of `pushers`, stretching the tracks joined at
/// its ends. The indices it moved.
fn push_one(
    world: &RouteWorld,
    index: usize,
    pushers: &[Pusher],
    moved: &mut HashMap<usize, Shape>,
) -> Option<Vec<usize>> {
    let item = &world.items[index];
    if moved.contains_key(&index) || !matches!(item.item, ItemRef::Track(_)) {
        return None;
    }
    let Shape::Capsule { a, b, radius } = item.shape else {
        return None;
    };
    if world.is_locked(index) || world.is_anchored(index, a) || world.is_anchored(index, b) {
        return None;
    }
    let joined_ends = [world.joined_at(index, a), world.joined_at(index, b)];
    if keeps_unmoved_link(world, index, &joined_ends) {
        return None;
    }
    let offset = push_offset(world, item, pushers)?;
    let mut touched = vec![index];
    let ends = [(a, a.plus(offset)), (b, b.plus(offset))];
    for ((end, new_end), joined_here) in ends.into_iter().zip(joined_ends) {
        for joined in joined_here {
            let Shape::Capsule {
                a: ja,
                b: jb,
                radius: jr,
            } = world.items[joined].shape
            else {
                return None;
            };
            if moved.contains_key(&joined) || world.is_locked(joined) {
                return None;
            }
            let (ja, jb) = if ja == end {
                (new_end, jb)
            } else {
                (ja, new_end)
            };
            moved.insert(
                joined,
                Shape::Capsule {
                    a: ja,
                    b: jb,
                    radius: jr,
                },
            );
            touched.push(joined);
        }
    }
    moved.insert(
        index,
        Shape::Capsule {
            a: a.plus(offset),
            b: b.plus(offset),
            radius,
        },
    );
    Some(touched)
}

/// Whether track `index`, or a track joined at one of its ends, is held by
/// same-net copper that would not follow the push: a branch ending on the
/// track's side (a T-junction), or copper overlapping it rather than
/// sharing an end. Moving it would leave that copper disconnected.
fn keeps_unmoved_link(world: &RouteWorld, index: usize, joined_ends: &[Vec<usize>; 2]) -> bool {
    let all: Vec<usize> = joined_ends.iter().flatten().copied().collect();
    if world.is_joined_elsewhere(index, &[], &all) {
        return true;
    }
    // A joined track turns about its far end: what holds that end stays
    // linked, anything else on it would be left behind.
    let Shape::Capsule { a: ta, b: tb, .. } = world.items[index].shape else {
        return true;
    };
    let mut moving = all.clone();
    moving.push(index);
    all.iter().any(|&joined| {
        let Shape::Capsule { a, b, .. } = world.items[joined].shape else {
            return true;
        };
        let far: Vec<PointNm> = [a, b].into_iter().filter(|&p| p != ta && p != tb).collect();
        world.is_joined_elsewhere(joined, &far, &moving)
    })
}

/// How far, square to the track, it must move to clear every pusher it is
/// too close to. `None` when a pusher crosses it or pushers sit on both
/// sides.
fn push_offset(world: &RouteWorld, item: &CopperItem, pushers: &[Pusher]) -> Option<PointNm> {
    let Shape::Capsule { a, b, radius } = item.shape else {
        return None;
    };
    let axis = b.minus(a);
    if axis == PointNm::ORIGIN {
        return None;
    }
    let length = (axis.x as f64).hypot(axis.y as f64);
    let normal = (-axis.y as f64 / length, axis.x as f64 / length);
    let pressing: Vec<&CopperItem> = pushers
        .iter()
        .map(|(_, p)| p)
        .filter(|p| world.too_close(p, item, &item.shape))
        .collect();
    // Whether the pushers sit on the side the normal points to.
    let mut positive: Option<bool> = None;
    let mut distance = 0.0_f64;
    for pusher in &pressing {
        let Shape::Capsule {
            a: pa,
            b: pb,
            radius: pr,
        } = pusher.shape
        else {
            return None;
        };
        if segments_intersect(a, b, pa, pb) {
            return None;
        }
        let (on_track, on_pusher) = item.shape.separation(&pusher.shape).between;
        let toward = on_pusher.minus(on_track);
        let across = toward.x as f64 * normal.0 + toward.y as f64 * normal.1;
        if positive.is_some_and(|held| held != (across >= 0.0)) {
            return None;
        }
        positive = Some(across >= 0.0);
        let want = (radius + pr + world.clearance(pusher, item)) as f64 - across.abs();
        distance = distance.max(want);
    }
    let Some(positive) = positive else {
        return Some(PointNm::ORIGIN);
    };
    // Away from the pushers.
    let side = if positive { -1.0 } else { 1.0 };
    let mut distance = distance.max(0.0).ceil() + 1.0;
    // A pusher at an angle to the track can need a little more than the
    // square-on estimate: grow by what is still missing.
    for _ in 0..8 {
        let offset = PointNm::new(
            (side * normal.0 * distance).round() as i64,
            (side * normal.1 * distance).round() as i64,
        );
        let shape = item.shape.translated(offset);
        let missing = pressing
            .iter()
            .map(|p| shortfall(world, p, item, &shape))
            .max()
            .unwrap_or(0);
        if missing <= 0 {
            return Some(offset);
        }
        distance += missing as f64 + 1.0;
    }
    None
}

/// How much closer than the rule `pusher` still is to `item` at `shape`,
/// in nm; 0 or less when clear.
fn shortfall(world: &RouteWorld, pusher: &CopperItem, item: &CopperItem, shape: &Shape) -> i64 {
    let required = world.clearance(pusher, item);
    if pusher.shape.overlaps(shape) {
        return required.max(1);
    }
    let gap = pusher.shape.separation(shape);
    if gap.is_under(required) {
        (required - gap.distance_nm()).max(1)
    } else {
        0
    }
}

/// Item `index` with `shape` in place of its board copper.
fn with_shape(world: &RouteWorld, index: usize, shape: Shape) -> CopperItem {
    let item = &world.items[index];
    CopperItem {
        item: item.item,
        net: item.net,
        layers: item.layers,
        shape,
        pad_center: None,
    }
}

/// Check every moved track against the board, the other moved tracks, the
/// legs and the keepouts, and list the moves.
fn finish(
    world: &RouteWorld,
    legs: &[NewCopper],
    moved: &HashMap<usize, Shape>,
) -> Option<Vec<Pushed>> {
    let mut pushed = Vec::with_capacity(moved.len());
    for (&index, &shape) in moved {
        let track = with_shape(world, index, shape);
        let as_new = NewCopper {
            kind: NewKind::Track,
            net: track.net,
            layers: track.layers,
            shape,
        };
        let crowds_a_leg = legs
            .iter()
            .any(|leg| world.too_close(&leg.as_item(), &track, &shape));
        if crowds_a_leg
            || world.in_keepout(&as_new)
            || !world.conflicts(&track, moved, Some(index)).is_empty()
        {
            return None;
        }
        let (ItemRef::Track(id), Shape::Capsule { a, b, .. }) = (track.item, shape) else {
            return None;
        };
        pushed.push(Pushed {
            track: id,
            start: a,
            end: b,
        });
    }
    pushed.sort_unstable_by_key(|p| p.track);
    Some(pushed)
}
