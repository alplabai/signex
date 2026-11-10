//! #1312 review of PR #2271, blocker 1 and S2: a placed part whose library
//! does not open keeps its pads' nets, the dialog notes it, and Messages
//! says which library failed. Fixture and helpers: `update_pcb.rs`.

use signex_app::diagnostics::{DiagnosticLevel, recent_entries};
use signex_engine::pcb::eco::EcoNote;

use super::*;

#[test]
fn a_library_that_does_not_open_leaves_its_parts_pads_alone_and_says_so() {
    let board = board_of(vec![board_footprint(0xa1, "R1", Some(R1), false)]);
    let mut f = fixture_with(
        |_| Vec::new(),
        Some((board, false)),
        ProjectDefaults::default(),
    );
    // The library is still mounted, but its file is gone from disk.
    let snxlib = f._lib.snxlib.clone();
    std::fs::remove_file(&snxlib).expect("remove the library file");

    send(&mut f.app, menu_row("update_pcb_from_schematic"));
    let r1 = Uuid::from_u128(0xa1);
    let list = list(&f);
    assert!(
        !list
            .rows
            .iter()
            .any(|r| matches!(r.change, EcoChange::PadNet { footprint, .. } if footprint == r1)),
        "R1's pin map is unknown, so its pads are not re-netted"
    );
    assert!(
        list.notes.iter().any(|n| matches!(
            n,
            EcoNote::FootprintUnresolved { footprint, reference, .. }
                if *footprint == r1 && reference == "R1"
        )),
        "the dialog notes R1: {:?}",
        list.notes
    );
    // The temp folder above the library's own: unique to this test, whatever
    // form the mounted path takes.
    let needle = snxlib
        .parent()
        .and_then(|d| d.parent())
        .and_then(|d| d.file_name())
        .expect("a temp folder")
        .to_string_lossy()
        .into_owned();
    assert!(
        recent_entries()
            .iter()
            .any(|e| e.level == DiagnosticLevel::Warning
                && e.message.contains(&needle)
                && e.message.contains("could not be opened")),
        "Messages names the library that did not open"
    );
}
