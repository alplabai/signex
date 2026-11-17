//! #2384: every file counts from the board origin, Y up, as the board's
//! own readouts do.

use signex_types::pcb_options::BoardOrigin;

use super::fixture::{fixture_board, p};
use super::{file, generated};

#[test]
fn every_file_counts_from_the_board_origin_with_y_up() {
    let mut board = fixture_board();
    board.options.origin = BoardOrigin::new(p(0.0, 80.0));
    board.footprints[0].position = p(10.0, 70.0);
    let set = generated(&board);

    let centroid = file(&set, "fab-centroid.csv").expect("centroid");
    assert!(
        centroid.contains("R1,10k,R_0603,10.000,10.000,0.000,Top\n"),
        "R1 at (10, 70) from an origin at (0, 80): {centroid}"
    );
    // R1's pad 1 sits 1 mm left of its origin.
    let top = file(&set, "fab-copper-l1-top.gbr").expect("top copper");
    assert!(top.contains("X9000000Y10000000D03*\n"), "{top}");
    // J1's hole at (10, 8) and the via at (15, 5).
    let pth = file(&set, "fab-pth.drl").expect("plated holes");
    assert!(pth.contains("X10.000Y72.000\n"), "{pth}");
    assert!(pth.contains("X15.000Y75.000\n"), "{pth}");
    let ipc = file(&set, "fab-netlist.ipc").expect("test netlist");
    assert!(ipc.contains("X+009000Y+010000"), "{ipc}");
}
