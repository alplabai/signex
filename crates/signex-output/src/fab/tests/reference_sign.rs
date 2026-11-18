//! Designators under EN IEC 81346-2 full class codes (#1416): the board
//! outputs print `-R1` from a signed copy of the board; the board itself
//! keeps the stored `R1`.

use signex_types::designator::DesignatorStandard;
use signex_types::property::PcbProperty;

use super::fixture::fixture_board;
use super::{file, generated};
use crate::show_board_references;

#[test]
fn the_fabrication_files_print_the_signed_designator() {
    let mut board = fixture_board();
    board.footprints[0].properties.push(PcbProperty {
        key: "Reference".into(),
        value: "R1".into(),
        ..Default::default()
    });
    let stored = board.clone();
    show_board_references(&mut board, Some(DesignatorStandard::Iec81346_2Full));
    assert_eq!(board.footprints[0].reference, "-R1");
    assert_eq!(board.footprints[0].properties[0].value, "-R1");
    assert_eq!(stored.footprints[0].reference, "R1");

    let set = generated(&board);
    let centroid = file(&set, "fab-centroid.csv").expect("centroid");
    assert!(centroid.contains("\n-R1,10k,R_0603,"), "{centroid}");
    let ipc = file(&set, "fab-netlist.ipc").expect("test netlist");
    let designator = |line: &str| line.get(20..26).map(str::to_owned);
    assert!(
        ipc.lines().any(|l| designator(l).as_deref() == Some("-R1   ")),
        "{ipc}"
    );
}

#[test]
fn other_standards_leave_the_board_as_stored() {
    for standard in [None, Some(DesignatorStandard::Iec81346_2)] {
        let mut board = fixture_board();
        show_board_references(&mut board, standard);
        assert_eq!(board.footprints[0].reference, "R1");
    }
}
