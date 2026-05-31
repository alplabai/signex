//! Benchmark fixture guardrails for PCB zone stack compositing order.

use signex_renderer::pcb::PcbSnapshot;
use signex_types::pcb::PcbBoard;

fn benchmark_fixture_board() -> PcbBoard {
    serde_json::from_str(include_str!("fixtures/pcb_zone_stack_benchmark_fixture.json"))
        .expect("valid PCB zone stack benchmark fixture")
}

#[test]
fn benchmark_fixture_zone_order_is_layer_then_priority_then_net() {
    let board = benchmark_fixture_board();
    let snapshot = PcbSnapshot::from_board(&board);

    let ordered: Vec<(String, u32, u32)> = snapshot
        .zones
        .iter()
        .map(|zone| (zone.layer_name.clone(), zone.priority, zone.net))
        .collect();

    let expected = vec![
        ("b.cu".to_string(), 0, 0),
        ("b.cu".to_string(), 1, 2),
        ("b.cu".to_string(), 1, 5),
        ("b.cu".to_string(), 3, 1),
        ("in1.cu".to_string(), 1, 0),
        ("in1.cu".to_string(), 1, 3),
        ("in1.cu".to_string(), 2, 2),
        ("f.cu".to_string(), 0, 4),
        ("f.cu".to_string(), 0, 9),
        ("f.cu".to_string(), 2, 1),
        ("eco1.user".to_string(), 4, 7),
    ];

    assert_eq!(ordered, expected);
}

#[test]
fn benchmark_fixture_rule_area_order_uses_same_layer_top_rule() {
    let board = benchmark_fixture_board();
    let snapshot = PcbSnapshot::from_board(&board);

    let ordered: Vec<(String, u32, u32)> = snapshot
        .rule_areas
        .iter()
        .map(|zone| (zone.layer_name.clone(), zone.priority, zone.net))
        .collect();

    let expected = vec![
        ("f.cu".to_string(), 1, 0),
        ("f.cu".to_string(), 1, 2),
    ];

    assert_eq!(ordered, expected);
}
