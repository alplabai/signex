use uuid::Uuid;

use super::*;
use crate::shape::Shape;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn disc(item: ItemRef, x: i64, y: i64, r: i64) -> IndexedItem {
    let shape = Shape::Disc {
        center: PointNm::new(x, y),
        radius: r,
    };
    IndexedItem::new(item, shape).unwrap()
}

fn index() -> BoardIndex {
    BoardIndex::from_items(vec![
        disc(ItemRef::Via(id(1)), 0, 0, 500),
        disc(
            ItemRef::Pad {
                footprint: id(2),
                pad: id(3),
            },
            0,
            0,
            800,
        ),
        disc(ItemRef::Track(id(4)), 5_000, 0, 500),
    ])
}

#[test]
fn pick_prefers_via_over_pad() {
    let idx = index();
    let hit = pick(&idx, PointNm::ORIGIN, 0).unwrap();
    assert_eq!(hit.item, ItemRef::Via(id(1)));
    assert!(pick(&idx, PointNm::new(100_000, 0), 0).is_none());
}

#[test]
fn click_modes_combine() {
    let (a, b) = (ItemRef::Via(id(1)), ItemRef::Track(id(4)));
    let mut sel = Selection::default();
    assert!(sel.apply_click(Some(a), ClickMode::Replace));
    assert!(sel.apply_click(Some(b), ClickMode::Add));
    assert_eq!(sel.len(), 2);
    assert!(sel.apply_click(Some(a), ClickMode::Toggle));
    assert!(!sel.contains(a) && sel.contains(b));
    assert!(!sel.apply_click(None, ClickMode::Add));
    assert!(!sel.apply_click(None, ClickMode::Toggle));
    assert!(sel.apply_click(None, ClickMode::Replace));
    assert!(sel.is_empty());
}

#[test]
fn window_needs_the_item_inside_crossing_only_a_touch() {
    let idx = index();
    // The via and pad lie inside this box; the track at x=5000 is cut.
    let (from, to) = (PointNm::new(-1_000, -1_000), PointNm::new(4_800, 1_000));
    assert_eq!(marquee_select(&idx, from, to).len(), 2);
    assert_eq!(marquee_select(&idx, to, from).len(), 3);
}

#[test]
fn retain_in_drops_items_gone_from_the_index() {
    let idx = index();
    let mut sel = Selection::default();
    sel.apply_marquee(&[ItemRef::Via(id(1)), ItemRef::Via(id(99))], ClickMode::Add);
    assert!(sel.retain_in(&idx));
    assert_eq!(sel.len(), 1);
}
