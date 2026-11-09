//! The optional `lib_ref` footprint column (#1312, ADR-0005 section 13):
//! written only when a footprint carries a library identity, so older files
//! re-save byte-identical, and refused on load when malformed.

use super::*;
use crate::part::FootprintSourceRef;
use crate::pcb::{Footprint, Point as PcbPoint};
use uuid::Uuid;

const OLD_BOARD: &str = include_str!("../../tests/corpus/pcb/footprint-schematic-link.snxpcb");

fn source(n: u128, version: &str) -> FootprintSourceRef {
    FootprintSourceRef {
        library: Uuid::from_u128(0xabc_0000 + n),
        footprint: Uuid::from_u128(0xdef_0000 + n),
        version: version.into(),
    }
}

fn footprint(n: u128, library_ref: Option<FootprintSourceRef>) -> Footprint {
    Footprint {
        uuid: Uuid::from_u128(n),
        reference: format!("R{n}"),
        value: String::new(),
        footprint_id: "R0603".into(),
        position: PcbPoint { x: 1.0, y: 2.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: Vec::new(),
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn board(refs: [Option<FootprintSourceRef>; 2]) -> PcbBoard {
    let mut board = super::tests::empty_board();
    let [first, second] = refs;
    board.footprints = vec![footprint(1, first), footprint(2, second)];
    board
}

#[test]
fn a_board_saved_before_the_column_re_saves_byte_identical() {
    let parsed = SnxPcb::parse(OLD_BOARD).unwrap();
    assert!(
        parsed
            .board
            .footprints
            .iter()
            .all(|f| f.library_ref.is_none())
    );
    let written = parsed.write_string().unwrap();
    assert!(!written.contains("lib_ref"), "{written}");
    assert_eq!(written, OLD_BOARD);
}

#[test]
fn footprints_without_a_library_identity_write_no_column() {
    let text = SnxPcb::new(board([None, None])).write_string().unwrap();
    assert!(!text.contains("lib_ref"), "{text}");
    assert_eq!(SnxPcb::parse(&text).unwrap().write_string().unwrap(), text);
}

#[test]
fn a_library_identity_round_trips_and_its_neighbour_stays_without() {
    let lib = source(7, "1.2.0");
    let text = SnxPcb::new(board([Some(lib.clone()), None]))
        .write_string()
        .unwrap();
    assert!(text.contains("lib_ref"), "{text}");
    assert!(text.contains(&lib.to_string()), "{text}");
    let parsed = SnxPcb::parse(&text).unwrap();
    assert_eq!(parsed.board.footprints[0].library_ref, Some(lib));
    assert_eq!(parsed.board.footprints[1].library_ref, None);
    assert!(
        parsed
            .board
            .footprints
            .iter()
            .all(|f| f.unknown_columns.is_empty()),
        "the column is read, not kept as a foreign cell"
    );
    assert_eq!(parsed.write_string().unwrap(), text);
}

#[test]
fn the_cell_is_library_colon_footprint_at_version() {
    let lib = source(7, "0.0.1");
    assert_eq!(
        lib.to_string(),
        format!("{}:{}@0.0.1", lib.library, lib.footprint)
    );
    assert_eq!(FootprintSourceRef::parse(&lib.to_string()), Some(lib));
}

#[test]
fn a_malformed_cell_is_refused_naming_the_field_and_value() {
    let lib = source(7, "1.2.0");
    let good = lib.to_string();
    let text = SnxPcb::new(board([Some(lib.clone()), None]))
        .write_string()
        .unwrap();
    // Same length each, so the TSV columns stay aligned.
    let cases = [
        good.replacen('@', "#", 1),
        good.replacen(':', ";", 1),
        good.replacen('a', "g", 1),
    ];
    for bad in &cases {
        let broken = text.replace(&good, bad);
        assert_ne!(broken, text);
        let error = SnxPcb::parse(&broken).unwrap_err().to_string();
        assert!(error.contains("lib_ref"), "{error}");
        assert!(error.contains(bad.as_str()), "{error}");
    }
    // An empty version is malformed too.
    let empty_version = format!("{}:{}@", lib.library, lib.footprint);
    assert_eq!(FootprintSourceRef::parse(&empty_version), None);
}

#[test]
fn parse_reads_only_the_exact_written_form() {
    let lib = source(3, "2.0.0");
    let upper = lib.to_string().to_uppercase();
    assert_eq!(FootprintSourceRef::parse(&upper), None);
    let simple = format!("{}:{}@2.0.0", lib.library.simple(), lib.footprint.simple());
    assert_eq!(FootprintSourceRef::parse(&simple), None);
    assert_eq!(FootprintSourceRef::parse("not a ref"), None);
}
