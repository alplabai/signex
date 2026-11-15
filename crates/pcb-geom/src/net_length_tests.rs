use super::*;
use signex_types::pcb::{PcbBoard, Point, Segment, Via, ViaType};
use signex_types::pcb_layer::InnerCopper;
use uuid::Uuid;

fn track(n: u128, (ax, ay): (f64, f64), (bx, by): (f64, f64), net: u32) -> Segment {
    Segment {
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
        uuid: Uuid::from_u128(0x99),
        position: Point::new(0.0, 0.0),
        diameter: 0.6,
        drill: 0.3,
        layers: layers.iter().map(|l| l.to_string()).collect(),
        net: 1,
        via_type,
        unknown_columns: Default::default(),
    }
}

fn board(segments: Vec<Segment>) -> PcbBoard {
    PcbBoard {
        sketch: None,
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
