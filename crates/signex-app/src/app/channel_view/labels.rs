//! Where a channel card's miniature draws its names (#3042, mockup row 151).
//! A name keeps a readable size whatever the card's scale, so at card scale
//! it is often larger than the part it names. Each name is set, in order,
//! at the first place around what it names that crosses no part outline,
//! no name already set and no edge of the drawing: inside its own part when
//! it fits there, else above, below, right or left of it, stepping further
//! out. A name with no free place takes the one that covers the least.

use iced::{Point, Rectangle, Size};

/// Clear space between a name and what it names, in px.
const GAP: f32 = 2.0;
/// Clear space inside a part outline around a name set inside it, in px.
const INNER: f32 = 2.0;
/// How many steps of one name height a name moves out looking for room.
const STEPS: u8 = 3;

/// A name to set on the drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Name {
    /// The point it names: its part's centre, or a point on its net.
    pub at: Point,
    /// Its size on the drawing.
    pub size: Size,
    /// The outline (an index into the parts) of the part it names.
    pub part: Option<usize>,
}

/// The box of each name in `names`, in order, laid out on `area` around the
/// part outlines `parts`.
pub(super) fn place(parts: &[Rectangle], names: &[Name], area: Rectangle) -> Vec<Rectangle> {
    let mut placed: Vec<Rectangle> = Vec::with_capacity(names.len());
    for name in names {
        let own = name.part.and_then(|i| parts.get(i).copied());
        let best = candidates(name, own)
            .into_iter()
            .map(|spot| inside(spot, area))
            .map(|spot| (covered(spot, own, parts, &placed), spot))
            .reduce(|best, next| if next.0 < best.0 { next } else { best })
            .map(|(_, spot)| spot)
            .unwrap_or_else(|| inside(centred(name.at, name.size), area));
        placed.push(best);
    }
    placed
}

/// The box of `size` centred on `at`.
fn centred(at: Point, size: Size) -> Rectangle {
    Rectangle::new(
        Point::new(at.x - size.width / 2.0, at.y - size.height / 2.0),
        size,
    )
}

/// The places to try for `name`, best first.
fn candidates(name: &Name, own: Option<Rectangle>) -> Vec<Rectangle> {
    let size = name.size;
    let around = own.unwrap_or(Rectangle::new(name.at, Size::ZERO));
    let mut spots = Vec::new();
    if let Some(part) = own
        && size.width + 2.0 * INNER <= part.width
        && size.height + 2.0 * INNER <= part.height
    {
        spots.push(centred(part.center(), size));
    }
    let left = around.x;
    let right = around.x + around.width;
    let top = around.y;
    let bottom = around.y + around.height;
    let mid = around.center();
    // Above and below: centred on it, then flush with either side.
    let xs = [mid.x - size.width / 2.0, left, right - size.width];
    for step in 0..STEPS {
        let out = GAP + f32::from(step) * size.height;
        for y in [top - out - size.height, bottom + out] {
            for x in xs {
                spots.push(Rectangle::new(Point::new(x, y), size));
            }
        }
        let y = mid.y - size.height / 2.0;
        for x in [right + out, left - out - size.width] {
            spots.push(Rectangle::new(Point::new(x, y), size));
        }
    }
    spots
}

/// `spot` moved inside `area`, its left and top edges kept when it is the
/// larger.
fn inside(spot: Rectangle, area: Rectangle) -> Rectangle {
    let x = spot.x.min(area.x + area.width - spot.width).max(area.x);
    let y = spot.y.min(area.y + area.height - spot.height).max(area.y);
    Rectangle::new(Point::new(x, y), spot.size())
}

/// How much of `spot` covers part outlines and names already set: the area
/// it shares with each, where a name wholly inside its own part `own`
/// crosses nothing.
fn covered(
    spot: Rectangle,
    own: Option<Rectangle>,
    parts: &[Rectangle],
    placed: &[Rectangle],
) -> f32 {
    let in_own = own.is_some_and(|part| contains(part, spot));
    let parts = parts
        .iter()
        .filter(|part| !(in_own && Some(**part) == own))
        .map(|part| shared(spot, *part));
    let names = placed.iter().map(|name| shared(spot, *name));
    parts.chain(names).sum()
}

/// Whether `inner` lies wholly inside `outer`.
fn contains(outer: Rectangle, inner: Rectangle) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

/// The area `a` and `b` share; touching edges share none.
pub(super) fn shared(a: Rectangle, b: Rectangle) -> f32 {
    let w = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
    let h = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
    if w > 0.0 && h > 0.0 { w * h } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 120.0,
    };
    const LABEL: Size = Size::new(32.0, 11.0);

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(w, h))
    }

    #[test]
    fn a_name_that_fits_its_part_sits_inside_it() {
        let parts = [rect(100.0, 20.0, 60.0, 40.0)];
        let names = [Name {
            at: parts[0].center(),
            size: LABEL,
            part: Some(0),
        }];
        let placed = place(&parts, &names, AREA);
        assert!(contains(parts[0], placed[0]), "{placed:?}");
    }

    #[test]
    fn a_name_larger_than_its_part_sits_beside_it_crossing_no_outline() {
        let parts = [rect(100.0, 40.0, 16.0, 9.0)];
        let names = [Name {
            at: parts[0].center(),
            size: LABEL,
            part: Some(0),
        }];
        let placed = place(&parts, &names, AREA);
        assert_eq!(shared(placed[0], parts[0]), 0.0, "{placed:?}");
        assert!(
            placed[0].y + placed[0].height <= parts[0].y,
            "above it first"
        );
    }

    #[test]
    fn names_never_cover_each_other_and_stay_on_the_drawing() {
        let parts = [rect(2.0, 2.0, 20.0, 40.0)];
        let names: Vec<Name> = (0..3)
            .map(|_| Name {
                at: Point::new(24.0, 10.0),
                size: LABEL,
                part: None,
            })
            .collect();
        let placed = place(&parts, &names, AREA);
        for (i, a) in placed.iter().enumerate() {
            assert!(contains(AREA, *a), "{a:?}");
            assert_eq!(shared(*a, parts[0]), 0.0, "{a:?}");
            for b in &placed[i + 1..] {
                assert_eq!(shared(*a, *b), 0.0, "{a:?} and {b:?}");
            }
        }
    }
}
