//! A planted link, folder or device is a refusal that is reported: nothing is
//! written through it and the target is untouched.

use super::{CAP, Journal, edit};
use crate::app::agent::journal::append_line;

fn append(dir: &std::path::Path) -> Result<(), String> {
    append_line(dir, Journal::Edits, &edit("a", 1).to_line(), CAP)
}

#[cfg(unix)]
#[test]
fn a_journal_that_is_a_symlink_is_refused_and_its_target_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("victim.txt");
    std::fs::write(&target, "keep me\n").unwrap();
    std::fs::create_dir(dir.path().join(".signex")).unwrap();
    std::os::unix::fs::symlink(&target, dir.path().join(".signex/agent-edits.jsonl")).unwrap();

    let refused = append(dir.path()).unwrap_err();

    assert!(refused.contains("agent-edits.jsonl"), "{refused}");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me\n");
}

/// A hard link to a file that lives elsewhere is a plain file with another
/// name: appending to it would write into the other file. Refused on the link
/// count, the lock file included, and the target is untouched.
#[test]
fn a_journal_that_is_a_hard_link_is_refused_and_its_target_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("victim.txt");
    std::fs::write(&target, "keep me\n").unwrap();
    std::fs::create_dir(dir.path().join(".signex")).unwrap();
    std::fs::hard_link(&target, dir.path().join(".signex/agent-edits.jsonl")).unwrap();

    let refused = append(dir.path()).unwrap_err();

    assert!(refused.contains("links"), "{refused}");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me\n");
}

#[test]
fn a_lock_file_that_is_a_hard_link_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("lock-target");
    std::fs::write(&target, "x").unwrap();
    std::fs::create_dir(dir.path().join(".signex")).unwrap();
    std::fs::hard_link(&target, dir.path().join(".signex/agent-journal.lock")).unwrap();

    assert!(append(dir.path()).unwrap_err().contains("links"));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "x");
}

/// The reader refuses a hard link too: the viewer never prints another file.
#[test]
fn a_hard_linked_journal_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("secret.txt");
    std::fs::write(&target, "not a journal\n").unwrap();
    std::fs::create_dir(dir.path().join(".signex")).unwrap();
    std::fs::hard_link(&target, dir.path().join(".signex/agent-edits.jsonl")).unwrap();

    let read = crate::app::agent::journal::read_journal::<crate::app::agent::journal::EditEntry>(
        &dir.path().join(".signex/agent-edits.jsonl"),
    );

    assert!(read.unwrap_err().contains("links"));
}

#[cfg(unix)]
#[test]
fn a_signex_folder_that_is_a_symlink_is_refused_and_nothing_is_created_through_it() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join(".signex")).unwrap();

    let refused = append(dir.path()).unwrap_err();

    assert!(refused.contains("link or not a folder"), "{refused}");
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn a_lock_file_that_is_a_symlink_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("lock-target");
    std::fs::write(&target, "x").unwrap();
    std::fs::create_dir(dir.path().join(".signex")).unwrap();
    std::os::unix::fs::symlink(&target, dir.path().join(".signex/agent-journal.lock")).unwrap();

    assert!(append(dir.path()).is_err());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "x");
}

#[test]
fn a_journal_that_is_a_folder_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".signex/agent-edits.jsonl")).unwrap();

    let refused = append(dir.path()).unwrap_err();

    assert!(
        refused.contains("not a regular file") || refused.contains("cannot open"),
        "{refused}"
    );
}

#[cfg(unix)]
#[test]
fn a_journal_that_is_a_named_pipe_is_refused_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".signex")).unwrap();
    let fifo = std::ffi::CString::new(
        dir.path()
            .join(".signex/agent-edits.jsonl")
            .to_string_lossy()
            .as_bytes(),
    )
    .unwrap();
    // SAFETY: `fifo` is a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);

    assert!(
        append(dir.path())
            .unwrap_err()
            .contains("not a regular file")
    );
}

#[test]
fn a_signex_name_that_is_a_file_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".signex"), "a file").unwrap();
    assert!(append(dir.path()).is_err());
}

#[cfg(unix)]
#[test]
fn a_read_only_folder_is_an_error_not_a_panic_and_creates_nothing() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    let can_still_write = std::fs::File::create(dir.path().join("probe")).is_ok();
    let result = append(dir.path());
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    if can_still_write {
        eprintln!("skipped: directory permissions are not enforced here (running as root?)");
        return;
    }
    assert!(result.unwrap_err().contains("cannot create"));
}
