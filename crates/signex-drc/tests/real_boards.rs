//! The design-rule check and the ratsnest on the five realistic corpus
//! boards (`signex-types/tests/corpus/pcb/real/`), against each board's own
//! net classes from its `.snxprj`.

use std::path::{Path, PathBuf};

use pcb_geom::ratsnest;
use signex_drc::{DrcReport, DrcRules, ViolationKind, run as run_drc};
use signex_types::format::SnxPcb;
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use signex_types::project::parse_project_bytes;

fn real_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../signex-types/tests/corpus/pcb/real")
}

fn load(name: &str) -> (PcbBoard, Vec<NetClass>) {
    let text = std::fs::read_to_string(real_dir().join(format!("{name}.snxpcb"))).unwrap();
    let board = SnxPcb::parse(&text).expect("board parses").board;
    let project = real_dir().join(format!("{name}.snxprj"));
    let bytes = std::fs::read(&project).unwrap();
    let classes = parse_project_bytes(&project, &bytes)
        .expect("project parses")
        .net_classes;
    (board, classes)
}

fn drc(name: &str) -> DrcReport {
    let (board, classes) = load(name);
    let rules = DrcRules::for_board(&board, &classes).expect("rules");
    assert!(!rules.from_default, "{name}: the board sets its clearance");
    run_drc(&board, &rules)
}

/// `(name, clearance, unconnected, keepout, ratsnest lines)`; shorts are
/// zero on every board.
const EXPECTED: [(&str, usize, usize, usize, usize); 5] = [
    ("linear-regulator", 0, 0, 0, 0),
    ("usb-c-power-input", 0, 0, 0, 0),
    ("mcu-breakout-tqfp32", 0, 0, 0, 0),
    ("sensor-4layer", 0, 0, 0, 0),
    ("mixed-tht-smd", 0, 0, 0, 0),
];

#[test]
fn every_real_board_is_free_of_shorts_and_matches_its_pinned_drc_counts() {
    for (name, clearance, unconnected, keepout, _) in EXPECTED {
        let report = drc(name);
        let listing = || {
            report
                .violations
                .iter()
                .map(|v| format!("  {:?} nets {:?} at {:?}", v.kind, v.nets, v.at.to_mm()))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(
            report.skipped.is_empty(),
            "{name}: skipped {:?}",
            report.skipped
        );
        assert_eq!(
            report.count(ViolationKind::Short),
            0,
            "{name}: shorts\n{}",
            listing()
        );
        assert_eq!(
            (
                report.count(ViolationKind::Clearance),
                report.count(ViolationKind::Unconnected),
                report.count(ViolationKind::Keepout),
            ),
            (clearance, unconnected, keepout),
            "{name}: (clearance, unconnected, keepout)\n{}",
            listing()
        );
    }
}

#[test]
fn every_real_board_builds_its_ratsnest_with_the_pinned_unrouted_count() {
    for (name, .., lines) in EXPECTED {
        let (board, _) = load(name);
        let first = ratsnest(&board);
        assert_eq!(first.len(), lines, "{name}: {first:?}");
        assert_eq!(ratsnest(&board), first, "{name}: not deterministic");
    }
}

/// Positive control: the clean reports above are not the check seeing
/// nothing. Every board's first track leaves a pad; on another net it shorts
/// that pad.
#[test]
fn a_track_moved_to_another_net_shorts_on_every_real_board() {
    for (name, ..) in EXPECTED {
        let (mut board, classes) = load(name);
        let own = board.segments[0].net;
        let other = board.nets.iter().map(|n| n.number).find(|&n| n != own);
        board.segments[0].net = other.expect("a second net");
        let rules = DrcRules::for_board(&board, &classes).unwrap();
        assert!(
            run_drc(&board, &rules).count(ViolationKind::Short) >= 1,
            "{name}"
        );
    }
}

#[test]
fn net_classes_raise_the_clearance_of_their_nets() {
    // The power class asks 0.25 mm against the board's 0.2 mm.
    let (board, classes) = load("linear-regulator");
    let rules = DrcRules::for_board(&board, &classes).unwrap();
    let net = |name: &str| board.nets.iter().find(|n| n.name == name).unwrap().number;
    assert_eq!(rules.clearance, 200_000);
    assert_eq!(rules.required(net("+5V"), net("LED_A")), 250_000);
    assert_eq!(rules.required(net("LED_A"), net("LED_A")), 200_000);
}
