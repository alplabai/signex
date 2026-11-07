//! Typed layers over the corpus (#2173). The byte round trip of these
//! fixtures is `round_trip_corpus`; this checks what the typed view reads.

use std::path::PathBuf;

use signex_types::format::{SnxPcb, pcb_layer_token};
use signex_types::pcb_layer::PcbLayer;

fn load(name: &str) -> (String, SnxPcb) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/pcb")
        .join(name);
    let text = std::fs::read_to_string(&path).expect("fixture reads");
    let pcb = SnxPcb::parse(&text).expect("fixture parses");
    (text, pcb)
}

#[test]
fn every_known_name_reads_as_its_layer_in_order() {
    let (_, pcb) = load("layers-every-known-name.snxpcb");
    let typed: Vec<_> = pcb
        .board
        .segments
        .iter()
        .map(|s| {
            s.pcb_layer()
                .unwrap_or_else(|| panic!("{} untyped", s.layer))
        })
        .collect();
    assert_eq!(typed, PcbLayer::all().collect::<Vec<_>>());
    for segment in &pcb.board.segments {
        assert_eq!(pcb_layer_token(segment.pcb_layer().unwrap()), segment.layer);
    }
}

#[test]
fn unknown_names_stay_untyped_and_are_written_back() {
    let (text, pcb) = load("layers-unknown-names.snxpcb");
    assert!(!pcb.board.segments.is_empty());
    for segment in &pcb.board.segments {
        assert_eq!(segment.pcb_layer(), None, "{}", segment.layer);
    }
    let via_layers: Vec<_> = pcb.board.vias[0].pcb_layers().collect();
    assert_eq!(via_layers.iter().filter(|l| l.is_none()).count(), 2);
    assert_eq!(pcb.write_string().expect("fixture writes"), text);
}

#[test]
fn existing_fixture_layers_are_all_typed() {
    for name in [
        "board-graphics.snxpcb",
        "pad-multi-layer.snxpcb",
        "stackup-four-layer.snxpcb",
        "tracks-outer-and-inner-layers.snxpcb",
    ] {
        let (_, pcb) = load(name);
        let b = &pcb.board;
        let singles = b
            .layers
            .iter()
            .map(|l| (l.name.as_str(), l.pcb_layer()))
            .chain(b.segments.iter().map(|s| (s.layer.as_str(), s.pcb_layer())))
            .chain(b.graphics.iter().map(|g| (g.layer.as_str(), g.pcb_layer())))
            .chain(
                b.footprints
                    .iter()
                    .map(|f| (f.layer.as_str(), f.pcb_layer())),
            );
        for (raw, typed) in singles {
            assert!(typed.is_some(), "{name}: {raw:?} untyped");
        }
        for pad in b.footprints.iter().flat_map(|f| &f.pads) {
            assert!(
                pad.pcb_layers().all(|l| l.is_some()),
                "{name}: {:?}",
                pad.layers
            );
        }
    }
}
