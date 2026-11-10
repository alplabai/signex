//! #2253 — the save commit lists the agent edits of the file it commits, on a
//! real repository, through the same worker a save uses.

use std::path::Path;
use std::sync::Arc;

use crate::app::Signex;
use crate::app::agent::journal::{EditEntry, JOURNAL_VERSION, Journal};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::CommitBytes;

/// Later than any commit a test makes, so the edit is "since the last commit".
const NEW: &str = "2099-01-01T00:00:00.000Z";

fn git_app(dir: &Path, summarise: bool) -> Signex {
    signex_library::git_identity::test_identity::install();
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    app.document_state.projects[0]
        .data
        .agent_journal
        .summarise_in_commit = summarise;
    app
}

fn edit_line(dir: &Path, file: &str, command: &str) -> String {
    let entry = EditEntry {
        v: JOURNAL_VERSION,
        ts: NEW.into(),
        session: "s".into(),
        pid: Some(7),
        client: Some("bot".into()),
        document: None,
        path: Some(dir.join(file).display().to_string()),
        command: command.into(),
        operands: Default::default(),
        revision_before: Some(1),
        revision_after: Some(2),
        group: None,
        outcome: "ran".into(),
        reason: None,
        truncated: false,
        replayable: false,
    };
    format!("{}\n", entry.to_line())
}

fn write_journal(dir: &Path, lines: &[String]) {
    let folder = dir.join(".signex");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(Journal::Edits.file_name()), lines.concat()).unwrap();
}

/// Queue the commit of `file` with its bytes, run it and wait for it.
fn save(app: &mut Signex, dir: &Path, file: &str, message: &str) {
    save_bytes(app, dir, file, message, file);
}

fn save_bytes(app: &mut Signex, dir: &Path, file: &str, message: &str, text: &str) {
    let content = CommitBytes {
        bytes: Arc::new(text.as_bytes().to_vec()),
        sequence: 0,
    };
    app.commit_bytes_to_project_git(&dir.join(file), message, content);
    let task = app.drain_pending_git_commits();
    crate::app::handlers::menu::export::settle(app, task);
}

/// The messages of every commit, oldest first.
fn messages(dir: &Path) -> Vec<String> {
    let repo = git2::Repository::open(dir).unwrap();
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)
        .unwrap();
    walk.map(|oid| {
        repo.find_commit(oid.unwrap())
            .unwrap()
            .message()
            .unwrap()
            .to_string()
    })
    .collect()
}

#[test]
fn with_the_setting_off_the_message_is_what_it_always_was() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = git_app(dir.path(), false);
    write_journal(
        dir.path(),
        &[edit_line(dir.path(), "a.snxsch", "color_net")],
    );

    save(&mut app, dir.path(), "a.snxsch", "Save A");

    assert_eq!(messages(dir.path()), ["Save A"], "byte for byte");
}

#[test]
fn with_the_setting_on_the_first_commit_lists_the_edits_since_no_head() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = git_app(dir.path(), true);
    write_journal(
        dir.path(),
        &[edit_line(dir.path(), "a.snxsch", "color_net")],
    );

    save(&mut app, dir.path(), "a.snxsch", "Save A");

    let commits = messages(dir.path());
    assert_eq!(commits.len(), 1);
    assert!(commits[0].starts_with("Save A\n\nAgent edits since the previous commit:\n- "));
    assert!(commits[0].contains("command=color_net doc=a.snxsch"));
}

#[test]
fn a_save_of_two_documents_gives_each_commit_only_its_own_edits() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = git_app(dir.path(), true);
    write_journal(
        dir.path(),
        &[
            edit_line(dir.path(), "a.snxsch", "color_net"),
            edit_line(dir.path(), "b.snxsch", "apply_edits"),
        ],
    );

    save(&mut app, dir.path(), "a.snxsch", "Save A");
    save(&mut app, dir.path(), "b.snxsch", "Save B");

    let commits = messages(dir.path());
    assert!(commits[0].contains("doc=a.snxsch") && !commits[0].contains("b.snxsch"));
    assert!(commits[1].contains("doc=b.snxsch") && !commits[1].contains("a.snxsch"));
}

#[test]
fn an_edit_of_an_unsaved_document_waits_for_its_own_commit() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = git_app(dir.path(), true);
    write_journal(
        dir.path(),
        &[edit_line(dir.path(), "b.snxsch", "apply_edits")],
    );

    save(&mut app, dir.path(), "a.snxsch", "Save A");
    save(&mut app, dir.path(), "b.snxsch", "Save B");

    let commits = messages(dir.path());
    assert_eq!(commits[0], "Save A", "not a's edit");
    assert!(commits[1].contains("command=apply_edits doc=b.snxsch"));
}

#[test]
fn the_project_file_commit_carries_assign_net_class() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = git_app(dir.path(), true);
    write_journal(
        dir.path(),
        &[edit_line(dir.path(), "a.snxsch", "assign_net_class")],
    );

    save(&mut app, dir.path(), "a.snxsch", "Save A");
    save(&mut app, dir.path(), "p.snxprj", "Save project");

    let commits = messages(dir.path());
    assert_eq!(commits[0], "Save A");
    assert!(commits[1].starts_with("Save project\n\nAgent edits"));
    assert!(commits[1].contains("command=assign_net_class"));
}

/// The "since" is the file's own last commit, read from the repository.
#[test]
fn an_edit_is_listed_once_not_again_in_the_files_next_commit() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = git_app(dir.path(), true);
    let old =
        edit_line(dir.path(), "a.snxsch", "color_net").replace(NEW, "2000-01-01T00:00:00.000Z");
    write_journal(dir.path(), &[old]);

    save_bytes(&mut app, dir.path(), "a.snxsch", "Save A", "one");
    save_bytes(&mut app, dir.path(), "a.snxsch", "Save A again", "two");

    let commits = messages(dir.path());
    assert!(commits[0].contains("command=color_net"), "{}", commits[0]);
    assert_eq!(commits[1], "Save A again");
}

#[test]
fn a_damaged_journal_does_not_stop_the_save() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = git_app(dir.path(), true);
    std::fs::create_dir_all(dir.path().join(".signex").join(Journal::Edits.file_name())).unwrap();

    save(&mut app, dir.path(), "a.snxsch", "Save A");

    assert_eq!(messages(dir.path()), ["Save A"]);
}
