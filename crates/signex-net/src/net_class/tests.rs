use signex_types::net::{Net, NetId};
use signex_types::net_class::NetLineStyle;
use signex_types::theme::Color;

use super::*;

fn net(id: u32, name: &str, wires: &[Uuid]) -> Net {
    Net {
        id: NetId(id),
        name: name.into(),
        class: None,
        wires: wires.to_vec(),
        junctions: Vec::new(),
        terminals: Vec::new(),
    }
}

fn styled(name: &str, nets: &[&str], patterns: &[&str], color: Option<Color>) -> NetClass {
    NetClass {
        name: name.into(),
        nets: nets.iter().map(|s| s.to_string()).collect(),
        patterns: patterns.iter().map(|s| s.to_string()).collect(),
        style: NetClassStyle {
            color,
            wire_width_mm: None,
            line: NetLineStyle::Dashed,
        },
        ..NetClass::default()
    }
}

#[test]
fn no_classes_yields_no_styles() {
    let w = Uuid::from_u128(1);
    let netlist = Netlist {
        nets: vec![net(1, "GND", &[w])],
    };
    assert!(element_class_styles(&netlist, &[]).is_empty());
}

#[test]
fn every_wire_of_a_member_net_gets_the_class_style() {
    let (a, b, c) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
    let red = Color::new(255, 0, 0, 255);
    let netlist = Netlist {
        nets: vec![net(1, "VCC_3V3", &[a, b]), net(2, "SDA", &[c])],
    };
    let classes = vec![styled("Power", &[], &["VCC*"], Some(red))];
    let styles = element_class_styles(&netlist, &classes);
    assert_eq!(styles.get(&a).and_then(|s| s.color), Some(red));
    assert_eq!(styles.get(&b).and_then(|s| s.color), Some(red));
    assert!(!styles.contains_key(&c));
}

#[test]
fn first_class_in_table_order_wins_when_several_match() {
    let w = Uuid::from_u128(1);
    let (red, blue) = (Color::new(255, 0, 0, 255), Color::new(0, 0, 255, 255));
    let netlist = Netlist {
        nets: vec![net(1, "VCC", &[w])],
    };
    let classes = vec![
        styled("First", &["VCC"], &[], Some(red)),
        styled("Second", &[], &["V*"], Some(blue)),
    ];
    assert_eq!(
        element_class_styles(&netlist, &classes)[&w].color,
        Some(red)
    );
    let swapped = vec![classes[1].clone(), classes[0].clone()];
    assert_eq!(
        element_class_styles(&netlist, &swapped)[&w].color,
        Some(blue)
    );
}

#[test]
fn a_class_without_style_is_membership_only() {
    let w = Uuid::from_u128(1);
    let netlist = Netlist {
        nets: vec![net(1, "GND", &[w])],
    };
    let mut c = NetClass::named("Plain");
    c.nets.push("GND".into());
    assert!(element_class_styles(&netlist, &[c]).is_empty());
}

#[test]
fn assign_sets_and_clears_the_class_field() {
    let mut netlist = Netlist {
        nets: vec![net(1, "GND", &[]), net(2, "SDA", &[])],
    };
    let classes = vec![styled("Power", &["GND"], &[], None)];
    assign_net_classes(&mut netlist, &classes);
    assert_eq!(netlist.nets[0].class, Some(NetClassId("Power".into())));
    assert_eq!(netlist.nets[1].class, None);
    assign_net_classes(&mut netlist, &[]);
    assert_eq!(netlist.nets[0].class, None, "stale class must be cleared");
}

#[test]
fn element_net_names_use_the_netlist_name_and_the_first_net_wins() {
    let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
    let netlist = Netlist {
        nets: vec![net(1, "/LOCAL", &[a]), net(2, "OTHER", &[a, b])],
    };
    let names = element_net_names(&netlist);
    assert_eq!(names[&a], "/LOCAL");
    assert_eq!(names[&b], "OTHER");
}

#[test]
fn a_point_is_attached_to_the_wire_it_ends_or_taps() {
    use signex_types::schematic::{Point, SchematicSheet, Wire};
    let (ends, taps) = (Uuid::from_u128(1), Uuid::from_u128(2));
    let mut sheet = SchematicSheet::default();
    sheet.wires.push(Wire {
        uuid: ends,
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet.wires.push(Wire {
        uuid: taps,
        start: Point::new(0.0, 5.0),
        end: Point::new(0.0, 20.0),
        ..Default::default()
    });
    assert_eq!(wire_at_point(&sheet, &Point::new(10.0, 0.0)), Some(ends));
    assert_eq!(wire_at_point(&sheet, &Point::new(0.0, 12.0)), Some(taps));
    assert_eq!(wire_at_point(&sheet, &Point::new(50.0, 50.0)), None);
}
