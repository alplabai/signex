//! Caner (2026-11-27): an imported board honours the thermal relief its
//! pours store. A routing-corpus board (`testdata/routing-corpus/`, #2481)
//! whose pours store it on fills with gaps and spokes round the pads of
//! their net, not solid copper. The corpus is stored in Git LFS: a board
//! whose objects were not fetched is skipped with a message.

use std::path::Path;

use pcb_geom::zone_fills;
use signex_types::format::SnxPcb;

const LFS_POINTER: &str = "version https://git-lfs.github.com/spec/v1";

#[test]
fn an_imported_board_fills_its_pours_with_the_thermal_relief_it_stores() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/routing-corpus/rp2040-designguide.snxpcb");
    let text = std::fs::read_to_string(&path).expect("the corpus board is in the tree");
    if text.starts_with(LFS_POINTER) {
        eprintln!("skipped: {} is a Git LFS pointer", path.display());
        return;
    }
    let stored = SnxPcb::parse(&text).expect("the board parses").board;
    assert!(
        stored.zones.iter().any(|z| z.is_pour() && z.thermal_relief),
        "a pour stores thermal relief on"
    );
    let mut solid = stored.clone();
    for zone in &mut solid.zones {
        zone.thermal_relief = false;
    }
    assert_ne!(
        zone_fills(&stored),
        zone_fills(&solid),
        "the stored relief changes the fill"
    );
}
