//! A component-table row one cell short of the 22-column header.
//!
//! Only a row shaped like a legacy 21-cell row reads (as no alternates); a
//! dropped middle cell, which shifts the later cells left, must not.

#![cfg(feature = "local-git")]

use std::path::PathBuf;

use chrono::Utc;
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::*;
use uuid::Uuid;

/// Index of `footprint_version` / `released` in the table header.
const FOOTPRINT_VERSION: usize = 19;
const RELEASED: usize = 17;

fn row(pn: &str, lib: Uuid) -> ComponentRow {
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::new("resistor"),
        datasheet: DatasheetRef::url(""),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Released,
        symbol_ref: PrimitiveRef::new(lib, Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", pn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: "0.0.1".into(),
        footprint_version: "0.0.1".into(),
        sim_version: String::new(),
        created: Utc::now(),
        updated: Utc::now(),
        content_hash: [0u8; 32],
        server_revision: None,
    }
}

/// A library with `R-ALT` (one alternate symbol) and `R-PLAIN`, so the table
/// has the 22-column header.
fn library() -> (tempfile::TempDir, PathBuf, Uuid) {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("L").join("L.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "L".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = LocalGitAdapter::init(
        &file,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .unwrap();
    let mut alt = row("R-ALT", library_id);
    alt.alternate_symbols.push(AlternateSymbol::new(
        PrimitiveRef::new(library_id, Uuid::now_v7()),
        "0.0.1",
        "IEC",
    ));
    adapter.insert_row("t", alt, "seed").unwrap();
    adapter
        .insert_row("t", row("R-PLAIN", library_id), "seed")
        .unwrap();
    drop(adapter);
    (dir, file, library_id)
}

/// Rewrite the tab-separated line naming `pn` with `edit`.
fn edit_line(file: &PathBuf, pn: &str, edit: impl Fn(&mut Vec<String>)) {
    let text = std::fs::read_to_string(file).unwrap();
    let out: Vec<String> = text
        .split('\n')
        .map(|line| {
            let mut cells: Vec<String> = line.split('\t').map(str::to_string).collect();
            if cells.get(1).map(String::as_str) == Some(pn) {
                edit(&mut cells);
                cells.join("\t")
            } else {
                line.to_string()
            }
        })
        .collect();
    std::fs::write(file, out.join("\n")).unwrap();
}

#[test]
fn a_genuinely_short_legacy_row_reads_with_no_alternates() {
    let (_dir, file, _) = library();
    edit_line(&file, "R-PLAIN", |cells| {
        assert_eq!(
            cells.pop().as_deref(),
            Some(""),
            "the empty alternates cell"
        );
    });
    let adapter = LocalGitAdapter::open(&file).unwrap();
    let rows = adapter.iter_rows().unwrap();
    let plain = rows
        .iter()
        .find(|(_, r)| r.internal_pn.as_str() == "R-PLAIN");
    assert!(plain.unwrap().1.alternate_symbols.is_empty());
    let alt = rows.iter().find(|(_, r)| r.internal_pn.as_str() == "R-ALT");
    assert_eq!(
        alt.unwrap().1.alternate_symbols.len(),
        1,
        "other rows keep theirs"
    );
}

#[test]
fn a_dropped_middle_cell_with_alternates_present_is_an_error() {
    let (_dir, file, _) = library();
    edit_line(&file, "R-ALT", |cells| {
        cells.remove(FOOTPRINT_VERSION);
    });
    let adapter = LocalGitAdapter::open(&file);
    // Whether open or the first row read refuses, the alternates must not
    // silently vanish.
    let read_ok = adapter.is_ok_and(|a| {
        a.iter_rows().is_ok_and(|rows| {
            rows.iter()
                .any(|(_, r)| r.internal_pn.as_str() == "R-ALT" && r.alternate_symbols.is_empty())
        })
    });
    assert!(!read_ok, "the alternates must not read as empty");
}

#[test]
fn a_dropped_cell_before_the_version_cells_is_an_error_even_without_alternates() {
    let (_dir, file, _) = library();
    edit_line(&file, "R-PLAIN", |cells| {
        cells.pop();
        cells.remove(RELEASED);
    });
    assert!(
        LocalGitAdapter::open(&file).is_err(),
        "the shifted `released` cell fails the shape check"
    );
}
