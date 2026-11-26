//! Cross-reference marks (#1273): text, dedup, placement beside the flag and
//! the link boxes the PDF writes.

use super::*;
use signex_types::schematic::Point;

fn target(page: usize, zone: Option<&str>) -> CrossRefTarget {
    CrossRefTarget {
        page,
        zone: zone.map(str::to_string),
        x_mm: 12.0,
        y_mm: 34.0,
    }
}

fn sheet_with(rotation: f64) -> (SchematicSheet, Uuid) {
    let mut sheet = SchematicSheet::default();
    let label = Label {
        rotation,
        ..Label::new("VCC", Point::new(100.0, 50.0), LabelType::Global)
    };
    let uuid = label.uuid;
    sheet.labels.push(label);
    (sheet, uuid)
}

fn page_for(uuid: Uuid, targets: Vec<CrossRefTarget>) -> PageCrossRefs {
    PageCrossRefs {
        path: PathBuf::from("/p/a.snxsch"),
        placement: InstancePath::root(),
        labels: vec![(uuid, targets)],
    }
}

#[test]
fn an_entry_is_the_page_and_zone_or_the_page_alone() {
    assert_eq!(target(2, Some("B3")).entry(), "2/B3");
    assert_eq!(target(4, None).entry(), "4");
    assert_eq!(
        cross_ref_text(&[
            target(2, Some("B3")),
            target(4, None),
            target(2, Some("B3"))
        ])
        .as_deref(),
        Some("[2/B3, 4]")
    );
    assert_eq!(cross_ref_text(&[]), None);
}

#[test]
fn the_mark_lists_each_page_and_zone_once() {
    let (sheet, uuid) = sheet_with(0.0);
    let page = page_for(
        uuid,
        vec![
            target(2, Some("B3")),
            target(2, Some("B3")),
            target(4, Some("A1")),
        ],
    );
    let marks = lower_cross_refs(&sheet, &page);
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].text, "[2/B3, 4/A1]");
    assert_eq!(marks[0].links.len(), 2);
    assert_eq!(marks[0].links[1].page, 4);
    assert_eq!(marks[0].links[1].target_mm, (12.0, 34.0));
}

#[test]
fn a_label_without_targets_or_gone_from_the_sheet_has_no_mark() {
    let (sheet, uuid) = sheet_with(0.0);
    assert!(lower_cross_refs(&sheet, &page_for(uuid, Vec::new())).is_empty());
    let gone = page_for(Uuid::from_u128(7), vec![target(2, None)]);
    assert!(lower_cross_refs(&sheet, &gone).is_empty());
}

/// Only Global labels carry a mark: a net label of the same id does not.
#[test]
fn a_net_label_has_no_mark() {
    let (mut sheet, uuid) = sheet_with(0.0);
    sheet.labels[0].label_type = LabelType::Net;
    assert!(lower_cross_refs(&sheet, &page_for(uuid, vec![target(2, None)])).is_empty());
}

/// The mark sits past the far end of the flag, on the side it points to.
#[test]
fn the_mark_sits_past_the_flag_on_its_side() {
    let (right, uuid) = sheet_with(0.0);
    let mark = &lower_cross_refs(&right, &page_for(uuid, vec![target(2, None)]))[0];
    assert!(mark.x > 100.0, "{mark:?}");
    assert!(mark.y < 50.0 && mark.y + mark.size_mm > 50.0, "{mark:?}");

    let (left, uuid) = sheet_with(180.0);
    let mark = &lower_cross_refs(&left, &page_for(uuid, vec![target(2, None)]))[0];
    assert!(mark.links[0].x1 < 100.0, "{mark:?}");

    let (down, uuid) = sheet_with(90.0);
    let mark = &lower_cross_refs(&down, &page_for(uuid, vec![target(2, None)]))[0];
    assert!(mark.y > 50.0, "{mark:?}");

    let (up, uuid) = sheet_with(270.0);
    let mark = &lower_cross_refs(&up, &page_for(uuid, vec![target(2, None)]))[0];
    assert!(mark.links[0].y1 < 50.0, "{mark:?}");
}

/// Each entry's link box lies inside the line, in order, without overlap.
#[test]
fn the_link_boxes_follow_the_entries() {
    let (sheet, uuid) = sheet_with(0.0);
    let page = page_for(uuid, vec![target(2, Some("B3")), target(10, Some("C7"))]);
    let mark = &lower_cross_refs(&sheet, &page)[0];
    let [first, second] = mark.links.as_slice() else {
        panic!("two links: {mark:?}");
    };
    assert!(mark.x < first.x0 && first.x0 < first.x1, "{mark:?}");
    assert!(first.x1 < second.x0 && second.x0 < second.x1, "{mark:?}");
    assert_eq!((first.y0, first.y1), (second.y0, second.y1));
    assert!((first.y0 - mark.y).abs() < 1e-9);
}

#[test]
fn a_page_is_found_by_its_placement_else_its_first() {
    let mut refs = PortCrossRefs::default();
    let path = Path::new("/p/b.snxsch");
    let one = InstancePath::from_chain(vec![Uuid::from_u128(1)]);
    let two = InstancePath::from_chain(vec![Uuid::from_u128(2)]);
    refs.push(path, &one, Uuid::from_u128(10), vec![target(1, None)]);
    refs.push(path, &two, Uuid::from_u128(10), vec![target(3, None)]);
    refs.push(path, &two, Uuid::from_u128(11), vec![target(4, None)]);

    assert_eq!(refs.pages().len(), 2);
    assert_eq!(refs.for_page(path, Some(&two)).unwrap().labels.len(), 2);
    assert_eq!(refs.for_page(path, None).unwrap().placement, one);
    let elsewhere = InstancePath::from_chain(vec![Uuid::from_u128(9)]);
    assert_eq!(
        refs.for_page(path, Some(&elsewhere)).unwrap().placement,
        one
    );
    assert!(refs.for_page(Path::new("/p/c.snxsch"), None).is_none());
}
