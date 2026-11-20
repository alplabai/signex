//! The Gerber Viewer's read problems reach Messages: a catalog sentence per
//! kind, with the file and the line, at the level of the severity.

use signex_gerber::{ProblemKind, ReadProblem, Severity, SourceLine};
use signex_i18n::I18n;

use super::{TARGET, level_of, problem_text, report_read_problems};
use crate::diagnostics::{DiagnosticLevel, diagnostic_code_from_target, recent_entries};

/// One of every kind. [`kind_number`] has no wildcard, so a new kind does
/// not compile until it is added here and worded in the catalog.
fn every_kind() -> Vec<ProblemKind> {
    vec![
        ProblemKind::UnknownApertureMacro {
            name: "THERMAL80".into(),
            flashes: 2,
        },
        ProblemKind::BadMacroParameters {
            name: "RECT2".into(),
        },
        ProblemKind::UndefinedAperture {
            d_code: 21,
            uses: 3,
        },
        ProblemKind::BadApertureDefinition { d_code: 22 },
        ProblemKind::MissingFormat,
        ProblemKind::MissingUnits,
        ProblemKind::BadCoordinate,
        ProblemKind::UnsupportedCommand {
            command: "G54".into(),
        },
        ProblemKind::UnclosedRegion,
        ProblemKind::MissingEndOfFile,
        ProblemKind::ToolWithoutSize { tool: 2, holes: 2 },
        ProblemKind::UndefinedTool { tool: 7, holes: 1 },
        ProblemKind::BadExcellonHeader {
            text: "T02C".into(),
        },
        ProblemKind::NoFileFunction,
        ProblemKind::NotText,
        ProblemKind::NotAFabricationFile,
        ProblemKind::BadJobFile {
            why: "expected value".into(),
        },
    ]
}

fn kind_number(kind: &ProblemKind) -> usize {
    match kind {
        ProblemKind::UnknownApertureMacro { .. } => 0,
        ProblemKind::BadMacroParameters { .. } => 1,
        ProblemKind::UndefinedAperture { .. } => 2,
        ProblemKind::BadApertureDefinition { .. } => 3,
        ProblemKind::MissingFormat => 4,
        ProblemKind::MissingUnits => 5,
        ProblemKind::BadCoordinate => 6,
        ProblemKind::UnsupportedCommand { .. } => 7,
        ProblemKind::UnclosedRegion => 8,
        ProblemKind::MissingEndOfFile => 9,
        ProblemKind::ToolWithoutSize { .. } => 10,
        ProblemKind::UndefinedTool { .. } => 11,
        ProblemKind::BadExcellonHeader { .. } => 12,
        ProblemKind::NoFileFunction => 13,
        ProblemKind::NotText => 14,
        ProblemKind::NotAFabricationFile => 15,
        ProblemKind::BadJobFile { .. } => 16,
    }
}

const KINDS: usize = 17;

fn problem(file: &str, line: u32, kind: ProblemKind) -> ReadProblem {
    ReadProblem {
        at: SourceLine {
            file: file.to_string(),
            line,
        },
        kind,
        text: String::new(),
    }
}

fn plain(text: String) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn the_sample_list_holds_every_kind_once() {
    let mut numbers: Vec<usize> = every_kind().iter().map(kind_number).collect();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers, (0..KINDS).collect::<Vec<_>>());
}

#[test]
fn every_kind_has_its_own_catalog_sentence_with_the_file_and_line() {
    let i18n = I18n::en_us();
    let mut sentences = Vec::new();
    for kind in every_kind() {
        let text = plain(
            problem_text(&problem("dcdc-buck-L2.gbr", 1234, kind.clone()), &i18n).into_string(),
        );
        assert!(
            text.starts_with("Gerber Viewer: dcdc-buck-L2.gbr, line 1234: "),
            "{kind:?}: {text}"
        );
        assert!(
            !text.contains("gerber-viewer-read"),
            "{kind:?} reads its id: {text}"
        );
        sentences.push(text);
    }
    let mut unique = sentences.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        sentences.len(),
        "each kind has its own sentence"
    );
}

#[test]
fn a_sentence_carries_the_kind_data() {
    let i18n = I18n::en_us();
    let text =
        |kind: ProblemKind| plain(problem_text(&problem("f.gbr", 25, kind), &i18n).into_string());
    assert_eq!(
        text(ProblemKind::UnknownApertureMacro {
            name: "THERMAL80".into(),
            flashes: 2
        }),
        "Gerber Viewer: f.gbr, line 25: aperture macro THERMAL80 is not defined; \
         its 2 flashes are drawn as markers."
    );
    assert_eq!(
        text(ProblemKind::ToolWithoutSize { tool: 2, holes: 1 }),
        "Gerber Viewer: f.gbr, line 25: tool T02 has no size; its hole is drawn as a cross."
    );
    assert!(
        text(ProblemKind::UndefinedAperture {
            d_code: 1021,
            uses: 1
        })
        .contains("aperture D1021 ")
    );
}

#[test]
fn levels_follow_the_severity() {
    assert_eq!(level_of(Severity::Error), log::Level::Error);
    assert_eq!(level_of(Severity::Warning), log::Level::Warn);
    assert_eq!(level_of(Severity::Info), log::Level::Info);
}

#[test]
fn each_problem_is_one_messages_row_at_its_level_under_the_viewer_source() {
    // Installs the logger first.
    let said = crate::diagnostics::said_on_this_thread();
    if crate::diagnostics::configured_level() < log::LevelFilter::Info {
        eprintln!("skipped: SIGNEX_LOG/RUST_LOG filters out INFO records");
        return;
    }
    let i18n = I18n::en_us();
    let error = problem("gerber-read-row-error.gbr", 25, ProblemKind::BadCoordinate);
    let warning = problem(
        "gerber-read-row-warning.gbr",
        1,
        ProblemKind::NoFileFunction,
    );
    let info = problem(
        "gerber-read-row-info.txt",
        1,
        ProblemKind::NotAFabricationFile,
    );
    report_read_problems(&[&error, &warning, &info], &i18n);
    assert_eq!(
        crate::diagnostics::said_on_this_thread() - said,
        3,
        "one row per problem"
    );
    let code = diagnostic_code_from_target(TARGET);
    assert_eq!(code, "SIGNEX-GERBER_VIEWER");
    let entries = recent_entries();
    for (file, level) in [
        ("gerber-read-row-error.gbr", DiagnosticLevel::Error),
        ("gerber-read-row-warning.gbr", DiagnosticLevel::Warning),
        ("gerber-read-row-info.txt", DiagnosticLevel::Info),
    ] {
        let rows: Vec<_> = entries
            .iter()
            .filter(|e| e.message.contains(file))
            .collect();
        assert_eq!(rows.len(), 1, "{file}: {rows:#?}");
        assert_eq!(rows[0].level, level, "{file}");
        assert_eq!(rows[0].code, code, "{file}");
        assert!(plain(rows[0].message.clone()).contains(", line "), "{file}");
    }
}
