//! The ratsnest on the five realistic corpus boards
//! (`signex-types/tests/corpus/pcb/real/`). Their design-rule check:
//! `signex-drc`'s `tests/real_boards.rs`.

use std::path::Path;

use pcb_geom::ratsnest;
use signex_types::format::SnxPcb;

/// `(name, ratsnest lines)`: every board is fully routed.
const EXPECTED: [(&str, usize); 5] = [
    ("linear-regulator", 0),
    ("usb-c-power-input", 0),
    ("mcu-breakout-tqfp32", 0),
    ("sensor-4layer", 0),
    ("mixed-tht-smd", 0),
];

#[test]
fn every_real_board_builds_its_ratsnest_with_the_pinned_unrouted_count() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../signex-types/tests/corpus/pcb/real");
    for (name, lines) in EXPECTED {
        let text = std::fs::read_to_string(dir.join(format!("{name}.snxpcb"))).unwrap();
        let board = SnxPcb::parse(&text).expect("board parses").board;
        let first = ratsnest(&board);
        assert_eq!(first.len(), lines, "{name}: {first:?}");
        assert_eq!(ratsnest(&board), first, "{name}: not deterministic");
    }
}
