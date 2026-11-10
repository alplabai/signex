//! #2253: the agent edits a save commit lists in its body, per committed file.

use std::convert::Infallible;
use std::path::Path;

use chrono::{DateTime, Utc};

use super::super::queue::{JournalMark, Queued};
use super::super::summary::{MAX_LISTED, message_with_agent_summary};
use super::super::{EditEntry, Journal};
use super::edit;

const OLD: &str = "2000-01-01T00:00:00.000Z";
const NEW: &str = "2099-01-01T00:00:00.000Z";

fn time(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text).unwrap().to_utc()
}

/// A journal in `root` holding `entries`, one line each.
fn journal(root: &Path, entries: Vec<EditEntry>) {
    let dir = root.join(".signex");
    std::fs::create_dir_all(&dir).unwrap();
    let text: String = entries
        .into_iter()
        .map(|entry| format!("{}\n", entry.to_line()))
        .collect();
    std::fs::write(dir.join(Journal::Edits.file_name()), text).unwrap();
}

fn of(root: &Path, file: &str, command: &str, ts: &str) -> EditEntry {
    let mut entry = edit(command, 1);
    entry.path = Some(root.join(file).display().to_string());
    entry.ts = ts.into();
    entry
}

fn message(root: &Path, file: &str, since: Option<&str>) -> String {
    message_with_agent_summary(
        root,
        Path::new(file),
        "Save it",
        Ok::<_, Infallible>(since.map(time)),
        &JournalMark::settled(),
    )
}

#[test]
fn an_edit_of_the_file_is_listed_under_the_unchanged_subject() {
    let root = tempfile::tempdir().unwrap();
    journal(
        root.path(),
        vec![of(root.path(), "a.snxsch", "color_net", NEW)],
    );

    let text = message(root.path(), "a.snxsch", None);

    assert!(text.starts_with("Save it\n\nAgent edits since the previous commit:\n"));
    assert!(text.contains(
        "- 2099-01-01T00:00:00.000Z client=signex-cli command=color_net doc=a.snxsch outcome=ran"
    ));
}

#[test]
fn only_the_edits_after_the_files_last_commit_are_listed() {
    let root = tempfile::tempdir().unwrap();
    journal(
        root.path(),
        vec![
            of(root.path(), "a.snxsch", "color_net", OLD),
            of(root.path(), "a.snxsch", "apply_edits", NEW),
        ],
    );

    let text = message(root.path(), "a.snxsch", Some("2050-01-01T00:00:00Z"));

    assert!(text.contains("command=apply_edits"), "{text}");
    assert!(!text.contains("command=color_net"), "{text}");
}

#[test]
fn nothing_after_the_last_commit_leaves_the_message_as_it_was() {
    let root = tempfile::tempdir().unwrap();
    journal(
        root.path(),
        vec![of(root.path(), "a.snxsch", "color_net", OLD)],
    );

    assert_eq!(
        message(root.path(), "a.snxsch", Some("2050-01-01T00:00:00Z")),
        "Save it"
    );
}

#[test]
fn an_edit_of_another_document_waits_for_that_documents_own_commit() {
    let root = tempfile::tempdir().unwrap();
    journal(
        root.path(),
        vec![of(root.path(), "b.snxsch", "apply_edits", NEW)],
    );

    let for_a = message(root.path(), "a.snxsch", None);
    let for_b = message(root.path(), "b.snxsch", None);

    assert_eq!(for_a, "Save it", "not a's edit");
    assert!(for_b.contains("doc=b.snxsch"), "{for_b}");
}

#[test]
fn assign_net_class_goes_to_the_project_file_commit_not_the_sheets() {
    let root = tempfile::tempdir().unwrap();
    journal(
        root.path(),
        vec![of(root.path(), "a.snxsch", "assign_net_class", NEW)],
    );

    assert_eq!(message(root.path(), "a.snxsch", None), "Save it");
    assert!(
        message(root.path(), "p.snxprj", None).contains("command=assign_net_class"),
        "the project file's commit lists it"
    );
}

#[test]
fn an_edit_that_names_no_document_goes_to_the_project_file_commit() {
    let root = tempfile::tempdir().unwrap();
    let mut entry = of(root.path(), "a.snxsch", "new_project_with_profile", NEW);
    entry.path = None;
    entry.document = None;
    journal(root.path(), vec![entry]);

    assert_eq!(message(root.path(), "a.snxsch", None), "Save it");
    assert!(message(root.path(), "p.snxprj", None).contains("doc=project"));
}

#[test]
fn a_long_list_is_cut_to_the_newest_and_counts_the_rest() {
    let root = tempfile::tempdir().unwrap();
    let entries = (0..MAX_LISTED + 5)
        .map(|n| of(root.path(), "a.snxsch", &format!("step_{n:02}"), NEW))
        .collect();
    journal(root.path(), entries);

    let text = message(root.path(), "a.snxsch", None);

    let listed = text.lines().filter(|line| line.starts_with("- ")).count();
    assert_eq!(listed, MAX_LISTED);
    assert!(
        text.contains("step_24") && text.contains("step_05"),
        "{text}"
    );
    assert!(!text.contains("step_04"), "{text}");
    assert!(
        text.ends_with("and 5 more earlier edit(s) not listed"),
        "{text}"
    );
}

#[test]
fn no_journal_leaves_the_message_as_it_was() {
    let root = tempfile::tempdir().unwrap();

    assert_eq!(message(root.path(), "a.snxsch", None), "Save it");
}

#[test]
fn an_unreadable_journal_keeps_the_plain_message_and_says_why() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().unwrap();
    // A folder where the journal file should be: not a regular file.
    std::fs::create_dir_all(root.path().join(".signex").join(Journal::Edits.file_name())).unwrap();
    let before = crate::diagnostics::newest_entry_id();

    assert_eq!(message(root.path(), "a.snxsch", None), "Save it");

    let said = crate::diagnostics::entries_since(before)
        .entries
        .iter()
        .any(|entry| entry.message.contains(&root.path().display().to_string()));
    assert!(said, "Messages names the journal that could not be read");
}

#[test]
fn damaged_lines_are_skipped_said_and_the_rest_still_listed() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().unwrap();
    journal(
        root.path(),
        vec![of(root.path(), "a.snxsch", "color_net", NEW)],
    );
    let file = root.path().join(".signex").join(Journal::Edits.file_name());
    let good = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{{not json}}\n{good}")).unwrap();
    let before = crate::diagnostics::newest_entry_id();

    let text = message(root.path(), "a.snxsch", None);

    assert!(text.contains("command=color_net"), "{text}");
    let said = crate::diagnostics::entries_since(before)
        .entries
        .iter()
        .any(|entry| entry.message.contains("not journal entries"));
    assert!(said, "Messages says a line was skipped");
}

#[test]
fn an_unreadable_last_commit_keeps_the_plain_message_and_says_why() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().unwrap();
    journal(
        root.path(),
        vec![of(root.path(), "a.snxsch", "color_net", NEW)],
    );
    let before = crate::diagnostics::newest_entry_id();

    let text = message_with_agent_summary(
        root.path(),
        Path::new("unique_last_commit_error.snxsch"),
        "Save it",
        Err::<Option<DateTime<Utc>>, _>("git open: boom"),
        &JournalMark::settled(),
    );

    assert_eq!(text, "Save it");
    let said = crate::diagnostics::entries_since(before)
        .entries
        .iter()
        .any(|entry| {
            entry.message.contains("unique_last_commit_error.snxsch")
                && entry.message.contains("git open: boom")
        });
    assert!(said);
}

/// `Key: value` at the start of a line is what git reads as a trailer.
fn looks_like_a_trailer(line: &str) -> bool {
    line.split_once(':').is_some_and(|(token, _)| {
        !token.is_empty() && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

#[test]
fn no_text_of_an_entry_can_start_a_line_or_a_trailer() {
    let root = tempfile::tempdir().unwrap();
    let mut entry = of(root.path(), "a.snxsch", "color_net", NEW);
    entry.client = Some("bot\nSigned-off-by: someone\r\u{1b}[31m".into());
    entry.reason = Some("why\n\nReviewed-by: someone\u{7}".into());
    entry.outcome = "failed".into();
    journal(root.path(), vec![entry]);

    let text = message(root.path(), "a.snxsch", None);

    let body: Vec<&str> = text.lines().skip(2).collect();
    assert_eq!(body.len(), 2, "the header and one edit line: {text}");
    for line in &body[1..] {
        assert!(line.starts_with("- "), "{line}");
    }
    for line in text.lines() {
        assert!(!looks_like_a_trailer(line), "{line}");
    }
    assert!(!text.contains(['\r', '\u{1b}', '\u{7}']), "{text:?}");
}

/// Line and paragraph separators and bidirectional controls are not control
/// characters, but a reader (or a tool) can still break a line at them or
/// reorder the text.
#[test]
fn separators_and_bidi_controls_are_replaced_too() {
    let root = tempfile::tempdir().unwrap();
    let mut entry = of(root.path(), "a.snxsch", "color_net", NEW);
    entry.client = Some("bot\u{2028}Signed-off-by: x\u{2029}\u{202e}evil".into());
    entry.reason = Some(
        "\u{200e}\u{200f}\u{202a}\u{202b}\u{202c}\u{202d}\u{2066}\u{2067}\u{2068}\u{2069}r".into(),
    );
    journal(root.path(), vec![entry]);

    let text = message(root.path(), "a.snxsch", None);

    for c in [
        '\u{2028}', '\u{2029}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}',
        '\u{202d}', '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
    ] {
        assert!(!text.contains(c), "{c:?} survived: {text:?}");
    }
    assert!(text.contains("client=bot Signed-off-by: x  evil"), "{text}");
}

#[test]
fn a_long_client_is_cut_so_no_121_character_run_survives() {
    let root = tempfile::tempdir().unwrap();
    let mut entry = of(root.path(), "a.snxsch", "color_net", NEW);
    entry.client = Some("x".repeat(500));
    journal(root.path(), vec![entry]);

    let text = message(root.path(), "a.snxsch", None);

    assert!(text.contains(&"x".repeat(120)), "{text}");
    assert!(!text.contains(&"x".repeat(121)), "{text}");
}

/// Whole seconds, strictly after the last commit's second: an edit in the
/// same second as the commit is not listed again (documented gap).
#[test]
fn an_edit_in_the_same_second_as_the_last_commit_is_not_listed() {
    let root = tempfile::tempdir().unwrap();
    journal(
        root.path(),
        vec![
            of(
                root.path(),
                "a.snxsch",
                "color_net",
                "2026-01-01T00:00:00.500Z",
            ),
            of(
                root.path(),
                "a.snxsch",
                "apply_edits",
                "2026-01-01T00:00:01.000Z",
            ),
        ],
    );

    let text = message(root.path(), "a.snxsch", Some("2026-01-01T00:00:00Z"));

    assert!(!text.contains("command=color_net"), "{text}");
    assert!(text.contains("command=apply_edits"), "{text}");
}

#[test]
fn a_project_wide_edit_naming_a_sheet_outside_the_folder_is_in_the_project_file_commit() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let mut entry = of(root.path(), "x.snxsch", "assign_net_class", NEW);
    entry.path = Some(outside.path().join("x.snxsch").display().to_string());
    journal(root.path(), vec![entry]);

    let text = message(root.path(), "p.snxprj", None);

    assert!(text.contains("command=assign_net_class"), "{text}");
    assert_eq!(message(root.path(), "x.snxsch", None), "Save it");
}

/// A document that has not been saved has no path: its edits are in no
/// commit (documented gap), and not in the project file's.
#[test]
fn an_edit_of_a_document_never_saved_is_in_no_commit() {
    let root = tempfile::tempdir().unwrap();
    let mut entry = of(root.path(), "a.snxsch", "color_net", NEW);
    entry.path = None;
    assert!(entry.document.is_some());
    journal(root.path(), vec![entry]);

    assert_eq!(message(root.path(), "p.snxprj", None), "Save it");
    assert_eq!(message(root.path(), "a.snxsch", None), "Save it");
}

/// A line the journal queue still holds is not on disk yet: the worker waits
/// for it, so the edit of an agent that has just run is listed.
#[test]
fn an_edit_still_on_the_journal_queue_is_listed() {
    let root = tempfile::tempdir().unwrap();
    let (mut app, _task) = crate::app::Signex::new();
    let line = of(root.path(), "a.snxsch", "color_net", NEW).to_line();
    let write = app.agent.journal.push(
        root.path(),
        Queued {
            journal: Journal::Edits,
            line,
            max_lines: 10_000,
        },
    );
    let mark = app.agent.journal.mark();
    let dir = root.path().to_path_buf();
    let worker = std::thread::spawn(move || {
        message_with_agent_summary(
            &dir,
            Path::new("a.snxsch"),
            "Save it",
            Ok::<_, Infallible>(None),
            &mark,
        )
    });
    // The write has not run: a worker that does not wait reads nothing.
    std::thread::sleep(std::time::Duration::from_millis(300));
    crate::app::handlers::menu::export::settle(&mut app, write);

    let text = worker.join().unwrap();

    assert!(text.contains("command=color_net"), "{text}");
}
