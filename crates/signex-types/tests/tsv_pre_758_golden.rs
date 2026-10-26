//! #768 item 4 — a `.snxsch` saved by the writer that predates the
//! unknown-column support (#758) carries no foreign columns. Loading it and
//! saving it again must reproduce the file byte for byte: the unknown-column
//! machinery may not reorder, requote or re-pad a file that does not use it.
//!
//! The fixture was produced by the writer at the commit before #758
//! (`ec4d2fa88^`), not by the current one, so it is an independent witness
//! of the old output.

use signex_types::format::SnxSchematic;

const PRE_758: &str = include_str!("corpus/schematic/pre-758-no-unknown-columns.snxsch");

#[test]
fn pre_758_file_reserializes_byte_identically() {
    let loaded = SnxSchematic::parse(PRE_758).expect("a pre-#758 file still loads");
    assert!(
        loaded
            .sheet
            .wires
            .iter()
            .all(|w| w.unknown_columns.is_empty())
            && loaded
                .sheet
                .junctions
                .iter()
                .all(|j| j.unknown_columns.is_empty())
            && loaded
                .sheet
                .symbols
                .iter()
                .all(|s| s.unknown_columns.is_empty()),
        "a file with no foreign columns must load with none"
    );
    let saved = SnxSchematic::new(loaded.sheet)
        .write_string()
        .expect("a pre-#758 file still saves");
    assert_eq!(saved, PRE_758, "re-saving must not change a single byte");
}
