//! Designators under EN IEC 81346-2 full class codes (#1416): the board
//! outputs print `-R1` from a signed copy of the board, and the board
//! itself keeps the stored `R1`. The IPC-D-356 test netlist is the
//! exception: its 6-character field would cut `-CAA100` into `-CAA10`, so it
//! writes the stored designator.

use signex_types::designator::DesignatorStandard;
use signex_types::pcb::PcbBoard;
use signex_types::property::PcbProperty;

use super::fixture::{fixture_board, footprint, p};
use super::{FabExtras, FabOptions, FabSet, file};
use crate::fab::{OdbForm, generate};
use crate::show_board_references;

const FULL: Option<DesignatorStandard> = Some(DesignatorStandard::Iec81346_2Full);

/// Every output of `board`, as Generate writes it: from the signed copy,
/// with the standard it was signed under.
fn generated_under_full_codes(board: &PcbBoard) -> FabSet {
    let options = FabOptions {
        odb: Some(OdbForm::Directory),
        designator_standard: FULL,
        ..FabOptions::all("fab")
    };
    generate(board, &FabExtras::default(), &options).expect("generates")
}

/// The designator column (21-26) of every IPC-D-356 part record, trimmed.
fn test_netlist_designators(set: &FabSet) -> Vec<String> {
    let ipc = file(set, "fab-netlist.ipc").expect("test netlist");
    ipc.lines()
        .filter(|l| l.starts_with("317") || l.starts_with("327"))
        .filter_map(|l| l.get(20..26))
        .map(|d| d.trim().to_owned())
        .filter(|d| d != "VIA")
        .collect()
}

#[test]
fn the_fabrication_files_print_the_signed_designator() {
    let mut board = fixture_board();
    board.footprints[0].properties.push(PcbProperty {
        key: "Reference".into(),
        value: "R1".into(),
        ..Default::default()
    });
    let stored = board.clone();
    show_board_references(&mut board, FULL);
    assert_eq!(board.footprints[0].reference, "-R1");
    assert_eq!(board.footprints[0].properties[0].value, "-R1");
    assert_eq!(stored.footprints[0].reference, "R1");

    let set = generated_under_full_codes(&board);
    let centroid = file(&set, "fab-centroid.csv").expect("centroid");
    assert!(centroid.contains("\n-R1,10k,R_0603,"), "{centroid}");
    let xml = file(&set, "fab-ipc2581.xml").expect("IPC-2581");
    assert!(xml.contains("refDes=\"-R1\""), "{xml}");
    let components: Vec<&str> = set
        .files
        .iter()
        .filter(|f| f.name.ends_with("/components"))
        .flat_map(|f| f.contents.lines())
        .filter(|l| l.starts_with("CMP "))
        .collect();
    assert!(
        components.iter().any(|l| l.split(' ').nth(6) == Some("-R1")),
        "{components:?}"
    );
    let designators = test_netlist_designators(&set);
    assert!(designators.iter().any(|d| d == "R1"), "{designators:?}");
    assert!(
        designators.iter().all(|d| !d.starts_with('-')),
        "{designators:?}"
    );
}

/// The maintainer's 2026-11-18 decision: `CAA10` and `CAA100` stay two
/// designators in the test netlist, both unsigned, where a signed `-CAA100`
/// would be cut to `-CAA10`.
#[test]
fn the_test_netlist_keeps_long_codes_apart_and_unsigned() {
    let mut board = fixture_board();
    let pads = board.footprints[0].pads.clone();
    let a = footprint(60, "CAA10", p(3.0, 3.0), 0.0, "TopCopper", pads.clone());
    let b = footprint(61, "CAA100", p(8.0, 3.0), 0.0, "TopCopper", pads);
    board.footprints = vec![a, b];
    show_board_references(&mut board, FULL);
    assert_eq!(board.footprints[1].reference, "-CAA100");

    let mut designators = test_netlist_designators(&generated_under_full_codes(&board));
    designators.sort();
    designators.dedup();
    assert_eq!(designators, ["CAA10", "CAA100"]);
}

#[test]
fn other_standards_leave_the_board_as_stored() {
    for standard in [None, Some(DesignatorStandard::Iec81346_2)] {
        let mut board = fixture_board();
        show_board_references(&mut board, standard);
        assert_eq!(board.footprints[0].reference, "R1");
    }
}
