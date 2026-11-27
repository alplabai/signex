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

    // Step 3: the import writes, commits and mounts; the dialog closes.
    settle(&mut app, d(ImportDialogMsg::ImportPressed));
    assert!(app.library.import_dialog.is_none());
    assert!(!preview_dir.exists(), "the temp folder is gone");
    let last = app.library.import.last.as_ref().unwrap();
    let outcome = last.result.as_ref().expect("imported");
    assert_eq!(outcome.run.summary.counts.parts, 3);
    let file = outcome.library_file.clone().unwrap();
    assert!(file.starts_with(&out));
    assert!(outcome.commit.is_some());
    assert!(app.library.library_at(&file).is_some(), "mounted");
    assert!(app.library.import.running.is_none());
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
