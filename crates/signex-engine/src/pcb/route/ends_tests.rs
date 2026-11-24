//! Every pad at a route's end is judged, on the leg's layer or the end
//! via's span, whatever the board's order, free pads included.

use serde_json::json;
use signex_types::pcb::{Pad, Point};
use signex_types::pcb_layer::{CopperLayer, InnerCopper};
use uuid::Uuid;

use super::tests::{A, B, board, style};
use super::*;
use crate::pcb::draw::new_via;

const AT: Point = Point::new(0.0, 0.0);

fn pad_json(n: u128, layer: &str, net: (u32, &str)) -> serde_json::Value {
    json!({
        "uuid": Uuid::from_u128(0x200 + n), "number": "1", "pad_type": "smd",
        "shape": "circle", "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
        "drill": null, "layers": [layer], "net": {"number": net.0, "name": net.1}
    })
}

/// A part `P{n}` at the origin holding one pad on `layer`.
fn part(n: u128, layer: &str, net: (u32, &str)) -> serde_json::Value {
    json!({
        "uuid": Uuid::from_u128(0x100 + n), "reference": format!("P{n}"), "value": "",
        "footprint_id": "Pad", "layer": layer, "rotation": 0.0,
        "position": {"x": 0.0, "y": 0.0},
        "pads": [pad_json(n, layer, net)]
    })
}

fn inner() -> CopperLayer {
    CopperLayer::Inner(InnerCopper::new(1).expect("inner 1"))
}

/// Whether a net A leg on `layer` ending at the origin needs a via and is
/// refused, the same with the two parts either way round.
fn judged(first: serde_json::Value, second: serde_json::Value, layer: CopperLayer) -> (bool, bool) {
    let outcome = |parts: Vec<serde_json::Value>| {
        let board = board(parts, Vec::new());
        let mut leg = style(layer);
        let via = end_via(&board, AT, &mut leg, |at| {
            new_via(&board, Uuid::from_u128(9), at)
        });
        let end = pad_end(&board, AT, &leg, via.as_ref());
        (via.is_some(), matches!(end, PadEnd::OtherNet { .. }))
    };
    let one = outcome(vec![first.clone(), second.clone()]);
    let other = outcome(vec![second, first]);
    assert_eq!(one, other, "the board's order changed the answer");
    one
}

#[test]
fn an_own_top_pad_over_a_foreign_bottom_pad_takes_a_top_leg() {
    let own = part(1, "TopCopper", A);
    let foreign = part(2, "BottomCopper", B);
    assert_eq!(judged(own, foreign, CopperLayer::Top), (false, false));
}

#[test]
fn a_foreign_top_pad_over_an_own_bottom_pad_refuses_a_top_leg() {
    let foreign = part(1, "TopCopper", B);
    let own = part(2, "BottomCopper", A);
    assert_eq!(judged(foreign, own, CopperLayer::Top), (false, true));
}

#[test]
fn an_inner_leg_is_refused_when_its_via_would_reach_a_foreign_pad() {
    let own = part(1, "TopCopper", A);
    let foreign = part(2, "BottomCopper", B);
    assert_eq!(judged(own, foreign, inner()), (true, true));
}

#[test]
fn a_layer_change_via_is_refused_on_a_foreign_pad_of_its_span() {
    for parts in [
        vec![part(1, "TopCopper", A), part(2, "BottomCopper", B)],
        vec![part(2, "BottomCopper", B), part(1, "TopCopper", A)],
    ] {
        let board = board(parts, Vec::new());
        let via = new_via(&board, Uuid::from_u128(9), AT);
        assert!(matches!(
            via_pad_end(&board, &via, A.0),
            PadEnd::OtherNet { .. }
        ));
    }
}

fn free_pad(layer: &str, net: (u32, &str)) -> Pad {
    serde_json::from_value(pad_json(7, layer, net)).unwrap()
}

#[test]
fn a_leg_onto_a_foreign_free_pad_is_refused() {
    let mut board = board(Vec::new(), Vec::new());
    board.free.pads.push(free_pad("TopCopper", B));
    let leg = style(CopperLayer::Top);
    assert!(matches!(
        pad_end(&board, AT, &leg, None),
        PadEnd::OtherNet { .. }
    ));
}

#[test]
fn a_same_net_free_pad_off_the_legs_layer_gets_an_end_via() {
    let mut board = board(Vec::new(), Vec::new());
    board.free.pads.push(free_pad("BottomCopper", A));
    let mut leg = style(CopperLayer::Top);
    let via = end_via(&board, AT, &mut leg, |at| {
        new_via(&board, Uuid::from_u128(9), at)
    });
    assert_eq!(via.as_ref().map(|via| via.net), Some(A.0));
    assert_eq!(pad_end(&board, AT, &leg, via.as_ref()), PadEnd::Free);
}
