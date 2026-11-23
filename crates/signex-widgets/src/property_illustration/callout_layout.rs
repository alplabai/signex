//! Keeps the labels of a drawing inside its bounds and clear of each other.
//!
//! The canvas does not measure text, so a label's box is estimated from its
//! character count with a slightly wide advance. A label moves as little as
//! it can; an object's own name (the monospace role) never moves, since it
//! belongs on the object, and the others make room for it.

use iced::{Point, Rectangle, Size, Vector};

use super::draw::label_placement;
use super::geometry::{Baseline, Fit, LabelPlacement, callout_geometry, leader_label};
use super::illustration::Part;
use super::metrics::Metrics;
use super::primitive::{Anchor, Primitive, TextRole};

/// Average advance of a character, in text sizes.
const ADVANCE_EM: f32 = 0.58;
const MONO_ADVANCE_EM: f32 = 0.62;
/// The canvas text's line height, in text sizes.
const LINE_HEIGHT_EM: f32 = 1.3;
/// Free space kept between two labels, in captions.
const CLEARANCE_EM: f32 = 0.2;
/// Slack on the clearance test, so a box placed exactly at it counts as clear.
const TOUCH_TOLERANCE: f32 = 1e-3;

/// Who keeps its place when two labels want the same spot: lower ranks are
/// placed first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum LabelRank {
    /// An object's own name: never moved.
    Pinned,
    /// A dimension or leader label.
    Callout,
    /// A free caption or note.
    Note,
}

/// One label's wanted box, in screen space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LabelSlot {
    pub rect: Rectangle,
    pub rank: LabelRank,
}

/// The shift of each slot that keeps it inside `bounds` and `clearance` away
/// from the others. A label with no free spot stays inside the bounds; one
/// wider than them keeps its start visible.
pub(super) fn fit_labels(slots: &[LabelSlot], bounds: Size, clearance: f32) -> Vec<Vector> {
    let mut order: Vec<usize> = (0..slots.len()).collect();
    order.sort_by_key(|&i| slots[i].rank);
    let mut shifts = vec![Vector::new(0.0, 0.0); slots.len()];
    let mut placed: Vec<Rectangle> = Vec::with_capacity(slots.len());
    for i in order {
        let wanted = slots[i].rect;
        let spot = if slots[i].rank == LabelRank::Pinned {
            wanted.position()
        } else {
            free_spot(wanted, &placed, bounds, clearance)
        };
        shifts[i] = spot - wanted.position();
        placed.push(Rectangle::new(spot, wanted.size()));
    }
    shifts
}

/// The spot nearest to `wanted` that is inside `bounds` and clear of `placed`.
fn free_spot(wanted: Rectangle, placed: &[Rectangle], bounds: Size, clearance: f32) -> Point {
    let (max_x, max_y) = (bounds.width - wanted.width, bounds.height - wanted.height);
    let inside = Point::new(wanted.x.min(max_x).max(0.0), wanted.y.min(max_y).max(0.0));
    let mut xs = vec![inside.x];
    let mut ys = vec![inside.y];
    for p in placed {
        xs.extend([p.x - wanted.width - clearance, p.x + p.width + clearance]);
        ys.extend([p.y - wanted.height - clearance, p.y + p.height + clearance]);
    }
    xs.retain(|x| *x == inside.x || (*x >= 0.0 && *x <= max_x));
    ys.retain(|y| *y == inside.y || (*y >= 0.0 && *y <= max_y));

    let mut best: Option<(f32, Point)> = None;
    for &x in &xs {
        for &y in &ys {
            let spot = Point::new(x, y);
            let candidate = Rectangle::new(spot, wanted.size());
            if placed.iter().any(|p| crowds(candidate, *p, clearance)) {
                continue;
            }
            let (dx, dy) = (x - inside.x, y - inside.y);
            let cost = dx * dx + dy * dy;
            if best.is_none_or(|(least, _)| cost < least) {
                best = Some((cost, spot));
            }
        }
    }
    best.map_or(inside, |(_, spot)| spot)
}

/// True when `a` and `b` are closer than `clearance` on both axes.
fn crowds(a: Rectangle, b: Rectangle, clearance: f32) -> bool {
    let reach = clearance - TOUCH_TOLERANCE;
    a.x < b.x + b.width + reach
        && b.x < a.x + a.width + reach
        && a.y < b.y + b.height + reach
        && b.y < a.y + a.height + reach
}

/// The estimated box of `text` drawn at `placement`.
fn label_box(
    placement: LabelPlacement,
    text: &str,
    role: TextRole,
    metrics: &Metrics,
) -> Rectangle {
    let size = metrics.text(role);
    let advance = match role {
        TextRole::Mono => MONO_ADVANCE_EM,
        TextRole::Caption | TextRole::Note => ADVANCE_EM,
    };
    let width = text.chars().count() as f32 * advance * size;
    let height = size * LINE_HEIGHT_EM;
    let x = match placement.anchor {
        Anchor::Start => placement.at.x,
        Anchor::Middle => placement.at.x - width / 2.0,
        Anchor::End => placement.at.x - width,
    };
    let y = match placement.baseline {
        Baseline::Above => placement.at.y - height,
        Baseline::Centre => placement.at.y - height / 2.0,
    };
    Rectangle::new(Point::new(x, y), Size::new(width, height))
}

/// The label slot of `part`, if it draws text.
fn part_slot(part: &Part, fit: &Fit, metrics: &Metrics) -> Option<LabelSlot> {
    let (placement, text, role, rank) = match &part.primitive {
        Primitive::Callout(callout) => {
            let text = callout.label.as_ref()?;
            let placement = callout_geometry(callout, fit, metrics).label?;
            (placement, text, TextRole::Caption, LabelRank::Callout)
        }
        Primitive::Leader(leader) => {
            let end = fit.point(leader.label_at);
            let placement = leader_label(end, leader.anchor, metrics);
            let role = TextRole::Caption;
            (placement, &leader.label, role, LabelRank::Callout)
        }
        Primitive::Label(label) => {
            let rank = match label.role {
                TextRole::Mono => LabelRank::Pinned,
                TextRole::Caption | TextRole::Note => LabelRank::Note,
            };
            (label_placement(label, fit), &label.text, label.role, rank)
        }
        _ => return None,
    };
    let text = text.as_str();
    (!text.is_empty()).then(|| LabelSlot {
        rect: label_box(placement, text, role, metrics),
        rank,
    })
}

/// The shift of each of `parts`' labels in a drawing of `bounds`; a part
/// without text gets none.
pub(super) fn label_shifts<'a>(
    parts: impl Iterator<Item = &'a Part>,
    fit: &Fit,
    metrics: &Metrics,
    bounds: Size,
) -> Vec<Vector> {
    let slots: Vec<Option<LabelSlot>> = parts.map(|p| part_slot(p, fit, metrics)).collect();
    let drawn: Vec<LabelSlot> = slots.iter().flatten().copied().collect();
    let mut fitted = fit_labels(&drawn, bounds, metrics.text_size * CLEARANCE_EM).into_iter();
    slots
        .iter()
        .map(|slot| match slot {
            Some(_) => fitted.next().unwrap_or(Vector::new(0.0, 0.0)),
            None => Vector::new(0.0, 0.0),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::illustration::Illustration;
    use super::super::primitive::{Callout, Leader, Text, TextLabel};
    use super::super::style::Ink;
    use super::*;
    use signex_i18n::UserText;

    /// The dock widths the Properties drawings must hold up at.
    const DOCK_WIDTHS: [f32; 2] = [220.0, 300.0];
    const CAPTION: f32 = 11.0;
    const BOUNDS: Size = Size::new(100.0, 50.0);

    fn slot(x: f32, y: f32, w: f32, rank: LabelRank) -> LabelSlot {
        LabelSlot {
            rect: Rectangle::new(Point::new(x, y), Size::new(w, 10.0)),
            rank,
        }
    }

    fn moved(slot: &LabelSlot, shift: Vector) -> Rectangle {
        Rectangle::new(slot.rect.position() + shift, slot.rect.size())
    }

    fn text(s: &str) -> Text {
        UserText::new(s.to_owned()).into()
    }

    #[test]
    fn a_label_that_fits_stays_where_it_is() {
        let slots = [
            slot(10.0, 10.0, 20.0, LabelRank::Callout),
            slot(50.0, 10.0, 20.0, LabelRank::Note),
        ];
        let shifts = fit_labels(&slots, BOUNDS, 2.0);
        assert_eq!(shifts, vec![Vector::new(0.0, 0.0); 2]);
    }

    #[test]
    fn a_label_past_an_edge_moves_just_inside() {
        let left = [slot(-8.0, 10.0, 20.0, LabelRank::Callout)];
        assert_eq!(fit_labels(&left, BOUNDS, 2.0)[0], Vector::new(8.0, 0.0));
        let right = [slot(90.0, 45.0, 20.0, LabelRank::Callout)];
        assert_eq!(fit_labels(&right, BOUNDS, 2.0)[0], Vector::new(-10.0, -5.0));
    }

    #[test]
    fn a_label_wider_than_the_drawing_keeps_its_start_visible() {
        let wide = [slot(30.0, 10.0, 140.0, LabelRank::Callout)];
        assert_eq!(fit_labels(&wide, BOUNDS, 2.0)[0], Vector::new(-30.0, 0.0));
    }

    #[test]
    fn two_labels_on_one_spot_end_up_clear_of_each_other() {
        let slots = [
            slot(10.0, 20.0, 30.0, LabelRank::Callout),
            slot(15.0, 22.0, 30.0, LabelRank::Callout),
        ];
        let shifts = fit_labels(&slots, BOUNDS, 2.0);
        assert_eq!(shifts[0], Vector::new(0.0, 0.0), "the first keeps its spot");
        let (a, b) = (moved(&slots[0], shifts[0]), moved(&slots[1], shifts[1]));
        assert!(!crowds(a, b, 2.0), "{a:?} and {b:?}");
    }

    #[test]
    fn an_object_name_never_moves_and_a_callout_makes_room() {
        let slots = [
            slot(20.0, 20.0, 30.0, LabelRank::Callout),
            slot(20.0, 20.0, 30.0, LabelRank::Pinned),
        ];
        let shifts = fit_labels(&slots, BOUNDS, 2.0);
        assert_eq!(shifts[1], Vector::new(0.0, 0.0));
        assert!(!crowds(
            moved(&slots[0], shifts[0]),
            moved(&slots[1], shifts[1]),
            2.0
        ));
    }

    #[test]
    fn a_note_gives_way_to_a_callout_whatever_the_drawing_order() {
        let slots = [
            slot(10.0, 20.0, 30.0, LabelRank::Note),
            slot(10.0, 20.0, 30.0, LabelRank::Callout),
        ];
        let shifts = fit_labels(&slots, BOUNDS, 2.0);
        assert_eq!(shifts[1], Vector::new(0.0, 0.0));
        assert_ne!(shifts[0], Vector::new(0.0, 0.0));
    }

    #[test]
    fn a_full_drawing_keeps_every_label_inside() {
        let slots: Vec<_> = (0..12)
            .map(|_| slot(40.0, 20.0, 60.0, LabelRank::Callout))
            .collect();
        for (s, shift) in slots.iter().zip(fit_labels(&slots, BOUNDS, 2.0)) {
            let r = moved(s, shift);
            assert!(r.x >= 0.0 && r.x + r.width <= BOUNDS.width, "{r:?}");
            assert!(r.y >= 0.0 && r.y + r.height <= BOUNDS.height, "{r:?}");
        }
    }

    /// The labels of the pad drawing (mockup row 125) on its 264 x 140 view.
    fn pad_drawing() -> Illustration {
        let leader = |at: (f32, f32), end: (f32, f32), label: &str, anchor| {
            Part::leader(Leader {
                at: Point::new(at.0, at.1),
                label_at: Point::new(end.0, end.1),
                label: text(label),
                anchor,
            })
        };
        Illustration::new(264.0, 140.0)
            .part(Part::callout(
                Callout::new(Point::new(49.0, 37.0), Point::new(159.0, 37.0))
                    .label(text("Size X 1.550")),
            ))
            .part(Part::callout(
                Callout::new(Point::new(172.0, 58.0), Point::new(172.0, 102.0))
                    .label(text("Size Y 0.600"))
                    .label_at(Point::new(177.0, 83.0), Anchor::Start),
            ))
            .part(leader(
                (150.0, 60.0),
                (168.0, 30.0),
                "Corner radius 25%",
                Anchor::Start,
            ))
            .part(leader(
                (76.0, 100.0),
                (58.0, 126.0),
                "Top Paste",
                Anchor::End,
            ))
            .part(leader(
                (161.0, 104.0),
                (168.0, 112.0),
                "Top Solder mask",
                Anchor::Start,
            ))
            .part(Part::label(
                TextLabel {
                    at: Point::new(4.0, 12.0),
                    text: text("Pad 1 \u{b7} Top Layer"),
                    anchor: Anchor::Start,
                    role: TextRole::Note,
                },
                Ink::Faint,
            ))
    }

    #[test]
    fn the_pad_labels_fit_and_stay_clear_at_both_dock_widths() {
        let drawing = pad_drawing();
        let metrics = Metrics::new(CAPTION);
        for width in DOCK_WIDTHS {
            let bounds = Size::new(width, metrics.height(drawing.height_in_em()));
            let fit = Fit::new(drawing.view_size(), bounds);
            let shifts = label_shifts(drawing.visible_parts(), &fit, &metrics, bounds);
            let boxes: Vec<Rectangle> = drawing
                .visible_parts()
                .zip(&shifts)
                .filter_map(|(p, s)| Some(moved(&part_slot(p, &fit, &metrics)?, *s)))
                .collect();
            assert_eq!(boxes.len(), 6, "every label has a box");
            for (i, a) in boxes.iter().enumerate() {
                assert!(
                    a.x >= 0.0 && a.x + a.width <= bounds.width,
                    "{width}: {a:?}"
                );
                assert!(
                    a.y >= 0.0 && a.y + a.height <= bounds.height,
                    "{width}: {a:?}"
                );
                for b in &boxes[i + 1..] {
                    let clearance = CAPTION * CLEARANCE_EM;
                    assert!(!crowds(*a, *b, clearance), "{width}: {a:?} and {b:?}");
                }
            }
        }
    }

    #[test]
    fn a_part_without_text_gets_no_shift() {
        let line = Part::new(
            Primitive::Line {
                from: Point::new(-50.0, 0.0),
                to: Point::new(500.0, 0.0),
            },
            crate::property_illustration::Style::edge(),
        );
        let bounds = Size::new(100.0, 100.0);
        let fit = Fit::new(bounds, bounds);
        let shifts = label_shifts([&line].into_iter(), &fit, &Metrics::new(CAPTION), bounds);
        assert_eq!(shifts, vec![Vector::new(0.0, 0.0)]);
    }
}
