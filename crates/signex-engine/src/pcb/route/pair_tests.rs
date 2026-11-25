//! #2389 — differential pairs: found by name or by class, their width and
//! gap from the class within the rule clearance, and their lengths matched
//! by lengthening the shorter net in one undo step.

use pcb_geom::{CopperItem, net_routed_length_mm};
use serde_json::json;
use signex_drc::DrcRules;
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::tests::track;
use super::*;
use crate::pcb::BoardEngine;

fn named_board(segments: Vec<serde_json::Value>) -> PcbBoard {
    serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [
            {"number": 1, "name": "USB_P"}, {"number": 2, "name": "USB_N"},
            {"number": 3, "name": "CLK+"}, {"number": 4, "name": "CLK-"},
            {"number": 5, "name": "LVDS_A"}, {"number": 6, "name": "LVDS_B"},
            {"number": 7, "name": "GND"}
        ],
        "segments": segments
    }))
    .unwrap()
}

fn lvds_class() -> NetClass {
    NetClass {
        nets: vec!["LVDS_A".into(), "LVDS_B".into()],
        diff_pair_gap: 0.15,
        diff_pair_width: 0.12,
        ..NetClass::named("LVDS")
    }
}

#[test]
fn a_pair_is_found_by_its_name_ending_from_either_net() {
    let board = named_board(Vec::new());
    let usb = Some(PairNets {
        positive: 1,
        negative: 2,
    });
    assert_eq!(diff_pair_of(&board, &[], 1), usb);
    assert_eq!(diff_pair_of(&board, &[], 2), usb, "from the negative net");
    assert_eq!(
        diff_pair_of(&board, &[], 4),
        Some(PairNets {
            positive: 3,
            negative: 4
        })
    );
    assert_eq!(diff_pair_of(&board, &[], 7), None, "GND pairs with nothing");
    assert_eq!(diff_pair_of(&board, &[], 0), None, "no net");
}

#[test]
fn a_pair_is_found_by_a_class_with_a_pair_gap_holding_two_nets() {
    let board = named_board(Vec::new());
    let classes = [lvds_class()];
    let lvds = Some(PairNets {
        positive: 5,
        negative: 6,
    });
    assert_eq!(diff_pair_of(&board, &classes, 6), lvds);
    assert_eq!(diff_pair_of(&board, &[], 5), None, "no class, no name");
    let no_gap = NetClass {
        diff_pair_gap: 0.0,
        ..lvds_class()
    };
    assert_eq!(diff_pair_of(&board, &[no_gap], 5), None, "not a pair class");
    let three = NetClass {
        nets: vec!["LVDS_A".into(), "LVDS_B".into(), "GND".into()],
        ..lvds_class()
    };
    assert_eq!(diff_pair_of(&board, &[three], 5), None, "three nets");
}

#[test]
fn the_pair_width_and_gap_come_from_the_class_within_the_rules() {
    let class = lvds_class();
    assert_eq!(pair_width_gap(Some(&class), 0.25, 0.2, 0.1), (0.12, 0.15));
    assert_eq!(
        pair_width_gap(Some(&class), 0.25, 0.2, 0.2),
        (0.12, 0.2),
        "never under the rule clearance"
    );
    assert_eq!(pair_width_gap(None, 0.25, 0.2, 0.1), (0.25, 0.2));
}

#[test]
fn matching_a_pair_lengthens_the_shorter_net_in_one_step() {
    let board = named_board(vec![
        track(11, (0.0, 0.0), (20.0, 0.0), 1),
        track(12, (0.0, 10.0), (15.0, 10.0), 2),
    ]);
    let before = serde_json::to_string(&board).unwrap();
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let meander = Meander {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        pitch_mm: 1.0,
        amplitude_mm: 3.0,
    };
    let pair = PairNets {
        positive: 1,
        negative: 2,
    };
    let (commands, tuned) = tune_pair(&board, pair, &meander, Uuid::now_v7).expect("matched");
    assert_eq!(tuned.net, 2, "the shorter net");
    let mut engine = BoardEngine::new(board, 1);
    engine.execute_batch(commands).unwrap();
    let p = net_routed_length_mm(engine.board(), 1).unwrap();
    let n = net_routed_length_mm(engine.board(), 2).unwrap();
    assert!((p - 20.0).abs() < 1e-9, "the longer net is left: {p}");
    assert!((n - 20.0).abs() < 1e-3, "matched: {n}");
    engine.undo().unwrap();
    assert_eq!(serde_json::to_string(engine.board()).unwrap(), before);
    let again = named_board(vec![
        track(11, (0.0, 0.0), (20.0, 0.0), 1),
        track(12, (0.0, 10.0), (20.0, 10.0), 2),
    ]);
    assert!(matches!(
        tune_pair(&again, pair, &meander, Uuid::now_v7),
        Err(TuneRefusal::LongEnough { .. })
    ));
}

#[test]
fn several_nets_are_tuned_to_one_target_and_long_ones_left() {
    let board = named_board(vec![
        track(11, (0.0, 0.0), (20.0, 0.0), 5),
        track(12, (0.0, 10.0), (20.0, 10.0), 6),
        track(13, (0.0, 20.0), (30.0, 20.0), 7),
    ]);
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let meander = Meander {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        pitch_mm: 1.0,
        amplitude_mm: 3.0,
    };
    let tuned = tune_nets(&board, &[5, 6, 7], 26.0, &meander, Uuid::now_v7).expect("tuned");
    assert!(
        matches!(
            tuned.skipped.as_slice(),
            [(7, TuneRefusal::LongEnough { .. })]
        ),
        "{:?}",
        tuned.skipped
    );
    let commands = tuned.commands;
    let before = serde_json::to_string(&board).unwrap();
    let mut engine = BoardEngine::new(board, 1);
    engine.execute_batch(commands).unwrap();
    for net in [5, 6] {
        let length = net_routed_length_mm(engine.board(), net).unwrap();
        assert!((length - 26.0).abs() < 1e-3, "net {net}: {length}");
    }
    let long = net_routed_length_mm(engine.board(), 7).unwrap();
    assert!((long - 30.0).abs() < 1e-9, "already longer: left");
    engine.undo().unwrap();
    assert_eq!(
        serde_json::to_string(engine.board()).unwrap(),
        before,
        "one undo step"
    );
}

#[test]
fn a_net_with_only_locked_tracks_is_left_and_named_while_the_others_are_tuned() {
    // Net 5: locked, short. Net 6: free, short. Net 7: locked, long enough.
    let mut board = named_board(vec![
        track(11, (0.0, 0.0), (20.0, 0.0), 5),
        track(12, (0.0, 10.0), (20.0, 10.0), 6),
        track(13, (0.0, 20.0), (30.0, 20.0), 7),
    ]);
    board
        .segments
        .iter_mut()
        .filter(|s| s.net != 6)
        .for_each(|s| s.locked = true);
    let rules = DrcRules::for_board(&board, &[]).unwrap();
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let meander = Meander {
        clearance: &clearance,
        reach: rules.widest_clearance(),
        pitch_mm: 1.0,
        amplitude_mm: 3.0,
    };
    let tuned = tune_nets(&board, &[5, 6, 7], 26.0, &meander, Uuid::now_v7).expect("net 6 tuned");
    assert_eq!(tuned.skipped.len(), 2, "{:?}", tuned.skipped);
    assert_eq!(
        tuned.skipped[0],
        (
            5,
            TuneRefusal::Locked(vec![format!("track {}", Uuid::from_u128(11))])
        )
    );
    assert!(
        matches!(tuned.skipped[1], (7, TuneRefusal::LongEnough { .. })),
        "a locked net already long enough says so, not that it is locked"
    );
    let locked: Vec<_> = board
        .segments
        .iter()
        .filter(|s| s.locked)
        .cloned()
        .collect();
    let mut engine = BoardEngine::new(board, 1);
    engine.execute_batch(tuned.commands).unwrap();
    let length = net_routed_length_mm(engine.board(), 6).unwrap();
    assert!((length - 26.0).abs() < 1e-3, "net 6: {length}");
    assert!(
        locked.iter().all(|s| engine.board().segments.contains(s)),
        "the locked tracks stay as they were"
    );
}

/// #2492: a pair member routed along a track of its own net already on the
/// board lays only what that track does not cover, ending on it.
#[test]
fn a_pair_member_along_its_own_track_lays_only_the_rest() {
    let board = named_board(vec![
        track(21, (0.0, 0.5), (1.0, 0.5), 1),
        track(22, (0.0, -0.5), (1.0, -0.5), 2),
        track(23, (4.0, -0.25), (12.0, -0.25), 2),
    ]);
    let pair = diff_pair_of(&board, &[], 1).expect("USB_P pairs with USB_N");
    let members = [
        BusMember {
            net: pair.positive,
            anchor: Point::new(1.0, 0.5),
        },
        BusMember {
            net: pair.negative,
            anchor: Point::new(1.0, -0.5),
        },
    ];
    let style = BusStyle {
        layer: signex_types::pcb_layer::CopperLayer::Top,
        width_mm: 0.25,
        pitch_mm: 0.5,
    };
    let centre = [bus_start(&members).unwrap(), Point::new(10.0, 0.0)];
    let tracks = bus_tracks(&board, &members, &centre, &style, Uuid::now_v7).expect("tracks");
    let negative: Vec<_> = tracks.iter().filter(|t| t.net == 2).collect();
    assert_eq!(negative.len(), 2, "the join and the run up to the track");
    assert_eq!(negative[1].end, Point::new(4.0, -0.25), "ends on the track");
    assert_eq!(tracks.iter().filter(|t| t.net == 1).count(), 2, "USB_P is whole");
}
