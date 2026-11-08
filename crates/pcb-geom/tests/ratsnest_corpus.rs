//! The ratsnest on a real `.snxpcb` corpus board, and its cost on 10,000
//! tracks (#2192).

use std::path::Path;
use std::time::Instant;

use pcb_geom::{PointNm, RatLine, ratsnest};
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point, Segment, Zone};
use uuid::Uuid;

fn corpus_board(name: &str) -> PcbBoard {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb")
        .join(name);
    let text = std::fs::read_to_string(path).expect("corpus file");
    SnxPcb::parse(&text).expect("corpus parses").board
}

#[test]
fn the_routed_corpus_board_has_one_pad_per_net_so_no_ratsnest() {
    assert!(ratsnest(&corpus_board("routed_board.snxpcb")).is_empty());
}

#[test]
fn putting_both_corpus_pads_on_one_net_draws_the_unrouted_link() {
    let mut board = corpus_board("routed_board.snxpcb");
    let pads = &mut board.footprints[0].pads;
    pads[1].net = pads[0].net.clone();
    // 1 mm wide pads at 1 mm pitch touch, which is copper joined; narrow
    // them so the two stand apart.
    for pad in pads.iter_mut() {
        pad.size.x = 0.5;
    }
    // Footprint at (50, 25) mm, pads local (50.5, 25) and (51.5, 25); the
    // tracks are at y = 200 mm.
    assert_eq!(
        ratsnest(&board),
        vec![RatLine {
            net: 1,
            from: PointNm::new(100_500_000, 50_000_000),
            to: PointNm::new(101_500_000, 50_000_000),
        }]
    );
}

/// 100 nets, each a row of 100 tracks between two pads 200 mm apart. On
/// every even row one track is on the other layer, which splits the row.
fn ten_thousand_tracks() -> PcbBoard {
    let mut board = corpus_board("routed_board.snxpcb");
    let template = board.footprints.remove(0);
    board.footprints.clear();
    board.vias.clear();
    board.segments.clear();
    for row in 0..100u32 {
        let mut fp = template.clone();
        fp.uuid = Uuid::from_u128(10_000 + u128::from(row));
        fp.position = Point::new(0.0, f64::from(row));
        fp.pads.truncate(2);
        for (k, x) in [0.0, 200.0].into_iter().enumerate() {
            let pad = &mut fp.pads[k];
            pad.uuid = Uuid::from_u128(20_000 + u128::from(row) * 2 + k as u128);
            pad.position = Point::new(x, 0.0);
            pad.net.as_mut().unwrap().number = row + 1;
        }
        board.footprints.push(fp);
        for col in 0..100u32 {
            let x = f64::from(col) * 2.0;
            let split = row % 2 == 0 && col == 50;
            board.segments.push(Segment {
                uuid: Uuid::from_u128(u128::from(row) * 100 + u128::from(col) + 1),
                start: Point::new(x, f64::from(row)),
                end: Point::new(x + 2.0, f64::from(row)),
                width: 0.2,
                layer: if split { "BottomCopper" } else { "TopCopper" }.into(),
                net: row + 1,
                unknown_columns: Default::default(),
            });
        }
    }
    board
}

#[test]
fn ten_thousand_tracks_are_joined_within_the_debug_budget() {
    let board = ten_thousand_tracks();
    assert_eq!(board.segments.len(), 10_000);
    let started = Instant::now();
    let lines = ratsnest(&board);
    let took = started.elapsed();
    println!("ratsnest 10k tracks: {took:?}, {} lines", lines.len());
    // Each of the 50 split rows needs one link, pad to pad.
    assert_eq!(lines.len(), 50);
    // Loose for debug CI; release is a few tens of ms (see docs).
    assert!(took.as_millis() < 5_000, "{took:?}");
}

/// The 10k-track board plus 300 zones: three per net, 30 mm wide strips along
/// the net's row, 20 mm apart so neighbours overlap each other and the row's
/// tracks and pads. Every third zone is on the bottom layer.
fn ten_thousand_tracks_and_zones() -> PcbBoard {
    let mut board = ten_thousand_tracks();
    board.zones.clear();
    for k in 0..3u32 {
        for row in 0..100u32 {
            let (x0, y) = (f64::from(k) * 20.0 + 90.0, f64::from(row));
            let corners = [
                (x0, y - 0.4),
                (x0 + 30.0, y - 0.4),
                (x0 + 30.0, y + 0.4),
                (x0, y + 0.4),
            ];
            board.zones.push(Zone {
                uuid: Uuid::from_u128(40_000 + u128::from(k * 100 + row)),
                net: row + 1,
                net_name: String::new(),
                layer: if (k + row) % 3 == 0 {
                    "BottomCopper"
                } else {
                    "TopCopper"
                }
                .into(),
                outline: corners.iter().map(|&(x, y)| Point::new(x, y)).collect(),
                holes: Vec::new(),
                priority: 0,
                fill_type: String::new(),
                thermal_relief: false,
                thermal_gap: 0.0,
                thermal_width: 0.0,
                clearance: 0.0,
                min_thickness: 0.0,
                unknown: Default::default(),
            });
        }
    }
    board
}

#[test]
fn ten_thousand_tracks_and_three_hundred_zones_stay_within_the_debug_budget() {
    let board = ten_thousand_tracks_and_zones();
    assert_eq!(board.zones.len(), 300);
    let started = Instant::now();
    let lines = ratsnest(&board);
    let took = started.elapsed();
    println!(
        "ratsnest 10k tracks + 300 zones: {took:?}, {} lines",
        lines.len()
    );
    // The zones only ever join, so no row gains a link.
    assert!(lines.len() <= 50, "{} lines", lines.len());
    assert!(took.as_millis() < 5_000, "{took:?}");
}
