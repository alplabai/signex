use super::*;
use signex_types::pcb::{
    Footprint, NetDef, Pad, PadNet, PadShape, PadType, Point, Segment, Via, ViaType,
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn pad(n: u128, x: f64, y: f64, net: u32) -> Pad {
    Pad {
        uuid: id(n),
        number: n.to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(x, y),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: (net != 0).then(|| PadNet {
            number: net,
            name: format!("N{net}"),
        }),
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        layer_sizes: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn footprint(n: u128, pads: Vec<Pad>) -> Footprint {
    Footprint {
        uuid: id(1000 + n),
        reference: format!("U{n}"),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> Segment {
    Segment {
        uuid: id(2000 + n),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        locked: false,
    }
}

fn via(n: u128, at: (f64, f64), net: u32) -> Via {
    Via {
        uuid: id(3000 + n),
        position: Point::new(at.0, at.1),
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        layer_sizes: None,
        locked: false,
    }
}

fn board(footprints: Vec<Footprint>, segments: Vec<Segment>, vias: Vec<Via>) -> PcbBoard {
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
        nets: vec![
            NetDef {
                number: 0,
                name: String::new(),
            },
            NetDef {
                number: 1,
                name: "N1".into(),
            },
            NetDef {
                number: 2,
                name: "N2".into(),
            },
        ],
        footprints,
        segments,
        vias,
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

/// N1 joins its two pads with a 10 mm run through a via; N2's two pads are
/// not joined.
fn sample() -> PcbBoard {
    let fp = footprint(
        1,
        vec![
            pad(1, 0.0, 0.0, 1),
            pad(2, 10.0, 0.0, 1),
            pad(3, 0.0, 5.0, 2),
            pad(4, 10.0, 5.0, 2),
        ],
    );
    board(
        vec![fp],
        vec![
            track(1, (0.0, 0.0), (4.0, 0.0), 1),
            track(2, (4.0, 0.0), (10.0, 0.0), 1),
        ],
        vec![via(1, (4.0, 0.0), 1)],
    )
}

#[test]
fn every_named_net_gets_a_row_with_its_length_members_and_routed_state() {
    let rows = net_reports(&sample());
    assert_eq!(rows.len(), 2, "net 0 is no net");
    let n1 = &rows[0];
    assert_eq!((n1.net, n1.name.as_str()), (1, "N1"));
    assert!((n1.routed_length_mm - 10.0).abs() < 1e-9);
    assert_eq!((n1.tracks, n1.vias, n1.pads), (2, 1, 2));
    assert!(n1.is_routed(), "{n1:?}");
    let n2 = &rows[1];
    assert_eq!((n2.tracks, n2.vias, n2.pads), (0, 0, 2));
    assert_eq!(n2.routed_length_mm, 0.0);
    assert_eq!(n2.unrouted, 1);
    assert!(!n2.is_routed());
}

#[test]
fn a_net_is_found_by_name_without_case() {
    let b = sample();
    assert_eq!(net_named(&b, "n2"), Some(2));
    assert_eq!(net_named(&b, " N1 "), Some(1));
    assert_eq!(net_named(&b, ""), None, "net 0 is no net");
    assert_eq!(net_named(&b, "VCC"), None);
}
