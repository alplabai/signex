// Standalone binary: declared as a `[[test]]` with `required-features` in Cargo.toml.
//! Conformance fixtures for `docs/LIBRARY_FORMAT.md` (#3105).
//!
//! Builds a small library in code through the public API, with fixed ids and
//! timestamps, and checks it against `docs/fixtures/library-format/`: the
//! writer must produce the committed bytes, every committed file must re-save
//! byte-identically, Signex must open the library, and each row's
//! `content_hash` must be the SHA-256 of the canonical JSON the spec
//! describes. Regenerate with `SIGNEX_BLESS_LIBRARY_FORMAT=1`.
//!
//! The data is synthetic: it exercises every cell form, it is not a set of
//! electrically sensible parts.

#![cfg(feature = "local-git")]

mod ids;
mod primitives;
mod rows;

use ids::*;
use primitives::*;
use rows::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LocalGitAdapter, component_to_library_row};
use signex_library::alternate_symbol::AlternateSymbol;
use signex_library::component::{
    ComponentRow, DatasheetDoc, DatasheetRef, PinPadOverride, PlmReserved,
};
use signex_library::distributor::PriceBreak;
use signex_library::footprint_binding::FootprintBinding;
use signex_library::identity::{ComponentClass, InternalPn};
use signex_library::library_file::{
    ClassEntry, ColumnType, FORMAT_TOKEN, LibraryFile, LibrarySection, LibraryTable, SnxlibManifest,
};
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMode, UserEntry, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::{AlternateStatus, DistributorListing, ManufacturerPart};
use signex_library::param::{ParamMap, ParamValue};
use signex_library::pricing::CachedPricing;
use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};
use signex_library::primitive::symbol::{
    ComponentType, PinSymbolKind, SymbolGraphic, SymbolGraphicKind,
};
use signex_library::primitive::{
    Footprint, FootprintFile, LayerId, Pad, PadKind, PadShape, PinDirection, PinOrientation,
    Polygon, PrimitiveRef, StepAttachment, Symbol, SymbolFile, SymbolPin,
};
use signex_library::tables::{ALTERNATE_SYMBOLS_COLUMN, TABLE_HEADER};

// ── The fixture set ──────────────────────────────────────────────────────

/// Every fixture file, relative to [`fixture_root`], with its bytes.
fn generated() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let lib = |rel: &str| format!("library/{rel}");
    out.insert(
        lib(SNXLIB_NAME),
        library_file().write().expect("write .snxlib"),
    );
    out.insert(
        lib(SYMBOL_ENVELOPE),
        symbol_envelope().to_toml_string().expect("write .snxsym"),
    );
    for (fp, file) in footprints() {
        let rel = lib(&format!("footprints/{}.snxfpt", fp.uuid));
        let text = footprint_envelope(fp, file)
            .to_toml_string()
            .expect("write .snxfpt");
        out.insert(rel, text);
    }
    for r in rows() {
        let json = String::from_utf8(canonical_json(&r)).expect("utf-8 json");
        out.insert(format!("content-hash/{}.json", r.row_id), json);
    }
    out
}

/// The committed fixture files; panics when the core files are missing, so
/// an empty or misplaced fixture folder cannot pass as "nothing to check".
fn committed() -> BTreeMap<String, String> {
    let root = fixture_root();
    let mut out = BTreeMap::new();
    for dir in ["library", "content-hash"] {
        collect(&root, &root.join(dir), &mut out);
    }
    let mut required = vec![
        format!("library/{SNXLIB_NAME}"),
        format!("library/{SYMBOL_ENVELOPE}"),
    ];
    required.extend(footprints().map(|(fp, _)| format!("library/footprints/{}.snxfpt", fp.uuid)));
    for rel in required {
        assert!(
            out.contains_key(&rel),
            "fixture {rel} is missing under {}; bless with {BLESS_VAR}=1",
            root.display()
        );
    }
    out
}

fn collect(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("fixture dir entry").path();
        if path.is_dir() {
            collect(root, &path, out);
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .expect("under the fixture root")
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(&path).expect("fixture is UTF-8");
        out.insert(rel, text);
    }
}

fn blessing() -> bool {
    std::env::var(BLESS_VAR).as_deref() == Ok("1")
}

/// Rewrite the fixtures from [`generated`]. Called only by
/// [`the_writer_produces_the_committed_fixtures`]; every other test skips
/// while blessing, so no test reads a half-written fixture under a runner
/// that runs tests in parallel or in separate processes.
fn bless() {
    let root = fixture_root();
    for dir in ["library", "content-hash"] {
        let path = root.join(dir);
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("clear old fixtures");
        }
    }
    for (rel, text) in generated() {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("fixture parent")).expect("mkdir");
        std::fs::write(&path, text).expect("write fixture");
    }
}

/// True (and a note on stderr) when this test should not run: the fixtures
/// are being rewritten.
fn skip_while_blessing() -> bool {
    if blessing() {
        eprintln!("{BLESS_VAR}=1: skipped; run again without it to check the new fixtures");
    }
    blessing()
}

fn first_difference(want: &str, got: &str) -> String {
    let (mut w, mut g) = (want.lines(), got.lines());
    for line in 1.. {
        match (w.next(), g.next()) {
            (None, None) => return "identical lines; line endings differ".into(),
            (a, b) if a != b => return format!("line {line}: committed {a:?}, written {b:?}"),
            _ => {}
        }
    }
    unreachable!()
}

fn assert_same(rel: &str, committed: &str, written: &str) {
    assert!(
        committed == written,
        "{rel} differs from what the writer produces ({}); rerun with {BLESS_VAR}=1 \
         if the format change is intended, and update docs/LIBRARY_FORMAT.md",
        first_difference(committed, written)
    );
}

fn read_fixture(rel: &str) -> String {
    std::fs::read_to_string(fixture_root().join(rel))
        .unwrap_or_else(|e| panic!("read fixture {rel}: {e}; bless with {BLESS_VAR}=1"))
}

// ── Tests ────────────────────────────────────────────────────────────────

#[test]
fn the_writer_produces_the_committed_fixtures() {
    if blessing() {
        bless();
    }
    let written = generated();
    let on_disk = committed();
    assert_eq!(
        on_disk.keys().collect::<Vec<_>>(),
        written.keys().collect::<Vec<_>>(),
        "fixture file set differs; rerun with {BLESS_VAR}=1"
    );
    for (rel, text) in &written {
        assert_same(rel, &on_disk[rel], text);
    }
}

#[test]
fn every_committed_file_resaves_byte_identically() {
    if skip_while_blessing() {
        return;
    }
    for (rel, text) in committed() {
        let resaved = if rel.ends_with(".snxlib") {
            LibraryFile::parse(&text).unwrap().write().unwrap()
        } else if rel.ends_with(".snxsym") {
            SymbolFile::from_toml_str(&text)
                .unwrap()
                .to_toml_string()
                .unwrap()
        } else if rel.ends_with(".snxfpt") {
            FootprintFile::from_toml_str(&text)
                .unwrap()
                .to_toml_string()
                .unwrap()
        } else {
            continue;
        };
        assert_same(&rel, &text, &resaved);
    }
}

/// Copy the fixture library to a scratch folder and open it as Signex does.
fn open_copy() -> (tempfile::TempDir, LocalGitAdapter) {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("Conformance");
    for (rel, text) in committed() {
        let Some(rel) = rel.strip_prefix("library/") else {
            continue;
        };
        let path = dest.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let adapter = LocalGitAdapter::open(dest.join(SNXLIB_NAME)).expect("Signex opens the fixture");
    (dir, adapter)
}

#[test]
fn signex_opens_the_fixture_and_every_reference_resolves() {
    if skip_while_blessing() {
        return;
    }
    let (_dir, adapter) = open_copy();
    assert_eq!(adapter.library_id(), id(LIBRARY_ID));
    assert_eq!(adapter.list_tables().unwrap(), ["logic", "resistors"]);
    let mut seen = 0;
    for (_, r) in adapter.iter_rows().unwrap() {
        seen += 1;
        adapter.get_symbol(r.symbol_ref.uuid).unwrap();
        for alt in &r.alternate_symbols {
            adapter.get_symbol(alt.symbol_ref.uuid).unwrap();
        }
        for binding in r.footprint_bindings() {
            let fp = adapter.get_footprint(binding.footprint.uuid).unwrap();
            assert_eq!(fp.uuid, binding.footprint.uuid);
        }
    }
    assert_eq!(seen, 3);
}

#[test]
fn rows_read_back_and_resave_to_the_same_cells() {
    if skip_while_blessing() {
        return;
    }
    let (_dir, adapter) = open_copy();
    let mut file = LibraryFile::parse(&read_fixture(&format!("library/{SNXLIB_NAME}"))).unwrap();
    for (name, table) in &mut file.tables {
        table.rows = adapter
            .read_table(name)
            .unwrap()
            .iter()
            .map(|r| component_to_library_row(r).map(Arc::new).unwrap())
            .collect();
    }
    assert_same(
        SNXLIB_NAME,
        &read_fixture(&format!("library/{SNXLIB_NAME}")),
        &file.write().unwrap(),
    );
}

#[test]
fn content_hash_is_sha256_of_the_canonical_json() {
    if skip_while_blessing() {
        return;
    }
    let (_dir, adapter) = open_copy();
    for (_, r) in adapter.iter_rows().unwrap() {
        let json = read_fixture(&format!("content-hash/{}.json", r.row_id));
        let digest: [u8; 32] = Sha256::digest(json.as_bytes()).into();
        assert_eq!(digest, r.content_hash, "stored hash of {}", r.internal_pn);
        assert_eq!(
            digest,
            signex_library::hash_row_content(&r).unwrap(),
            "hash.rs of {}",
            r.internal_pn
        );
        assert_eq!(json.as_bytes(), canonical_json(&r), "spec transcription");
    }
}

/// Every fixture footprint converts to a board footprint, as placing it on
/// a board does: the named layer tokens resolve and pad coordinates are
/// copied unchanged (the footprint frame is the board's, +y down the screen).
#[test]
fn every_fixture_footprint_converts_to_a_board_footprint() {
    if skip_while_blessing() {
        return;
    }
    let (_dir, adapter) = open_copy();
    let expected_layers = ["TopCopper", "TopSolderMask", "TopPaste"];
    for (fp, _) in footprints() {
        let address = lib_ref(&fp.uuid.to_string());
        let resolved = signex_library::part_footprint::resolve_named_footprint(&address, |a, _| {
            adapter.get_footprint(a.uuid).ok()
        })
        .unwrap_or_else(|e| panic!("{} does not convert: {e}", fp.name));
        let board = &resolved.footprint;
        assert_eq!(board.pads.len(), fp.pads.len(), "{}", fp.name);
        for (lib_pad, board_pad) in fp.pads.iter().zip(&board.pads) {
            assert_eq!(board_pad.number, lib_pad.number);
            assert_eq!(
                board_pad.layers, expected_layers,
                "{} pad {}",
                fp.name, lib_pad.number
            );
            assert_eq!(
                [board_pad.position.x, board_pad.position.y],
                lib_pad.position,
                "{} pad {}",
                fp.name,
                lib_pad.number
            );
        }
    }
}

/// The string, number and time forms `docs/LIBRARY_FORMAT.md` §5 lists,
/// written through a symbol envelope.
#[test]
fn scalar_forms_are_the_ones_the_spec_lists() {
    let strings = [
        ("plain", r#"description = "plain""#),
        ("a'b", r#"description = "a'b""#),
        ("a\"b", r#"description = 'a"b'"#),
        ("a\\b", r#"description = 'a\b'"#),
        ("a'b\"c", r#"description = """a'b"c""""#),
        ("a\\b'c", r#"description = '''a\b'c'''"#),
        ("tab\tx", r#"description = "tab\tx""#),
        ("x\u{7f}y", r#"description = "x\u007Fy""#),
        ("l1\nl2", "description = \"\"\"\nl1\nl2\"\"\""),
    ];
    for (value, line) in strings {
        let mut sym = resistor_symbol();
        sym.description = value.into();
        let mut file = symbol_envelope();
        file.symbols = vec![sym];
        let text = file.to_toml_string().unwrap();
        assert!(
            text.contains(&format!("\n{line}\n")),
            "{value:?} not written as {line}"
        );
        let back = SymbolFile::from_toml_str(&text).unwrap();
        assert_eq!(back.symbols[0].description, value);
    }

    let mut sym = resistor_symbol();
    sym.anchor = [1e-7, -0.0];
    sym.designator_pos = Some([1e21, 123_456_789.125]);
    sym.pins[0].position = [1e-7, -0.0];
    sym.pins[1].position = [0.1 + 0.2, 1e21];
    sym.created = Utc.timestamp_opt(1_700_000_000, 120_000_000).unwrap();
    sym.updated = Utc.timestamp_opt(1_700_000_000, 123_456_789).unwrap();
    let mut file = symbol_envelope();
    file.symbols = vec![sym];
    let text = file.to_toml_string().unwrap();
    for expected in [
        "anchor = [\n    0.0000001,\n    -0.0,\n]",
        "designator_pos = [\n    1000000000000000000000.0,\n    123456789.125,\n]",
        "\n1\t1\tPassive\t0.0000001\t0\tRight\t2.54\t",
        "\n2\t2\tPassive\t0.30000000000000004\t1000000000000000000000\tLeft\t",
        "created = \"2023-11-14T22:13:20.120Z\"",
        "updated = \"2023-11-14T22:13:20.123456789Z\"",
    ] {
        assert!(text.contains(expected), "missing {expected:?}");
    }

    let mut r = row_nand();
    r.created = Utc.timestamp_opt(1_700_000_000, 120_000_000).unwrap();
    let cells = component_to_library_row(&r).unwrap().cells;
    assert_eq!(cells["created"], "2023-11-14T22:13:20.120+00:00");

    let mut r = row_nand();
    r.parameters = [
        ("a", 1e-6),
        ("b", 1e-5),
        ("c", 0.00012),
        ("d", 1e15),
        ("e", 1e16),
        ("f", 1.5e16),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), ParamValue::Number(v)))
    .collect();
    let cells = component_to_library_row(&r).unwrap().cells;
    assert_eq!(
        cells["parameters"],
        concat!(
            r#"{"a":{"kind":"Number","value":1e-6},"b":{"kind":"Number","value":0.00001},"#,
            r#""c":{"kind":"Number","value":0.00012},"#,
            r#""d":{"kind":"Number","value":1000000000000000.0},"#,
            r#""e":{"kind":"Number","value":1e+16},"f":{"kind":"Number","value":1.5e+16}}"#
        )
    );
}
