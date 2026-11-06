//! `connected_net_items`: what Select > Connection picks (#2106).
use super::*;
use signex_types::schematic::{Bus, SelectedItem, SelectedKind};

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn wire_at(n: u128, a: Point, b: Point) -> Wire {
    Wire {
        uuid: id(n),
        start: a,
        end: b,
        ..Default::default()
    }
}

fn item(n: u128, kind: SelectedKind) -> SelectedItem {
    SelectedItem::new(id(n), kind)
}

fn sorted(mut v: Vec<SelectedItem>) -> Vec<SelectedItem> {
    v.sort_by_key(|i| i.uuid);
    v
}

#[test]
fn a_wire_ending_mid_span_on_another_is_selected_with_it() {
    // T-junction: the endpoint-only walk missed this.
    let mut sheet = empty_sheet();
    sheet.wires.push(wire_at(1, pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire_at(2, pt(5.0, 0.0), pt(5.0, 5.0)));
    sheet.junctions.push(Junction {
        uuid: id(3),
        position: pt(5.0, 0.0),
        ..Default::default()
    });
    sheet.wires.push(wire_at(4, pt(50.0, 0.0), pt(60.0, 0.0)));

    let got = sorted(connected_net_items(&sheet, item(2, SelectedKind::Wire)));
    assert_eq!(
        got,
        vec![
            item(1, SelectedKind::Wire),
            item(2, SelectedKind::Wire),
            item(3, SelectedKind::Junction),
        ]
    );
}

#[test]
fn a_same_name_label_pulls_in_the_disjoint_wire_it_names() {
    let mut sheet = empty_sheet();
    sheet.wires.push(wire_at(1, pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.wires.push(wire_at(2, pt(50.0, 0.0), pt(60.0, 0.0)));
    for (n, p) in [(11, pt(0.0, 0.0)), (12, pt(50.0, 0.0))] {
        let mut l = label("VCC", p, LabelType::Net);
        l.uuid = id(n);
        sheet.labels.push(l);
    }

    let got = sorted(connected_net_items(&sheet, item(1, SelectedKind::Wire)));
    assert_eq!(
        got,
        vec![
            item(1, SelectedKind::Wire),
            item(2, SelectedKind::Wire),
            item(11, SelectedKind::Label),
            item(12, SelectedKind::Label),
        ]
    );
}

#[test]
fn a_bus_touching_a_wire_is_not_part_of_the_net() {
    let mut sheet = empty_sheet();
    sheet.wires.push(wire_at(1, pt(0.0, 0.0), pt(10.0, 0.0)));
    sheet.buses.push(Bus {
        uuid: id(20),
        start: pt(10.0, 0.0),
        end: pt(10.0, 10.0),
    });

    assert_eq!(
        connected_net_items(&sheet, item(1, SelectedKind::Wire)),
        vec![item(1, SelectedKind::Wire)]
    );
    assert_eq!(
        connected_net_items(&sheet, item(20, SelectedKind::Bus)),
        vec![item(20, SelectedKind::Bus)]
    );
}

#[test]
fn a_bus_seed_takes_the_joined_run_and_its_labels() {
    let mut sheet = empty_sheet();
    for (n, a, b) in [
        (20, pt(0.0, 0.0), pt(10.0, 0.0)),
        (21, pt(10.0, 0.0), pt(10.0, 10.0)),
        (22, pt(50.0, 0.0), pt(60.0, 0.0)),
    ] {
        sheet.buses.push(Bus {
            uuid: id(n),
            start: a,
            end: b,
        });
    }
    let mut on_bus = label("D[0..7]", pt(4.0, 0.0), LabelType::Net);
    on_bus.uuid = id(30);
    sheet.labels.push(on_bus);

    let got = sorted(connected_net_items(&sheet, item(20, SelectedKind::Bus)));
    assert_eq!(
        got,
        vec![
            item(20, SelectedKind::Bus),
            item(21, SelectedKind::Bus),
            item(30, SelectedKind::Label),
        ]
    );
}

#[test]
fn a_label_on_a_bus_seeds_the_bus_run() {
    let mut sheet = empty_sheet();
    for (n, a, b) in [
        (20, pt(0.0, 0.0), pt(10.0, 0.0)),
        (21, pt(10.0, 0.0), pt(10.0, 10.0)),
    ] {
        sheet.buses.push(Bus {
            uuid: id(n),
            start: a,
            end: b,
        });
    }
    let mut on_bus = label("D[0..7]", pt(10.0, 0.0), LabelType::Net);
    on_bus.uuid = id(30);
    sheet.labels.push(on_bus);

    let got = sorted(connected_net_items(&sheet, item(30, SelectedKind::Label)));
    assert_eq!(
        got,
        vec![
            item(20, SelectedKind::Bus),
            item(21, SelectedKind::Bus),
            item(30, SelectedKind::Label),
        ]
    );
}

#[test]
fn a_t_joined_bus_is_one_run() {
    let mut sheet = empty_sheet();
    for (n, a, b) in [
        (20, pt(0.0, 0.0), pt(20.0, 0.0)),
        (21, pt(10.0, 0.0), pt(10.0, 10.0)),
    ] {
        sheet.buses.push(Bus {
            uuid: id(n),
            start: a,
            end: b,
        });
    }
    let got = sorted(connected_net_items(&sheet, item(20, SelectedKind::Bus)));
    assert_eq!(
        got,
        vec![item(20, SelectedKind::Bus), item(21, SelectedKind::Bus)]
    );
}

#[test]
fn a_plain_net_label_mid_span_on_a_bus_is_not_on_the_bus() {
    let mut sheet = empty_sheet();
    sheet.buses.push(Bus {
        uuid: id(20),
        start: pt(0.0, 0.0),
        end: pt(10.0, 0.0),
    });
    let mut sda = label("SDA", pt(4.0, 0.0), LabelType::Net);
    sda.uuid = id(30);
    sheet.labels.push(sda);

    assert_eq!(
        connected_net_items(&sheet, item(20, SelectedKind::Bus)),
        vec![item(20, SelectedKind::Bus)]
    );
    assert_eq!(
        connected_net_items(&sheet, item(30, SelectedKind::Label)),
        vec![item(30, SelectedKind::Label)]
    );
}

#[test]
fn a_plain_net_label_at_a_bus_end_belongs_to_the_run() {
    let mut sheet = empty_sheet();
    sheet.buses.push(Bus {
        uuid: id(20),
        start: pt(0.0, 0.0),
        end: pt(10.0, 0.0),
    });
    let mut sda = label("SDA", pt(10.0, 0.0), LabelType::Net);
    sda.uuid = id(30);
    sheet.labels.push(sda);

    assert_eq!(
        sorted(connected_net_items(&sheet, item(20, SelectedKind::Bus))),
        vec![item(20, SelectedKind::Bus), item(30, SelectedKind::Label)]
    );
}

#[test]
fn a_junction_on_a_bus_alone_seeds_the_bus_run() {
    let mut sheet = empty_sheet();
    for (n, a, b) in [
        (20, pt(0.0, 0.0), pt(20.0, 0.0)),
        (21, pt(10.0, 0.0), pt(10.0, 10.0)),
    ] {
        sheet.buses.push(Bus {
            uuid: id(n),
            start: a,
            end: b,
        });
    }
    sheet.junctions.push(Junction {
        uuid: id(3),
        position: pt(10.0, 0.0),
        ..Default::default()
    });
    assert_eq!(
        sorted(connected_net_items(&sheet, item(3, SelectedKind::Junction))),
        vec![
            item(3, SelectedKind::Junction),
            item(20, SelectedKind::Bus),
            item(21, SelectedKind::Bus),
        ]
    );
}

#[test]
fn an_isolated_or_unknown_seed_comes_back_alone() {
    let mut sheet = empty_sheet();
    let mut l = label("X", pt(1.0, 1.0), LabelType::Net);
    l.uuid = id(5);
    sheet.labels.push(l);
    assert_eq!(
        connected_net_items(&sheet, item(5, SelectedKind::Label)),
        vec![item(5, SelectedKind::Label)]
    );
    assert_eq!(
        connected_net_items(&sheet, item(99, SelectedKind::Wire)),
        vec![item(99, SelectedKind::Wire)]
    );
    assert_eq!(
        connected_net_items(&sheet, item(7, SelectedKind::Symbol)),
        vec![item(7, SelectedKind::Symbol)]
    );
}
