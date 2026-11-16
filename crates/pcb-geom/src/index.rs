//! An R-tree of a board's pads, tracks and vias (and, for selection, the
//! footprint bodies and the drawn areas), and the hit-tests on it.
//!
//! Envelopes are `i128` so every area rstar computes while it builds the
//! tree fits, at any coordinate [`MAX_ABS_NM`](crate::MAX_ABS_NM) allows.

use std::collections::{BTreeMap, HashSet};

use rstar::{AABB, RTree, RTreeObject};
use signex_types::pcb::{PcbBoard, Point, Segment, Via, Zone};
use uuid::Uuid;

use crate::body::footprint_body;
use crate::free::{arc_is_copper, arc_shapes, dimension_shapes, free_pad_shape, text_shape};
use crate::pick_filter::PickKind;
use crate::rect::RectNm;
use crate::shape::Shape;
use crate::shape_overlap::point_in_polygon;
use crate::transform::pad_shape;
use crate::units::{GeomError, MAX_ABS_NM, PointNm, mm_to_nm};

/// A model item the index holds, by identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemRef {
    Pad {
        footprint: Uuid,
        pad: Uuid,
    },
    Track(Uuid),
    Via(Uuid),
    /// A footprint as one placed part, hit on its body
    /// ([`footprint_body`](crate::footprint_body)). Only the selection index
    /// ([`BoardIndex::build_selectable`]) holds it.
    Footprint(Uuid),
    /// A keepout, solid region or fill drawn on the board (#2383), as
    /// [`PickKind::of_zone`] names it. Only the selection index holds it:
    /// one zero-width capsule per outline edge, plus its area for a click
    /// or a box inside it.
    Zone {
        zone: Uuid,
        kind: PickKind,
    },
    /// A board string (#2373). Only the selection index holds it.
    Text(Uuid),
    /// A free arc (#2373): one capsule per chord. The copper index holds
    /// it on a copper layer; the selection index on any layer.
    Arc(Uuid),
    /// A pad with no footprint (#2373).
    FreePad(Uuid),
    /// A board dimension (#2373): its strokes and the box of its value.
    /// Only the selection index holds it.
    Dimension(Uuid),
}

impl ItemRef {
    /// Whether the index holds the item as several entries that share its
    /// identity: a drawn area's edges, an arc's chords, a dimension's
    /// strokes.
    pub fn is_multi_part(self) -> bool {
        matches!(self, Self::Zone { .. } | Self::Arc(_) | Self::Dimension(_))
    }
}

/// An item and its copper, as stored in the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedItem {
    pub item: ItemRef,
    pub shape: Shape,
    envelope: AABB<[i128; 2]>,
}

impl IndexedItem {
    /// Refuses a shape beyond [`MAX_ABS_NM`], like the model boundary does.
    pub fn new(item: ItemRef, shape: Shape) -> Result<Self, GeomError> {
        shape.check_limits()?;
        Ok(IndexedItem {
            item,
            shape,
            envelope: envelope_of(&shape.aabb()),
        })
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
    /// The drawn areas, for the hits inside them.
    areas: Vec<Area>,
    /// How many entries each arc and dimension has: a box takes one only
    /// when it holds them all.
    parts: BTreeMap<ItemRef, usize>,
}

/// A drawn area's outline: its edges are in the tree, one entry each.
#[derive(Debug, Clone)]
struct Area {
    item: ItemRef,
    ring: Vec<PointNm>,
    bounds: RectNm,
}

impl BoardIndex {
    /// Index every pad, track and via of `board`. An item whose geometry
    /// cannot convert is listed in [`rejected`](Self::rejected), not dropped
    /// silently.
    pub fn build(board: &PcbBoard) -> Self {
        Self::build_with(board, false)
    }

    /// [`build`](Self::build) plus each footprint's body as an
    /// [`ItemRef::Footprint`] and each drawn area as an [`ItemRef::Zone`],
    /// for hover and selection. Connectivity reads
    /// [`build`](Self::build), which holds copper only.
    pub fn build_selectable(board: &PcbBoard) -> Self {
        Self::build_with(board, true)
    }

    fn build_with(board: &PcbBoard, bodies: bool) -> Self {
        let mut items = Vec::new();
        let mut rejected = Vec::new();
        let mut areas = Vec::new();
        let mut keep = |item: ItemRef, shape: Result<Shape, GeomError>| match shape {
            Ok(shape) => match IndexedItem::new(item, shape) {
                Ok(indexed) => items.push(indexed),
                Err(error) => rejected.push(Rejected { item, error }),
            },
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
        for pad in &board.free.pads {
            keep(ItemRef::FreePad(pad.uuid), free_pad_shape(pad));
        }
        let mut parts = |item: ItemRef, shapes: Result<Vec<Shape>, GeomError>| {
            for shape in shapes.unwrap_or_else(|error| {
                rejected.push(Rejected { item, error });
                Vec::new()
            }) {
                match IndexedItem::new(item, shape) {
                    Ok(indexed) => items.push(indexed),
                    Err(error) => rejected.push(Rejected { item, error }),
                }
            }
        };
        for arc in &board.free.arcs {
            if bodies || arc_is_copper(arc) {
                parts(ItemRef::Arc(arc.uuid), arc_shapes(arc));
            }
        }
        if bodies {
            for dimension in &board.free.dimensions {
                parts(
                    ItemRef::Dimension(dimension.uuid),
                    dimension_shapes(dimension),
                );
            }
            for text in &board.texts {
                let shape =
                    text_shape(text).and_then(|s| IndexedItem::new(ItemRef::Text(text.uuid), s));
                match shape {
                    Ok(indexed) => items.push(indexed),
                    Err(error) => rejected.push(Rejected {
                        item: ItemRef::Text(text.uuid),
                        error,
                    }),
                }
            }
            for footprint in &board.footprints {
                let item = ItemRef::Footprint(footprint.uuid);
                match footprint_body(footprint).and_then(|s| IndexedItem::new(item, s)) {
                    Ok(indexed) => items.push(indexed),
                    Err(error) => rejected.push(Rejected { item, error }),
                }
            }
            for zone in &board.zones {
                add_area(zone, &mut areas, &mut items, &mut rejected);
            }
        }
        let mut seen = HashSet::new();
        items.retain(|indexed| {
            // An area's edges, an arc's chords and a dimension's strokes
            // share its identity.
            if indexed.item.is_multi_part() {
                return true;
            }
            let first = seen.insert(indexed.item);
            if !first {
                rejected.push(Rejected {
                    item: indexed.item,
                    error: GeomError::DuplicateItem,
                });
            }
            first
        });
        BoardIndex {
            parts: count_parts(&items),
            tree: RTree::bulk_load(items),
            rejected,
            areas,
        }
    }

    /// An index over items already in nm. Item identity is the caller's
    /// to keep unique: two items with one [`ItemRef`] answer as one hit.
    pub fn from_items(items: Vec<IndexedItem>) -> Self {
        BoardIndex {
            parts: count_parts(&items),
            tree: RTree::bulk_load(items),
            rejected: Vec::new(),
            areas: Vec::new(),
        }
    }

    /// This index without the items `drop` names, its areas kept for the
    /// rest (the selection index on a view that hides layers, #2223).
    pub fn without(self, drop: impl Fn(ItemRef) -> bool) -> Self {
        let items: Vec<IndexedItem> = self
            .tree
            .iter()
            .filter(|i| !drop(i.item))
            .cloned()
            .collect();
        BoardIndex {
            parts: count_parts(&items),
            tree: RTree::bulk_load(items),
            rejected: self.rejected,
            areas: self.areas.into_iter().filter(|a| !drop(a.item)).collect(),
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
    ///
    /// `tolerance` is capped at [`MAX_ABS_NM`] and `p` at four times that,
    /// beyond which nothing on the board can be reached, so no input
    /// overflows.
    pub fn hit_point(&self, p: PointNm, tolerance: i64) -> Vec<ItemRef> {
        sorted(self.items_at(p, tolerance).into_iter().map(|e| e.item))
    }

    /// The indexed items [`hit_point`](Self::hit_point) finds, with their
    /// copper, in no fixed order.
    pub fn items_at(&self, p: PointNm, tolerance: i64) -> Vec<&IndexedItem> {
        let tolerance = tolerance.min(MAX_ABS_NM);
        let far = 4 * MAX_ABS_NM;
        let p = PointNm::new(p.x.clamp(-far, far), p.y.clamp(-far, far));
        let query = envelope_of(&RectNm::point(p).inflate(tolerance));
        let mut hits: Vec<&IndexedItem> = self
            .tree
            .locate_in_envelope_intersecting(&query)
            .filter(|entry| entry.shape.contains(p, tolerance))
            .collect();
        // Inside an area, off its edges: one of its edges answers for it.
        for area in &self.areas {
            if hits.iter().any(|h| h.item == area.item)
                || !area.bounds.contains_point(p)
                || !point_in_polygon(&area.ring, p)
            {
                continue;
            }
            let mut edges = self
                .tree
                .locate_in_envelope_intersecting(&envelope_of(&area.bounds));
            if let Some(edge) = edges.find(|e| e.item == area.item) {
                hits.push(edge);
            }
        }
        hits
    }

    /// [`hit_point`](Self::hit_point) for a model point and tolerance in mm.
    pub fn hit_point_mm(&self, p: Point, tolerance_mm: f64) -> Result<Vec<ItemRef>, GeomError> {
        let p = PointNm::from_mm(p, "hit point")?;
        Ok(self.hit_point(p, mm_to_nm(tolerance_mm, "hit tolerance")?))
    }

    /// The items whose bounding box meets `rect`, in no fixed order: the
    /// broad phase of a pair search, before any shape test.
    pub fn bounds_meeting(&self, rect: &RectNm) -> impl Iterator<Item = &IndexedItem> {
        self.tree
            .locate_in_envelope_intersecting(&envelope_of(rect))
    }

    /// Every item `rect` selects under `mode`, sorted by [`ItemRef`].
    pub fn hit_rect(&self, rect: &RectNm, mode: RectSelect) -> Vec<ItemRef> {
        // Corners are capped at four times MAX_ABS_NM, like `hit_point`'s
        // point: nothing on the board lies beyond, and no input overflows.
        let far = 4 * MAX_ABS_NM;
        let clamp = |p: PointNm| PointNm::new(p.x.clamp(-far, far), p.y.clamp(-far, far));
        let rect = &RectNm::new(clamp(rect.min), clamp(rect.max));
        let candidates = self
            .tree
            .locate_in_envelope_intersecting(&envelope_of(rect));
        let hits = candidates
            .filter(|entry| match mode {
                // A convex outline is inside an axis-aligned rectangle
                // exactly when its bounding box is.
                RectSelect::Inside => rect.contains_rect(&entry.shape.aabb()),
                RectSelect::Touching => entry.shape.touches_rect(rect),
            })
            .map(|entry| entry.item);
        self.with_areas(hits, mode, rect.min)
    }

    /// The items a convex polygon `poly` selects: those wholly inside it, or
    /// those sharing a point with it (a marquee on a turned view, #2316).
    ///
    /// A corner beyond four times [`MAX_ABS_NM`] selects nothing: no drag
    /// reaches there, and clamping a turned box corner by corner, as
    /// [`Self::hit_rect`] does to its two corners, would bend it out of shape.
    pub fn hit_polygon(&self, poly: &[PointNm], mode: RectSelect) -> Vec<ItemRef> {
        let far = 4 * MAX_ABS_NM;
        if poly.iter().any(|p| p.x.abs() > far || p.y.abs() > far) {
            return Vec::new();
        }
        let Some(first) = poly.first() else {
            return Vec::new();
        };
        let bounds = poly.iter().fold(RectNm::new(*first, *first), |r, p| {
            RectNm::new(
                PointNm::new(r.min.x.min(p.x), r.min.y.min(p.y)),
                PointNm::new(r.max.x.max(p.x), r.max.y.max(p.y)),
            )
        });
        let candidates = self
            .tree
            .locate_in_envelope_intersecting(&envelope_of(&bounds));
        let hits = candidates
            .filter(|entry| match mode {
                RectSelect::Inside => entry.shape.is_inside_convex(poly),
                RectSelect::Touching => entry.shape.overlaps_polygon(poly),
            })
            .map(|entry| entry.item);
        self.with_areas(hits, mode, *first)
    }

    /// The items a box selects, from its hits on the tree: an area is inside
    /// only when every edge of it is, and touched too when the box (one of
    /// whose points is `probe`) lies inside it, clear of its edges.
    fn with_areas(
        &self,
        hits: impl Iterator<Item = ItemRef>,
        mode: RectSelect,
        probe: PointNm,
    ) -> Vec<ItemRef> {
        let mut edges: BTreeMap<ItemRef, usize> = BTreeMap::new();
        let mut items = Vec::new();
        for item in hits {
            if item.is_multi_part() {
                *edges.entry(item).or_default() += 1;
            } else {
                items.push(item);
            }
        }
        // An arc or dimension is inside when every part of it is.
        for (&item, &hit) in &edges {
            let Some(&all) = self.parts.get(&item) else {
                continue;
            };
            let selected = match mode {
                RectSelect::Inside => hit == all,
                RectSelect::Touching => hit > 0,
            };
            if selected {
                items.push(item);
            }
        }
        for area in &self.areas {
            let hit = edges.get(&area.item).copied().unwrap_or(0);
            let selected = match mode {
                RectSelect::Inside => hit == area.ring.len(),
                RectSelect::Touching => hit > 0 || point_in_polygon(&area.ring, probe),
            };
            if selected {
                items.push(area.item);
            }
        }
        sorted(items.into_iter())
    }
}

/// Index `zone` as an area when it is a kind the board editor picks: one
/// zero-width capsule per edge and its ring. A zone whose identity another
/// area already has, or whose outline cannot convert, is listed as refused.
fn add_area(
    zone: &Zone,
    areas: &mut Vec<Area>,
    items: &mut Vec<IndexedItem>,
    rejected: &mut Vec<Rejected>,
) {
    let Some(kind) = PickKind::of_zone(zone).filter(|k| k.is_pickable()) else {
        return;
    };
    if zone.outline.len() < 3 {
        return;
    }
    let item = ItemRef::Zone {
        zone: zone.uuid,
        kind,
    };
    if areas
        .iter()
        .any(|a: &Area| matches!(a.item, ItemRef::Zone { zone: id, .. } if id == zone.uuid))
    {
        rejected.push(Rejected {
            item,
            error: GeomError::DuplicateItem,
        });
        return;
    }
    let ring: Result<Vec<PointNm>, GeomError> = zone
        .outline
        .iter()
        .map(|p| PointNm::from_mm(*p, "zone.outline"))
        .collect();
    let edges = ring.and_then(|ring| {
        let edges = (0..ring.len())
            .map(|i| {
                let shape = Shape::Capsule {
                    a: ring[i],
                    b: ring[(i + 1) % ring.len()],
                    radius: 0,
                };
                IndexedItem::new(item, shape)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((ring, edges))
    });
    match edges {
        Ok((ring, edges)) => {
            let Some(bounds) = RectNm::bounding(ring.iter().copied()) else {
                return;
            };
            items.extend(edges);
            areas.push(Area { item, ring, bounds });
        }
        Err(error) => rejected.push(Rejected { item, error }),
    }
}

/// How many entries each multi-part item other than a drawn area has.
fn count_parts(items: &[IndexedItem]) -> BTreeMap<ItemRef, usize> {
    let mut parts = BTreeMap::new();
    for indexed in items {
        if indexed.item.is_multi_part() && !matches!(indexed.item, ItemRef::Zone { .. }) {
            *parts.entry(indexed.item).or_default() += 1;
        }
    }
    parts
}

fn sorted(items: impl Iterator<Item = ItemRef>) -> Vec<ItemRef> {
    let mut items: Vec<ItemRef> = items.collect();
    items.sort_unstable();
    items.dedup();
    items
}

pub(crate) fn envelope_of(rect: &RectNm) -> AABB<[i128; 2]> {
    AABB::from_corners(
        [i128::from(rect.min.x), i128::from(rect.min.y)],
        [i128::from(rect.max.x), i128::from(rect.max.y)],
    )
}

/// A track's copper; a negative width counts as its magnitude.
pub fn track_shape(segment: &Segment) -> Result<Shape, GeomError> {
    Ok(Shape::Capsule {
        a: PointNm::from_mm(segment.start, "track.start")?,
        b: PointNm::from_mm(segment.end, "track.end")?,
        radius: (mm_to_nm(segment.width, "track.width")? / 2).abs(),
    })
}

/// A via's copper; a negative diameter counts as its magnitude.
pub fn via_shape(via: &Via) -> Result<Shape, GeomError> {
    Ok(Shape::Disc {
        center: PointNm::from_mm(via.position, "via.position")?,
        radius: (mm_to_nm(via.diameter, "via.diameter")? / 2).abs(),
    })
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
