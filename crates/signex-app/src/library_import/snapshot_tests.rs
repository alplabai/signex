//! An import end to end: a fake converter writes the conformance library,
//! the app commits it, mounts it for all projects and lists its parts; a
//! re-import adds one commit; a dirty tree refuses the update.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use super::error::ImportError;
use super::run::RunMode;
use super::run_tests::{FAST, request, sh, summary, write_report_line};
use super::snapshot::{ImportJob, MountScope, check_update};
use crate::app::Signex;

fn fixture_library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/fixtures/library-format/library")
}

/// Copies the conformance library on a first import; on `--update`
/// rewrites one file so the update has something to commit. Leaves a
/// marker beside the output so a test can tell it ran.
fn converter_body() -> String {
    format!(
        r#"update=no; for a in "$@"; do [ "$a" = "--update" ] && update=yes; done
: > "$out.ran"
echo '{{"event":"phase","phase":"write","total":1}}'
if [ "$update" = no ]; then mkdir -p "$out" && cp -R '{fixture}/.' "$out/"; fi
echo "run: $update" > "$out/import-report.md"
{report}{summary}
exit 0
"#,
        fixture = fixture_library().display(),
        report = write_report_line(),
        summary = summary("ok", true),
    )
}

fn job(dir: &Path, mode: RunMode) -> ImportJob {
    ImportJob {
        command: sh(dir, &converter_body()),
        converter: "example-import".into(),
        converter_version: "0.16.0".into(),
        request: request(dir.join("Conformance"), mode),
        limits: FAST,
    }
}

fn commit_count(dir: &Path) -> usize {
    let repo = git2::Repository::open(dir).unwrap();
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    walk.count()
}

fn import_through_the_app(app: &mut Signex, job: ImportJob) {
    let task = app.start_library_import(job, MountScope::AllProjects);
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .unwrap();
}

#[test]
fn an_import_is_committed_mounted_for_all_projects_and_lists_its_parts() {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("Conformance");
    let (mut app, _boot) = Signex::new();

    import_through_the_app(&mut app, job(dir.path(), RunMode::Import));

    let last = app.library.import.last.as_ref().expect("finished");
    let outcome = last.result.as_ref().expect("imported");
    let file = outcome.library_file.clone().unwrap();
    assert_eq!(file, out.join("Conformance.snxlib"));
    assert!(outcome.commit.is_some());
    assert!(app.library.import.running.is_none());
    assert_eq!(commit_count(&out), 1);
    let repo = git2::Repository::open(&out).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let message = head.message().unwrap().to_string();
    assert!(message.contains("Passives.lib1"), "{message}");
    assert!(
        message.contains("Signex-Import: example-import 0.16.0"),
        "{message}"
    );
    assert!(
        std::fs::read_to_string(out.join(".gitattributes"))
            .unwrap()
            .contains("*.snxfpt diff=snxfpt")
    );

    assert!(
        app.library.global_libraries.iter().any(|e| e.path == file),
        "recorded for all projects"
    );
    let mounted = app
        .library
        .open_libraries
        .iter()
        .find(|l| l.root == file)
        .expect("mounted");
    assert_eq!(mounted.cached_components.len(), 3, "its parts are listed");

    // Re-import: exactly one new commit, still mounted.
    import_through_the_app(&mut app, job(dir.path(), RunMode::Update));
    let last = app.library.import.last.as_ref().unwrap();
    let update = last.result.as_ref().expect("updated");
    assert!(update.commit.is_some());
    assert_eq!(update.local_commits, 0);
    assert_eq!(commit_count(&out), 2);
    assert!(app.library.library_at(&file).is_some());
}

#[test]
fn a_dirty_tree_refuses_the_update_before_the_converter_runs() {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("Conformance");
    job(dir.path(), RunMode::Import)
        .run(&AtomicBool::new(false), &mut |_| {})
        .unwrap();
    std::fs::remove_file(dir.path().join("Conformance.ran")).unwrap();
    std::fs::write(out.join("symbols/mine.snxsym"), "x\n").unwrap();

    let failure = job(dir.path(), RunMode::Update)
        .run(&AtomicBool::new(false), &mut |_| {})
        .unwrap_err();

    assert!(
        matches!(&failure.error, ImportError::UncommittedChanges { paths } if paths == &["symbols/mine.snxsym"]),
        "{:?}",
        failure.error
    );
    assert!(!dir.path().join("Conformance.ran").exists(), "not run");
    assert!(out.join("symbols/mine.snxsym").exists(), "the edit is kept");
    assert_eq!(commit_count(&out), 1);
}

#[test]
fn local_commits_are_counted_for_the_warning() {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("Conformance");
    job(dir.path(), RunMode::Import)
        .run(&AtomicBool::new(false), &mut |_| {})
        .unwrap();
    std::fs::write(out.join("symbols/mine.snxsym"), "x\n").unwrap();
    signex_library::import_snapshot::commit_snapshot(&out, "add my symbol\n").unwrap();

    assert_eq!(check_update(&out).unwrap().local_commits, 1);
    let updated = job(dir.path(), RunMode::Update)
        .run(&AtomicBool::new(false), &mut |_| {})
        .unwrap();
    assert_eq!(updated.local_commits, 1);
    assert_eq!(commit_count(&out), 3);
}

#[test]
fn a_preview_writes_no_repository() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = job(dir.path(), RunMode::Preview)
        .run(&AtomicBool::new(false), &mut |_| {})
        .unwrap();
    assert!(outcome.library_file.is_none() && outcome.commit.is_none());
    assert!(outcome.run.report.is_some());
    assert!(!dir.path().join("Conformance/.git").exists());
}

#[test]
fn an_update_of_a_folder_without_history_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("Conformance");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("Conformance.snxlib"), "").unwrap();
    let failure = job(dir.path(), RunMode::Update)
        .run(&AtomicBool::new(false), &mut |_| {})
        .unwrap_err();
    assert!(matches!(
        failure.error,
        ImportError::NotAnImportedLibrary { .. }
    ));
    assert!(!dir.path().join("Conformance.ran").exists(), "not run");
}

fn imported(dir: &Path) -> PathBuf {
    signex_library::git_identity::test_identity::install();
    job(dir, RunMode::Import)
        .run(&AtomicBool::new(false), &mut |_| {})
        .unwrap();
    dir.join("Conformance")
}

fn update_with(dir: &Path, body: &str) -> ImportJob {
    let mut update = job(dir, RunMode::Update);
    update.command = sh(dir, body);
    update
}

#[test]
fn a_failed_update_is_restored_to_the_last_import() {
    let dir = tempfile::tempdir().unwrap();
    let out = imported(dir.path());
    let before = std::fs::read(out.join("Conformance.snxlib")).unwrap();
    let body = "echo broken > \"$out/Conformance.snxlib\"\necho x > \"$out/symbols/added.snxsym\"\n\
                echo '{\"event\":\"summary\",\"ok\":false,\"error\":\"disk full\",\"exit\":\"failed\",\"library\":null,\"report\":null,\"files\":[],\"warnings\":[]}'\nexit 1\n";
    let failure = update_with(dir.path(), body)
        .run(&AtomicBool::new(false), &mut |_| {})
        .unwrap_err();
    assert_eq!(failure.restored, Some(Ok(())));
    assert_eq!(
        std::fs::read(out.join("Conformance.snxlib")).unwrap(),
        before
    );
    assert!(!out.join("symbols/added.snxsym").exists());
    assert!(check_update(&out).is_ok(), "clean again");
    assert_eq!(commit_count(&out), 1);
}

#[test]
fn a_silent_update_is_restored_too() {
    let dir = tempfile::tempdir().unwrap();
    let out = imported(dir.path());
    let body = "echo x > \"$out/symbols/added.snxsym\"\nexec sleep 30\n";
    let failure = update_with(dir.path(), body)
        .run(&AtomicBool::new(false), &mut |_| {})
        .unwrap_err();
    assert_eq!(failure.error, ImportError::Silent { seconds: 1 });
    assert_eq!(failure.restored, Some(Ok(())));
    assert!(!out.join("symbols/added.snxsym").exists());
}

#[test]
fn an_update_holds_the_library_write_lock() {
    let dir = tempfile::tempdir().unwrap();
    let out = imported(dir.path());
    let file = out.join("Conformance.snxlib");
    let mut refused = None;
    let mut update = job(dir.path(), RunMode::Update);
    // The callback waits out the lock; the run must not count that as silence.
    update.limits.silence = std::time::Duration::from_secs(60);
    update
        .run(&AtomicBool::new(false), &mut |_| {
            if refused.is_none() {
                refused = Some(signex_library::hold_library_write_lock(&file).is_err());
            }
        })
        .unwrap();
    assert_eq!(
        refused,
        Some(true),
        "an edit during the update waits and is refused"
    );
    assert!(
        signex_library::hold_library_write_lock(&file).is_ok(),
        "released after"
    );
}

#[test]
fn an_update_is_refused_while_the_library_has_unsaved_work() {
    let dir = tempfile::tempdir().unwrap();
    let out = imported(dir.path());
    let (mut app, _boot) = Signex::new();
    app.document_state
        .mark_document_dirty(&out.join("symbols/conformance-symbols.snxsym"));

    let task = app.start_library_import(job(dir.path(), RunMode::Update), MountScope::AllProjects);
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .unwrap();

    assert!(app.library.import.last.is_none(), "nothing ran");
    let card = app.document_view.error_notice.as_ref().expect("refused");
    use signex_i18n::RenderTranslatable as _;
    assert!(
        card.detail.render_en_us().contains("conformance-symbols"),
        "{}",
        card.detail.render_en_us()
    );
}

#[test]
fn a_library_id_already_open_elsewhere_is_not_recorded() {
    signex_library::git_identity::test_identity::install();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let (mut app, _boot) = Signex::new();
    import_through_the_app(&mut app, job(first.path(), RunMode::Import));
    let first_file = first.path().join("Conformance/Conformance.snxlib");
    assert!(app.library.library_at(&first_file).is_some());

    import_through_the_app(&mut app, job(second.path(), RunMode::Import));

    let second_file = second.path().join("Conformance/Conformance.snxlib");
    assert!(app.library.library_at(&second_file).is_none());
    assert!(
        !app.library
            .global_libraries
            .iter()
            .any(|e| e.path == second_file),
        "no dangling global entry"
    );
    let card = app.document_view.error_notice.as_ref().expect("reported");
    use signex_i18n::RenderTranslatable as _;
    assert!(card.detail.render_en_us().contains("same library id"));
}
