//! A bottom-side footprint of a real `.snxpcb` file is mirrored (#2174).

use std::path::Path;

use pcb_geom::{BoardIndex, ItemRef, PointNm, Side, pad_world_center};
use signex_types::format::SnxPcb;

const MM: i64 = 1_000_000;

fn corpus_board() -> signex_types::pcb::PcbBoard {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb/footprint-bottom-layer.snxpcb");
    let text = std::fs::read_to_string(path).expect("corpus file");
    SnxPcb::parse(&text).expect("corpus parses").board
}

#[test]
fn the_corpus_footprint_is_on_the_bottom() {
    let board = corpus_board();
    assert_eq!(board.footprints[0].layer, "BottomCopper");
    assert_eq!(Side::of_layer(&board.footprints[0].layer), Side::Bottom);
}

#[test]
fn the_bottom_pad_is_hit_at_its_mirrored_centre() {
    let board = corpus_board();
    let footprint = &board.footprints[0];
    let pad = &footprint.pads[0];
    // Local (11, 20) mm: mirror x -> (-11, 20), turn 180 deg -> (11, -20),
    // then the footprint origin (50, 25) mm.
    let centre = PointNm::new(61 * MM, 5 * MM);
    assert_eq!(pad_world_center(footprint, pad), Ok(centre));

    let index = BoardIndex::build(&board);
    let hit = ItemRef::Pad {
        footprint: footprint.uuid,
        pad: pad.uuid,
    };
    assert_eq!(index.hit_point(centre, 0), vec![hit]);
    // Where an unmirrored pad would sit.
    assert!(index.hit_point(PointNm::new(39 * MM, 5 * MM), 0).is_empty());
}
