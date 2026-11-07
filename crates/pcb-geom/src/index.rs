//! An R-tree of a board's pads, tracks and vias, and the hit-tests on it.
//!
//! Envelopes are `i128` so every area rstar computes while it builds the
//! tree fits, at any coordinate [`MAX_ABS_NM`](crate::MAX_ABS_NM) allows.

use rstar::{AABB, RTree, RTreeObject};
use signex_types::pcb::{PcbBoard, Point, Segment, Via};
use uuid::Uuid;

use crate::rect::RectNm;
use crate::shape::Shape;
use crate::transform::pad_shape;
use crate::units::{GeomError, PointNm, mm_to_nm};

/// A model item the index holds, by identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemRef {
    Pad { footprint: Uuid, pad: Uuid },
    Track(Uuid),
    Via(Uuid),
}

/// An item and its copper, as stored in the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedItem {
    pub item: ItemRef,
    pub shape: Shape,
    envelope: AABB<[i128; 2]>,
}

impl IndexedItem {
    pub fn new(item: ItemRef, shape: Shape) -> Self {
        IndexedItem {
            item,
            shape,
            envelope: envelope_of(&shape.aabb()),
        }
    }
}

impl RTreeObject for IndexedItem {
    type Envelope = AABB<[i128; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// An item left out of the index because its geometry did not convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejected {
    pub item: ItemRef,
    pub error: GeomError,
}

/// How a marquee selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RectSelect {
    /// The item's copper lies wholly inside the rectangle.
    Inside,
    /// The item's copper shares a point with the rectangle.
    Touching,
}

/// The spatial index of one board.
#[derive(Debug, Default)]
pub struct BoardIndex {
    tree: RTree<IndexedItem>,
    rejected: Vec<Rejected>,
}

impl BoardIndex {
    /// Index every pad, track and via of `board`. An item whose geometry
    /// cannot convert is listed in [`rejected`](Self::rejected), not dropped
    /// silently.
    pub fn build(board: &PcbBoard) -> Self {
        let mut items = Vec::new();
        let mut rejected = Vec::new();
        let mut keep = |item: ItemRef, shape: Result<Shape, GeomError>| match shape {
            Ok(shape) => items.push(IndexedItem::new(item, shape)),
            Err(error) => rejected.push(Rejected { item, error }),
        };
        for footprint in &board.footprints {
            for pad in &footprint.pads {
                let item = ItemRef::Pad {
                    footprint: footprint.uuid,
                    pad: pad.uuid,
                };
                keep(item, pad_shape(footprint, pad));
            }
        }
        for segment in &board.segments {
            keep(ItemRef::Track(segment.uuid), track_shape(segment));
        }
        for via in &board.vias {
            keep(ItemRef::Via(via.uuid), via_shape(via));
        }
        BoardIndex {
            tree: RTree::bulk_load(items),
            rejected,
        }
    }

    /// An index over items already in nm.
    pub fn from_items(items: Vec<IndexedItem>) -> Self {
        BoardIndex {
            tree: RTree::bulk_load(items),
            rejected: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.tree.size()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    pub fn rejected(&self) -> &[Rejected] {
        &self.rejected
    }

    pub fn items(&self) -> impl Iterator<Item = &IndexedItem> {
        self.tree.iter()
    }

    /// Every item whose copper lies within `tolerance` of `p`, sorted by
    /// [`ItemRef`] so the result is the same however the tree was built.
    pub fn hit_point(&self, p: PointNm, tolerance: i64) -> Vec<ItemRef> {
        let query = envelope_of(&RectNm::point(p).inflate(tolerance));
        sorted(
            self.tree
                .locate_in_envelope_intersecting(&query)
                .filter(|entry| entry.shape.contains(p, tolerance))
                .map(|entry| entry.item),
        )
    }

    /// [`hit_point`](Self::hit_point) for a model point and tolerance in mm.
    pub fn hit_point_mm(&self, p: Point, tolerance_mm: f64) -> Result<Vec<ItemRef>, GeomError> {
        let p = PointNm::from_mm(p, "hit point")?;
        Ok(self.hit_point(p, mm_to_nm(tolerance_mm, "hit tolerance")?))
    }

    /// Every item `rect` selects under `mode`, sorted by [`ItemRef`].
    pub fn hit_rect(&self, rect: &RectNm, mode: RectSelect) -> Vec<ItemRef> {
        let candidates = self
            .tree
            .locate_in_envelope_intersecting(&envelope_of(rect));
        sorted(
            candidates
                .filter(|entry| match mode {
                    // A convex outline is inside an axis-aligned rectangle
                    // exactly when its bounding box is.
                    RectSelect::Inside => rect.contains_rect(&entry.shape.aabb()),
                    RectSelect::Touching => entry.shape.touches_rect(rect),
                })
                .map(|entry| entry.item),
        )
    }
}

fn sorted(items: impl Iterator<Item = ItemRef>) -> Vec<ItemRef> {
    let mut items: Vec<ItemRef> = items.collect();
    items.sort_unstable();
    items.dedup();
    items
}

fn envelope_of(rect: &RectNm) -> AABB<[i128; 2]> {
    AABB::from_corners(
        [i128::from(rect.min.x), i128::from(rect.min.y)],
        [i128::from(rect.max.x), i128::from(rect.max.y)],
    )
}

/// A track's copper; a negative width counts as its magnitude.
pub(crate) fn track_shape(segment: &Segment) -> Result<Shape, GeomError> {
    Ok(Shape::Capsule {
        a: PointNm::from_mm(segment.start, "track.start")?,
        b: PointNm::from_mm(segment.end, "track.end")?,
        radius: (mm_to_nm(segment.width, "track.width")? / 2).abs(),
    })
}

/// A via's copper; a negative diameter counts as its magnitude.
pub(crate) fn via_shape(via: &Via) -> Result<Shape, GeomError> {
    Ok(Shape::Disc {
        center: PointNm::from_mm(via.position, "via.position")?,
        radius: (mm_to_nm(via.diameter, "via.diameter")? / 2).abs(),
    })
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
