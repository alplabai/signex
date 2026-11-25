//! The copper and keepouts a new route is checked against (#2389): the
//! board's copper in an R-tree (copper of no net too: pads, tracks, arcs
//! and vias of net 0, and drawings and strings on copper, #2883, #2515),
//! and the keepouts that forbid tracks
//! or vias. A keepout forbids what the design rule check says it forbids
//! (`signex_drc`, keepout check): tracks for `no_routing` or `no_copper`,
//! vias for `no_vias`, `no_copper` or `no_drilling`, the last on every
//! layer.

use std::collections::{HashMap, HashSet};

use rstar::{AABB, RTree, RTreeObject};
use signex_types::pcb::PcbBoard;
use signex_types::pcb_sketch::PcbKeepout;
use uuid::Uuid;

use crate::copper::{CopperItem, collect_copper, layer_bit, no_net_copper};
use crate::index::{ItemRef, envelope_of};
use crate::rect::RectNm;
use crate::shape::Shape;
use crate::units::PointNm;

/// The clearance in nm two copper items of different nets need where they
/// share copper layers: the design rules' answer for the pair.
pub type ClearanceRule<'a> = dyn Fn(&CopperItem, &CopperItem) -> i64 + 'a;

/// What kind of copper the router adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewKind {
    Track,
    Via,
}

/// Copper the router is about to add, or a track it is moving.
#[derive(Debug, Clone, PartialEq)]
pub struct NewCopper {
    pub kind: NewKind,
    pub net: u32,
    /// A [`layer_bit`] mask.
    pub layers: u32,
    pub shape: Shape,
}

impl NewCopper {
    /// A track from `a` to `b` of half width `radius`.
    pub fn track(a: PointNm, b: PointNm, radius: i64, net: u32, layers: u32) -> Self {
        NewCopper {
            kind: NewKind::Track,
            net,
            layers,
            shape: Shape::Capsule { a, b, radius },
        }
    }

    /// The item the clearance rule is asked about; its id is never read.
    pub(crate) fn as_item(&self) -> CopperItem {
        let item = match self.kind {
            NewKind::Track => ItemRef::Track(Uuid::nil()),
            NewKind::Via => ItemRef::Via(Uuid::nil()),
        };
        CopperItem {
            item,
            net: self.net,
            layers: self.layers,
            shape: self.shape.clone(),
            pad_center: None,
        }
    }
}

/// A keepout in nm with what it forbids new copper.
struct KeepoutArea {
    polygon: Vec<PointNm>,
    bounds: RectNm,
    layers: u32,
    tracks: bool,
    vias: bool,
    drilling: bool,
}

impl KeepoutArea {
    /// `None` for a keepout with no copper layer, no corner, or a corner
    /// the geometry cannot hold.
    fn of(keepout: &PcbKeepout) -> Option<Self> {
        let layers = keepout
            .layers
            .iter()
            .filter_map(|layer| layer.copper())
            .fold(0, |mask, layer| mask | 1 << layer_bit(layer));
        let polygon = keepout
            .boundary
            .iter()
            .map(|&p| PointNm::from_mm(p, "keepout.boundary"))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let bounds = RectNm::bounding(polygon.iter().copied())?;
        let forbids = keepout.forbids;
        (layers != 0).then_some(KeepoutArea {
            polygon,
            bounds,
            layers,
            tracks: forbids.no_routing || forbids.no_copper,
            vias: forbids.no_vias || forbids.no_copper,
            drilling: forbids.no_drilling,
        })
    }

    fn forbids(&self, new: &NewCopper) -> bool {
        let (forbidden, layers) = match new.kind {
            NewKind::Track => (self.tracks, new.layers),
            NewKind::Via if self.drilling => (true, u32::MAX),
            NewKind::Via => (self.vias, new.layers),
        };
        forbidden
            && layers & self.layers != 0
            && new.shape.aabb().intersects(&self.bounds)
            && new.shape.overlaps_polygon(&self.polygon)
    }
}

/// An item's place in the tree.
struct Slot {
    index: usize,
    envelope: AABB<[i128; 2]>,
}

impl RTreeObject for Slot {
    type Envelope = AABB<[i128; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// The copper a route meets, with the rule that says how far from it new
/// copper stays.
pub struct RouteWorld<'a> {
    pub(super) items: Vec<CopperItem>,
    tree: RTree<Slot>,
    keepouts: Vec<KeepoutArea>,
    rule: &'a ClearanceRule<'a>,
    /// How far the broad phase looks: the widest clearance the rules can
    /// ask of any pair.
    reach: i64,
    /// Nets routed together (a differential pair, a bus): none is an
    /// obstacle to another ([`Self::with_kin`]).
    kin: Vec<u32>,
    /// The board's locked tracks: a push never moves one.
    locked_tracks: HashSet<Uuid>,
}

impl<'a> RouteWorld<'a> {
    /// [`Self::with_reach`] looking as far as the widest clearance any item
    /// asks of an item like itself. Rules scoped to a pair of object kinds
    /// need [`Self::with_reach`].
    pub fn new(board: &PcbBoard, rule: &'a ClearanceRule<'a>) -> Self {
        Self::with_reach(board, rule, 0)
    }

    /// The netted copper of `board` (`collect_copper`), its copper of no
    /// net (`no_net_copper`, #2515) and its keepouts, the board sketch's
    /// and those drawn as zones. The broad phase looks
    /// `reach` nm round new copper, or further when an item asks more of an
    /// item like itself; `reach` is the widest clearance the rules can ask
    /// of any pair, as the design rule check reads it.
    pub fn with_reach(board: &PcbBoard, rule: &'a ClearanceRule<'a>, reach: i64) -> Self {
        let mut items = collect_copper(board);
        items.extend(no_net_copper(board));
        let slots = items
            .iter()
            .enumerate()
            .map(|(index, item)| Slot {
                index,
                envelope: envelope_of(&item.shape.aabb()),
            })
            .collect();
        let reach = items
            .iter()
            .map(|item| rule(item, item))
            .fold(reach, i64::max);
        let drawn: Vec<PcbKeepout> = board.zones.iter().filter_map(|z| z.as_keepout()).collect();
        let sketch = board.sketch.iter().flat_map(|s| s.keepouts.iter());
        let keepouts = sketch.chain(&drawn).filter_map(KeepoutArea::of).collect();
        RouteWorld {
            items,
            tree: RTree::bulk_load(slots),
            keepouts,
            rule,
            reach: reach.max(0),
            kin: Vec::new(),
            locked_tracks: board
                .segments
                .iter()
                .filter(|s| s.locked)
                .map(|s| s.uuid)
                .collect(),
        }
    }

    /// This world with `nets` routed together: copper of one of them is no
    /// obstacle to new copper of another, so the centre line of a pair or
    /// a bus walks past its own members' pads. Their gaps to each other are
    /// checked on the member tracks, in a world without kin.
    #[must_use]
    pub fn with_kin(mut self, nets: &[u32]) -> Self {
        self.kin = nets.to_vec();
        self
    }

    /// Whether copper of nets `a` and `b` never needs a clearance: one net,
    /// or two nets routed together.
    fn is_kin(&self, a: u32, b: u32) -> bool {
        a == b || (self.kin.contains(&a) && self.kin.contains(&b))
    }

    /// Whether `new` keeps the clearance from every other net's copper and
    /// stays out of every keepout that forbids it.
    pub fn is_clear(&self, new: &NewCopper) -> bool {
        !self.in_keepout(new)
            && self
                .conflicts(&new.as_item(), &HashMap::new(), None)
                .is_empty()
    }

    pub(super) fn in_keepout(&self, new: &NewCopper) -> bool {
        self.keepouts.iter().any(|k| k.forbids(new))
    }

    /// The clearance `a` and `b` need, by the caller's rule.
    pub(super) fn clearance(&self, a: &CopperItem, b: &CopperItem) -> i64 {
        (self.rule)(a, b)
    }

    /// Whether `a` and the copper `b_shape` of item `b` come closer than
    /// the rule asks (touching included); false for one net, kin nets or no
    /// shared layer.
    pub(super) fn too_close(&self, a: &CopperItem, b: &CopperItem, b_shape: &Shape) -> bool {
        if self.is_kin(a.net, b.net) || a.layers & b.layers == 0 {
            return false;
        }
        let required = self.clearance(a, b);
        a.shape.overlaps(b_shape) || a.shape.separation(b_shape).is_under(required)
    }

    /// The items (by index) `candidate` comes too close to. `moved` holds
    /// the new copper of tracks being pushed, read in place of their
    /// board copper; `own` is the candidate's own index, never its own
    /// conflict.
    pub(super) fn conflicts(
        &self,
        candidate: &CopperItem,
        moved: &HashMap<usize, Shape>,
        own: Option<usize>,
    ) -> Vec<usize> {
        let reach = self.clearance(candidate, candidate).max(self.reach);
        let near = candidate.shape.aabb().inflate(reach);
        let mut found: Vec<usize> = self
            .tree
            .locate_in_envelope_intersecting(&envelope_of(&near))
            .map(|slot| slot.index)
            .filter(|index| !moved.contains_key(index))
            .filter(|&index| {
                let other = &self.items[index];
                Some(index) != own && self.too_close(candidate, other, &other.shape)
            })
            .collect();
        found.extend(moved.iter().filter_map(|(&index, shape)| {
            let other = &self.items[index];
            let hit = Some(index) != own
                && shape.aabb().intersects(&near)
                && self.too_close(candidate, other, shape);
            hit.then_some(index)
        }));
        found.sort_unstable();
        found.dedup();
        found
    }

    /// The tracks of item `index`'s net and layers that end at `end`,
    /// other than the item itself: what moves with that end when it moves.
    pub(super) fn joined_at(&self, index: usize, end: PointNm) -> Vec<usize> {
        let item = &self.items[index];
        let mut found: Vec<usize> = self
            .tree
            .locate_in_envelope_intersecting(&envelope_of(&RectNm::point(end)))
            .map(|slot| slot.index)
            .filter(|&other| {
                let o = &self.items[other];
                other != index
                    && o.net == item.net
                    && o.layers == item.layers
                    && matches!(o.item, ItemRef::Track(_))
                    && matches!(o.shape, Shape::Capsule { a, b, .. } if a == end || b == end)
            })
            .collect();
        found.sort_unstable();
        found
    }

    /// Whether item `index` is a locked track: one a push cannot move or
    /// stretch, so the route walks around it instead.
    pub(super) fn is_locked(&self, index: usize) -> bool {
        matches!(self.items[index].item, ItemRef::Track(id) if self.locked_tracks.contains(&id))
    }

    /// Whether `end` of item `index` sits on a pad or via of its net: an
    /// end that cannot move.
    pub(super) fn is_anchored(&self, index: usize, end: PointNm) -> bool {
        let item = &self.items[index];
        self.tree
            .locate_in_envelope_intersecting(&envelope_of(&RectNm::point(end)))
            .any(|slot| {
                let o = &self.items[slot.index];
                slot.index != index
                    && o.net == item.net
                    && o.layers & item.layers != 0
                    && !matches!(o.item, ItemRef::Track(_))
                    && o.shape.contains(end, 0)
            })
    }

    /// Whether copper of item `index`'s net on its layers touches it other
    /// than the items in `moving` (which move with it) and copper holding
    /// one of the `kept` points (which stay where they are): a connection a
    /// push would break, a T-junction or an overlap.
    pub(super) fn is_joined_elsewhere(
        &self,
        index: usize,
        kept: &[PointNm],
        moving: &[usize],
    ) -> bool {
        let item = &self.items[index];
        self.tree
            .locate_in_envelope_intersecting(&envelope_of(&item.shape.aabb()))
            .map(|slot| slot.index)
            .any(|other| {
                let o = &self.items[other];
                other != index
                    && !moving.contains(&other)
                    && o.net == item.net
                    && o.layers & item.layers != 0
                    && o.shape.overlaps(&item.shape)
                    && !kept.iter().any(|&p| o.shape.contains(p, 0))
            })
    }

    /// Whether the track or via `id` comes closer than the rule to another
    /// net's copper, or sits in a keepout that forbids it. False for an
    /// item this world does not hold.
    pub fn is_crowded(&self, id: ItemRef) -> bool {
        let Some(index) = self.items.iter().position(|c| c.item == id) else {
            return false;
        };
        let item = &self.items[index];
        let kind = match item.item {
            ItemRef::Via(_) => NewKind::Via,
            _ => NewKind::Track,
        };
        let new = NewCopper {
            kind,
            net: item.net,
            layers: item.layers,
            shape: item.shape.clone(),
        };
        self.in_keepout(&new)
            || !self
                .conflicts(item, &HashMap::new(), Some(index))
                .is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::test_board::{at, board, nm, pad_at, track_at};

    const CLEARANCE: i64 = 200_000;

    fn rule(_: &CopperItem, _: &CopperItem) -> i64 {
        CLEARANCE
    }

    #[test]
    fn new_copper_keeps_the_rule_clearance_from_other_nets_only() {
        let board = board(vec![pad_at(1, 5.0, 0.0, 1.0, 2)], vec![]);
        let world = RouteWorld::new(&board, &rule);
        let near = |y: f64, net| NewCopper::track(at(0.0, y), at(10.0, y), nm(0.15), net, 1);
        // The pad's edge is at y = 0.5: a track edge 0.2 mm off it is legal.
        assert!(world.is_clear(&near(0.85, 1)), "exactly at clearance");
        assert!(!world.is_clear(&near(0.84, 1)), "closer than the rule");
        assert!(world.is_clear(&near(0.0, 2)), "its own net is no obstacle");
        let bottom = NewCopper {
            layers: 1 << 31,
            ..near(0.0, 1)
        };
        assert!(world.is_clear(&bottom), "another layer is no obstacle");
    }

    /// #2389: the nets of a pair or a bus are no obstacle to each other's
    /// new copper; every other net still is.
    #[test]
    fn kin_nets_are_no_obstacle_to_each_other() {
        let board = board(
            vec![pad_at(1, 5.0, 0.0, 1.0, 2), pad_at(2, 5.0, 3.0, 1.0, 4)],
            vec![],
        );
        let through = |y: f64| NewCopper::track(at(0.0, y), at(10.0, y), nm(0.15), 3, 1);
        let plain = RouteWorld::new(&board, &rule);
        assert!(!plain.is_clear(&through(0.0)), "net 2's pad is in the way");
        let pair = RouteWorld::new(&board, &rule).with_kin(&[2, 3]);
        assert!(pair.is_clear(&through(0.0)), "net 2 is routed with net 3");
        assert!(!pair.is_clear(&through(3.0)), "net 4 is not kin");
        let unrelated = NewCopper::track(at(0.0, 0.0), at(10.0, 0.0), nm(0.15), 5, 1);
        assert!(
            !pair.is_clear(&unrelated),
            "kin covers the routed nets only"
        );
    }

    /// #2515: a pad and a track of no net are obstacles to new copper of
    /// every net.
    #[test]
    fn copper_of_no_net_is_an_obstacle_to_every_net() {
        let mut unnetted = pad_at(1, 5.0, 0.0, 1.0, 0);
        unnetted.pads[0].net = None;
        let board = board(
            vec![unnetted],
            vec![track_at(1, (0.0, 3.0), (10.0, 3.0), 0)],
        );
        let world = RouteWorld::new(&board, &rule);
        let through = |y: f64, net| NewCopper::track(at(0.0, y), at(10.0, y), nm(0.15), net, 1);
        for net in [1, 2] {
            assert!(!world.is_clear(&through(0.0, net)), "the pad, net {net}");
            assert!(!world.is_clear(&through(3.0, net)), "the track, net {net}");
            assert!(world.is_clear(&through(1.5, net)), "between, net {net}");
        }
    }

    #[test]
    fn a_track_end_on_a_pad_is_anchored_and_one_on_a_track_joins_it() {
        let board = board(
            vec![pad_at(1, 0.0, 0.0, 1.0, 2)],
            vec![
                track_at(1, (0.0, 0.0), (5.0, 0.0), 2),
                track_at(2, (5.0, 0.0), (5.0, 5.0), 2),
            ],
        );
        let world = RouteWorld::new(&board, &rule);
        let index = |id: u128| {
            world
                .items
                .iter()
                .position(|c| c.item == ItemRef::Track(Uuid::from_u128(id)))
                .unwrap()
        };
        let (first, second) = (index(1), index(2));
        assert!(world.is_anchored(first, PointNm::new(0, 0)));
        assert!(!world.is_anchored(first, PointNm::new(5_000_000, 0)));
        assert_eq!(
            world.joined_at(first, PointNm::new(5_000_000, 0)),
            vec![second]
        );
    }
}
