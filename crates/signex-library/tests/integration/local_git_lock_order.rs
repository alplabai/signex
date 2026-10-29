//! Lock order and fairness of a library write (#858, #893).
//!
//! The order is documented in `write_lock.rs`: write queue, write lock,
//! `git_lock`, repository commit lock. These tests hold one lock from the
//! test thread and assert the other path neither waits for it nor starves:
//! a path that took the two in the wrong order would sit out its 5 s wait and
//! fail here instead of deadlocking CI.

#![cfg(feature = "local-git")]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use signex_library::adapter::{CommitOutcome, LibraryAdapter};
use signex_library::adapters::local_git_project::LocalGitProjectAdapter;

use super::local_git_rollback::{class, init};

/// Long enough to be a hang, short of the 5 s lock wait.
const FAST: Duration = Duration::from_millis(2000);

fn hold(path: &Path) -> std::fs::File {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let handle = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .unwrap();
    handle.lock().unwrap();
    handle
}

/// A committer that needs no write lock never waits for one: it must not take
/// it while holding the commit lock.
#[test]
fn commits_without_a_write_lock_ignore_a_held_write_lock() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, adapter) = init(&dir);
    let root = file.parent().unwrap().to_path_buf();
    let _write_lock = hold(&signex_library::write_lock_path(&file));
    std::fs::write(root.join("a.txt"), "a").unwrap();
    std::fs::write(root.join("b.txt"), "b").unwrap();

    let started = Instant::now();
    let outcome = adapter
        .commit_external_change(&root.join("a.txt"), "a")
        .unwrap();
    assert_eq!(outcome, CommitOutcome::Committed);
    LocalGitProjectAdapter::open_or_init(root.clone())
        .unwrap()
        .commit_path(Path::new("b.txt"), "b")
        .unwrap();
    assert!(started.elapsed() < FAST, "{:?}", started.elapsed());
}

/// A write-locked path (restoring a file of the library) never waits for the
/// commit lock: nothing is committed while the write lock is held for it.
#[test]
fn a_restore_ignores_a_held_commit_lock() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, _adapter) = init(&dir);
    let root = file.parent().unwrap().to_path_buf();
    let project = LocalGitProjectAdapter::open_or_init(root.clone()).unwrap();
    std::fs::write(root.join("doc.txt"), "one").unwrap();
    let first = project
        .commit_path(Path::new("doc.txt"), "one")
        .unwrap()
        .unwrap();
    std::fs::write(root.join("doc.txt"), "two").unwrap();
    project.commit_path(Path::new("doc.txt"), "two").unwrap();

    let _commit_lock = hold(&root.join(".git").join("signex").join("commit.lock"));
    let started = Instant::now();
    let bytes = project.restore_at(Path::new("doc.txt"), first).unwrap();
    assert_eq!(bytes, b"one");
    assert!(started.elapsed() < FAST, "{:?}", started.elapsed());
}

/// A library save needs the write lock before the commit lock: with the write
/// lock held elsewhere it waits for that one, and with only the commit lock
/// held it waits for that one, never for both in a cycle. Releasing either
/// lets it finish.
#[test]
fn a_save_takes_the_write_lock_before_the_commit_lock() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, adapter) = init(&dir);
    let root = file.parent().unwrap().to_path_buf();
    let commit_lock = hold(&root.join(".git").join("signex").join("commit.lock"));
    let saver = std::thread::spawn(move || adapter.add_library_class(class("x"), "add x"));
    // The save has taken the write lock and waits for the commit lock: the
    // write lock is not available to a third party.
    std::thread::sleep(Duration::from_millis(300));
    let probe = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(signex_library::write_lock_path(&file))
        .unwrap();
    assert!(probe.try_lock().is_err(), "the save holds the write lock");
    drop(probe);
    drop(commit_lock);
    saver.join().unwrap().unwrap();
}

/// A writer that takes the lock again and again cannot starve another: the
/// queued writer gets the next turn instead of waiting out its 5 s behind a
/// streak. The streak holds the lock for 100 ms a time and leaves only the
/// reopening of the lock file between two turns, like a slow disk does.
#[test]
fn a_writer_taking_turns_in_a_loop_cannot_starve_another_writer() {
    let dir = tempfile::tempdir().unwrap();
    let (file, _, _adapter) = init(&dir);
    let target = file.parent().unwrap().join("symbols").join("pair.snxsym");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let turns = Arc::new(AtomicUsize::new(0));
    let looping = {
        let (stop, turns, file, target) =
            (stop.clone(), turns.clone(), file.clone(), target.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                turns.fetch_add(1, Ordering::SeqCst);
                signex_library::rewrite_file_under_library_lock(&file, &target, &mut |_| {
                    std::thread::sleep(Duration::from_millis(100));
                    Ok(None)
                })
                .unwrap();
            }
        })
    };
    // Let the streak settle before the other writer asks.
    while turns.load(Ordering::SeqCst) < 3 {
        std::thread::sleep(Duration::from_millis(5));
    }

    let started = Instant::now();
    let other = signex_library::rewrite_file_under_library_lock(&file, &target, &mut |_| Ok(None));
    let waited = started.elapsed();
    stop.store(true, Ordering::SeqCst);
    looping.join().unwrap();

    other.expect("the second writer is not starved");
    assert!(waited < FAST, "waited {waited:?} behind the streak");
}
