//! #828 -- saving a primitive that holds a NaN or infinite number fails
//! loudly and leaves the file already on disk exactly as it was.
//!
//! Before the encoder guard a release build wrote an empty cell (the
//! value silently reset on reopen) and a debug build panicked inside
//! `debug_assert!`.

use std::path::{Path, PathBuf};

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::primitive::{Footprint, Pad, Symbol, SymbolPin};
use uuid::Uuid;

fn library(dir: &Path) -> PathBuf {
    signex_library::git_identity::test_identity::install();
    let file = dir.join("Lib").join("Lib.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "Lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    drop(
        LocalGitAdapter::init(
            &file,
            manifest,
            LibraryInitOptions {
                enable_git: false,
                use_lfs: false,
            },
        )
        .expect("init succeeds"),
    );
    file
}

/// The single primitive file with `extension` under `<library>/<dir>`.
fn only_file(library: &Path, dir: &str, extension: &str) -> PathBuf {
    let found: Vec<PathBuf> = std::fs::read_dir(library.parent().unwrap().join(dir))
        .expect("primitive directory exists after the first save")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == extension))
        .collect();
    assert_eq!(found.len(), 1, "expected one .{extension}: {found:?}");
    found.into_iter().next().unwrap()
}

#[test]
fn a_non_finite_symbol_pin_fails_the_save_and_keeps_the_file() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let dir = tempfile::tempdir().unwrap();
        let lib = library(dir.path());
        let adapter = LocalGitAdapter::open(&lib).unwrap();
        let mut symbol = Symbol::empty("R");
        symbol.pins.push(SymbolPin::new("1", "A"));
        adapter.save_symbol(symbol.clone(), "good").unwrap();
        let path = only_file(&lib, "symbols", "snxsym");
        let before = std::fs::read(&path).unwrap();

        symbol.pins[0].length = bad;
        let err = adapter
            .save_symbol(symbol, "bad")
            .expect_err("a non-finite pin length must fail the save");

        let message = err.to_string();
        assert!(
            message.contains("length") && message.contains("non-finite"),
            "the error must name the field: {message}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before, "{bad}");
    }
}

#[test]
fn a_non_finite_footprint_pad_fails_the_save_and_keeps_the_file() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let dir = tempfile::tempdir().unwrap();
        let lib = library(dir.path());
        let adapter = LocalGitAdapter::open(&lib).unwrap();
        let mut footprint = Footprint::empty("R0805");
        footprint.pads.push(Pad {
            number: "1".into(),
            ..Pad::default()
        });
        adapter.save_footprint(footprint.clone(), "good").unwrap();
        let path = only_file(&lib, "footprints", "snxfpt");
        let before = std::fs::read(&path).unwrap();

        footprint.pads[0].rotation = bad;
        let err = adapter
            .save_footprint(footprint, "bad")
            .expect_err("a non-finite pad rotation must fail the save");

        let message = err.to_string();
        assert!(
            message.contains("rotation") && message.contains("non-finite"),
            "the error must name the field: {message}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before, "{bad}");
    }
}
