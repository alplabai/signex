use super::*;
use signex_types::pcb::{Segment, Via, ViaType};
use uuid::Uuid;

const TRACK: Uuid = Uuid::from_u128(1);
const VIA: Uuid = Uuid::from_u128(2);
const CROSSING: Uuid = Uuid::from_u128(3);

fn board() -> PcbBoard {
    let track = |id: Uuid, a: (f64, f64), b: (f64, f64)| Segment {
        uuid: id,
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 0,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        locked: false,
    };
    PcbBoard {
        stackup: None,
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
        // A 2 mm run along y = 10, and one crossing it at x = 1.
        segments: vec![
            track(TRACK, (0.0, 10.0), (2.0, 10.0)),
            track(CROSSING, (1.0, 9.0), (1.0, 11.0)),
        ],
        // 0.6 mm across, 3 mm right of the run's end.
        vias: vec![Via {
            uuid: VIA,
            position: Point::new(5.0, 10.0),
            diameter: 0.6,
            drill: 0.3,
            layers: vec!["TopCopper".into(), "BottomCopper".into()],
            net: 0,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
            attributes: Default::default(),
            layer_sizes: None,
            locked: false,
        }],
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn a_point_span_reads_its_length_and_both_legs() {
    let span = Span::between(Point::new(1.0, 2.0), Point::new(4.0, 6.0));
    assert!(close(span.distance_mm, 5.0));
    assert!(close(span.dx_mm, 3.0));
    assert!(close(span.dy_mm, 4.0));
}

#[test]
fn the_edge_gap_runs_between_the_outlines_not_the_centres() {
    let span =
        edge_gap(&board(), ItemRef::Track(TRACK), ItemRef::Via(VIA)).expect("both on the board");
    // 3 mm between centre line end and via centre, less the half width
    // (0.125) and the via radius (0.3).
    assert!(close(span.distance_mm, 2.575), "{span:?}");
    assert!(
        close(span.from.x, 2.125) && close(span.from.y, 10.0),
        "{span:?}"
    );
    assert!(close(span.to.x, 4.7) && close(span.to.y, 10.0), "{span:?}");
}

#[test]
fn touching_items_measure_zero_and_a_missing_item_measures_nothing() {
    let b = board();
    let span = edge_gap(&b, ItemRef::Track(TRACK), ItemRef::Track(CROSSING)).expect("both");
    assert_eq!(span.distance_mm, 0.0);
    assert_eq!(span.from, span.to);
    let gone = ItemRef::Via(Uuid::from_u128(99));
    assert!(edge_gap(&b, ItemRef::Track(TRACK), gone).is_none());
}
