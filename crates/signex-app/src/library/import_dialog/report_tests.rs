//! The end of step 3 (#3121): the result card and loss report a finished
//! import ends on, the same report opened from the Library Browser, and
//! Save report…, Open report and the library menu.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use signex_i18n::I18n;
use signex_types::theme::{ThemeId, theme_tokens};

use super::result::{ImportResult, Mounted, ReportNote, RequiredLibrary, ResultTab, RunFacts};
use super::tests::abs;
use super::{ImportDialogContext, ImportDialogMsg, ImportDialogState, Step, view};
use crate::app::{Message, Signex};
use crate::library::messages::{LibraryImportMsg, LibraryMessage};
use crate::library_import::report::{REPORT_JSON, REPORT_MD, Severity, parse_report};
use crate::library_import::run_state::{ImportFinished, ImportRun, MountTarget};
use crate::library_import::{ExitKind, ImportJobOutcome, RunMode, RunOutcome, Summary};
use crate::task_driver::TaskDriver;

const REPORT: &str =
    include_str!("../../../../../docs/fixtures/library-converter/import-report.json");

fn d(m: ImportDialogMsg) -> Message {
    Message::Library(super::msg(m))
}

fn dialog(app: &Signex) -> &ImportDialogState {
    app.library.import_dialog.as_deref().expect("open")
}

fn run_outcome(exit: ExitKind, library: Option<PathBuf>) -> RunOutcome {
    RunOutcome {
        exit,
        summary: Summary {
            ok: true,
            error: None,
            exit,
            library: library.clone(),
            report: None,
            files: Vec::new(),
            counts: crate::library_import::events::Counts {
                parts: 3,
                skipped: 1,
                ..Default::default()
            },
            warnings: Vec::new(),
        },
        library,
        report: Some(parse_report(REPORT).unwrap()),
        report_problem: None,
        stderr_tail: String::new(),
    }
}

fn job_outcome(exit: ExitKind) -> ImportJobOutcome {
    ImportJobOutcome {
        run: run_outcome(exit, Some(PathBuf::from(abs("/out/Passives")))),
        library_file: Some(PathBuf::from(abs("/out/Passives/Passives.snxlib"))),
        commit: Some("0123456789abcdef".into()),
        local_commits: 0,
        library_id: None,
        requires: Vec::new(),
        staged: None,
    }
}

/// The app with the dialog waiting for an import into `/out/Passives`.
fn importing() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.library.import.running = Some(ImportRun {
        output: abs("/out/Passives").into(),
        mode: RunMode::Import,
        target: MountTarget::Global,
        cancel: Arc::new(AtomicBool::new(false)),
        group: Default::default(),
        stage: Default::default(),
        progress: None,
    });
    app.library.import_dialog = Some(Box::new(ImportDialogState {
        step: Step::Import,
        awaiting: Some(RunMode::Import),
        location: abs("/out/Passives"),
        filter: "left over".into(),
        ..ImportDialogState::new()
    }));
    app
}

fn finished(exit: ExitKind) -> Message {
    Message::Library(LibraryMessage::Import(LibraryImportMsg::Finished(
        Box::new(ImportFinished {
            result: Ok(job_outcome(exit)),
            target: MountTarget::Global,
        }),
    )))
}

fn stored(dir: &std::path::Path) -> ImportResult {
    ImportResult::stored(dir.join("Passives.snxlib"), parse_report(REPORT))
}

#[test]
fn a_finished_import_ends_on_its_result_card_and_the_report_is_a_tab_away() {
    let mut app = importing();
    drop(app.update(finished(ExitKind::Partial)));
    let state = dialog(&app);
    assert_eq!(state.step, Step::Import);
    assert!(state.awaiting.is_none());
    assert_eq!(state.result_tab, ResultTab::Result);
    assert!(state.filter.is_empty(), "a fresh filter for the new report");
    let result = state.finished().expect("the result");
    assert_eq!(result.library_dir, PathBuf::from(abs("/out/Passives")));
    assert_eq!(result.library_name(), "Passives");
    assert_eq!(
        result.report_md(),
        PathBuf::from(abs("/out/Passives")).join(REPORT_MD)
    );
    assert_eq!(
        result.run.as_ref().map(|r| (r.mode, r.exit)),
        Some((RunMode::Import, ExitKind::Partial))
    );
    assert_eq!(state.report_items().len(), 3, "the run's report");
    assert!(!state.is_stored_report());
    assert!(
        app.document_view.error_notice.is_none(),
        "the dialog shows the partial result; no notice repeats it"
    );
    drop(app.update(d(ImportDialogMsg::ResultTabPicked(ResultTab::Report))));
    assert_eq!(dialog(&app).result_tab, ResultTab::Report);
    // Back has nothing to go back to; Close closes.
    drop(app.update(d(ImportDialogMsg::BackPressed)));
    assert!(dialog(&app).finished().is_some());
    drop(app.update(d(ImportDialogMsg::Closed)));
    assert!(app.library.import_dialog.is_none());
}

#[test]
fn the_report_is_grouped_by_severity_then_code_and_follows_the_filter() {
    let mut text = parse_report(REPORT).unwrap();
    // A second item under the first item's code.
    let mut twin = text.items[0].clone();
    twin.source = "Passives:R2".into();
    text.items.push(twin);
    let code = text.items[0].code.clone();
    let severity = text.items[0].severity;
    let mut s = ImportDialogState::for_stored_report(ImportResult::stored(
        PathBuf::from("/l/P/P.snxlib"),
        Ok(text),
    ));
    let groups = s.report_groups();
    let total: usize = groups.iter().map(|g| g.count).sum();
    assert_eq!(total, 4);
    let order: Vec<Severity> = groups.iter().map(|g| g.severity).collect();
    let mut sorted = order.clone();
    sorted.sort_by_key(|sev| {
        super::preview::CHIP_SEVERITIES
            .iter()
            .position(|c| c == sev)
            .unwrap_or(usize::MAX)
    });
    assert_eq!(order, sorted, "most severe first");
    let group = groups.iter().find(|g| g.severity == severity).unwrap();
    assert_eq!(group.codes[0].code, code, "the code with most items first");
    assert_eq!(group.codes[0].items.len(), 2);
    // Hiding the severity removes its group; a filter narrows the rows.
    s.toggle_severity(severity);
    assert!(s.report_groups().iter().all(|g| g.severity != severity));
    s.toggle_severity(severity);
    s.filter = "Passives:R2".into();
    let groups = s.report_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].count, 1);
}

#[test]
fn the_library_menu_opens_the_stored_report_in_the_dialog() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(REPORT_JSON), REPORT).unwrap();
    let file = dir.path().join("Passives.snxlib");
    let (mut app, _boot) = Signex::new();
    drop(app.update(d(ImportDialogMsg::LibraryMenuToggled(file.clone()))));
    assert_eq!(app.library.import_menu.as_ref(), Some(&file));
    drop(app.update(d(ImportDialogMsg::LibraryMenuToggled(file.clone()))));
    assert!(
        app.library.import_menu.is_none(),
        "a second right-click closes it"
    );
    drop(app.update(d(ImportDialogMsg::LibraryMenuToggled(file.clone()))));
    let task = app.update(d(ImportDialogMsg::StoredReportRequested(file.clone())));
    assert!(app.library.import_menu.is_none(), "the menu closes");
    TaskDriver::new().settle(&mut app, task).unwrap();
    let state = dialog(&app);
    assert!(state.is_stored_report());
    assert_eq!(state.result_tab, ResultTab::Report);
    assert_eq!(state.report_items().len(), 3);
    assert_eq!(
        super::view::header_note(state, &I18n::en_us())
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], ""),
        "Loss report · Passives"
    );
}

#[test]
fn a_library_without_a_report_says_so_and_an_open_dialog_stays() {
    let (mut app, _boot) = Signex::new();
    drop(app.update(d(ImportDialogMsg::StoredReportRead {
        library_file: "/l/Mine/Mine.snxlib".into(),
        report: Err("not found".into()),
    })));
    let result = dialog(&app).finished().expect("open on the reason");
    assert!(result.report.is_none());
    assert_eq!(result.report_problem.as_deref(), Some("not found"));
    // Another library's report does not replace an open dialog.
    drop(app.update(d(ImportDialogMsg::StoredReportRead {
        library_file: "/l/Other/Other.snxlib".into(),
        report: Ok(Box::new(parse_report(REPORT).unwrap())),
    })));
    assert_eq!(dialog(&app).finished().unwrap().library_name(), "Mine");
}

#[test]
fn save_report_copies_the_readable_report_and_says_where() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(REPORT_MD), "# Loss report\n").unwrap();
    let (mut app, _boot) = Signex::new();
    app.library.import_dialog = Some(Box::new(ImportDialogState::for_stored_report(stored(
        dir.path(),
    ))));
    // A dismissed picker writes nothing.
    drop(app.update(d(ImportDialogMsg::ReportSavePicked(None))));
    assert!(dialog(&app).report_note.is_none());
    let target = dir.path().join("out").join("saved.md");
    std::fs::create_dir(dir.path().join("out")).unwrap();
    let task = app.update(d(ImportDialogMsg::ReportSavePicked(Some(target.clone()))));
    TaskDriver::new().settle(&mut app, task).unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "# Loss report\n");
    assert_eq!(dialog(&app).report_note, Some(ReportNote::Saved(target)));
    // A failed copy says why and leaves the target alone.
    std::fs::remove_file(dir.path().join(REPORT_MD)).unwrap();
    let kept = dir.path().join("out").join("kept.md");
    std::fs::write(&kept, "mine").unwrap();
    let task = app.update(d(ImportDialogMsg::ReportSavePicked(Some(kept.clone()))));
    TaskDriver::new().settle(&mut app, task).unwrap();
    assert!(matches!(
        dialog(&app).report_note,
        Some(ReportNote::Failed(_))
    ));
    assert_eq!(std::fs::read_to_string(&kept).unwrap(), "mine");
}

#[test]
fn a_report_that_does_not_open_says_why_and_the_browser_needs_the_library_open() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _boot) = Signex::new();
    app.library.import_dialog = Some(Box::new(ImportDialogState::for_stored_report(stored(
        dir.path(),
    ))));
    drop(app.update(d(ImportDialogMsg::ReportOpened(Err("no viewer".into())))));
    let Some(ReportNote::Failed(reason)) = &dialog(&app).report_note else {
        panic!("a failed open is shown");
    };
    assert_eq!(
        reason.id(),
        Some(signex_i18n::ids::LIBRARY_IMPORT_DIALOG_REPORT_OPEN_FAILED)
    );
    // The library is not open: Open in Library Browser does nothing.
    drop(app.update(d(ImportDialogMsg::OpenInBrowserPressed)));
    assert!(app.library.import_dialog.is_some());
}

#[test]
fn the_result_and_the_report_build_in_both_graphite_themes() {
    let i18n = I18n::en_us();
    let mut run = ImportDialogState::new();
    run.show_result(ImportResult::of_run(
        RunMode::Update,
        &job_outcome(ExitKind::Partial),
        std::path::Path::new(&abs("/out/Passives")),
    ));
    let mut report = run.clone();
    report.result_tab = ResultTab::Report;
    report.report_note = Some(ReportNote::Saved("/tmp/r.md".into()));
    let mut none_match = report.clone();
    none_match.filter = "no such item".into();
    let unreadable = ImportDialogState::for_stored_report(ImportResult::stored(
        "/l/P/P.snxlib".into(),
        Err("missing".into()),
    ));
    let facts = RunFacts {
        mode: RunMode::Import,
        exit: ExitKind::Ok,
        counts: Default::default(),
        commit: None,
        local_commits: 2,
        mounted: Some(Mounted::Clash("/l/Other/Other.snxlib".into())),
        required: vec![RequiredLibrary {
            id: uuid::Uuid::nil(),
            open_as: None,
        }],
    };
    let mut empty = ImportDialogState::new();
    empty.show_result(ImportResult {
        library_dir: "/out/E".into(),
        library_file: None,
        report: None,
        report_problem: Some("the summary names no report".into()),
        run: Some(facts),
    });
    let stored_report = ImportDialogState::for_stored_report(stored(std::path::Path::new("/l/P")));
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        for state in [
            &run,
            &report,
            &none_match,
            &unreadable,
            &empty,
            &stored_report,
        ] {
            for library_open in [false, true] {
                let ctx = ImportDialogContext {
                    running: None,
                    project_open: true,
                    library_open,
                    look: crate::library::browser::PreviewLook::for_tests(),
                };
                drop(view(state, ctx, &tokens, &i18n, false));
            }
        }
    }
}
