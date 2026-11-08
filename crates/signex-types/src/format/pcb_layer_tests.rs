//! Layer-name codec (#2173): every known layer has one name, and a name
//! this build does not know reads as `None` and is kept as written.

use std::collections::HashSet;

use super::{parse_pcb_layer, pcb_layer_token};
use crate::pcb_layer::{CopperLayer, InnerCopper, MechanicalLayer, PcbLayer, TechnicalLayer};

#[test]
fn every_layer_round_trips_through_its_token() {
    for layer in PcbLayer::all() {
        let token = pcb_layer_token(layer);
        assert_eq!(parse_pcb_layer(&token), Some(layer), "{token}");
    }
}

#[test]
fn tokens_are_distinct() {
    let tokens: HashSet<String> = PcbLayer::all().map(pcb_layer_token).collect();
    assert_eq!(tokens.len(), PcbLayer::COUNT);
}

#[test]
fn names_already_on_disk_keep_their_meaning() {
    let inner = |n| PcbLayer::Copper(CopperLayer::Inner(InnerCopper::new(n).unwrap()));
    let cases = [
        ("TopCopper", PcbLayer::Copper(CopperLayer::Top)),
        ("Inner1Copper", inner(1)),
        ("Inner2Copper", inner(2)),
        ("Inner30Copper", inner(30)),
        ("BottomCopper", PcbLayer::Copper(CopperLayer::Bottom)),
        ("EdgeCuts", PcbLayer::Technical(TechnicalLayer::EdgeCuts)),
        (
            "TopSilkscreen",
            PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
        ),
        (
            "BottomSilkscreen",
            PcbLayer::Technical(TechnicalLayer::SilkscreenBottom),
        ),
        ("TopFab", PcbLayer::Technical(TechnicalLayer::FabTop)),
        (
            "Mechanical1",
            PcbLayer::Mechanical(MechanicalLayer::new(1).unwrap()),
        ),
        (
            "Mechanical32",
            PcbLayer::Mechanical(MechanicalLayer::new(32).unwrap()),
        ),
    ];
    for (token, layer) in cases {
        assert_eq!(parse_pcb_layer(token), Some(layer), "{token}");
        assert_eq!(pcb_layer_token(layer), token);
    }
}

#[test]
fn unknown_and_non_canonical_names_are_not_typed() {
    for token in [
        "",
        "Margin ",
        " Margin",
        "margin",
        "topcopper",
        "Inner0Copper",
        "Inner31Copper",
        "Inner01Copper",
        "Inner+1Copper",
        "InnerCopper",
        "Inner256Copper",
        "User.1",
        "User.6",
        "User.0",
        "User.7",
        "User1",
        "Mechanical",
        "Mechanical0",
        "Mechanical01",
        "Mechanical+1",
        "Mechanical33",
        "Mechanical256",
        "mechanical1",
        "TopCopper,BottomCopper",
    ] {
        assert_eq!(parse_pcb_layer(token), None, "{token:?}");
    }
}

#[test]
fn mechanical_layers_are_board_wide_and_do_not_flip() {
    let all: Vec<_> = PcbLayer::all()
        .filter(|l| matches!(l, PcbLayer::Mechanical(_)))
        .collect();
    assert_eq!(all.len(), 32);
    for layer in all {
        assert_eq!(layer.side(), None, "{layer:?}");
        assert_eq!(layer.flipped(), layer, "{layer:?}");
    }
}

#[test]
fn accessors_keep_unknown_names_as_strings() {
    let segment = crate::pcb::Segment {
        uuid: uuid::Uuid::nil(),
        start: crate::pcb::Point { x: 0.0, y: 0.0 },
        end: crate::pcb::Point { x: 1.0, y: 0.0 },
        width: 0.25,
        layer: "Mechanical33".into(),
        net: 0,
        unknown_columns: Default::default(),
    };
    assert_eq!(segment.pcb_layer(), None);
    assert_eq!(segment.layer, "Mechanical33");

    let via = crate::pcb::Via {
        uuid: uuid::Uuid::nil(),
        position: crate::pcb::Point { x: 0.0, y: 0.0 },
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "Mechanical33".into()],
        net: 0,
        via_type: crate::pcb::ViaType::Through,
        unknown_columns: Default::default(),
    };
    let typed: Vec<_> = via.pcb_layers().collect();
    assert_eq!(typed, vec![Some(PcbLayer::Copper(CopperLayer::Top)), None]);
}
