use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::{CopperLayer, InnerCopper, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::*;

/// A two-layer board with nets 3 (`GND`) and 4 (`VCC`) and a net-3 track
/// along y = 0 on the top layer.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"clearance": 0.2},
        "nets": [{"number": 3, "name": "GND"}, {"number": 4, "name": "VCC"}],
        "segments": [{"uuid": Uuid::from_u128(2), "start": {"x": 0.0, "y": 0.0},
                      "end": {"x": 10.0, "y": 0.0}, "width": 0.5,
                      "layer": "TopCopper", "net": 3}]
    }))
    .unwrap()
}

fn square(x: f64, y: f64) -> Vec<Point> {
    draw::rectangle(Point::new(x, y), Point::new(x + 4.0, y + 4.0))
}

const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);

#[test]
fn a_named_area_is_the_area_the_tool_adds() {
    let board = board();
    let id = Uuid::from_u128(9);
    for kind in [
        AreaKind::Fill,
        AreaKind::SolidRegion,
        AreaKind::PolygonPour,
        AreaKind::Keepout,
        AreaKind::PourCutout,
    ] {
        let named = named_area(&board, kind, id, square(1.0, 0.0), TOP, None);
        assert_eq!(
            named.ok(),
            new_area(&board, kind, id, square(1.0, 0.0), TOP),
            "{}",
            kind.id()
        );
    }
    let pour = named_area(
        &board,
        AreaKind::PolygonPour,
        id,
        square(1.0, 0.0),
        TOP,
        None,
    )
    .unwrap();
    assert!(pour.is_pour());
    assert_eq!(pour.net, 3, "the net of the copper under its first corner");
}

#[test]
fn a_named_net_takes_the_place_of_the_copper_under_the_corners() {
    let board = board();
    let id = Uuid::from_u128(9);
    let pour = named_area(
        &board,
        AreaKind::PolygonPour,
        id,
        square(1.0, 0.0),
        TOP,
        Some(4),
    );
    let pour = pour.unwrap();
    assert_eq!((pour.net, pour.net_name.as_str()), (4, "VCC"));
    let netless = named_area(&board, AreaKind::Fill, id, square(1.0, 0.0), TOP, Some(0)).unwrap();
    assert_eq!((netless.net, netless.net_name.as_str()), (0, ""));
    assert_eq!(
        named_area(
            &board,
            AreaKind::Keepout,
            id,
            square(1.0, 0.0),
            TOP,
            Some(4)
        ),
        Err(AreaRefusal::TakesNoNet)
    );
}

#[test]
fn a_named_layer_must_hold_the_area_and_be_on_the_board() {
    let board = board();
    let id = Uuid::from_u128(9);
    let silk = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    let keep_out = PcbLayer::Technical(TechnicalLayer::KeepoutTop);
    let inner = PcbLayer::Copper(CopperLayer::Inner(InnerCopper::new(2).unwrap()));
    assert_eq!(
        named_area(
            &board,
            AreaKind::PolygonPour,
            id,
            square(1.0, 5.0),
            silk,
            None
        ),
        Err(AreaRefusal::NeedsCopper)
    );
    assert_eq!(
        named_area(
            &board,
            AreaKind::PourCutout,
            id,
            square(1.0, 5.0),
            keep_out,
            None
        ),
        Err(AreaRefusal::NeedsCopper)
    );
    assert_eq!(
        named_area(&board, AreaKind::Keepout, id, square(1.0, 5.0), silk, None),
        Err(AreaRefusal::NeedsKeepoutLayer)
    );
    let keepout = named_area(
        &board,
        AreaKind::Keepout,
        id,
        square(1.0, 5.0),
        keep_out,
        None,
    );
    assert_eq!(keepout.unwrap().layer, "TopKeepout");
    assert_eq!(
        named_area(
            &board,
            AreaKind::SolidRegion,
            id,
            square(1.0, 5.0),
            inner,
            None
        ),
        Err(AreaRefusal::LayerNotOnBoard),
        "a two-layer board has no inner copper"
    );
}

#[test]
fn an_outline_with_no_area_is_refused() {
    let board = board();
    let id = Uuid::from_u128(9);
    let line = vec![
        Point::new(0.0, 5.0),
        Point::new(2.0, 5.0),
        Point::new(4.0, 5.0),
    ];
    assert_eq!(
        named_area(&board, AreaKind::SolidRegion, id, line, TOP, None),
        Err(AreaRefusal::NoArea)
    );
    let two = vec![Point::new(0.0, 5.0), Point::new(2.0, 7.0)];
    assert_eq!(
        named_area(&board, AreaKind::PourCutout, id, two, TOP, None),
        Err(AreaRefusal::NoArea)
    );
}
