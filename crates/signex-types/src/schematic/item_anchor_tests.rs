use super::*;

#[test]
fn item_anchor_is_the_position_of_a_point_item_and_the_midpoint_of_a_run() {
    let mut sheet = SchematicSheet::default();
    let wire_id = Uuid::now_v7();
    let bus_id = Uuid::now_v7();
    let no_connect_id = Uuid::now_v7();
    sheet.wires.push(Wire {
        uuid: wire_id,
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 4.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    });
    sheet.buses.push(Bus {
        uuid: bus_id,
        start: Point::new(2.0, 2.0),
        end: Point::new(4.0, 8.0),
    });
    sheet.no_connects.push(NoConnect {
        uuid: no_connect_id,
        position: Point::new(7.0, 9.0),
    });

    let at = |id, kind| sheet.item_anchor(&SelectedItem::new(id, kind));
    assert_eq!(at(wire_id, SelectedKind::Wire), Some(Point::new(5.0, 2.0)));
    assert_eq!(at(bus_id, SelectedKind::Bus), Some(Point::new(3.0, 5.0)));
    assert_eq!(
        at(no_connect_id, SelectedKind::NoConnect),
        Some(Point::new(7.0, 9.0))
    );
    assert_eq!(at(Uuid::now_v7(), SelectedKind::Wire), None, "no such wire");
    assert_eq!(at(wire_id, SelectedKind::BusEntry), None, "no single point");
}
