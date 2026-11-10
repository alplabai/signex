//! Two writers at once (a window and a headless run, or two handles) never
//! tear or lose a line: the lock is on the OS, not in the process.
#![allow(
    clippy::disallowed_methods,
    reason = "the test starts its own binary as the second writer process"
)]

use std::process::Command;

use super::{CAP, Journal, edit, read_edits};
use crate::app::agent::journal::append_line;

const PER_WRITER: u64 = 150;
const CHILD_DIR: &str = "SIGNEX_JOURNAL_CHILD_DIR";
const CHILD_ID: &str = "SIGNEX_JOURNAL_CHILD_ID";

fn append_many(dir: &std::path::Path, writer: u64) {
    for n in 0..PER_WRITER {
        let mut entry = edit("color_net", writer * 1_000 + n);
        // A long operand makes a line that a torn write would show in.
        entry.operands.insert("note".into(), "x".repeat(2_000));
        append_line(dir, Journal::Edits, &entry.to_line(), CAP).unwrap();
    }
}

/// Run by the parent below as a child process; a plain run of the suite does
/// nothing here.
#[test]
fn child_appender() {
    let (Ok(dir), Ok(id)) = (std::env::var(CHILD_DIR), std::env::var(CHILD_ID)) else {
        return;
    };
    append_many(std::path::Path::new(&dir), id.parse().unwrap());
}

#[test]
fn two_processes_and_a_thread_appending_at_once_lose_and_tear_no_line() {
    let dir = tempfile::tempdir().unwrap();
    let exe = std::env::current_exe().unwrap();
    let spawn = |id: u64| {
        Command::new(&exe)
            .args([
                "--exact",
                "app::agent::journal::tests::concurrent::child_appender",
                "--nocapture",
            ])
            .env(CHILD_DIR, dir.path())
            .env(CHILD_ID, id.to_string())
            .spawn()
            .expect("the test binary starts")
    };
    let (mut one, mut two) = (spawn(1), spawn(2));
    append_many(dir.path(), 3);
    assert!(one.wait().unwrap().success());
    assert!(two.wait().unwrap().success());

    let report = read_edits(dir.path());

    assert_eq!(report.entries.len() as u64, 3 * PER_WRITER);
    assert!(!report.partial_tail && report.damaged == 0 && report.newer == 0);
    for writer in 1..=3 {
        let mine: Vec<u64> = report
            .entries
            .iter()
            .filter_map(|e| e.revision_before)
            .filter(|r| r / 1_000 == writer)
            .collect();
        let want: Vec<u64> = (0..PER_WRITER).map(|n| writer * 1_000 + n).collect();
        assert_eq!(mine, want, "writer {writer}: every line, in its own order");
    }
}

/// The guarantee is an OS lock on the sibling file, not luck: a handle that
/// holds it (as another process would) keeps an append, and the rotation that
/// follows it, waiting until it lets go.
#[test]
fn an_append_waits_for_a_foreign_holder_of_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".signex")).unwrap();
    let holder = std::fs::File::create(dir.path().join(".signex/agent-journal.lock")).unwrap();
    holder.lock().unwrap();
    let journal = dir.path().join(".signex/agent-edits.jsonl");

    std::thread::scope(|scope| {
        let writer = scope.spawn(|| {
            append_line(
                dir.path(),
                Journal::Edits,
                &edit("color_net", 1).to_line(),
                CAP,
            )
        });
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert!(
            !journal.exists() || std::fs::metadata(&journal).unwrap().len() == 0,
            "the append went ahead without the lock"
        );
        drop(holder);
        writer.join().unwrap().unwrap();
    });

    assert_eq!(read_edits(dir.path()).entries.len(), 1);
}

#[test]
fn many_handles_in_one_process_serialise_on_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    std::thread::scope(|scope| {
        for writer in 0..6 {
            let path = dir.path();
            scope.spawn(move || append_many(path, writer));
        }
    });
    let report = read_edits(dir.path());
    assert_eq!(report.entries.len() as u64, 6 * PER_WRITER);
    assert_eq!(report.damaged, 0);
}
