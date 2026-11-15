//! #2389 — length tuning: a track becomes an accordion that brings its net
//! to the target, on the side clear of other nets, in one undo step that
//! passes the design rule check.

use pcb_geom::{CopperItem, net_routed_length_mm};
use signex_drc::DrcRules;
use uuid::Uuid;

use super::tests::{board, track};
use super::*;
use crate::pcb::BoardEngine;

const TRACK: u128 = 11;

/// Net A's 20 mm track along y = 0 and a net-B track 2 mm above its middle.
fn tuning_board() -> PcbBoard {
    board(
        Vec::new(),
        vec![
            track(TRACK, (0.0, 0.0), (20.0, 0.0), 1),
            track(12, (5.0, 2.0), (15.0, 2.0), 2),
        ],
    )
}

fn tune(
    board: &PcbBoard,
    rules: &DrcRules,
    target_mm: f64,
) -> Result<Vec<BoardCommand>, TuneRefusal> {
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let meander = Meander {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        pitch_mm: 1.0,
        amplitude_mm: 3.0,
    };
    tune_length(
        board,
        Uuid::from_u128(TRACK),
        target_mm,
        &meander,
        Uuid::now_v7,
    )
}

#[test]
fn tuning_reaches_the_target_on_the_clear_side_in_one_step() {
    let board = tuning_board();
    let before = serde_json::to_string(&board).unwrap();
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let commands = tune(&board, &rules, 26.0).expect("tuned");
    let mut engine = BoardEngine::new(board, 1);
    engine.execute_batch(commands).unwrap();
    let tuned = engine.board();
    let length = net_routed_length_mm(tuned, 1).unwrap();
    assert!((length - 26.0).abs() < 1.0e-3, "net length {length}");
    let net_a: Vec<_> = tuned.segments.iter().filter(|s| s.net == 1).collect();
    assert_eq!(net_a.len(), 5, "one bump: five tracks");
    assert!(
        net_a.iter().all(|s| s.start.y <= 0.0 && s.end.y <= 0.0),
        "the bump goes down, away from the net-B track"
    );
    let report = signex_drc::run(tuned, &rules);
    assert!(report.is_clean(), "{:?}", report.violations);
    engine.undo().unwrap();
    assert_eq!(serde_json::to_string(engine.board()).unwrap(), before);
}

#[test]
fn tuning_refuses_a_net_long_enough_or_a_track_too_short() {
    let board = tuning_board();
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let long_enough = tune(&board, &rules, 15.0);
    assert!(
        matches!(
            long_enough,
            Err(TuneRefusal::LongEnough { current_mm }) if (current_mm - 20.0).abs() < 1.0e-9
        ),
        "{long_enough:?}"
    );
    assert_eq!(tune(&board, &rules, 200.0), Err(TuneRefusal::TooShort));
}

#[test]
fn tuning_refuses_a_shortfall_smaller_than_one_meander() {
    // A 1 mm pitch: a bump is at least 0.5 mm tall and adds 1 mm.
    let board = tuning_board();
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let refused = tune(&board, &rules, 20.05);
    assert!(
        matches!(
            refused,
            Err(TuneRefusal::TooLittle { least_mm }) if (least_mm - 1.0).abs() < 1.0e-9
        ),
        "{refused:?}"
    );
}
