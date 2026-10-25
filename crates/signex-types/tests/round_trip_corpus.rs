//! Fixture-corpus round-trip harness for the native `.snxsch` / `.snxpcb`
//! formats (#334).
//!
//! The files under `tests/corpus/` are checked-in goldens. Per file the
//! harness asserts that it (1) parses and (2) re-serialises to the exact
//! on-disk bytes (the no-edit-save gate). Byte equality already implies a
//! second parse -> write hop is identical, so none is asserted. Every
//! failing fixture is reported in one run.
//!
//! Set `SIGNEX_BLESS_CORPUS=1` to rewrite each fixture from its parsed
//! form instead of asserting; see `tests/corpus/README.md`.

use std::fs;
use std::path::{Path, PathBuf};

use signex_types::format::{SnxPcb, SnxSchematic};

/// Lower bound on the number of discovered fixtures. Raise it when the
/// corpus grows; the guard exists so deleting a fixture can never pass
/// silently.
const MIN_CORPUS_FILES: usize = 77;

const BLESS_ENV: &str = "SIGNEX_BLESS_CORPUS";

#[derive(Clone, Copy, Debug)]
enum Kind {
    Schematic,
    Pcb,
}

fn kind_of(path: &Path) -> Option<Kind> {
    match path.extension()?.to_str()? {
        "snxsch" => Some(Kind::Schematic),
        "snxpcb" => Some(Kind::Pcb),
        _ => None,
    }
}

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// Every `*.snxsch` / `*.snxpcb` under `root`, recursively, sorted so
/// the report order is stable across platforms.
fn discover(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if kind_of(&path).is_some() {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Parse `text` and write it back out.
fn parse_and_write(kind: Kind, text: &str) -> Result<String, String> {
    match kind {
        Kind::Schematic => SnxSchematic::parse(text)
            .map_err(|e| format!("parse failed: {e}"))?
            .write_string()
            .map_err(|e| format!("write failed: {e}")),
        Kind::Pcb => SnxPcb::parse(text)
            .map_err(|e| format!("parse failed: {e}"))?
            .write_string()
            .map_err(|e| format!("write failed: {e}")),
    }
}

/// Describe the first place two texts differ, by 1-based line number.
fn first_difference(expected: &str, actual: &str) -> String {
    let mut want = expected.split('\n');
    let mut got = actual.split('\n');
    let mut line = 1;
    loop {
        match (want.next(), got.next()) {
            (Some(w), Some(g)) if w == g => line += 1,
            (None, None) => return "texts are identical".to_string(),
            (w, g) => {
                return format!(
                    "first difference at line {line}:\n    on disk : {:?}\n    re-saved: {:?}",
                    w.unwrap_or("<end of file>"),
                    g.unwrap_or("<end of file>")
                );
            }
        }
    }
}

/// Run the assertions against one fixture's bytes.
fn check_bytes(kind: Kind, bytes: &[u8]) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("not valid UTF-8: {e}"))?;
    let rewritten = parse_and_write(kind, text)?;
    if rewritten != text {
        return Err(format!(
            "not byte-stable: a no-edit save changes the file; {}",
            first_difference(text, &rewritten)
        ));
    }
    Ok(())
}

fn check_file(path: &Path) -> Result<(), String> {
    let kind = kind_of(path).ok_or("unsupported extension")?;
    let bytes = fs::read(path).map_err(|e| format!("cannot read: {e}"))?;
    check_bytes(kind, &bytes)
}

fn bless_file(path: &Path) -> Result<(), String> {
    let kind = kind_of(path).ok_or("unsupported extension")?;
    let bytes = fs::read(path).map_err(|e| format!("cannot read: {e}"))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| format!("not valid UTF-8: {e}"))?;
    let rewritten = parse_and_write(kind, text)?;
    if rewritten.as_bytes() != bytes.as_slice() {
        fs::write(path, rewritten).map_err(|e| format!("cannot write: {e}"))?;
    }
    Ok(())
}

/// Fail when fewer than `MIN_CORPUS_FILES` fixtures were discovered.
fn check_count(found: usize, root: &Path) -> Result<(), String> {
    if found >= MIN_CORPUS_FILES {
        return Ok(());
    }
    Err(format!(
        "corpus shrank: found {found} fixtures under {}, MIN_CORPUS_FILES is {MIN_CORPUS_FILES}",
        root.display()
    ))
}

fn relative(path: &Path) -> String {
    path.strip_prefix(corpus_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

#[test]
fn corpus_round_trips_byte_for_byte() {
    let root = corpus_root();
    let fixtures = discover(&root).expect("corpus directory must be readable");

    if let Err(why) = check_count(fixtures.len(), &root) {
        panic!("{why}");
    }

    let blessing = std::env::var_os(BLESS_ENV).is_some_and(|v| v == "1");
    let failures: Vec<String> = fixtures
        .iter()
        .filter_map(|path| {
            let outcome = if blessing {
                bless_file(path)
            } else {
                check_file(path)
            };
            outcome
                .err()
                .map(|why| format!("  {}: {why}", relative(path)))
        })
        .collect();

    assert!(
        failures.is_empty(),
        "{} of {} corpus fixtures failed:\n{}\n(an intentional format change: rerun with {BLESS_ENV}=1 and review the diff)",
        failures.len(),
        fixtures.len(),
        failures.join("\n")
    );
}

// ---- positive controls: the harness must actually fail when it should ----

fn seed(rel: &str) -> (Kind, Vec<u8>) {
    let path = corpus_root().join(rel);
    (kind_of(&path).unwrap(), fs::read(&path).unwrap())
}

#[test]
fn a_single_edited_byte_is_reported_with_its_line() {
    let (kind, mut bytes) = seed("schematic/regulator_stage.snxsch");
    // Re-pad one TSV column: still valid, but not what the writer emits.
    let at = bytes
        .windows(3)
        .position(|w| w == b"VIN")
        .expect("seed carries the VIN label");
    bytes.insert(at, b' ');
    let err = check_bytes(kind, &bytes).expect_err("an edited byte must fail the gate");
    assert!(err.contains("not byte-stable"), "{err}");
    assert!(err.contains("first difference at line"), "{err}");
    assert!(err.contains(" VIN"), "{err}");
}

#[test]
fn reordered_map_keys_are_not_byte_stable() {
    let (kind, bytes) = seed("schematic/regulator_stage.snxsch");
    let text = String::from_utf8(bytes).unwrap();
    let company = "company = \"Alp Lab\"
";
    let date = "date = \"2026-07-18\"
";
    let sorted = format!("{company}{date}");
    assert!(text.contains(&sorted), "seed title_block is sorted");
    let swapped = text.replace(&sorted, &format!("{date}{company}"));
    let err = check_bytes(kind, swapped.as_bytes()).expect_err("unsorted keys must fail the gate");
    assert!(err.contains("not byte-stable"), "{err}");
    assert!(err.contains("company"), "{err}");
}

#[test]
fn trailing_whitespace_edit_is_not_byte_stable() {
    let (kind, mut bytes) = seed("pcb/empty_board.snxpcb");
    bytes.push(b'\n');
    let err = check_bytes(kind, &bytes).expect_err("an extra byte must fail the gate");
    assert!(err.contains("not byte-stable"), "{err}");
}

#[test]
fn unparseable_fixture_is_reported_as_a_parse_failure() {
    let err = check_bytes(Kind::Schematic, b"format = [").expect_err("garbage must not pass");
    assert!(err.contains("parse failed"), "{err}");
}

#[test]
fn discovery_recurses_and_ignores_other_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("a/b")).unwrap();
    fs::write(dir.path().join("a/b/x.snxsch"), "").unwrap();
    fs::write(dir.path().join("y.snxpcb"), "").unwrap();
    fs::write(dir.path().join("README.md"), "").unwrap();
    let found = discover(dir.path()).unwrap();
    assert_eq!(found.len(), 2, "{found:?}");
}

#[test]
fn bless_is_a_no_op_on_a_canonical_tree() {
    let dir = tempfile::tempdir().unwrap();
    for rel in [
        "schematic/hostile_label_text.snxsch",
        "pcb/routed_board.snxpcb",
    ] {
        let name = Path::new(rel).file_name().unwrap();
        let copy = dir.path().join(name);
        fs::copy(corpus_root().join(rel), &copy).unwrap();
        let before = fs::read(&copy).unwrap();
        bless_file(&copy).unwrap();
        assert_eq!(before, fs::read(&copy).unwrap(), "{rel}");
    }
}

#[test]
fn deleting_a_fixture_trips_the_count_guard() {
    let found = discover(&corpus_root()).unwrap().len();
    assert!(check_count(found, &corpus_root()).is_ok());
    let err = check_count(found - 1, &corpus_root()).expect_err("one fewer fixture must fail");
    assert!(err.contains("corpus shrank"), "{err}");
}
