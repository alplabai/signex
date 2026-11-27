//! The Import Library dialog's state machine (mockup row 211): what step 1
//! detects and refuses, the preview's filters, the Cancel rule, and the
//! steps' transitions through the app's `update`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use signex_i18n::I18n;
use signex_types::theme::{ThemeId, theme_tokens};

use super::source::Blocker;
use super::{
    ImportDialogContext, ImportDialogMsg, ImportDialogState, ImportEnd, PreviewState, SourceKind,
    Step, view,
};
use crate::app::{Message, Signex};
use crate::library::messages::{LibraryImportMsg, LibraryMessage};
use crate::library_import::describe::parse_describe;
use crate::library_import::report::{Severity, parse_report};
use crate::library_import::run_state::{ImportFinished, ImportRun, MountTarget};
use crate::library_import::{
    ConverterCommand, DiscoveredConverter, Discovery, ImportError, ModelsMode, ProgressEvent,
    RunFailure, RunMode, RunOutcome, Summary,
};

const A: &str = r#"{"contract":"1.0","converter":"a-import","version":"1.0","inputs":[
    {"label":"Integrated library","extensions":["lib1"],"kind":"library"},
    {"label":"Database library","extensions":["dblink"],"kind":"database"}]}"#;
const B: &str = r#"{"contract":"1.0","converter":"b-import","version":"2.0","inputs":[
    {"label":"Integrated library","extensions":["lib1"],"kind":"library"},
    {"label":"Footprint library","extensions":["fp1"],"kind":"library"}]}"#;
const REPORT: &str =
    include_str!("../../../../../docs/fixtures/library-converter/import-report.json");
const ROOT: &str = "/home/u/Signex/Libraries";

fn converter(path: &str, json: &str) -> DiscoveredConverter {
    DiscoveredConverter {
        path: PathBuf::from(path),
        command: ConverterCommand::new(PathBuf::from(path)),
        description: parse_describe(json).unwrap(),
    }
}

fn found(converters: Vec<DiscoveredConverter>) -> ImportDialogState {
    ImportDialogState {
        discovery: Some(Discovery {
            converters,
            skipped: Vec::new(),
        }),
        ..ImportDialogState::new()
    }
}

fn picked(state: &mut ImportDialogState, paths: &[&str]) {
    let paths = paths.iter().map(PathBuf::from).collect();
    state.apply_sources(paths, Some(Path::new(ROOT)));
}

fn d(m: ImportDialogMsg) -> Message {
    Message::Library(super::msg(m))
}

fn import(m: LibraryImportMsg) -> Message {
    Message::Library(LibraryMessage::Import(m))
}

fn dialog(app: &Signex) -> &ImportDialogState {
    app.library.import_dialog.as_deref().expect("open")
}

fn outcome() -> RunOutcome {
    RunOutcome {
        exit: crate::library_import::ExitKind::Partial,
        summary: Summary {
            ok: true,
            error: None,
            exit: crate::library_import::ExitKind::Partial,
            library: None,
            report: None,
            files: Vec::new(),
            counts: crate::library_import::events::Counts {
                parts: 3,
                skipped: 1,
                ..Default::default()
            },
            warnings: Vec::new(),
        },
        library: None,
        report: Some(parse_report(REPORT).unwrap()),
        report_problem: None,
        stderr_tail: String::new(),
    }
}

#[test]
fn the_menu_command_opens_the_dialog_on_step_one_looking_for_converters() {
    let (mut app, _boot) = Signex::new();
    drop(app.update(super::requested_message()));
    let state = dialog(&app);
    assert_eq!(state.step, Step::Source);
    assert_eq!(state.blocker(), Some(Blocker::Looking));
    assert!(state.mount_for_all && state.include_models, "both on");
}

#[test]
fn with_no_converter_next_stays_off_and_the_notice_shows() {
    let (mut app, _boot) = Signex::new();
    drop(app.update(super::requested_message()));
    drop(app.update(d(ImportDialogMsg::DiscoveryFinished(None))));
    assert_eq!(dialog(&app).blocker(), Some(Blocker::NotInstalled));
    // Look again searches anew.
    drop(app.update(d(ImportDialogMsg::RescanPressed)));
    assert_eq!(dialog(&app).blocker(), Some(Blocker::Looking));
}

#[test]
fn a_pick_detects_its_kind_and_the_name_and_folder_follow() {
    let mut s = found(vec![converter("/bin/a", A)]);
    assert_eq!(s.blocker(), Some(Blocker::NoSource));
    picked(&mut s, &["/in/Passives.LIB1"]);
    assert_eq!(s.detected_kinds()[0].label, "Integrated library");
    assert_eq!(
        s.chosen_converter().unwrap().description.converter,
        "a-import"
    );
    assert_eq!(s.name, "Passives");
    assert_eq!(s.location, format!("{ROOT}/Passives"));
    assert_eq!(s.blocker(), None);
    // The folder follows the name until it is changed by hand.
    s.set_name("Caps".into(), Some(Path::new(ROOT)));
    assert_eq!(s.location, format!("{ROOT}/Caps"));
    s.set_location("/work/libs/caps".into());
    s.set_name("Other".into(), Some(Path::new(ROOT)));
    assert_eq!(s.location, "/work/libs/caps");
    // A typed name is kept by the next pick.
    s.name_edited = true;
    picked(&mut s, &["/in/Resistors.lib1"]);
    assert_eq!(s.name, "Other");
}

#[test]
fn several_converters_offer_a_choice_and_the_pick_is_kept() {
    let mut s = found(vec![converter("/bin/a", A), converter("/bin/b", B)]);
    picked(&mut s, &["/in/P.lib1"]);
    assert_eq!(s.readers().len(), 2);
    assert_eq!(s.chosen_converter().unwrap().path, Path::new("/bin/a"));
    s.converter = Some("/bin/b".into());
    assert_eq!(s.chosen_converter().unwrap().path, Path::new("/bin/b"));
    // Several files go to the one converter that reads them all.
    picked(&mut s, &["/in/P.lib1", "/in/F.fp1"]);
    assert_eq!(s.readers().len(), 1);
    assert_eq!(s.chosen_converter().unwrap().path, Path::new("/bin/b"));
    assert_eq!(s.detected_kinds().len(), 2);
}

#[test]
fn an_unreadable_or_mixed_pick_and_a_bad_name_or_folder_keep_next_off() {
    let mut s = found(vec![converter("/bin/a", A), converter("/bin/b", B)]);
    picked(&mut s, &["/in/notes.txt"]);
    assert_eq!(
        s.blocker(),
        Some(Blocker::Unreadable("/in/notes.txt".into()))
    );
    picked(&mut s, &["/in/x.dblink", "/in/y.fp1"]);
    assert_eq!(s.blocker(), Some(Blocker::NoCommonConverter));
    picked(&mut s, &["/in/P.lib1"]);
    s.name_edited = true;
    s.set_name("a/b".into(), Some(Path::new(ROOT)));
    assert_eq!(s.blocker(), Some(Blocker::InvalidName));
    s.set_name("Fine".into(), Some(Path::new(ROOT)));
    s.set_location("relative/dir".into());
    assert_eq!(s.blocker(), Some(Blocker::LocationNotAbsolute));
    s.set_location(String::new());
    assert_eq!(s.blocker(), Some(Blocker::NoLocation));
}

#[test]
fn the_picker_offers_files_or_one_database_link_from_every_converter() {
    let mut s = found(vec![converter("/bin/a", A), converter("/bin/b", B)]);
    assert!(s.offers(SourceKind::Files) && s.offers(SourceKind::Database));
    let files = s.picker_filters();
    assert_eq!(files.len(), 2, "the shared label is listed once: {files:?}");
    assert_eq!(files[1].extensions, ["fp1"]);
    s.source_kind = SourceKind::Database;
    assert_eq!(s.picker_filters()[0].extensions, ["dblink"]);
}

#[test]
fn a_database_link_gets_the_override_and_the_options_reach_the_request() {
    let mut s = found(vec![converter("/bin/a", A)]);
    picked(&mut s, &["/in/Company.dblink"]);
    assert!(s.has_database_input());
    s.database = "  /data/parts.db ".into();
    s.include_models = false;
    let request = s
        .request(RunMode::Preview, PathBuf::from("/tmp/p"))
        .unwrap();
    assert_eq!(request.db.as_deref(), Some("/data/parts.db"));
    assert_eq!(request.models, ModelsMode::Skip);
    assert_eq!(request.name.as_deref(), Some("Company"));
    // A library file never passes a database.
    picked(&mut s, &["/in/P.lib1"]);
    assert!(!s.has_database_input());
    let request = s.request(RunMode::Import, PathBuf::from("/o")).unwrap();
    assert_eq!(request.db, None);
}

#[test]
fn the_preview_list_filters_by_severity_and_text() {
    let mut s = ImportDialogState {
        preview: Some(PreviewState::Ready(Box::new(outcome()))),
        ..ImportDialogState::new()
    };
    assert_eq!(s.parts_to_import(), 3);
    assert_eq!(s.visible_items().len(), 3);
    let counts = s.severity_counts();
    assert!(counts.contains(&(Severity::Error, 1)), "{counts:?}");
    s.toggle_severity(Severity::Error);
    assert!(
        s.visible_items()
            .iter()
            .all(|(_, i)| i.severity != Severity::Error)
    );
    s.toggle_severity(Severity::Error);
    s.filter = "SOIC".into();
    let shown = s.visible_items();
    assert!(!shown.is_empty() && shown.len() < 3, "{shown:?}");
    s.selected_item = Some(shown[0].0);
    assert!(s.selected_report_item().is_some());
    s.filter = "no such text".into();
    assert!(s.selected_report_item().is_none(), "hidden by the filter");
}

/// The app with the dialog on step 3 and a run in flight on item "A".
fn importing() -> (Signex, Arc<AtomicBool>) {
    let (mut app, _boot) = Signex::new();
    let cancel = Arc::new(AtomicBool::new(false));
    app.library.import.running = Some(ImportRun {
        output: "/out/P".into(),
        mode: RunMode::Import,
        target: MountTarget::Global,
        cancel: cancel.clone(),
        group: Default::default(),
        progress: Some(ProgressEvent::Progress {
            done: 1,
            total: Some(3),
            item: Some("A".into()),
        }),
    });
    app.library.import_dialog = Some(Box::new(ImportDialogState {
        step: Step::Import,
        awaiting: Some(RunMode::Import),
        location: "/out/P".into(),
        ..ImportDialogState::new()
    }));
    (app, cancel)
}

fn cancelled(removed: bool) -> Message {
    let mut failure = RunFailure::new(ImportError::Cancelled);
    failure.removed_output = removed;
    import(LibraryImportMsg::Finished(Box::new(ImportFinished {
        result: Err(failure),
        target: MountTarget::Global,
    })))
}

#[test]
fn cancel_stops_after_the_current_item_and_says_what_was_removed() {
    let (mut app, cancel) = importing();
    drop(app.update(d(ImportDialogMsg::CancelPressed)));
    assert!(dialog(&app).cancel_pending);
    assert!(!cancel.load(Ordering::Relaxed), "item A is still in flight");
    // A repeat of the same event is a heartbeat, not the next item.
    let same = ProgressEvent::Progress {
        done: 1,
        total: Some(3),
        item: Some("A".into()),
    };
    drop(app.update(import(LibraryImportMsg::ProgressReported(same))));
    assert!(!cancel.load(Ordering::Relaxed));
    let next = ProgressEvent::Progress {
        done: 2,
        total: Some(3),
        item: Some("B".into()),
    };
    drop(app.update(import(LibraryImportMsg::ProgressReported(next))));
    assert!(cancel.load(Ordering::Relaxed), "A is done: the run stops");
    drop(app.update(cancelled(true)));
    let state = dialog(&app);
    assert_eq!(
        state.import_end,
        Some(ImportEnd::Cancelled { removed: true })
    );
    assert!(
        app.document_view.error_notice.is_none(),
        "no notice repeats it"
    );
}

#[test]
fn cancel_with_no_item_in_flight_stops_at_once_and_esc_cancels_too() {
    let (mut app, cancel) = importing();
    app.library.import.running.as_mut().unwrap().progress = Some(ProgressEvent::Phase {
        phase: "write".into(),
        total: Some(3),
    });
    // Esc (Closed) while importing cancels instead of closing.
    drop(app.update(d(ImportDialogMsg::Closed)));
    assert!(cancel.load(Ordering::Relaxed));
    assert!(
        app.library.import_dialog.is_some(),
        "stays until it stopped"
    );
    drop(app.update(cancelled(true)));
    drop(app.update(d(ImportDialogMsg::Closed)));
    assert!(app.library.import_dialog.is_none());
}

#[test]
fn a_dry_run_result_or_failure_lands_in_step_two_and_back_returns_to_step_one() {
    let (mut app, _boot) = Signex::new();
    let preview_dir = std::env::temp_dir().join("not-a-preview-dir");
    app.library.import.running = Some(ImportRun {
        output: preview_dir.clone(),
        mode: RunMode::Preview,
        target: MountTarget::Global,
        cancel: Arc::new(AtomicBool::new(false)),
        group: Default::default(),
        progress: None,
    });
    app.library.import_dialog = Some(Box::new(ImportDialogState {
        step: Step::Preview,
        awaiting: Some(RunMode::Preview),
        preview: Some(PreviewState::Running),
        ..ImportDialogState::new()
    }));
    let failed = RunFailure::new(ImportError::ConverterFailed {
        message: "bad input".into(),
    });
    drop(app.update(import(LibraryImportMsg::Finished(Box::new(
        ImportFinished {
            result: Err(failed),
            target: MountTarget::Global,
        },
    )))));
    assert!(matches!(
        dialog(&app).preview,
        Some(PreviewState::Failed(_))
    ));
    assert!(
        app.document_view.error_notice.is_none(),
        "shown in the dialog"
    );
    drop(app.update(d(ImportDialogMsg::BackPressed)));
    let state = dialog(&app);
    assert_eq!(state.step, Step::Source);
    assert!(state.preview.is_none());
}

#[test]
fn back_during_a_dry_run_cancels_it_and_its_result_is_ignored() {
    let (mut app, _boot) = Signex::new();
    let cancel = Arc::new(AtomicBool::new(false));
    app.library.import.running = Some(ImportRun {
        output: "/tmp/x".into(),
        mode: RunMode::Preview,
        target: MountTarget::Global,
        cancel: cancel.clone(),
        group: Default::default(),
        progress: None,
    });
    app.library.import_dialog = Some(Box::new(ImportDialogState {
        step: Step::Preview,
        awaiting: Some(RunMode::Preview),
        preview: Some(PreviewState::Running),
        ..ImportDialogState::new()
    }));
    drop(app.update(d(ImportDialogMsg::BackPressed)));
    assert!(cancel.load(Ordering::Relaxed));
    assert_eq!(dialog(&app).step, Step::Source);
    drop(app.update(cancelled(true)));
    assert!(dialog(&app).preview.is_none(), "no longer awaited");
    assert!(
        app.document_view.error_notice.is_none(),
        "a dry run never raises a notice"
    );
}

#[test]
fn a_folder_that_holds_files_is_refused_and_the_check_removes_nothing() {
    use crate::app::dispatch::library::import_dialog_target_problem as problem;
    let dir = tempfile::tempdir().unwrap();
    let absent = dir.path().join("new");
    assert!(problem(&absent).is_none());
    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    assert!(problem(&empty).is_none());
    assert!(empty.exists(), "the check never removes anything");
    let full = dir.path().join("full");
    std::fs::create_dir(&full).unwrap();
    std::fs::write(full.join("mine.txt"), "x").unwrap();
    assert!(problem(&full).is_some());
    assert!(full.join("mine.txt").exists());
    let file = dir.path().join("file");
    std::fs::write(&file, "x").unwrap();
    assert!(problem(&file).is_some());
    #[cfg(unix)]
    {
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&empty, &link).unwrap();
        let why = problem(&link).expect("a link is refused");
        assert_eq!(
            why.id(),
            Some(signex_i18n::ids::LIBRARY_IMPORT_DIALOG_TARGET_LINK)
        );
    }
}

#[test]
fn every_step_builds_in_both_graphite_themes() {
    let i18n = I18n::en_us();
    let mut step_one = found(vec![converter("/bin/a", A), converter("/bin/b", B)]);
    picked(&mut step_one, &["/in/P.lib1", "/in/Company.dblink"]);
    let mut db = found(vec![converter("/bin/a", A)]);
    picked(&mut db, &["/in/Company.dblink"]);
    let states = [
        ImportDialogState::new(),
        found(Vec::new()),
        step_one,
        db,
        ImportDialogState {
            step: Step::Preview,
            preview: Some(PreviewState::Ready(Box::new(outcome()))),
            selected_item: Some(0),
            ..ImportDialogState::new()
        },
        ImportDialogState {
            step: Step::Preview,
            preview: Some(PreviewState::Running),
            ..ImportDialogState::new()
        },
        ImportDialogState {
            step: Step::Import,
            phases: vec![super::progress::PhaseLine {
                phase: "write".into(),
                done: 2,
                total: Some(3),
            }],
            cancel_pending: true,
            ..ImportDialogState::new()
        },
        ImportDialogState {
            step: Step::Import,
            import_end: Some(ImportEnd::Cancelled { removed: true }),
            ..ImportDialogState::new()
        },
    ];
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        for state in &states {
            for windowed in [false, true] {
                for project_open in [false, true] {
                    let ctx = ImportDialogContext {
                        running: None,
                        project_open,
                    };
                    drop(view(state, ctx, &tokens, &i18n, windowed));
                }
            }
        }
    }
}

#[test]
fn the_step_note_names_the_step() {
    let i18n = I18n::en_us();
    let note = super::view::step_note(Step::Preview, &i18n).into_string();
    let plain: String = note
        .chars()
        .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
        .collect();
    assert_eq!(plain, "Step 2 of 3 · Preview");
}

#[test]
fn a_second_cancel_or_esc_stops_a_converter_stuck_on_one_item() {
    for second in [ImportDialogMsg::CancelPressed, ImportDialogMsg::Closed] {
        let (mut app, cancel) = importing();
        drop(app.update(d(ImportDialogMsg::CancelPressed)));
        // The converter keeps repeating its progress on item A (§3.1).
        for _ in 0..3 {
            let same = ProgressEvent::Progress {
                done: 1,
                total: Some(3),
                item: Some("A".into()),
            };
            drop(app.update(import(LibraryImportMsg::ProgressReported(same))));
        }
        assert!(!cancel.load(Ordering::Relaxed), "still waiting for A");
        assert!(dialog(&app).cancel_pending && !dialog(&app).stop_requested);
        drop(app.update(d(second)));
        assert!(cancel.load(Ordering::Relaxed), "stopped now");
        assert!(dialog(&app).stop_requested);
        drop(app.update(cancelled(true)));
        drop(app.update(d(ImportDialogMsg::Closed)));
        assert!(app.library.import_dialog.is_none(), "the window closes");
    }
}

#[test]
fn next_waits_while_a_left_dry_run_is_still_stopping() {
    let (mut app, _boot) = Signex::new();
    let mut state = found(vec![converter("/bin/a", A)]);
    picked(&mut state, &["/in/P.lib1"]);
    // Back left this dialog's dry run stopping.
    state.run_stopping = true;
    app.library.import_dialog = Some(Box::new(state));
    app.library.import.running = Some(ImportRun {
        output: "/tmp/signex-library-preview-old".into(),
        mode: RunMode::Preview,
        target: MountTarget::Global,
        cancel: Arc::new(AtomicBool::new(true)),
        group: Default::default(),
        progress: None,
    });
    drop(app.update(d(ImportDialogMsg::NextPressed)));
    assert_eq!(dialog(&app).blocker(), Some(Blocker::RunStopping));
    assert!(!dialog(&app).checking_target, "nothing started");
    drop(app.update(cancelled(true)));
    assert_eq!(dialog(&app).blocker(), None, "free again");
    assert!(app.document_view.error_notice.is_none());
}

#[test]
fn next_waits_for_a_run_the_dialog_did_not_start_and_says_so_neutrally() {
    let (mut app, _boot) = Signex::new();
    let mut state = found(vec![converter("/bin/a", A)]);
    picked(&mut state, &["/in/P.lib1"]);
    app.library.import_dialog = Some(Box::new(state));
    app.library.import.running = Some(ImportRun {
        output: "/elsewhere/Other".into(),
        mode: RunMode::Import,
        target: MountTarget::Global,
        cancel: Arc::new(AtomicBool::new(false)),
        group: Default::default(),
        progress: None,
    });
    drop(app.update(d(ImportDialogMsg::NextPressed)));
    let state = dialog(&app);
    assert_eq!(state.blocker(), Some(Blocker::OtherRun));
    assert!(!state.run_stopping, "not a run of this dialog's");
    let i18n = I18n::en_us();
    let line = super::view::blocker_line(state, &i18n).unwrap().into_string();
    assert_eq!(plain(&line), "Another library import is running.");
    drop(app.update(cancelled(false)));
    assert_eq!(dialog(&app).blocker(), None, "free again");
}

#[test]
fn a_dry_run_nobody_waits_for_leaves_no_temp_folder() {
    let dir = std::env::temp_dir().join(format!(
        "signex-library-preview-test-{}",
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("import-report.json"), REPORT).unwrap();
    let (mut app, _boot) = Signex::new();
    app.library.import.running = Some(ImportRun {
        output: dir.clone(),
        mode: RunMode::Preview,
        target: MountTarget::Global,
        cancel: Arc::new(AtomicBool::new(true)),
        group: Default::default(),
        progress: None,
    });
    let done = crate::library_import::ImportJobOutcome {
        run: outcome(),
        library_file: None,
        commit: None,
        local_commits: 0,
        library_id: None,
    };
    let task = app.update(import(LibraryImportMsg::Finished(Box::new(
        ImportFinished {
            result: Ok(done),
            target: MountTarget::Global,
        },
    ))));
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .unwrap();
    assert!(!dir.exists(), "the left dry run's folder is removed");
}

#[test]
fn a_refused_folder_says_nothing_was_changed() {
    let (mut app, _cancel) = importing();
    let failed = RunFailure::refused(ImportError::OutputNotEmpty {
        path: "/out/P".into(),
    });
    drop(app.update(import(LibraryImportMsg::Finished(Box::new(
        ImportFinished {
            result: Err(failed),
            target: MountTarget::Global,
        },
    )))));
    let end = dialog(&app).import_end.clone().expect("ended");
    assert!(matches!(end, ImportEnd::Refused { .. }), "{end:?}");
    let i18n = I18n::en_us();
    let (_, _, second) = super::view::end_words(&end, "/out/P".into(), &i18n);
    let second = plain(&second.into_string());
    assert!(second.contains("nothing was changed in /out/P"), "{second}");
    assert!(!second.contains("Nothing was kept"), "{second}");
    // A run that wrote and then failed still says what became of it.
    let wrote = ImportEnd::Failed {
        reason: ImportError::Cancelled.to_translatable(),
        removed: true,
    };
    let (_, _, second) = super::view::end_words(&wrote, "/out/P".into(), &i18n);
    assert!(plain(&second.into_string()).starts_with("Nothing was kept"));
}

/// `text` without Fluent's isolation marks.
fn plain(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
        .collect()
}

#[test]
fn quitting_stops_a_running_import() {
    let (mut app, cancel) = importing();
    app.stop_library_import_at_quit();
    assert!(cancel.load(Ordering::Relaxed));
}
