//! #2373: free arcs, pads and dimensions and board strings in the pick
//! index and the copper list.

use signex_types::pcb::{BoardText, DrillDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point};
use signex_types::pcb_arc::TrackArc;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use crate::{
    BoardIndex, ItemRef, PointNm, RectNm, RectSelect, arc_shapes, collect_copper, free_pad_shape,
    layer_bit, net_at, net_copper, net_items, net_members, net_routed_length_mm,
};

fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        stackup: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: Vec::new(),
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        options: Default::default(),
        unknown: Default::default(),
    }
}

/// A half turn of radius 5 about (5, 0) on top copper, net 4.
fn arc(layer: &str, net: u32) -> TrackArc {
    TrackArc {
        uuid: Uuid::from_u128(1),
        start: Point::new(0.0, 0.0),
        mid: Point::new(5.0, 5.0),
        end: Point::new(10.0, 0.0),
        width: 0.5,
        layer: layer.into(),
        net,
    }
}

fn pad(net: u32) -> Pad {
    Pad {
        uuid: Uuid::from_u128(2),
        number: "1".into(),
        pad_type: PadType::Thru,
        shape: PadShape::Circle,
        position: Point::new(30.0, 30.0),
        size: Point::new(2.0, 2.0),
        drill: Some(DrillDef {
            diameter: 1.0,
            shape: String::new(),
        }),
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: Some(PadNet {
            number: net,
            name: "N".into(),
        }),
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        layer_sizes: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn at(x: f64, y: f64) -> PointNm {
    PointNm::from_mm(Point::new(x, y), "at").unwrap()
}

#[test]
fn a_copper_arc_is_copper_of_its_net_along_its_bend() {
    let mut b = board();
    b.free.arcs.push(arc("TopCopper", 4));
    let top = 1 << layer_bit(CopperLayer::Top);
    assert_eq!(net_at(&b, at(5.0, 5.0), top), Some(4), "on the arc's top");
    assert_eq!(net_at(&b, at(5.0, 0.0), top), None, "inside the bend");
    assert!(
        collect_copper(&b)
            .iter()
            .all(|c| c.item == ItemRef::Arc(b.free.arcs[0].uuid))
    );
    assert_eq!(net_items(&b, 4), vec![ItemRef::Arc(b.free.arcs[0].uuid)]);
    let length = net_routed_length_mm(&b, 4).unwrap();
    assert!(
        (length - 5.0 * std::f64::consts::PI).abs() < 1e-3,
        "{length}"
    );
}

/// #2440: Highlight Net draws every chord of a copper arc and the free pads
/// of the net; Select Net takes the arc as one item and the free pad.
#[test]
fn a_net_highlights_and_selects_its_copper_arcs_and_free_pads() {
    let mut b = board();
    b.free.arcs.push(arc("TopCopper", 4));
    let silk = TrackArc {
        uuid: Uuid::from_u128(3),
        ..arc("TopSilkscreen", 4)
    };
    b.free.arcs.push(silk);
    b.free.pads.push(pad(4));
    b.free.pads.push(Pad {
        uuid: Uuid::from_u128(4),
        ..pad(5)
    });
    let mut lit = arc_shapes(&b.free.arcs[0]).unwrap();
    assert!(lit.len() > 1, "the arc is several chords");
    lit.push(free_pad_shape(&b.free.pads[0]).unwrap());
    assert_eq!(net_copper(&b, 4), lit, "the silk arc and net 5 stay dark");
    assert_eq!(
        net_members(&b, 4),
        vec![
            ItemRef::Arc(Uuid::from_u128(1)),
            ItemRef::FreePad(Uuid::from_u128(2))
        ]
    );
    assert!(net_copper(&b, 0).is_empty());
}

#[test]
fn an_arc_off_copper_is_picked_but_is_not_copper() {
    let mut b = board();
    b.free.arcs.push(arc("TopSilkscreen", 0));
    assert!(collect_copper(&b).is_empty());
    assert!(
        BoardIndex::build(&b).is_empty(),
        "connectivity holds copper only"
    );
    let index = BoardIndex::build_selectable(&b);
    let id = ItemRef::Arc(b.free.arcs[0].uuid);
    assert_eq!(index.hit_point(at(5.0, 5.0), 0), vec![id]);
}

#[test]
fn a_box_takes_an_arc_inside_only_when_it_holds_every_chord() {
    let mut b = board();
    b.free.arcs.push(arc("TopCopper", 4));
    let index = BoardIndex::build_selectable(&b);
    let id = ItemRef::Arc(b.free.arcs[0].uuid);
    let half = RectNm::new(at(-1.0, -1.0), at(5.0, 6.0));
    assert!(index.hit_rect(&half, RectSelect::Inside).is_empty());
    assert_eq!(index.hit_rect(&half, RectSelect::Touching), vec![id]);
    let all = RectNm::new(at(-1.0, -1.0), at(11.0, 6.0));
    assert_eq!(index.hit_rect(&all, RectSelect::Inside), vec![id]);
}

#[test]
fn a_free_pad_is_copper_that_ends_ratsnest_lines() {
    let mut b = board();
    b.free.pads.push(pad(7));
    let copper = collect_copper(&b);
    assert_eq!(copper.len(), 1);
    assert_eq!(copper[0].item, ItemRef::FreePad(b.free.pads[0].uuid));
    assert_eq!(copper[0].pad_center, Some(at(30.0, 30.0)));
    assert_eq!(
        copper[0].layers,
        u32::MAX,
        "a through pad reaches every layer"
    );
    let index = BoardIndex::build(&b);
    assert_eq!(
        index.hit_point(at(30.5, 30.0), 0),
        vec![ItemRef::FreePad(b.free.pads[0].uuid)]
    );
}

#[test]
fn strings_and_dimensions_are_picked_on_what_they_draw() {
    let mut b = board();
    b.texts.push(BoardText {
        uuid: Uuid::from_u128(5),
        text: "ABCD".into(),
        position: Point::new(0.0, 20.0),
        layer: "TopSilkscreen".into(),
        font_size: 1.0,
        rotation: 0.0,
    });
    b.free.dimensions.push(LinearDimension::new(
        Uuid::from_u128(6),
        Point::new(0.0, 40.0),
        Point::new(20.0, 40.0),
        3.0,
    ));
    let index = BoardIndex::build_selectable(&b);
    assert_eq!(
        index.hit_point(at(1.0, 20.2), 0),
        vec![ItemRef::Text(Uuid::from_u128(5))],
        "inside the glyph box"
    );
    assert!(
        index.hit_point(at(3.0, 20.0), 0).is_empty(),
        "past the last glyph"
    );
    assert_eq!(
        index.hit_point(at(10.0, 43.0), 0),
        vec![ItemRef::Dimension(Uuid::from_u128(6))],
        "on the dimension line"
    );
    assert!(BoardIndex::build(&b).is_empty(), "drawings are not copper");
}

/// Two free pads of net 4 at the ends of a copper arc of net 4: the arc's
/// chords are one copper, so the pads need no ratsnest line between them.
#[test]
fn an_arc_joins_the_pads_at_its_ends_as_one_copper() {
    let mut b = board();
    let end_pad = |n: u128, x: f64| Pad {
        uuid: Uuid::from_u128(n),
        position: Point::new(x, 0.0),
        drill: None,
        pad_type: PadType::Smd,
        layers: vec!["TopCopper".into()],
        ..pad(4)
    };
    b.free.pads.push(end_pad(10, 0.0));
    b.free.pads.push(end_pad(11, 10.0));
    assert_eq!(crate::ratsnest(&b).len(), 1, "unrouted: one line");
    b.free.arcs.push(arc("TopCopper", 4));
    assert!(crate::ratsnest(&b).is_empty(), "the arc routes the pair");
}
