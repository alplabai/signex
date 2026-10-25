//! Round-trip corpus for `.snxprj` project files (#337).
//!
//! `parse_project` patches `dir` to the file's real location, so that one key
//! is never byte-comparable; fixtures store `"dir": ""` and the harness
//! restores that placeholder after a save. Everything else, including key
//! order and unknown keys, must reproduce the fixture exactly.

use std::fs;
use std::path::{Path, PathBuf};

use signex_types::project::{parse_project, write_project};

/// Lower bound on `.snxprj` fixtures so deleting one cannot pass silently.
const MIN_PROJECT_FIXTURES: usize = 5;

fn project_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/project")
}

fn fixtures() -> Vec<PathBuf> {
    let mut found: Vec<_> = fs::read_dir(project_dir())
        .expect("project corpus directory must be readable")
        .map(|e| e.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "snxprj"))
        .collect();
    found.sort();
    found
}

/// Load `path` from a scratch copy, save it back, and return the saved text
/// with the patched `dir` turned back into the fixture's empty placeholder.
fn resave(path: &Path) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let copy = scratch.path().join(path.file_name().unwrap());
    fs::copy(path, &copy).unwrap();
    let data = parse_project(&copy).expect("fixture must parse");
    write_project(&copy, &data).expect("fixture must save");
    let saved = fs::read_to_string(&copy).unwrap();
    let patched = format!("\"dir\": {}", serde_json::to_string(&data.dir).unwrap());
    assert!(
        saved.contains(&patched),
        "saved file carries the patched dir"
    );
    saved.replace(&patched, "\"dir\": \"\"")
}

#[test]
fn project_fixtures_round_trip_byte_for_byte() {
    let found = fixtures();
    assert!(
        found.len() >= MIN_PROJECT_FIXTURES,
        "project corpus shrank: {} < {MIN_PROJECT_FIXTURES}",
        found.len()
    );
    let failures: Vec<String> = found
        .iter()
        .filter_map(|path| {
            let on_disk = fs::read_to_string(path).unwrap();
            (resave(path) != on_disk).then(|| format!("  {}", path.display()))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} project fixtures are not byte-stable:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn unknown_keys_and_format_token_survive_a_save() {
    let saved = resave(&project_dir().join("unknown-keys-preserved.snxprj"));
    for needle in [
        "future_top_level",
        "future_library_key",
        "future_sheet_key",
        "snxprj/1",
    ] {
        assert!(saved.contains(needle), "{needle} dropped on save");
    }
}
