use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// R1 (10k, R_0603) and R2 (10k, R_0805) with one pad each on VCC.
fn board() -> PcbBoard {
    let fp = |n: u128, reference: &str, footprint: &str| {
        serde_json::json!({
            "uuid": id(n), "reference": reference, "value": "10k",
            "footprint_id": footprint,
            "position": {"x": n as f64, "y": 0.0}, "layer": "TopCopper",
            "pads": [{"uuid": id(n + 100), "number": "1", "pad_type": "smd",
                      "shape": "rect", "position": {"x": 0.0, "y": 0.0},
                      "size": {"x": 1.0, "y": 1.0}, "drill": null,
                      "layers": ["TopCopper"], "net": {"number": 1, "name": "VCC"}}]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": id(1000),
        "nets": [{"number": 0, "name": ""}, {"number": 1, "name": "VCC"}],
        "footprints": [fp(1, "R1", "R_0603"), fp(2, "R2", "R_0805")]
    }))
    .unwrap()
}

#[test]
fn a_scan_finds_designators_values_footprints_and_nets_without_case() {
    let b = board();
    let kinds = |query: &str| -> Vec<(BoardFindKind, String)> {
        scan_board(&b, query)
            .into_iter()
            .map(|hit| (hit.kind, hit.text))
            .collect()
    };
    assert_eq!(kinds("r1"), [(BoardFindKind::Designator, "R1".to_string())]);
    assert_eq!(kinds("10K").len(), 2, "both values");
    assert_eq!(
        kinds("0805"),
        [(BoardFindKind::Footprint, "R_0805".to_string())]
    );
    let net = scan_board(&b, "vcc");
    assert_eq!(net.len(), 1);
    assert_eq!(net[0].kind, BoardFindKind::Net);
    assert_eq!(net[0].items.len(), 2, "the net's two pads");
    assert!(scan_board(&b, "  ").is_empty());
}

#[test]
fn replace_writes_designators_and_values_as_one_batch_and_skips_nets() {
    let mut engine = BoardEngine::new(board(), 1);
    let hits = scan_board(engine.board(), "10k");
    let commands = replace_commands(engine.board(), &hits, "10k", "4k7");
    assert_eq!(commands.len(), 2, "one per footprint");
    engine.execute_batch(commands).unwrap();
    assert!(engine.board().footprints.iter().all(|fp| fp.value == "4k7"));

    let nets = scan_board(engine.board(), "VCC");
    assert!(replace_commands(engine.board(), &nets, "VCC", "3V3").is_empty());
    assert!(!BoardFindKind::Net.is_replaceable());
}
