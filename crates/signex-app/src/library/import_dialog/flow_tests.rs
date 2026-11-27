//! The Import Library dialog end to end, headless: a fake converter on a
//! temp PATH is discovered, previewed with `--dry-run` into a temp folder,
//! then imported, committed and mounted. The converter is run as
//! `/bin/sh <script>`, so no test execs a file it has just written.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{ImportDialogMsg, PreviewState, Step};
use crate::app::{Message, Signex};
use crate::library_import::ConverterCommand;
use crate::library_import::discovery::{DiscoveryEnv, discover_cached};
use crate::library_import::run_tests::{PREAMBLE, summary, write_report_line};
use crate::task_driver::TaskDriver;

const DESCRIBE: &str = r#"{"contract":"1.0","converter":"fake-import","version":"1.2.3","inputs":[{"label":"Fake library","extensions":["fakelib"],"kind":"library"}]}"#;

fn fixture_library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/fixtures/library-format/library")
}

/// Answers `describe`; a dry run writes only the report, an import copies
/// the conformance library too.
fn script() -> String {
    format!(
        r#"{PREAMBLE}if [ "$1" = describe ]; then echo '{DESCRIBE}'; exit 0; fi
dry=no; for a in "$@"; do [ "$a" = "--dry-run" ] && dry=yes; done
echo '{{"event":"phase","phase":"read","total":1}}'
echo '{{"event":"progress","done":1,"total":1,"item":"Fake:R1"}}'
if [ "$dry" = no ]; then mkdir -p "$out" && cp -R '{fixture}/.' "$out/"; fi
{report}{summary}
exit 0
"#,
        fixture = fixture_library().display(),
        report = write_report_line(),
        summary = summary("ok", true),
    )
}

/// A converter named `signex-fake-import` in `bin`, found as discovery
/// finds one on PATH.
fn install_fake(bin: &Path) -> crate::library_import::DiscoveryCache {
    use std::os::unix::fs::PermissionsExt;
    let program = bin.join("signex-fake-import");
    std::fs::write(&program, script()).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env = DiscoveryEnv {
        path: Some(bin.as_os_str().to_owned()),
        list: None,
        singles: Vec::new(),
    };
    discover_cached(None, &env, Duration::from_secs(10), |path| {
        ConverterCommand {
            program: PathBuf::from("/bin/sh"),
            leading_args: vec![path.into()],
        }
    })
}

fn d(m: ImportDialogMsg) -> Message {
    Message::Library(super::msg(m))
}

fn settle(app: &mut Signex, message: Message) {
    let task = app.update(message);
    TaskDriver::new().settle(app, task).unwrap();
}

#[test]
fn discover_preview_and_import_through_the_dialog() {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let cache = install_fake(&bin);
    assert_eq!(cache.discovery.converters.len(), 1, "{:?}", cache.discovery);
    let source = dir.path().join("Passives.fakelib");
    std::fs::write(&source, "source").unwrap();
    let out = dir.path().join("libs").join("Passives");

    let (mut app, _boot) = Signex::new();
    // The open's own search reads this process's PATH; the test answers
    // with the temp PATH's instead.
    drop(app.update(super::requested_message()));
    drop(app.update(d(ImportDialogMsg::DiscoveryFinished(Some(Box::new(cache))))));
    drop(app.update(d(ImportDialogMsg::SourcesPicked(Some(vec![
        source.clone(),
    ])))));
    drop(app.update(d(ImportDialogMsg::LocationEdited(
        out.display().to_string(),
    ))));
    let state = app.library.import_dialog.as_deref().unwrap();
    assert_eq!(state.name, "Passives");
    assert_eq!(state.blocker(), None);

    // Step 2: the dry run writes only its report, into a temp folder.
    settle(&mut app, d(ImportDialogMsg::NextPressed));
    let state = app.library.import_dialog.as_deref().unwrap();
    assert_eq!(state.step, Step::Preview);
    assert!(
        matches!(state.preview, Some(PreviewState::Ready(_))),
        "{:?}",
        state.preview
    );
    assert_eq!(state.parts_to_import(), 3);
    assert_eq!(state.report_items().len(), 3);
    let preview_dir = state.preview_dir.clone().expect("a temp folder");
    assert!(preview_dir.join("import-report.json").exists());
    assert!(!out.exists(), "nothing is written before Import");
    assert!(app.library.import.last.as_ref().is_some());

    // Step 3: the import writes, commits and mounts; the dialog ends on
    // the result card with the run's report.
    settle(&mut app, d(ImportDialogMsg::ImportPressed));
    let state = app.library.import_dialog.as_deref().expect("the result");
    let result = state.finished().expect("finished");
    let facts = result.run.as_ref().expect("a run's result");
    assert_eq!(facts.mounted, Some(super::result::Mounted::AllProjects));
    assert!(facts.commit.is_some(), "the import is the first commit");
    assert_eq!(state.report_items().len(), 3);
    assert!(!preview_dir.exists(), "the temp folder is gone");
    let last = app.library.import.last.as_ref().unwrap();
    let outcome = last.result.as_ref().expect("imported");
    assert_eq!(outcome.run.summary.counts.parts, 3);
    let file = outcome.library_file.clone().unwrap();
    assert!(file.starts_with(&out));
    assert!(outcome.commit.is_some());
    assert!(app.library.library_at(&file).is_some(), "mounted");
    assert!(app.library.import.running.is_none());
    // Open in Library Browser closes the dialog on the library's tab.
    settle(&mut app, d(ImportDialogMsg::OpenInBrowserPressed));
    assert!(app.library.import_dialog.is_none());
}

/// The dialog through Import into `out` from `source`, ending on the
/// result.
fn import_through_the_dialog(app: &mut Signex, bin: &Path, source: &Path, out: &Path) {
    drop(app.update(super::requested_message()));
    let cache = install_fake(bin);
    drop(app.update(d(ImportDialogMsg::DiscoveryFinished(Some(Box::new(cache))))));
    drop(app.update(d(ImportDialogMsg::SourcesPicked(Some(vec![
        source.to_path_buf(),
    ])))));
    drop(app.update(d(ImportDialogMsg::LocationEdited(
        out.display().to_string(),
    ))));
    settle(app, d(ImportDialogMsg::NextPressed));
}

/// #3123: Import into a folder that holds an imported library offers an
/// update; once taken, the preview's button runs `--update` and the result
/// says it was updated, with no new commit when nothing changed.
#[test]
fn an_import_into_an_imported_library_offers_an_update_and_runs_it() {
    use crate::library_import::RunMode;
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let source = dir.path().join("Passives.fakelib");
    std::fs::write(&source, "source").unwrap();
    let out = dir.path().join("libs").join("Passives");
    let (mut app, _boot) = Signex::new();
    import_through_the_dialog(&mut app, &bin, &source, &out);
    settle(&mut app, d(ImportDialogMsg::ImportPressed));
    drop(app.update(d(ImportDialogMsg::Closed)));

    // The same source into the same folder: offered, not run.
    import_through_the_dialog(&mut app, &bin, &source, &out);
    let state = app.library.import_dialog.as_deref().unwrap();
    assert_eq!(state.step, Step::Source);
    let offer = state.update_offer.expect("an update is offered");
    assert!(!offer.accepted);
    assert_eq!(offer.local_commits, 0);
    assert_eq!(
        state.blocker(),
        Some(super::source::Blocker::UpdateOffered)
    );
    assert!(app.library.import.running.is_none(), "nothing ran");

    // Taken: the preview, then the update.
    settle(&mut app, d(ImportDialogMsg::UpdateAccepted));
    let state = app.library.import_dialog.as_deref().unwrap();
    assert_eq!(state.step, Step::Preview);
    assert!(state.is_update());
    settle(&mut app, d(ImportDialogMsg::ImportPressed));
    let state = app.library.import_dialog.as_deref().unwrap();
    let facts = state
        .finished()
        .and_then(|r| r.run.as_ref())
        .expect("the update's result");
    assert_eq!(facts.mode, RunMode::Update);
    assert_eq!(facts.commit, None, "an unchanged re-import adds no commit");
    assert!(out.join("import-report.json").exists());
}

#[test]
fn a_folder_that_holds_files_sends_the_import_back_to_step_one() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let cache = install_fake(&bin);
    let out = dir.path().join("taken");
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join("mine.txt"), "keep").unwrap();

    let (mut app, _boot) = Signex::new();
    drop(app.update(super::requested_message()));
    drop(app.update(d(ImportDialogMsg::DiscoveryFinished(Some(Box::new(cache))))));
    drop(app.update(d(ImportDialogMsg::SourcesPicked(Some(vec![
        dir.path().join("P.fakelib"),
    ])))));
    drop(app.update(d(ImportDialogMsg::LocationEdited(
        out.display().to_string(),
    ))));
    settle(&mut app, d(ImportDialogMsg::NextPressed));
    let state = app.library.import_dialog.as_deref().unwrap();
    assert_eq!(state.step, Step::Source);
    assert!(state.target_problem.is_some());
    assert!(app.library.import.running.is_none(), "no run started");
    assert_eq!(
        std::fs::read_to_string(out.join("mine.txt")).unwrap(),
        "keep"
    );
}

/// Cancel always gives the window back, whatever the converter does: it
/// repeats its progress on one item forever, says nothing, or ignores
/// termination and never exits. The run is started through the engine and
/// the dialog's own Cancel (pressed twice while an item is in flight).
#[test]
fn cancel_gives_the_window_back_whatever_the_converter_does() {
    use crate::library_import::run_tests::{request, sh};
    use crate::library_import::{ImportJob, MountScope, ProgressEvent, RunLimits, RunMode};
    let heartbeat = "while :; do echo '{\"event\":\"progress\",\"done\":1,\"total\":3,\"item\":\"A\"}'; sleep 0.05; done\n";
    let silent = "sleep 30\n";
    let stubborn = "trap '' TERM INT HUP; while :; do sleep 0.1; done\n";
    for (body, in_flight) in [(heartbeat, true), (silent, false), (stubborn, false)] {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("P");
        let job = ImportJob {
            command: sh(dir.path(), body),
            converter: "fake-import".into(),
            converter_version: "1".into(),
            request: request(out.clone(), RunMode::Import),
            limits: RunLimits {
                silence: Duration::from_secs(60),
                poll: Duration::from_millis(10),
            },
            group: Default::default(),
        };
        let (mut app, _boot) = Signex::new();
        let task = app.start_library_import(job, MountScope::AllProjects);
        app.library.import_dialog = Some(Box::new(super::ImportDialogState {
            step: Step::Import,
            awaiting: Some(RunMode::Import),
            location: out.display().to_string(),
            ..super::ImportDialogState::new()
        }));
        if in_flight {
            app.library.import.running.as_mut().unwrap().progress = Some(ProgressEvent::Progress {
                done: 1,
                total: Some(3),
                item: Some("A".into()),
            });
        }
        drop(app.update(d(ImportDialogMsg::CancelPressed)));
        if in_flight {
            // Waiting for A; the second press stops it now.
            drop(app.update(d(ImportDialogMsg::CancelPressed)));
        }
        let started = std::time::Instant::now();
        TaskDriver::new().settle(&mut app, task).unwrap();
        assert!(started.elapsed() < Duration::from_secs(10), "{body}");
        let state = app.library.import_dialog.as_deref().expect("open");
        assert_eq!(
            state.import_end,
            Some(super::ImportEnd::Cancelled {
                folder: super::FolderAfter::Removed
            }),
            "{body}"
        );
        assert!(!out.exists(), "{body}: nothing kept");
        drop(app.update(d(ImportDialogMsg::Closed)));
        assert!(app.library.import_dialog.is_none(), "{body}: closes");
    }
}
