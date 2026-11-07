//! The committed stock library is the source of truth (#1892): maintainers
//! edit it in Signex with `SIGNEX_EDIT_STOCK=1`, so these tests check the
//! committed files, not that they still match the seed generator.
//!
//! What protects existing designs is identity: designs reference stock
//! primitives and rows by `PrimitiveRef { library_id, uuid }` and row id,
//! so every pinned id must survive any edit (#323), and the wrapper must
//! still open through the real adapter (#324). The seed generator itself
//! must stay idempotent, so a `--force` reseed writes identical bytes.

#[path = "../../examples/build_stock_library/main.rs"]
mod stock_gen;

use std::path::{Path, PathBuf};

use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::manifest::WorkflowMode;
use signex_library::primitive::symbol::{PinDirection, SymbolFile};
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

fn symbols_dir() -> PathBuf {
    stock_root().join("symbols")
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

/// Every committed stock symbol file, parsed, with its file name.
fn committed_symbols() -> Vec<(String, SymbolFile)> {
    let mut out: Vec<(String, SymbolFile)> = std::fs::read_dir(symbols_dir())
        .expect("read symbols dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "snxsym"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let file = SymbolFile::from_toml_str(&read_lf(&p))
                .unwrap_or_else(|e| panic!("parse {name}: {e}"));
            (name, file)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!out.is_empty(), "the stock library ships no symbols");
    out
}

fn committed_library() -> LibraryFile {
    LibraryFile::parse(&read_lf(
        &stock_root().join(stock_gen::manifest::SNXLIB_FILE_NAME),
    ))
    .expect("parse committed Stock.snxlib")
}

// ── The seed generator ───────────────────────────────────────────────

#[test]
fn generating_twice_is_byte_identical() {
    let first = stock_gen::render_all().expect("first render");
    let second = stock_gen::render_all().expect("second render");
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
fn writing_the_library_twice_is_byte_identical_and_never_inits_git() {
    let dir = tempfile::tempdir().expect("temp dir");
    stock_gen::write_library(dir.path()).expect("first write");
    let first = snapshot(dir.path());
    stock_gen::write_library(dir.path()).expect("second write");
    assert_eq!(first, snapshot(dir.path()), "second run changed bytes");
    assert!(!dir.path().join(".git").exists(), "no git init in the tree");
    assert!(dir.path().join("symbols").is_dir());
}

// ── Identity of the committed library ────────────────────────────────

#[test]
fn library_id_is_the_pinned_constant() {
    let committed = committed_library();
    assert_eq!(
        committed.manifest.library_id.to_string(),
        EXPECTED_LIBRARY_ID
    );
    assert_eq!(
        stock_gen::ids::STOCK_LIBRARY_ID,
        EXPECTED_LIBRARY_ID,
        "the generator's pin and the test's pin agree"
    );
    assert_eq!(
        signex_app::library::state::STOCK_LIBRARY_ID.to_string(),
        EXPECTED_LIBRARY_ID,
        "the app's STOCK_LIBRARY_ID is the committed manifest's id"
    );
}

#[test]
fn every_pinned_footprint_id_is_still_committed() {
    for row in stock_gen::ids::STOCK_IDS {
        let path = stock_dir().join(row.file_name());
        let committed = read_lf(&path);
        for id in row.all_ids() {
            assert!(
                committed.contains(&id.to_string()),
                "{} no longer carries pinned id {id}: designs placed with it would dangle",
                row.file_name()
            );
        }
    }
}

#[test]
fn every_pinned_symbol_id_is_still_committed() {
    let all: Vec<uuid::Uuid> = committed_symbols()
        .iter()
        .flat_map(|(_, f)| f.symbols.iter().map(|s| s.uuid))
        .collect();
    for (name, _, symbol) in stock_gen::ids::STOCK_SYMBOL_IDS {
        let id = uuid::Uuid::parse_str(symbol).expect("pinned symbol id");
        assert!(
            all.contains(&id),
            "stock symbol {name} ({id}) is gone: designs placed with it would dangle"
        );
    }
}

#[test]
fn every_pinned_row_id_is_still_committed() {
    let adapter = LocalGitAdapter::open(stock_root().join(stock_gen::manifest::SNXLIB_FILE_NAME))
        .expect("Stock.snxlib opens");
    let rows: Vec<uuid::Uuid> = adapter
        .iter_rows()
        .expect("read rows")
        .into_iter()
        .map(|(_, r)| r.row_id)
        .collect();
    for id in stock_gen::rows::pinned_row_ids() {
        assert!(rows.contains(&id), "stock row {id} is gone");
    }
}

// ── Shape of the committed library ───────────────────────────────────

#[test]
fn manifest_declares_local_git_personal_schema_classes_and_rows() {
    let lf = committed_library();
    assert_eq!(lf.manifest.mode, LibraryMode::LocalGit);
    assert_eq!(lf.manifest.workflow.mode, WorkflowMode::Personal);
    assert!(!lf.manifest.classes.is_empty(), "class registry is seeded");
    assert_eq!(lf.tables.len(), 1);
    let table = lf.tables.values().next().expect("one table");
    assert!(!table.columns.is_empty(), "schema is declared");
    assert!(
        table.rows.len() >= stock_gen::rows::pinned_row_ids().len(),
        "rows may be added, never removed"
    );
}

#[test]
fn committed_stock_library_opens_and_keeps_the_shipped_footprints() {
    let adapter = LocalGitAdapter::open(stock_root().join(stock_gen::manifest::SNXLIB_FILE_NAME))
        .expect("Stock.snxlib opens without a .git/");
    let names: Vec<String> = adapter
        .list_footprints()
        .expect("list footprints")
        .into_iter()
        .map(|s| s.name)
        .collect();
    for shipped in [
        "Chip-0805",
        "Chip-0805-Polarised",
        "Fiducial-1mm",
        "MountingHole-3.2mm",
        "QFN-16",
        "SOIC-8",
    ] {
        assert!(names.iter().any(|n| n == shipped), "{shipped} is gone");
    }
    for row in stock_gen::ids::STOCK_IDS {
        adapter
            .get_footprint(row.footprint_uuid())
            .unwrap_or_else(|e| panic!("load {}: {e}", row.file_name()));
    }
}

#[test]
fn every_committed_symbol_round_trips_byte_identical() {
    for (name, file) in committed_symbols() {
        let text = read_lf(&symbols_dir().join(&name));
        assert_eq!(
            file.to_toml_string().expect("write"),
            text,
            "{name} does not round-trip"
        );
    }
}

#[test]
fn every_stock_symbol_pin_is_on_the_schematic_grid_and_converts() {
    let grid = stock_gen::symbols::GRID_MM;
    for (_, file) in committed_symbols() {
        for symbol in &file.symbols {
            assert!(!symbol.pins.is_empty(), "{} has no pins", symbol.name);
            for pin in &symbol.pins {
                for v in pin.position {
                    let steps = v / grid;
                    assert!(
                        (steps - steps.round()).abs() < 1e-6,
                        "{} pin {} at {:?} is off the {grid} mm grid",
                        symbol.name,
                        pin.number,
                        pin.position
                    );
                }
            }
            let lib = symbol.to_lib_symbol(symbol.name.clone());
            assert_eq!(
                lib.pins.len(),
                symbol.pins.len(),
                "{} pins lost",
                symbol.name
            );
            assert!(!lib.graphics.is_empty(), "{} has no body", symbol.name);
            if symbol.designator == "#PWR?" {
                assert_eq!(symbol.pins.len(), 1);
                assert_eq!(symbol.pins[0].electrical, PinDirection::PowerInput);
            }
        }
    }
}

/// A wire saved on trunk ends on these tips; the reshaped R and L (#2101)
/// keep them, so no saved wire is left short of its pin.
#[test]
fn stock_r_and_l_keep_their_pin_tips() {
    for name in ["R", "L"] {
        let file =
            SymbolFile::from_toml_str(&read_lf(&symbols_dir().join(format!("{name}.snxsym"))))
                .expect("parse");
        let tips: Vec<[f64; 2]> = file.symbols[0].pins.iter().map(|p| p.position).collect();
        assert_eq!(
            tips,
            vec![[0.0, 3.81], [0.0, -3.81]],
            "{name}: a saved wire ends on these"
        );
    }
}

#[test]
fn logic_packages_are_14_pin_with_power_on_7_and_14() {
    let mut seen = 0;
    for (_, file) in committed_symbols() {
        for symbol in &file.symbols {
            if !symbol.name.starts_with("74HC") {
                continue;
            }
            seen += 1;
            let pins = &symbol.pins;
            let mut numbers: Vec<u32> = pins.iter().map(|p| p.number.parse().unwrap()).collect();
            numbers.sort();
            assert_eq!(numbers, (1..=14).collect::<Vec<_>>(), "{}", symbol.name);
            let find = |n: &str| pins.iter().find(|p| p.number == n).unwrap();
            assert_eq!(find("7").name, "GND");
            assert_eq!(find("7").electrical, PinDirection::PowerInput);
            assert_eq!(find("14").name, "VCC");
            assert_eq!(find("14").electrical, PinDirection::PowerInput);
            assert_eq!(symbol.designator, "U?");
        }
    }
    assert!(seen >= 7, "the seven shipped 74HC parts are present");
}

#[test]
fn connector_pin_count_matches_name() {
    let mut seen = 0;
    for (_, file) in committed_symbols() {
        for symbol in &file.symbols {
            let Some((kind, dims)) = symbol.name.split_once('_') else {
                continue;
            };
            if kind != "Conn" && kind != "Screw" {
                continue;
            }
            seen += 1;
            let (rows, cols) = dims.split_once('x').unwrap();
            let expected = rows.parse::<usize>().unwrap() * cols.parse::<usize>().unwrap();
            assert_eq!(symbol.pins.len(), expected, "{}", symbol.name);
            assert_eq!(symbol.designator, "J?");
            assert!(
                symbol
                    .pins
                    .iter()
                    .all(|p| p.electrical == PinDirection::Passive)
            );
        }
    }
    assert!(seen >= 11, "the eleven shipped connectors are present");
}

/// The 14 committed chip footprints (`Chip-*`, plain and `-Polarised`) are
/// what the generator renders: pads, silk, courtyard, fab outline and body
/// height all match, so a generator change that skipped the regenerate
/// fails here. Limited to the chip lands: the rest of the stock library may
/// be edited in the app (`SIGNEX_EDIT_STOCK=1`, #1892).
#[test]
fn committed_footprints_equal_the_generator_output() {
    let rendered = stock_gen::render_all().expect("render");
    let chips: Vec<_> = rendered
        .iter()
        .filter(|f| f.footprint_name.starts_with("Chip-"))
        .collect();
    assert_eq!(chips.len(), 14, "seven sizes, plain and polarised");
    for file in chips {
        let committed = read_lf(&stock_dir().join(&file.file_name));
        assert_eq!(
            committed,
            file.text.replace("\r\n", "\n"),
            "{} differs from the generator output",
            file.file_name
        );
    }
}
