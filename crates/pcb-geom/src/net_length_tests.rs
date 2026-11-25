use super::*;
use signex_types::pcb::{PcbBoard, Point, Segment, Via, ViaType};
use signex_types::pcb_layer::InnerCopper;
use uuid::Uuid;

fn track(n: u128, (ax, ay): (f64, f64), (bx, by): (f64, f64), net: u32) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid: Uuid::from_u128(n),
        start: Point::new(ax, ay),
        end: Point::new(bx, by),
        width: 0.2,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

fn via(via_type: ViaType, layers: &[&str]) -> Via {
    Via {
        attributes: Default::default(),
        uuid: Uuid::from_u128(0x99),
        position: Point::new(0.0, 0.0),
        diameter: 0.6,
        drill: 0.3,
        layers: layers.iter().map(|l| l.to_string()).collect(),
        net: 1,
        locked: false,
        layer_sizes: None,
        via_type,
        unknown_columns: Default::default(),
    }
}

fn board(segments: Vec<Segment>) -> PcbBoard {
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        options: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: Vec::new(),
        segments,
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}

#[test]
fn a_net_length_is_the_sum_of_its_tracks() {
    // A 3-4-5 triangle leg plus a straight run: 5 + 2.5; the other net's
    // track and the unrouted net 0 track do not count.
    let b = board(vec![
        track(1, (0.0, 0.0), (3.0, 4.0), 1),
        track(2, (3.0, 4.0), (5.5, 4.0), 1),
        track(3, (0.0, 0.0), (9.0, 0.0), 2),
        track(4, (0.0, 0.0), (1.0, 0.0), 0),
    ]);
    assert_eq!(net_routed_length_mm(&b, 1), Some(7.5));
    assert_eq!(net_routed_length_mm(&b, 2), Some(9.0));
    assert_eq!(net_routed_length_mm(&b, 0), None);
    assert_eq!(track_length_mm(&b.segments[0]), Some(5.0));
    assert_eq!(tracks_length_mm(&b.segments[1..3]), Some(11.5));
}

#[test]
fn net_items_and_item_net_agree() {
    let mut b = board(vec![track(1, (0.0, 0.0), (1.0, 0.0), 4)]);
    b.vias.push(via(ViaType::Through, &[]));
    b.vias[0].net = 4;
    let items = net_items(&b, 4);
    assert_eq!(
        items,
        vec![
            ItemRef::Track(Uuid::from_u128(1)),
            ItemRef::Via(Uuid::from_u128(0x99))
        ]
    );
    for item in items {
        assert_eq!(item_net(&b, item), Some(4));
    }
    assert!(net_items(&b, 0).is_empty());
}

#[test]
fn a_via_spans_the_stack_or_the_layers_it_lists() {
    let inner = |n| CopperLayer::Inner(InnerCopper::new(n).unwrap());
    let stack = [CopperLayer::Top, inner(1), inner(2), CopperLayer::Bottom];
    assert_eq!(via_span(&stack, &via(ViaType::Through, &[])), Some((1, 4)));
    assert_eq!(
        via_span(&stack, &via(ViaType::Blind, &["TopCopper", "Inner1Copper"])),
        Some((1, 2))
    );
    assert_eq!(
        via_span(
            &stack,
            &via(ViaType::Micro, &["Inner2Copper", "BottomCopper"])
        ),
        Some((3, 4))
    );
    assert_eq!(
        via_span(&stack, &via(ViaType::Blind, &["TopSilkscreen"])),
        None
    );
}

fn area(n: u128, fill: &str, net: u32) -> signex_types::pcb::Zone {
    signex_types::pcb::Zone {
        uuid: Uuid::from_u128(n),
        net,
        net_name: String::new(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(0.0, 1.0),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: fill.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled: false,
        unknown: Default::default(),
    }
}

/// #2403: a drawn area on a net is one of its members and answers its
/// net; a pour is one too since pours are pickable (#2216), and net 0 has
/// no members.
#[test]
fn a_net_has_its_drawn_areas_as_members() {
    let mut b = board(vec![track(1, (0.0, 0.0), (1.0, 0.0), 4)]);
    b.zones = vec![area(5, "solid", 4), area(6, "pour", 4), area(7, "solid", 0)];
    let region = ItemRef::Zone {
        zone: Uuid::from_u128(5),
        kind: crate::PickKind::Regions,
    };
    let pour = ItemRef::Zone {
        zone: Uuid::from_u128(6),
        kind: crate::PickKind::Polygons,
    };
    assert_eq!(
        net_members(&b, 4),
        vec![ItemRef::Track(Uuid::from_u128(1)), region, pour]
    );
    assert_eq!(item_net(&b, region), Some(4));
    let unnetted = ItemRef::Zone {
        zone: Uuid::from_u128(7),
        kind: crate::PickKind::Regions,
    };
    assert_eq!(item_net(&b, unnetted), None);
    assert!(net_members(&b, 0).is_empty());
}

/// #2403: the copper of a net is its tracks, vias and pads, nothing else.
#[test]
fn net_copper_is_the_copper_of_the_net() {
    let mut b = board(vec![
        track(1, (0.0, 0.0), (1.0, 0.0), 4),
        track(2, (0.0, 0.0), (1.0, 0.0), 5),
    ]);
    b.vias.push(via(ViaType::Through, &[]));
    b.vias[0].net = 4;
    assert_eq!(net_copper(&b, 4).len(), 2);
    assert_eq!(net_copper(&b, 5).len(), 1);
    assert!(net_copper(&b, 0).is_empty());
}

/// #2514: the nets of the picked copper, once each and ascending; net 0
/// and what is not picked are left out.
#[test]
fn nets_of_names_the_nets_of_the_picked_copper() {
    let mut b = board(vec![
        track(1, (0.0, 0.0), (1.0, 0.0), 7),
        track(2, (0.0, 0.0), (1.0, 0.0), 3),
        track(3, (0.0, 0.0), (1.0, 0.0), 0),
        track(4, (0.0, 0.0), (1.0, 0.0), 9),
    ]);
    b.vias.push(via(ViaType::Through, &[]));
    b.vias[0].net = 7;
    b.zones = vec![area(5, "solid", 2)];
    let picked = [1u128, 2, 3, 5, 0x99].map(Uuid::from_u128);
    assert_eq!(nets_of(&b, |id| picked.contains(&id)), vec![2, 3, 7]);
    assert!(nets_of(&b, |_| false).is_empty());
}
