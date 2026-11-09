//! The design-rule check on a real `.snxpcb` corpus board, and its cost on
//! 10,000 tracks (#2222).

use std::path::Path;
use std::time::Instant;

use pcb_geom::{DrcRules, ViolationKind, run_drc};
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point, Segment};
use uuid::Uuid;

fn corpus_board(name: &str) -> PcbBoard {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb")
        .join(name);
    let text = std::fs::read_to_string(path).expect("corpus file");
    SnxPcb::parse(&text).expect("corpus parses").board
}

#[test]
fn the_routed_corpus_board_has_two_touching_pads_of_different_nets() {
    let mut board = corpus_board("routed_board.snxpcb");
    let rules = DrcRules::for_board(&board, &[]).expect("rules");
    let report = run_drc(&board, &rules);
    assert!(report.skipped.is_empty());
    // 1 mm wide pads at 1 mm pitch share an edge.
    assert_eq!(report.count(ViolationKind::Short), 1, "{report:?}");
    assert_eq!(report.violations.len(), 1);

    // Narrowed to 0.5 mm they stand 0.5 mm apart, over the 0.2 mm default.
    for pad in &mut board.footprints[0].pads {
        pad.size.x = 0.5;
    }
    assert!(run_drc(&board, &rules).violations.is_empty());
}

/// 100 nets, each a row of 100 tracks 2 mm long and 0.2 mm wide, rows 1 mm
/// apart: the copper of neighbouring rows is 0.8 mm apart.
fn ten_thousand_tracks() -> PcbBoard {
    let mut board = corpus_board("routed_board.snxpcb");
    board.footprints.clear();
    board.vias.clear();
    board.segments.clear();
    for row in 0..100u32 {
        for col in 0..100u32 {
            let x = f64::from(col) * 2.0;
            board.segments.push(Segment {
                uuid: Uuid::from_u128(u128::from(row) * 100 + u128::from(col) + 1),
                start: Point::new(x, f64::from(row)),
                end: Point::new(x + 2.0, f64::from(row)),
                width: 0.2,
                layer: "TopCopper".into(),
                net: row + 1,
                unknown_columns: Default::default(),
            });
        }
    }
    board
}

#[test]
fn ten_thousand_tracks_are_checked_within_the_debug_budget() {
    let board = ten_thousand_tracks();
    assert_eq!(board.segments.len(), 10_000);

    // Gap between neighbouring rows is exactly 800_000 nm: at the rule, clean.
    let started = Instant::now();
    let at_rule = run_drc(&board, &DrcRules::new(800_000));
    let took = started.elapsed();
    println!("drc 10k tracks, clean: {took:?}");
    assert!(
        at_rule.violations.is_empty(),
        "{:?}",
        at_rule.violations.len()
    );
    assert!(took.as_millis() < 10_000, "{took:?}");

    // 1 nm over: every track meets the three tracks of the next row that
    // it faces (columns c - 1, c and c + 1), 99 row pairs of 298 each.
    let started = Instant::now();
    let over = run_drc(&board, &DrcRules::new(800_001));
    let took = started.elapsed();
    println!(
        "drc 10k tracks, {} violations: {took:?}",
        over.violations.len()
    );
    assert_eq!(over.count(ViolationKind::Clearance), 99 * 298);
    assert_eq!(over.violations.len(), 99 * 298);
    assert!(took.as_millis() < 10_000, "{took:?}");
}
