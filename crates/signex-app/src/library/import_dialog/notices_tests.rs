//! What the dialog says when a run goes wrong (#3124): no converter for the
//! chosen file, a converter too new for this Signex, a crash with the end
//! of its error output, a fatal exit, and a partial one.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use signex_i18n::{I18n, ids};
use signex_types::theme::{ThemeId, theme_tokens};

use super::notices::{RunProblem, STDERR_LINES, failure_reason, last_lines};
use super::result::ImportResult;
use super::source::Blocker;
use super::tests::{A, converter, dialog, found, import, picked};
use super::{
    FolderAfter, ImportDialogContext, ImportDialogState, ImportEnd, PreviewState, Step, view,
};
use crate::app::Signex;
use crate::library::messages::LibraryImportMsg;
use crate::library_import::report::parse_report;
use crate::library_import::run_state::{ImportFinished, ImportRun, MountTarget};
use crate::library_import::{ExitKind, ImportError, RunFailure, RunMode};

const REPORT: &str =
    include_str!("../../../../../docs/fixtures/library-converter/import-report.json");

/// The app with the dialog waiting for a `mode` run into `/out/P`.
fn waiting(mode: RunMode) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.library.import.running = Some(ImportRun {
        output: "/out/P".into(),
        mode,
        target: MountTarget::Global,
        cancel: Arc::new(AtomicBool::new(false)),
        group: Default::default(),
        progress: None,
    });
    let step = if mode == RunMode::Preview {
        Step::Preview
    } else {
        Step::Import
    };
    app.library.import_dialog = Some(Box::new(ImportDialogState {
        step,
        awaiting: Some(mode),
        preview: (mode == RunMode::Preview).then_some(PreviewState::Running),
        location: "/out/P".into(),
        ..ImportDialogState::new()
    }));
    app
}

fn failed_with(error: ImportError, stderr: &str, removed: bool) -> crate::app::Message {
    let mut failure = RunFailure::new(error);
    failure.stderr_tail = stderr.into();
    failure.removed_output = removed;
    import(LibraryImportMsg::Finished(Box::new(ImportFinished {
        result: Err(failure),
        target: MountTarget::Global,
    })))
}

#[test]
fn a_crash_and_a_fatal_exit_get_their_own_words_and_others_keep_theirs() {
    let crash = ImportError::NoSummary {
        status: "signal: 11 (SIGSEGV)".into(),
        stderr: "boom".into(),
    };
    assert_eq!(
        failure_reason(&crash).id(),
        Some(ids::LIBRARY_IMPORT_DIALOG_CRASHED)
    );
    let fatal = ImportError::ConverterFailed {
        message: "bad record".into(),
    };
    assert_eq!(
        failure_reason(&fatal).id(),
        Some(ids::LIBRARY_IMPORT_DIALOG_FATAL)
    );
    let silent = ImportError::Silent { seconds: 120 };
    assert_eq!(failure_reason(&silent), silent.to_translatable());
}

#[test]
fn the_error_output_shows_its_last_lines_without_blanks() {
    assert_eq!(last_lines("", STDERR_LINES), "");
    assert_eq!(last_lines("a\n\n  \nb  \n", STDERR_LINES), "a\nb");
    let many: String = (0..30).map(|i| format!("line {i}\n")).collect();
    let shown = last_lines(&many, STDERR_LINES);
    assert_eq!(shown.lines().count(), STDERR_LINES);
    assert!(shown.starts_with("line 18") && shown.ends_with("line 29"));
}

#[test]
fn a_crashed_import_says_so_with_the_end_of_its_error_output() {
    let mut app = waiting(RunMode::Import);
    let crash = ImportError::NoSummary {
        status: "signal: 11 (SIGSEGV)".into(),
        stderr: "Segmentation fault".into(),
    };
    drop(app.update(failed_with(crash, "reading R1\nSegmentation fault\n", true)));
    let Some(ImportEnd::Failed {
        reason,
        folder,
        stderr_tail,
    }) = dialog(&app).import_end.clone()
    else {
        panic!("{:?}", dialog(&app).import_end);
    };
    assert_eq!(reason.id(), Some(ids::LIBRARY_IMPORT_DIALOG_CRASHED));
    assert_eq!(folder, FolderAfter::Removed);
    assert_eq!(stderr_tail, "reading R1\nSegmentation fault");
    assert!(app.document_view.error_notice.is_none(), "shown in the dialog");
}

#[test]
fn a_fatal_dry_run_says_so_in_step_two_with_its_error_output() {
    let mut app = waiting(RunMode::Preview);
    let fatal = ImportError::ConverterFailed {
        message: "bad record".into(),
    };
    drop(app.update(failed_with(fatal, "at offset 12\n", true)));
    let Some(PreviewState::Failed(problem)) = &dialog(&app).preview else {
        panic!("{:?}", dialog(&app).preview);
    };
    assert_eq!(problem.reason.id(), Some(ids::LIBRARY_IMPORT_DIALOG_FATAL));
    assert_eq!(problem.stderr_tail, "at offset 12");
}

#[test]
fn a_partial_import_counts_what_it_lost() {
    let with_report = ImportResult::stored("/l/P/P.snxlib".into(), parse_report(REPORT));
    assert_eq!(with_report.lost_items(), 2, "one dropped, one not imported");
    let mut without = ImportResult::stored("/l/P/P.snxlib".into(), Err("none".into()));
    assert_eq!(without.lost_items(), 0);
    without.run = Some(super::result::RunFacts {
        mode: RunMode::Import,
        exit: ExitKind::Partial,
        counts: crate::library_import::events::Counts {
            skipped: 4,
            ..Default::default()
        },
        commit: None,
        local_commits: 0,
        mounted: None,
        required: Vec::new(),
    });
    assert_eq!(without.lost_items(), 4, "the summary's skipped");
}

#[test]
fn a_file_no_installed_converter_reads_keeps_next_off_and_every_notice_builds() {
    let mut unreadable = found(vec![converter("/bin/a", A)]);
    if let Some(discovery) = unreadable.discovery.as_mut() {
        discovery.skipped.push((
            PathBuf::from("/bin/signex-new-import"),
            ImportError::UnsupportedContract {
                converter: "new-import".into(),
                contract: "2.0".into(),
            },
        ));
    }
    picked(&mut unreadable, &["/in/Parts.newlib"]);
    assert_eq!(
        unreadable.blocker(),
        Some(Blocker::Unreadable("/in/Parts.newlib".into()))
    );
    let crashed = ImportDialogState {
        step: Step::Import,
        import_end: Some(ImportEnd::Failed {
            reason: failure_reason(&ImportError::NoSummary {
                status: "signal: 11".into(),
                stderr: String::new(),
            }),
            folder: FolderAfter::Removed,
            stderr_tail: "Segmentation fault".into(),
        }),
        ..ImportDialogState::new()
    };
    let preview_failed = ImportDialogState {
        step: Step::Preview,
        preview: Some(PreviewState::Failed(Box::new(RunProblem {
            reason: ImportError::Silent { seconds: 120 }.to_translatable(),
            stderr_tail: "stuck".into(),
        }))),
        ..ImportDialogState::new()
    };
    let mut partial = ImportDialogState::new();
    let mut result = ImportResult::stored("/out/P/P.snxlib".into(), parse_report(REPORT));
    result.run = Some(super::result::RunFacts {
        mode: RunMode::Import,
        exit: ExitKind::Partial,
        counts: Default::default(),
        commit: Some("abc".into()),
        local_commits: 0,
        mounted: Some(super::result::Mounted::AllProjects),
        required: Vec::new(),
    });
    partial.show_result(result);
    let i18n = I18n::en_us();
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        for state in [&unreadable, &crashed, &preview_failed, &partial] {
            let ctx = ImportDialogContext {
                running: None,
                project_open: false,
                library_open: false,
            };
            drop(view(state, ctx, &tokens, &i18n, true));
        }
    }
}
