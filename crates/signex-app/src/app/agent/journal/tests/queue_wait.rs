//! #2253: what a commit worker waits for on the journal queue is its own
//! project's lines, and only those, and never for ever.

use std::convert::Infallible;
use std::path::Path;
use std::time::Duration;

use super::super::Journal;
use super::super::queue::Queued;
use super::super::summary::message_with_agent_summary;
use super::edit;
use crate::app::Signex;

const NEW: &str = "2099-01-01T00:00:00.000Z";

fn line(dir: &Path, file: &str) -> Queued {
    let mut entry = edit("color_net", 1);
    entry.path = Some(dir.join(file).display().to_string());
    entry.ts = NEW.into();
    Queued {
        journal: Journal::Edits,
        line: entry.to_line(),
        max_lines: 10_000,
    }
}

/// P's line is queued and not written; Q's line is written first. Q's write
/// must not release the wait on P's mark.
#[test]
fn a_later_line_of_another_project_does_not_release_the_wait() {
    let p = tempfile::tempdir().unwrap();
    let q = tempfile::tempdir().unwrap();
    let (mut app, _task) = Signex::new();
    let write_p = app.agent.journal.push(p.path(), line(p.path(), "a.snxsch"));
    let mark_p = app.agent.journal.mark(p.path());
    let write_q = app.agent.journal.push(q.path(), line(q.path(), "b.snxsch"));
    let dir = p.path().to_path_buf();
    let worker = std::thread::spawn(move || {
        message_with_agent_summary(
            &dir,
            Path::new("a.snxsch"),
            "Save it",
            Ok::<_, Infallible>(None),
            &mark_p,
        )
    });
    std::thread::sleep(Duration::from_millis(300));

    crate::app::handlers::menu::export::settle(&mut app, write_q);
    // A worker released by Q's write would read P's empty journal by now.
    std::thread::sleep(Duration::from_millis(300));
    crate::app::handlers::menu::export::settle(&mut app, write_p);

    let text = worker.join().unwrap();
    assert!(text.contains("command=color_net"), "{text}");
}

/// A write of Q that has not run (or is slow) does not delay P's commit.
#[test]
fn an_unrun_write_in_another_project_does_not_delay_a_commit() {
    let p = tempfile::tempdir().unwrap();
    let q = tempfile::tempdir().unwrap();
    let (mut app, _task) = Signex::new();
    let write_p = app.agent.journal.push(p.path(), line(p.path(), "a.snxsch"));
    crate::app::handlers::menu::export::settle(&mut app, write_p);
    let _unrun = app.agent.journal.push(q.path(), line(q.path(), "b.snxsch"));
    let mark_p = app.agent.journal.mark(p.path());

    assert!(
        mark_p.wait(Duration::from_millis(200)),
        "P's wait ran into the limit because of Q's line"
    );
}

/// A drain that never finishes its write (a panic, or a task dropped unrun)
/// still counts its lines as ended, or every later commit would wait the
/// whole limit.
#[test]
fn a_drain_dropped_without_writing_does_not_hold_the_wait() {
    let p = tempfile::tempdir().unwrap();
    let (mut app, _task) = Signex::new();
    let write = app.agent.journal.push(p.path(), line(p.path(), "a.snxsch"));
    let mark = app.agent.journal.mark(p.path());

    drop(write);

    assert!(mark.wait(Duration::from_millis(200)));
}
