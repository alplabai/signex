//! #1117 — New Project writes and maintains a managed `.gitignore` block.

use std::path::Path;

use signex_app::app::{FileMsg, Message, Signex};
use signex_types::managed_block::{BEGIN_MARKER, END_MARKER};
use signex_types::project_vcs_files::GITIGNORE_BLOCK_BODY;
use tempfile::TempDir;

fn block() -> String {
    format!("{BEGIN_MARKER}\n{GITIGNORE_BLOCK_BODY}{END_MARKER}\n")
}

fn new_project(app: &mut Signex, dir: &Path) {
    crate::settle_support::settle(
        app,
        Message::File(FileMsg::NewProject(Some(dir.join("Fresh.snxprj")))),
    );
}

fn warned_about(marker: &str, needle: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(marker) && e.message.contains(needle))
}

#[test]
fn new_project_creates_a_gitignore_that_is_exactly_the_managed_block() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, tmp.path());

    assert!(app.document_state.error_notice.is_none());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
        block()
    );
}

#[test]
fn an_existing_gitignore_keeps_its_user_lines_and_the_old_block_is_replaced() {
    let tmp = TempDir::new().unwrap();
    let user_head = "target/\r\n*.log\n";
    let user_tail = "# mine\nnotes.txt";
    let old = format!("{BEGIN_MARKER}\nstale-pattern\n{END_MARKER}\n");
    std::fs::write(
        tmp.path().join(".gitignore"),
        format!("{user_head}{old}{user_tail}"),
    )
    .unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, tmp.path());
    let once = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    assert_eq!(once, format!("{user_head}{}{user_tail}", block()));

    // Running it again changes nothing.
    new_project(&mut app, tmp.path());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
        once
    );
}

#[test]
fn unbalanced_markers_leave_the_file_untouched_and_log_a_warning() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let broken = format!("mine\n{BEGIN_MARKER}\nno end marker\n");
    std::fs::write(tmp.path().join(".gitignore"), &broken).unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, tmp.path());

    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
        broken
    );
    let marker = tmp.path().display().to_string();
    assert!(
        warned_about(&marker, "unbalanced"),
        "the refusal must reach the Messages panel"
    );
    assert!(app.document_state.error_notice.is_none());
    assert!(tmp.path().join("Fresh.snxsch").exists(), "project created");
}

#[test]
fn a_gitignore_that_is_a_directory_is_refused_logged_and_new_project_still_succeeds() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    // A directory where the file belongs: refused as not a regular file.
    std::fs::create_dir(tmp.path().join(".gitignore")).unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, tmp.path());

    let marker = tmp.path().display().to_string();
    assert!(
        warned_about(&marker, ".gitignore"),
        "the failure must reach the Messages panel"
    );
    assert!(
        app.document_state.error_notice.is_none(),
        "New Project must still succeed"
    );
    assert!(tmp.path().join("Fresh.snxprj").exists());
    assert!(
        app.document_state
            .projects
            .iter()
            .any(|p| p.path == tmp.path().join("Fresh.snxprj")),
        "the project must open"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_gitignore_is_never_followed_and_the_refusal_is_logged() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let secret = outside.path().join("secret");
    std::fs::write(&secret, "TOP SECRET\n").unwrap();
    std::os::unix::fs::symlink(&secret, tmp.path().join(".gitignore")).unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, tmp.path());

    assert_eq!(std::fs::read_to_string(&secret).unwrap(), "TOP SECRET\n");
    assert!(
        std::fs::symlink_metadata(tmp.path().join(".gitignore"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        warned_about(&tmp.path().display().to_string(), "not a regular file"),
        "the refusal must reach the Messages panel"
    );
    assert!(
        app.document_state.error_notice.is_none(),
        "New Project still succeeds"
    );
}

#[test]
fn a_read_only_gitignore_is_refused_and_logged() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join(".gitignore");
    std::fs::write(&path, "mine\n").unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms).unwrap();
    let (mut app, _t) = Signex::new();

    new_project(&mut app, tmp.path());

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "mine\n");
    assert!(warned_about(&tmp.path().display().to_string(), "read-only"));
}
