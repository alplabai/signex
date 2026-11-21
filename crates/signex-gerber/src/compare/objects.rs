//! Pairing the objects of one package layer with the board's, and the
//! differences that are left. Two objects pair when they are the same kind
//! (flash, line, arc, region, hole) at the same place, each placing point
//! and the box of their shapes within the tolerance on each axis. Only
//! dark objects pair: clear ones cut out of a region and are drawn, not
//! compared. See `docs/GERBER_VIEWER.md`, "Compare with board".

use std::collections::BTreeMap;

use pcb_geom::{PointNm, RectNm, Shape};

use crate::package::Package;
use crate::{FileFunction, Image, Object, ObjectKind, Polarity, SourceLine};

use super::{Difference, DifferenceKind};

/// One package layer and what it is compared with.
pub(super) struct LayerInput<'a> {
    pub layer: usize,
    pub board: &'a FileFunction,
    pub package: &'a Image,
    /// The board's image of the layer; `None` when the board draws nothing
    /// there.
    pub reference: Option<&'a Image>,
    pub tolerance: i64,
    /// The board keep-outs of this copper layer, in the file frame.
    pub keepouts: &'a [Vec<PointNm>],
    pub pins: &'a [Pin],
}

/// A board pad's net and pin at a point. Holes carry no X2 attributes, so
/// a hole row names the pad flashed where it is drilled.
pub(super) struct Pin {
    at: PointNm,
    net: Option<String>,
    pin: Option<(String, String)>,
}

/// Every flash with a pin on the board's copper.
pub(super) fn pins_of(reference: &Package) -> Vec<Pin> {
    reference
        .layers
        .iter()
        .filter(|layer| layer.function.as_ref().is_some_and(FileFunction::is_copper))
        .flat_map(|layer| layer.image.objects.iter())
        .filter_map(|object| match object.kind {
            ObjectKind::Flash { at } if object.attributes.pin.is_some() => Some(Pin {
                at,
                net: object.attributes.net.clone(),
                pin: object.attributes.pin.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// The one row of a package layer with no board layer.
pub(super) fn unmatched(layer: usize, image: &Image) -> Difference {
    let extent = image.bounds();
    Difference {
        kind: DifferenceKind::UnmatchedLayer,
        layer,
        board_layer: None,
        at: extent.map_or(PointNm::ORIGIN, |(min, max)| middle(min, max)),
        extent,
        net: None,
        pin: None,
        size_in_file_nm: None,
        size_on_board_nm: None,
        inside_keepout: false,
        source: None,
    }
}

/// The differences of one matched layer, appended to `out`.
pub(super) fn compare_layer(input: &LayerInput<'_>, out: &mut Vec<Difference>) {
    let tolerance = input.tolerance;
    let (unreadable, readable): (Vec<&Object>, Vec<&Object>) = input
        .package
        .objects
        .iter()
        .partition(|object| object.shapes.is_empty());
    let board: Vec<&Object> = input
        .reference
        .map(|image| {
            image
                .objects
                .iter()
                .filter(|object| object.polarity == Polarity::Dark && !object.shapes.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let index = Index::of(&board, tolerance);
    let mut used = vec![false; board.len()];
    for object in readable.iter().filter(|o| o.polarity == Polarity::Dark) {
        let partner = index
            .candidates(object)
            .into_iter()
            .find(|&i| !used[i] && same_object(object, board[i], tolerance));
        match partner {
            Some(i) => {
                used[i] = true;
                out.extend(size_mismatch(input, object, board[i]));
            }
            None => out.push(extra(input, object)),
        }
    }
    // An object the reader could not build stands for the board object at
    // its place: that one is not missing, the row says Not readable.
    for object in &unreadable {
        if object.polarity == Polarity::Dark {
            let partner = index
                .candidates(object)
                .into_iter()
                .find(|&i| !used[i] && same_place(&object.kind, &board[i].kind, tolerance));
            if let Some(i) = partner {
                used[i] = true;
            }
        }
        out.push(row(
            DifferenceKind::NotReadable,
            input,
            object,
            Some(object.at.clone()),
        ));
    }
    for (object, used) in board.iter().zip(used) {
        if !used {
            let mut missing = row(DifferenceKind::MissingCopper, input, object, None);
            missing.size_on_board_nm = hole_size(object);
            out.push(missing);
        }
    }
}

/// An object of the package with no board partner.
fn extra(input: &LayerInput<'_>, object: &Object) -> Difference {
    let mut extra = row(
        DifferenceKind::ExtraCopper,
        input,
        object,
        Some(object.at.clone()),
    );
    extra.size_in_file_nm = hole_size(object);
    extra.inside_keepout = object.shapes.iter().any(|shape| {
        input
            .keepouts
            .iter()
            .any(|ring| shape.overlaps_polygon(ring))
    });
    extra
}

/// A paired hole whose sizes differ by more than the tolerance.
fn size_mismatch(input: &LayerInput<'_>, file: &Object, board: &Object) -> Option<Difference> {
    let (in_file, on_board) = (hole_size(file)?, hole_size(board)?);
    if (in_file - on_board).abs() <= input.tolerance {
        return None;
    }
    let mut mismatch = row(
        DifferenceKind::DrillSizeMismatch,
        input,
        file,
        Some(file.at.clone()),
    );
    mismatch.size_in_file_nm = Some(in_file);
    mismatch.size_on_board_nm = Some(on_board);
    Some(mismatch)
}

fn hole_size(object: &Object) -> Option<i64> {
    match object.kind {
        ObjectKind::Hole { diameter, .. } => diameter,
        _ => None,
    }
}

/// A difference about `object`: at the middle of its shapes (or of its
/// placing points when it has none), with the net and pin it carries, or
/// for a hole the pad's at the same place.
fn row(
    kind: DifferenceKind,
    input: &LayerInput<'_>,
    object: &Object,
    source: Option<SourceLine>,
) -> Difference {
    let extent = extent(object).map(|r| (r.min, r.max));
    let placed = RectNm::bounding(anchors(&object.kind));
    let at = extent
        .or(placed.map(|r| (r.min, r.max)))
        .map_or(PointNm::ORIGIN, |(min, max)| middle(min, max));
    let mut net = object.attributes.net.clone();
    let mut pin = object.attributes.pin.clone();
    if let ObjectKind::Hole { at: hole, .. } = object.kind
        && net.is_none()
        && pin.is_none()
        && let Some(pad) = input
            .pins
            .iter()
            .find(|pad| near(pad.at, hole, input.tolerance))
    {
        net = pad.net.clone();
        pin = pad.pin.clone();
    }
    Difference {
        kind,
        layer: input.layer,
        board_layer: Some(input.board.clone()),
        at,
        extent,
        net,
        pin,
        size_in_file_nm: None,
        size_on_board_nm: None,
        inside_keepout: false,
        source,
    }
}

fn middle(min: PointNm, max: PointNm) -> PointNm {
    PointNm::new(min.x + (max.x - min.x) / 2, min.y + (max.y - min.y) / 2)
}

/// What an object is, for pairing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    Flash,
    Line,
    Arc,
    Region,
    Hole,
}

fn class(kind: &ObjectKind) -> Class {
    match kind {
        ObjectKind::Flash { .. } => Class::Flash,
        ObjectKind::Line { .. } => Class::Line,
        ObjectKind::Arc { .. } => Class::Arc,
        ObjectKind::Region { .. } => Class::Region,
        ObjectKind::Hole { .. } => Class::Hole,
    }
}

/// The points that place an object.
fn anchors(kind: &ObjectKind) -> Vec<PointNm> {
    match kind {
        ObjectKind::Flash { at } => vec![*at],
        ObjectKind::Line { from, to } => vec![*from, *to],
        ObjectKind::Arc {
            from, to, center, ..
        } => vec![*from, *to, *center],
        ObjectKind::Region { outline } => outline.clone(),
        ObjectKind::Hole { at, slot_to, .. } => std::iter::once(*at).chain(*slot_to).collect(),
    }
}

/// The box of an object's shapes; `None` for none.
fn extent(object: &Object) -> Option<RectNm> {
    object
        .shapes
        .iter()
        .map(Shape::aabb)
        .reduce(|a, b| a.union(&b))
}

/// Within `tolerance` of each other on each axis.
fn near(a: PointNm, b: PointNm, tolerance: i64) -> bool {
    (a.x - b.x).abs() <= tolerance && (a.y - b.y).abs() <= tolerance
}

fn boxes_near(a: Option<RectNm>, b: Option<RectNm>, tolerance: i64) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => near(a.min, b.min, tolerance) && near(a.max, b.max, tolerance),
        (None, None) => true,
        _ => false,
    }
}

/// The two ends of a line or arc, in either order.
fn ends_near(a: (PointNm, PointNm), b: (PointNm, PointNm), tolerance: i64) -> bool {
    (near(a.0, b.0, tolerance) && near(a.1, b.1, tolerance))
        || (near(a.0, b.1, tolerance) && near(a.1, b.0, tolerance))
}

/// The same kind of object at the same place: each placing point within
/// the tolerance (a region by the box of its outline).
fn same_place(a: &ObjectKind, b: &ObjectKind, tolerance: i64) -> bool {
    use ObjectKind as K;
    match (a, b) {
        (K::Flash { at: p }, K::Flash { at: q }) => near(*p, *q, tolerance),
        (K::Line { from: f, to: t }, K::Line { from: g, to: u }) => {
            ends_near((*f, *t), (*g, *u), tolerance)
        }
        (
            K::Arc {
                from: f,
                to: t,
                center: c,
                ..
            },
            K::Arc {
                from: g,
                to: u,
                center: d,
                ..
            },
        ) => near(*c, *d, tolerance) && ends_near((*f, *t), (*g, *u), tolerance),
        (K::Region { outline: p }, K::Region { outline: q }) => boxes_near(
            RectNm::bounding(p.iter().copied()),
            RectNm::bounding(q.iter().copied()),
            tolerance,
        ),
        (
            K::Hole {
                at: p, slot_to: s, ..
            },
            K::Hole {
                at: q, slot_to: r, ..
            },
        ) => {
            near(*p, *q, tolerance)
                && match (s, r) {
                    (None, None) => true,
                    (Some(s), Some(r)) => near(*s, *r, tolerance),
                    _ => false,
                }
        }
        _ => false,
    }
}

/// The same object: the same place and, but for a hole (whose size is
/// its own row), the box of the shapes within the tolerance.
fn same_object(a: &Object, b: &Object, tolerance: i64) -> bool {
    same_place(&a.kind, &b.kind, tolerance)
        && (class(&a.kind) == Class::Hole || boxes_near(extent(a), extent(b), tolerance))
}

/// The board objects by kind and by a grid of cells as wide as the
/// tolerance, so an object looks only at its own and the eight next cells.
struct Index {
    cell: i64,
    buckets: BTreeMap<(Class, i64, i64), Vec<usize>>,
}

impl Index {
    fn of(objects: &[&Object], tolerance: i64) -> Self {
        let cell = tolerance.max(1);
        let mut buckets: BTreeMap<(Class, i64, i64), Vec<usize>> = BTreeMap::new();
        for (i, object) in objects.iter().enumerate() {
            let (x, y) = Self::cell_of(&object.kind, cell);
            buckets
                .entry((class(&object.kind), x, y))
                .or_default()
                .push(i);
        }
        Index { cell, buckets }
    }

    /// The cell of the lower-left corner of an object's placing points.
    fn cell_of(kind: &ObjectKind, cell: i64) -> (i64, i64) {
        let corner = RectNm::bounding(anchors(kind)).map_or(PointNm::ORIGIN, |r| r.min);
        (corner.x.div_euclid(cell), corner.y.div_euclid(cell))
    }

    /// The board objects that may pair with `object`, lowest index first.
    fn candidates(&self, object: &Object) -> Vec<usize> {
        let class = class(&object.kind);
        let (x, y) = Self::cell_of(&object.kind, self.cell);
        let mut found: Vec<usize> = (-1..=1)
            .flat_map(|dx| (-1..=1).map(move |dy| (class, x + dx, y + dy)))
            .filter_map(|key| self.buckets.get(&key))
            .flatten()
            .copied()
            .collect();
        found.sort_unstable();
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> PointNm {
        PointNm::new(x, y)
    }

    #[test]
    fn a_line_pairs_in_either_direction_within_the_tolerance() {
        let a = ObjectKind::Line {
            from: p(0, 0),
            to: p(1_000_000, 0),
        };
        let b = ObjectKind::Line {
            from: p(1_000_000, 5_000),
            to: p(5_000, 0),
        };
        assert!(same_place(&a, &b, 10_000));
        assert!(!same_place(&a, &b, 1_000));
        let flash = ObjectKind::Flash { at: p(0, 0) };
        assert!(!same_place(&a, &flash, 10_000), "kinds never pair across");
    }

    #[test]
    fn a_slot_pairs_only_with_a_slot() {
        let hole = ObjectKind::Hole {
            at: p(0, 0),
            diameter: Some(1_000_000),
            tool: 1,
            slot_to: None,
        };
        let slot = ObjectKind::Hole {
            at: p(0, 0),
            diameter: Some(1_000_000),
            tool: 1,
            slot_to: Some(p(2_000_000, 0)),
        };
        assert!(same_place(&hole, &hole, 0));
        assert!(!same_place(&hole, &slot, 10_000));
    }
}
