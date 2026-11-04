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
///
/// The wire table header of the checked-in fixtures was hand-migrated for
/// #1364 (the `net` column is gone); the binary must be built from a
/// post-#1364 base, an older one writes the old header and this panics.
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

// ---------------------------------------------------------------------------
// The value stamp (#2004 item 7): `[extras.symbols.<uuid>.library_stamp]`.
// ---------------------------------------------------------------------------

use crate::schematic::RowStamp;

/// A stamp with a value, an empty value ("the row offered nothing") and an
/// absent one (unknown).
fn stamped(n: u128) -> Symbol {
    Symbol {
        library_stamp: Some(RowStamp {
            footprint: Some("SOIC-8".into()),
            manufacturer: Some(String::new()),
            mpn: None,
            ..RowStamp::new(Uuid::from_u128(0x2000 + n))
        }),
        ..linked(n)
    }
}

fn stamp_of(sheet: &SchematicSheet) -> Option<RowStamp> {
    sheet.symbols[0].library_stamp.clone()
}

#[test]
fn a_value_stamp_round_trips_byte_identical_and_keeps_absent_apart_from_empty() {
    let text = write(&sheet(vec![stamped(1), unlinked(2)]));
    assert!(text.contains("[extras.symbols.00000000-0000-0000-0000-000000000001.library_stamp]"));
    assert!(text.contains("footprint = \"SOIC-8\""), "{text}");
    assert!(
        text.contains("manufacturer = \"\""),
        "empty is a value:\n{text}"
    );
    assert!(!text.contains("mpn ="), "absent stays absent:\n{text}");
    let back = SnxSchematic::parse(&text).unwrap().sheet;
    assert_eq!(stamp_of(&back), stamped(1).library_stamp);
    assert_eq!(write(&back), text, "the encoding must be stable");
}

#[test]
fn a_symbol_whose_only_extra_is_the_stamp_is_not_dropped() {
    let only = Symbol {
        uuid: Uuid::from_u128(5),
        reference: "R5".into(),
        library_stamp: Some(RowStamp::new(Uuid::from_u128(0x2005))),
        ..Default::default()
    };
    let text = write(&sheet(vec![only.clone()]));
    assert!(text.contains(".library_stamp]"), "{text}");
    assert_eq!(
        stamp_of(&SnxSchematic::parse(&text).unwrap().sheet),
        only.library_stamp
    );
}

#[test]
fn a_file_with_a_version_and_no_stamp_loads_with_none_and_resaves_identical() {
    let back = SnxSchematic::parse(WRITTEN).unwrap().sheet;
    assert_eq!(stamp_of(&back), None);
    assert_eq!(back.symbols[0].library_version, "1.0.2");
    assert_eq!(write(&back), WRITTEN);
}

const STAMP_WRITTEN: &str =
    include_str!("../../tests/fixtures/library_link/written_with_value_stamp.snxsch");
const STAMP_RESAVED: &str =
    include_str!("../../tests/fixtures/library_link/value_stamp_resaved_by_older_build.snxsch");

#[test]
fn this_build_writes_the_value_stamp_fixture_bytes() {
    assert_eq!(write(&sheet(vec![stamped(1), unlinked(2)])), STAMP_WRITTEN);
}

/// A build without the field (trunk before #2004) kept the table as unknown
/// keys and re-saved it; the stamp is still there, byte for byte.
#[test]
fn an_older_build_resave_keeps_the_value_stamp() {
    let back = SnxSchematic::parse(STAMP_RESAVED).unwrap().sheet;
    assert_eq!(stamp_of(&back), stamped(1).library_stamp);
    assert_eq!(write(&back), STAMP_WRITTEN, "no byte churn either way");
}

fn stamp_text(edit: impl Fn(String) -> String) -> String {
    edit(write(&sheet(vec![stamped(1)])))
}

/// Counts the warnings of target `signex::format` raised while `f` runs.
fn format_warnings<T>(f: impl FnOnce() -> T) -> (T, usize) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Count(AtomicUsize);
    impl tracing::Subscriber for Count {
        fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }
        fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            let meta = event.metadata();
            if meta.target() == "signex::format" && *meta.level() == tracing::Level::WARN {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }
    let count = std::sync::Arc::new(Count(AtomicUsize::new(0)));
    let out = tracing::subscriber::with_default(std::sync::Arc::clone(&count), f);
    (out, count.0.load(Ordering::Relaxed))
}

#[test]
fn a_malformed_value_stamp_loads_as_none_with_a_warning_and_the_sheet_opens() {
    // The stamp is the last table, so its `row_id` is the last one.
    let bad_row = stamp_text(|t| {
        let at = t.rfind("row_id = \"").unwrap();
        let end = at + t[at..].find('\n').unwrap();
        format!("{}row_id = \"nope\"{}", &t[..at], &t[end..])
    });
    let wrong_type = stamp_text(|t| t.replace("footprint = \"SOIC-8\"", "footprint = 7"));
    for text in [bad_row, wrong_type] {
        let (back, warnings) = format_warnings(|| SnxSchematic::parse(&text));
        let back = back
            .expect("a malformed stamp must not fail the sheet")
            .sheet;
        assert_eq!(back.symbols.len(), 1);
        assert_eq!(stamp_of(&back), None, "{text}");
        assert_eq!(
            back.symbols[0].library_version, "1.0.2",
            "the link survives"
        );
        assert_eq!(warnings, 1, "{text}");
    }
}

#[test]
fn a_key_a_later_build_adds_to_the_stamp_survives_a_save() {
    let text = stamp_text(|t| {
        t.replace("[extras.symbols.00000000-0000-0000-0000-000000000001.library_stamp]\n", "[extras.symbols.00000000-0000-0000-0000-000000000001.library_stamp]\ndatasheet = \"sheet.pdf\"\n")
    });
    let back = SnxSchematic::parse(&text).unwrap().sheet;
    assert!(stamp_of(&back).is_some());
    assert!(write(&back).contains("datasheet = \"sheet.pdf\""));
}

/// An older build keeps the stamp among the unknown tables, alphabetically;
/// this build writes it at the same place, so a re-save changes no byte even
/// beside a foreign table that sorts before or after it.
#[test]
fn the_value_stamp_sorts_among_foreign_tables_like_an_older_build_writes_it() {
    let head = "[extras.symbols.00000000-0000-0000-0000-000000000001.";
    let text = stamp_text(|t| {
        let at = t.find(&format!("{head}library_stamp]")).unwrap();
        format!(
            "{}{head}aaa_future]\nkey = 1\n\n{}\n\n{head}zzz_future]\nkey = 2\n",
            &t[..at],
            t[at..].trim_end()
        )
    });
    let back = SnxSchematic::parse(&text).unwrap().sheet;
    assert_eq!(stamp_of(&back), stamped(1).library_stamp);
    assert_eq!(write(&back), text);
}

/// Regenerates the value-stamp fixtures with a build from before #2004; see
/// `regenerate_link_fixtures` for the `resave` binary:
/// `OLDER_BUILD_RESAVE=<binary> cargo test -p signex-types regenerate_stamp_fixtures -- --ignored`
#[test]
#[ignore = "writes fixtures; needs a build of an older signex-types"]
fn regenerate_stamp_fixtures() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/library_link");
    let tool = std::env::var("OLDER_BUILD_RESAVE").expect("OLDER_BUILD_RESAVE");
    let written = format!("{dir}/written_with_value_stamp.snxsch");
    std::fs::write(&written, write(&sheet(vec![stamped(1), unlinked(2)]))).unwrap();
    let status = std::process::Command::new(tool)
        .args([
            &written,
            &format!("{dir}/value_stamp_resaved_by_older_build.snxsch"),
        ])
        .status()
        .unwrap();
    assert!(status.success());
}
