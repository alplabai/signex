//! A placed symbol's library link (#1338): `library_id`, `row_id` and the
//! pinned `library_version` ride in `[extras.symbols.<uuid>]`, only when set.

use super::*;
use crate::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

fn linked(n: u128) -> Symbol {
    Symbol {
        uuid: Uuid::from_u128(n),
        reference: format!("R{n}"),
        library_id: Some(Uuid::from_u128(0x1000 + n)),
        row_id: Some(Uuid::from_u128(0x2000 + n)),
        library_version: "1.0.2".into(),
        ..Default::default()
    }
}

fn unlinked(n: u128) -> Symbol {
    Symbol {
        uuid: Uuid::from_u128(n),
        reference: format!("C{n}"),
        ..Default::default()
    }
}

fn sheet(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::from_u128(100),
        symbols,
        ..Default::default()
    }
}

fn write(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn assert_links(sheet: &SchematicSheet) {
    let a = &sheet.symbols[0];
    assert_eq!(a.library_id, Some(Uuid::from_u128(0x1001)));
    assert_eq!(a.row_id, Some(Uuid::from_u128(0x2001)));
    assert_eq!(a.library_version, "1.0.2");
    let b = &sheet.symbols[1];
    assert_eq!((b.library_id, b.row_id), (None, None));
    assert!(b.library_version.is_empty());
}

#[test]
fn a_placed_symbols_library_link_and_version_survive_save_and_reload() {
    let text = write(&sheet(vec![linked(1), unlinked(2)]));
    let back = SnxSchematic::parse(&text).unwrap().sheet;
    assert_links(&back);
    assert_eq!(write(&back), text, "the encoding must be stable");
}

#[test]
fn a_symbol_whose_only_extra_is_the_link_is_not_elided() {
    let text = write(&sheet(vec![linked(1), unlinked(2)]));
    assert!(text.contains("library_version = \"1.0.2\""), "{text}");
}

#[test]
fn a_file_with_no_linked_symbols_adds_nothing_and_is_byte_identical() {
    let text = write(&sheet(vec![unlinked(2), unlinked(3)]));
    for key in ["library_id", "row_id", "library_version"] {
        assert!(!text.contains(key), "{key} must not appear:\n{text}");
    }
    assert_eq!(UNLINKED_BY_OLDER_BUILD, text);
    let again = write(&SnxSchematic::parse(&text).unwrap().sheet);
    assert_eq!(again, text);
}

#[test]
fn a_file_without_the_keys_loads_with_no_link() {
    let back = SnxSchematic::parse(UNLINKED_BY_OLDER_BUILD).unwrap().sheet;
    for s in &back.symbols {
        assert_eq!((s.library_id, s.row_id), (None, None));
        assert!(s.library_version.is_empty());
    }
}

const WRITTEN: &str = include_str!("../../tests/fixtures/library_link/written_by_1338.snxsch");
const RESAVED: &str =
    include_str!("../../tests/fixtures/library_link/resaved_by_older_build.snxsch");
const UNLINKED_BY_OLDER_BUILD: &str =
    include_str!("../../tests/fixtures/library_link/unlinked_by_older_build.snxsch");

fn fixture_sheet() -> SchematicSheet {
    sheet(vec![linked(1), unlinked(2)])
}

/// Writer: the link sits where an older build keeps unknown data.
#[test]
fn this_build_writes_the_fixture_bytes() {
    assert_eq!(write(&fixture_sheet()), WRITTEN);
}

/// A build that predates the keys (trunk at e5fc9378) parsed and re-saved
/// the file; the link is still there.
#[test]
fn an_older_build_resave_keeps_the_link() {
    assert_links(&SnxSchematic::parse(RESAVED).unwrap().sheet);
}

/// Regenerates the fixtures with a pre-#1338 `resave` example binary (see
/// `font_family_tests::regenerate_fixtures` for how to build one):
/// `OLDER_BUILD_RESAVE=<binary> cargo test -p signex-types regenerate_link_fixtures -- --ignored`
#[test]
#[ignore = "writes fixtures; needs a build of an older signex-types"]
fn regenerate_link_fixtures() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/library_link");
    let tool = std::env::var("OLDER_BUILD_RESAVE").expect("OLDER_BUILD_RESAVE");
    let resave = |input: &str, out: &str| {
        let status = std::process::Command::new(&tool)
            .args([input, out])
            .status()
            .unwrap();
        assert!(status.success());
    };
    let written = format!("{dir}/written_by_1338.snxsch");
    std::fs::write(&written, write(&fixture_sheet())).unwrap();
    resave(&written, &format!("{dir}/resaved_by_older_build.snxsch"));

    let tmp = format!("{dir}/unlinked_input.tmp");
    std::fs::write(&tmp, write(&sheet(vec![unlinked(2), unlinked(3)]))).unwrap();
    resave(&tmp, &format!("{dir}/unlinked_by_older_build.snxsch"));
    std::fs::remove_file(tmp).unwrap();
}

fn link_text(library_id: &str, row_id: &str) -> String {
    let text = write(&sheet(vec![linked(1)]));
    let id = Uuid::from_u128(0x1001).to_string();
    let row = Uuid::from_u128(0x2001).to_string();
    assert!(text.contains(&id) && text.contains(&row), "{text}");
    text.replace(&id, library_id).replace(&row, row_id)
}

#[test]
fn a_malformed_link_id_loads_as_no_link_and_the_sheet_opens() {
    for (library_id, row_id) in [("", "nope"), ("nope", ""), ("", "")] {
        let back = SnxSchematic::parse(&link_text(library_id, row_id))
            .expect("a malformed link id must not fail the sheet")
            .sheet;
        assert_eq!(back.symbols.len(), 1);
        let s = &back.symbols[0];
        assert_eq!(
            (s.library_id, s.row_id),
            (None, None),
            "{library_id:?} {row_id:?}"
        );
    }
}

#[test]
fn a_valid_id_beside_a_malformed_one_is_kept() {
    let row = Uuid::from_u128(0x2001).to_string();
    let back = SnxSchematic::parse(&link_text("nope", &row)).unwrap().sheet;
    assert_eq!(back.symbols[0].library_id, None);
    assert_eq!(back.symbols[0].row_id, Some(Uuid::from_u128(0x2001)));
}

#[test]
fn a_non_string_link_id_loads_as_no_link() {
    let text = link_text("x", "y")
        .replace("library_id = \"x\"", "library_id = 7")
        .replace("row_id = \"y\"", "row_id = true");
    let back = SnxSchematic::parse(&text).unwrap().sheet;
    assert_eq!(
        (back.symbols[0].library_id, back.symbols[0].row_id),
        (None, None)
    );
}
