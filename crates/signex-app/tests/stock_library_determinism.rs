//! The stock-library generator must be idempotent: designs reference
//! stock primitives by `PrimitiveRef { library_id, uuid }`, so a rerun
//! that re-minted ids or timestamps would silently break them (#323).
//! The `Stock.snxlib` wrapper holds to the same rule and must open
//! through the real adapter (#324).

#[path = "../examples/build_stock_library/main.rs"]
mod stock_gen;

use std::path::{Path, PathBuf};

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::manifest::WorkflowMode;
use signex_library::{LibraryFile, LibraryMode};

/// Pinned stock library id, spelled out here so editing the constant
/// in the generator fails this test instead of passing silently.
const EXPECTED_LIBRARY_ID: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae";

fn stock_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library")
}

fn stock_dir() -> PathBuf {
    stock_root().join("footprints")
}

fn read_lf(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .replace("\r\n", "\n")
}

/// Every file under `root` with its bytes, sorted by path.
fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                let bytes = std::fs::read(&path).expect("read file");
                out.push((path, bytes));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out.sort();
    out
}

#[test]
fn generating_twice_is_byte_identical() {
    let first = stock_gen::render_all().expect("first render");
    let second = stock_gen::render_all().expect("second render");
    assert_eq!(first.len(), 5, "five stock footprints");
    for (a, b) in first.iter().zip(&second) {
        assert_eq!(a.file_name, b.file_name);
        assert_eq!(
            a.text, b.text,
            "{} differs between two generator runs: an identity field is minted per run",
            a.file_name
        );
    }
}

#[test]
fn committed_stock_files_match_the_generator_and_id_table() {
    for file in stock_gen::render_all().expect("render") {
        let path = stock_dir().join(&file.file_name);
        assert_eq!(
            read_lf(&path),
            file.text,
            "{} is stale: rerun `cargo run --example build_stock_library -p signex-app`",
            file.file_name
        );
    }
    for row in stock_gen::ids::STOCK_IDS {
        let path = stock_dir().join(row.file_name());
        let committed = std::fs::read_to_string(&path).expect("read committed file");
        for id in row.all_ids() {
            assert!(
                committed.contains(&id.to_string()),
                "{} does not carry pinned id {id}",
                row.file_name()
            );
        }
    }
}

#[test]
fn committed_manifest_matches_the_generator() {
    let path = stock_root().join(stock_gen::manifest::SNXLIB_FILE_NAME);
    assert_eq!(
        read_lf(&path),
        stock_gen::manifest::render_manifest().expect("render manifest"),
        "Stock.snxlib is stale: rerun `cargo run --example build_stock_library -p signex-app`"
    );
}

#[test]
fn library_id_is_the_pinned_constant() {
    let committed = LibraryFile::parse(&read_lf(
        &stock_root().join(stock_gen::manifest::SNXLIB_FILE_NAME),
    ))
    .expect("parse committed Stock.snxlib");
    assert_eq!(
        committed.manifest.library_id.to_string(),
        EXPECTED_LIBRARY_ID
    );
    assert_eq!(
        stock_gen::manifest::stock_library_file()
            .manifest
            .library_id,
        committed.manifest.library_id
    );
}

#[test]
fn manifest_declares_local_git_personal_empty_schema_and_classes() {
    let lf = stock_gen::manifest::stock_library_file();
    assert_eq!(lf.manifest.mode, LibraryMode::LocalGit);
    assert_eq!(lf.manifest.workflow.mode, WorkflowMode::Personal);
    assert!(!lf.manifest.classes.is_empty(), "class registry is seeded");
    assert_eq!(lf.tables.len(), 1);
    let table = lf.tables.values().next().expect("one table");
    assert!(!table.columns.is_empty(), "schema is declared");
    assert!(table.rows.is_empty(), "no component rows yet");
}

#[test]
fn committed_stock_library_opens_and_lists_all_five_footprints() {
    let adapter = LocalGitAdapter::open(stock_root().join(stock_gen::manifest::SNXLIB_FILE_NAME))
        .expect("Stock.snxlib opens without a .git/");
    let mut names: Vec<String> = adapter
        .list_footprints()
        .expect("list footprints")
        .into_iter()
        .map(|s| s.name)
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "Fiducial-1mm",
            "MountingHole-3.2mm",
            "QFN-16",
            "R0805",
            "SOIC-8"
        ]
        .map(String::from)
        .to_vec()
    );
    for row in stock_gen::ids::STOCK_IDS {
        adapter
            .get_footprint(row.footprint_uuid())
            .unwrap_or_else(|e| panic!("load {}: {e}", row.file_name()));
    }
}

#[test]
fn writing_the_library_twice_is_byte_identical_and_never_inits_git() {
    let dir = tempfile::tempdir().expect("temp dir");
    stock_gen::write_library(dir.path()).expect("first write");
    let first = snapshot(dir.path());
    stock_gen::write_library(dir.path()).expect("second write");
    assert_eq!(first, snapshot(dir.path()), "second run changed bytes");
    assert!(!dir.path().join(".git").exists(), "no git init in the tree");
    assert!(dir.path().join("symbols").is_dir());
}
