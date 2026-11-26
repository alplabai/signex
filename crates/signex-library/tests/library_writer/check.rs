//! `check_library` (#3111): clean on the fixtures, the stock library and the
//! writer's output; one failing library per problem class.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use signex_library::alternate_symbol::AlternateSymbol;
use signex_library::check::{Problem, Severity, check_library, has_errors};
use signex_library::primitive::{
    Footprint, PrimitiveRef, SimFile, SimKind, SimModel, StepAttachment,
};
use signex_library::writer::LibraryWriter;

use crate::synthetic::{self, LIBRARY_ID, row_id};

const SNXLIB: &str = "Synthetic.snxlib";

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn codes(problems: &[Problem], severity: Severity) -> Vec<&'static str> {
    let mut codes: Vec<_> = problems
        .iter()
        .filter(|p| p.severity == severity)
        .map(|p| p.code)
        .collect();
    codes.dedup();
    codes
}

/// A clean synthetic library of 40 rows (symbols 0, footprints 0-3).
fn written(dir: &Path) -> PathBuf {
    let root = dir.join("Synthetic");
    synthetic::library(40).write(&root).unwrap();
    root
}

fn edit(path: &Path, f: impl FnOnce(String) -> String) {
    let text = std::fs::read_to_string(path).unwrap();
    std::fs::write(path, f(text)).unwrap();
}

/// The line of row `n` in the `.snxlib`.
fn row_line(root: &Path, n: usize) -> String {
    let text = std::fs::read_to_string(root.join(SNXLIB)).unwrap();
    let id = row_id(n).to_string();
    text.lines()
        .find(|l| l.starts_with(&id))
        .unwrap()
        .to_string()
}

fn replace_row_line(root: &Path, n: usize, f: impl FnOnce(&str) -> String) {
    let old = row_line(root, n);
    let new = f(&old);
    edit(&root.join(SNXLIB), |t| t.replace(&old, &new));
}

fn footprint_path(root: &Path, n: usize) -> PathBuf {
    let uuid = synthetic::footprint(n).footprints[0].uuid;
    root.join("footprints").join(format!("{uuid}.snxfpt"))
}

/// Assert `code` is reported, as an error unless `warning`.
fn assert_reports(problems: &[Problem], code: &str, severity: Severity) {
    assert!(
        problems
            .iter()
            .any(|p| p.code == code && p.severity == severity),
        "expected {severity:?} {code}, got {problems:#?}"
    );
}

#[test]
fn the_conformance_fixtures_pass() {
    let problems = check_library(&repo("docs/fixtures/library-format/library"), &[]);
    assert!(!has_errors(&problems), "{problems:#?}");
    // The fixture pins a STEP hash without shipping the model.
    assert_eq!(codes(&problems, Severity::Warning), ["model.missing"]);
}

/// The stock footprints carry `snxfpt/1`, read but no longer written
/// (format open point 8): reported as warnings, not errors.
#[test]
fn the_stock_library_passes_with_legacy_token_warnings() {
    let problems = check_library(&repo("assets/stock-library"), &[]);
    assert!(!has_errors(&problems), "{problems:#?}");
    assert_eq!(
        codes(&problems, Severity::Warning),
        ["footprint.legacy_token"]
    );
}

#[test]
fn the_writers_output_has_no_problem() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(check_library(&written(dir.path()), &[]), []);
}

#[test]
fn library_level_failures() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    assert_reports(
        &check_library(&empty, &[]),
        "library.not_found",
        Severity::Error,
    );

    let root = written(dir.path());
    std::fs::copy(root.join(SNXLIB), root.join("Second.snxlib")).unwrap();
    assert_reports(
        &check_library(&root, &[]),
        "library.ambiguous",
        Severity::Error,
    );
    std::fs::remove_file(root.join("Second.snxlib")).unwrap();

    edit(&root.join(SNXLIB), |t| format!("{t}\n"));
    assert_reports(
        &check_library(&root, &[]),
        "snxlib.not_canonical",
        Severity::Error,
    );
    edit(&root.join(SNXLIB), |t| {
        t.replace("library_id", "library_ix")
    });
    assert_reports(
        &check_library(&root, &[]),
        "library.unreadable",
        Severity::Error,
    );
}

#[test]
fn row_failures() {
    let dir = tempfile::tempdir().unwrap();
    let root = written(dir.path());
    replace_row_line(&root, 1, |l| {
        l.replace("Synthetic part 1", "Edited by hand")
    });
    replace_row_line(&root, 2, |l| l.replace("PN-000002", "PN-000003"));
    let problems = check_library(&root, &[]);
    assert_reports(&problems, "row.content_hash", Severity::Warning);
    assert_reports(&problems, "row.duplicate_internal_pn", Severity::Error);

    // Row 5 copied into another table.
    let line = row_line(&root, 5);
    edit(&root.join(SNXLIB), |t| {
        t.replacen(
            &row_line(&root, 6),
            &format!("{line}\n{}", row_line(&root, 6)),
            1,
        )
    });
    assert_reports(
        &check_library(&root, &[]),
        "row.duplicate_row_id",
        Severity::Error,
    );

    replace_row_line(&root, 7, |l| l.replacen("\tReleased\t", "\tBogus\t", 1));
    assert_reports(
        &check_library(&root, &[]),
        "table.unreadable",
        Severity::Error,
    );
}

/// A row the reader takes but a save refuses: an alternate symbol labelled
/// with the reserved `Default`.
#[test]
fn a_row_a_save_refuses_is_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    let mut w = synthetic::library(40);
    // A valid alternate (to a symbol the library lacks), whose label is
    // then made the reserved `Default` by hand.
    let mut row = synthetic::row(40, 1, 4);
    row.alternate_symbols = vec![AlternateSymbol::new(
        PrimitiveRef::new(LIBRARY_ID, Uuid::from_u128(7)),
        "1.0.0",
        "Alternate",
    )];
    w.add_row("ics", row);
    w.write(&root).unwrap();
    replace_row_line(&root, 40, |l| {
        l.replace("\"label\":\"Alternate\"", "\"label\":\"Default\"")
    });
    let problems = check_library(&root, &[]);
    assert_reports(&problems, "row.invalid", Severity::Error);
    assert_reports(&problems, "ref.missing_symbol", Severity::Error);
}

#[test]
fn reference_failures() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    let mut w = synthetic::library(40);
    let mut sim = synthetic::row(40, 1, 4);
    sim.sim_ref = Some(PrimitiveRef::new(LIBRARY_ID, Uuid::from_u128(9)));
    w.add_row("ics", sim);
    let mut foreign = synthetic::row(41, 1, 4);
    foreign.symbol_ref = PrimitiveRef::new(Uuid::from_u128(77), Uuid::from_u128(1));
    w.add_row("ics", foreign);
    w.write(&root).unwrap();
    std::fs::remove_file(footprint_path(&root, 3)).unwrap();
    let problems = check_library(&root, &[]);
    assert_reports(&problems, "ref.missing_sim", Severity::Error);
    assert_reports(&problems, "ref.missing_footprint", Severity::Error);
    assert_reports(&problems, "ref.unknown_library", Severity::Error);

    for entry in std::fs::read_dir(root.join("symbols")).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    assert_reports(
        &check_library(&root, &[]),
        "ref.missing_symbol",
        Severity::Error,
    );
}

/// A parts library whose symbols live in a second library.
#[test]
fn references_into_with_and_required_libraries() {
    let dir = tempfile::tempdir().unwrap();
    let symbols_id = Uuid::from_u128(0x5);
    let mut manifest = synthetic::manifest("Symbols");
    manifest.library_id = symbols_id;
    let mut lib = LibraryWriter::new("Symbols", manifest);
    lib.add_symbol_file(synthetic::envelope(0, vec![synthetic::symbol(0)]));
    let symbols_dir = dir.path().join("Symbols");
    lib.write(&symbols_dir).unwrap();

    let parts = |requires: Vec<String>| {
        let root = dir.path().join(format!("Parts{}", requires.len()));
        let mut w = synthetic::library(10);
        w.manifest_mut().library.requires = requires;
        let mut row = synthetic::row(10, 1, 1);
        row.symbol_ref = PrimitiveRef::new(symbols_id, synthetic::symbol(0).uuid);
        w.add_row("ics", row);
        w.write(&root).unwrap();
        root
    };
    let unlisted = parts(Vec::new());
    let with = [symbols_dir.clone()];
    let problems = check_library(&unlisted, &with);
    assert!(!has_errors(&problems), "{problems:#?}");
    assert_eq!(codes(&problems, Severity::Warning), ["ref.not_required"]);
    assert_reports(
        &check_library(&unlisted, &[]),
        "ref.unknown_library",
        Severity::Error,
    );

    let listed = parts(vec![symbols_id.to_string()]);
    assert_eq!(check_library(&listed, &with), []);
    let problems = check_library(&listed, &[]);
    assert_reports(&problems, "requires.missing", Severity::Error);
    assert_eq!(
        problems.len(),
        1,
        "references into it are not reported again: {problems:#?}"
    );

    let malformed = parts(vec!["a".into(), "b".into()]);
    assert_reports(
        &check_library(&malformed, &with),
        "manifest.requires_malformed",
        Severity::Error,
    );

    let twin = dir.path().join("Twin");
    synthetic::library(3).write(&twin).unwrap();
    assert_reports(
        &check_library(&listed, &[twin]),
        "library.duplicate_id",
        Severity::Error,
    );
    assert_reports(
        &check_library(&listed, &[dir.path().join("nowhere")]),
        "with.unreadable",
        Severity::Error,
    );
}

#[test]
fn primitive_file_failures() {
    let dir = tempfile::tempdir().unwrap();
    let root = written(dir.path());
    let symbols: Vec<PathBuf> = std::fs::read_dir(root.join("symbols"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    std::fs::copy(&symbols[0], root.join("symbols/copy.snxsym")).unwrap();
    assert_reports(
        &check_library(&root, &[]),
        "symbol.duplicate_uuid",
        Severity::Error,
    );
    std::fs::write(root.join("symbols/copy.snxsym"), "not toml [").unwrap();
    assert_reports(
        &check_library(&root, &[]),
        "symbol.unreadable",
        Severity::Error,
    );
    std::fs::remove_file(root.join("symbols/copy.snxsym")).unwrap();
    edit(&symbols[0], |t| {
        t.replace("version = \"1.0.0\"", "version = '1.0.0'")
    });
    assert_reports(
        &check_library(&root, &[]),
        "symbol.not_canonical",
        Severity::Error,
    );

    edit(&footprint_path(&root, 0), |t| {
        t.replace("snxfpt/1.1", "snxfpt/1")
    });
    assert_reports(
        &check_library(&root, &[]),
        "footprint.legacy_token",
        Severity::Warning,
    );
    edit(&footprint_path(&root, 0), |t| {
        t.replace("snxfpt/1\"", "snxfpt/9\"")
    });
    assert_reports(
        &check_library(&root, &[]),
        "format.unsupported",
        Severity::Error,
    );
    edit(&footprint_path(&root, 0), |t| {
        t.replace("snxfpt/9\"", "snxfpt/1.1\"")
    });
    edit(&footprint_path(&root, 1), |t| {
        t.replace("name = \"FP-1\"", "name = 'FP-1'")
    });
    assert_reports(
        &check_library(&root, &[]),
        "footprint.not_canonical",
        Severity::Error,
    );

    std::fs::rename(
        footprint_path(&root, 2),
        root.join(format!("footprints/{}.snxfpt", Uuid::from_u128(3))),
    )
    .unwrap();
    assert_reports(
        &check_library(&root, &[]),
        "footprint.misnamed",
        Severity::Error,
    );

    let mut two = synthetic::footprint(3);
    two.footprints.push(Footprint::with_identity(
        "extra",
        Uuid::from_u128(4),
        synthetic::time(),
    ));
    std::fs::write(footprint_path(&root, 3), two.to_toml_string().unwrap()).unwrap();
    assert_reports(
        &check_library(&root, &[]),
        "footprint.unreachable",
        Severity::Error,
    );

    let model = SimModel {
        uuid: Uuid::from_u128(5),
        ..SimModel::empty("model", SimKind::Ngspice)
    };
    let sims = root.join("sims");
    std::fs::create_dir(&sims).unwrap();
    let text = SimFile::from_model(model).to_toml_string().unwrap();
    std::fs::write(sims.join(format!("{}.snxsim", Uuid::from_u128(6))), text).unwrap();
    std::fs::write(sims.join(format!("{}.snxsim", Uuid::from_u128(7))), "x = [").unwrap();
    let problems = check_library(&root, &[]);
    assert_reports(&problems, "sim.misnamed", Severity::Error);
    assert_reports(&problems, "sim.unreadable", Severity::Error);
}

#[test]
fn step_model_failures() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Synthetic");
    let mut w = synthetic::library(40);
    let hash = w.add_step_model(b"ISO-10303-21;\nEND-ISO-10303-21;\n".to_vec());
    for (n, content_hash) in [(40, hash.clone()), (41, "NOT-A-HASH".to_string())] {
        let mut file = synthetic::footprint(n);
        file.footprints[0].step_attachment = Some(StepAttachment {
            content_hash,
            filename: "part.step".into(),
            offset_xyz: [0.0; 3],
            rotation_xyz: [0.0; 3],
        });
        w.add_footprint_file(file).unwrap();
    }
    w.write(&root).unwrap();
    let problems = check_library(&root, &[]);
    assert_reports(&problems, "model.bad_hash", Severity::Error);
    assert!(!problems.iter().any(|p| p.code == "model.missing"));

    let step = root.join(format!("step/{hash}.step"));
    std::fs::write(&step, b"changed").unwrap();
    assert_reports(
        &check_library(&root, &[]),
        "model.hash_mismatch",
        Severity::Error,
    );
    std::fs::remove_file(&step).unwrap();
    assert_reports(
        &check_library(&root, &[]),
        "model.missing",
        Severity::Warning,
    );
}
