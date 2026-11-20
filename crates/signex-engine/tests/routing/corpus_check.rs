//! The routing corpus checked per PR by loading alone (#2481): every board
//! parses, names only layers this build knows (copper items on copper), and
//! no track end lands on a pad of another net once the pads are placed the
//! way the board engine places them. Routing the corpus is the scheduled
//! soak's job; this test only reads the boards, so it stays short.

use std::thread;

use pcb_geom::{PointNm, layer_mask, pad_layers, pad_world_center};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::PcbLayer;

use crate::fixtures::{Fixture, corpus_paths, load};

/// The first layer name on `board` that is not a layer this build knows,
/// or a copper item's layer that is not copper.
fn unknown_layer(board: &PcbBoard) -> Option<String> {
    let known = |name: &str| parse_pcb_layer(name).is_some();
    let copper = |name: &str| matches!(parse_pcb_layer(name), Some(PcbLayer::Copper(_)));
    let copper_items = board
        .segments
        .iter()
        .map(|s| s.layer.as_str())
        .chain(
            board
                .vias
                .iter()
                .flat_map(|v| v.layers.iter().map(String::as_str)),
        )
        .chain(board.free.arcs.iter().map(|a| a.layer.as_str()))
        .chain(board.zones.iter().map(|z| z.layer.as_str()));
    let other_items = board
        .footprints
        .iter()
        .flat_map(|f| {
            std::iter::once(f.layer.as_str())
                .chain(
                    f.pads
                        .iter()
                        .flat_map(|p| p.layers.iter().map(String::as_str)),
                )
                .chain(f.graphics.iter().map(|g| g.layer.as_str()))
                .chain(f.properties.iter().filter_map(|p| p.layer.as_deref()))
        })
        .chain(board.graphics.iter().map(|g| g.layer.as_str()))
        .chain(board.texts.iter().map(|t| t.layer.as_str()))
        .chain(board.layers.iter().map(|l| l.name.as_str()));
    for name in copper_items {
        if !copper(name) {
            return Some(format!("copper item on {name:?}"));
        }
    }
    other_items
        .filter(|name| !known(name))
        .map(|name| format!("layer {name:?}"))
        .next()
}

/// A netted pad as placed: centre, the radius of its half short side, net,
/// and the copper layers it reaches (a layer-bit mask).
struct PlacedPad {
    at: PointNm,
    reach: i64,
    net: u32,
    layers: u32,
}

/// How many track and copper-arc ends touch only pads of other nets on
/// their layer.
fn ends_on_other_nets(board: &PcbBoard) -> usize {
    let mut pads: Vec<PlacedPad> = Vec::new();
    for fp in &board.footprints {
        for pad in &fp.pads {
            let Some(net) = pad.net.as_ref().map(|n| n.number).filter(|&n| n != 0) else {
                continue;
            };
            let Ok(at) = pad_world_center(fp, pad) else {
                continue;
            };
            let reach = (pad.size.x.abs().min(pad.size.y.abs()) * 0.5e6).round() as i64;
            pads.push(PlacedPad {
                at,
                reach,
                net,
                layers: pad_layers(fp, pad),
            });
        }
    }
    pads.sort_by_key(|p| p.at.x);
    let widest = pads.iter().map(|p| p.reach).max().unwrap_or(0);
    let mut ends: Vec<(Point, u32, &str)> = Vec::new();
    for s in &board.segments {
        ends.push((s.start, s.net, &s.layer));
        ends.push((s.end, s.net, &s.layer));
    }
    for a in &board.free.arcs {
        ends.push((a.start, a.net, &a.layer));
        ends.push((a.end, a.net, &a.layer));
    }
    let mut stray = 0;
    for (point, net, layer) in ends {
        let Ok(end) = PointNm::from_mm(point, "track end") else {
            continue;
        };
        let bits = layer_mask(std::iter::once(layer));
        let from = pads.partition_point(|p| p.at.x < end.x - widest);
        let mut own = false;
        let mut other = false;
        for pad in pads[from..].iter().take_while(|p| p.at.x <= end.x + widest) {
            let (dx, dy) = ((pad.at.x - end.x) as f64, (pad.at.y - end.y) as f64);
            if pad.layers & bits == 0 || dx.hypot(dy) > pad.reach as f64 {
                continue;
            }
            if pad.net == net {
                own = true;
            } else {
                other = true;
            }
        }
        if other && !own {
            stray += 1;
        }
    }
    stray
}

/// What is wrong with one corpus board, if anything.
fn fault(fixture: &Fixture) -> Option<String> {
    if let Some(layer) = unknown_layer(&fixture.board) {
        return Some(format!("{}: unknown {layer}", fixture.name));
    }
    match ends_on_other_nets(&fixture.board) {
        0 => None,
        n => Some(format!(
            "{}: {n} track ends on another net's pad",
            fixture.name
        )),
    }
}

/// Set to `1` where the corpus's Git LFS objects were fetched (CI does when
/// the corpus or its readers change): an unfetched board or an empty corpus
/// then fails instead of being skipped.
const REQUIRE_CORPUS: &str = "SIGNEX_REQUIRE_ROUTING_CORPUS";

#[test]
fn every_corpus_board_loads_with_the_converter_invariants() {
    let required = std::env::var(REQUIRE_CORPUS).is_ok_and(|v| v == "1");
    let paths = corpus_paths();
    assert!(
        !required || !paths.is_empty(),
        "{REQUIRE_CORPUS}=1 but testdata/routing-corpus holds no boards"
    );
    let workers = thread::available_parallelism().map_or(2, |n| n.get());
    let chunk = paths.len().div_ceil(workers).max(1);
    let faults: Vec<String> = thread::scope(|scope| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .filter_map(|path| match load(path) {
                            Some(fixture) => fault(&fixture),
                            None if required => Some(format!(
                                "{}: a Git LFS pointer, not a board ({REQUIRE_CORPUS}=1)",
                                path.display()
                            )),
                            None => None,
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a loader thread"))
            .collect()
    });
    assert!(faults.is_empty(), "{faults:#?}");
}
