//! #1378 — restore and discard must target the newest backup slot.

use super::*;
use crate::keymap::config_path_for_dir;

fn live_file(tmp: &tempfile::TempDir) -> PathBuf {
    let path = config_path_for_dir(tmp.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"live").unwrap();
    path
}

#[test]
fn the_newest_backup_is_the_highest_slot_not_the_plain_bak() {
    let tmp = tempfile::tempdir().unwrap();
    let path = live_file(&tmp);
    let first = back_up_profile_file_at(&path).unwrap().unwrap();
    let second = back_up_profile_file_at(&path).unwrap().unwrap();
    assert_ne!(first, second);

    // The status line named `second`; Restore/Discard must offer it too,
    // not the older `.bak` beside it.
    assert_eq!(newest_backup_at(&path).unwrap(), Some(second));
}

#[test]
fn discarding_the_newest_leaves_the_older_backups_and_exposes_the_next_newest() {
    let tmp = tempfile::tempdir().unwrap();
    let path = live_file(&tmp);
    let first = back_up_profile_file_at(&path).unwrap().unwrap();
    let second = back_up_profile_file_at(&path).unwrap().unwrap();

    assert!(discard_profile_backup_at(&second).unwrap());

    assert!(first.exists(), "an older backup must survive a discard");
    assert_eq!(newest_backup_at(&path).unwrap(), Some(first));
}

fn write_at(path: &Path, age_secs: u64) {
    std::fs::write(path, b"x").unwrap();
    let when = std::time::SystemTime::now() - std::time::Duration::from_secs(age_secs);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

// `.bak.2` was deleted outside the app, so the next backup refilled it: the
// newest file is in a LOW slot while a higher one is older.
#[test]
fn a_refilled_gap_wins_over_a_higher_but_older_slot() {
    let tmp = tempfile::tempdir().unwrap();
    let path = live_file(&tmp);
    write_at(&slot_path(&path, 1).unwrap(), 300);
    write_at(&slot_path(&path, 3).unwrap(), 200);
    let refilled = slot_path(&path, 2).unwrap();
    write_at(&refilled, 10);

    assert_eq!(newest_backup_at(&path).unwrap(), Some(refilled));
}

#[test]
fn equal_modification_times_fall_back_to_the_higher_slot() {
    let tmp = tempfile::tempdir().unwrap();
    let path = live_file(&tmp);
    let one = slot_path(&path, 1).unwrap();
    let three = slot_path(&path, 3).unwrap();
    write_at(&one, 50);
    std::fs::write(&three, b"x").unwrap();
    let t = std::fs::metadata(&one).unwrap().modified().unwrap();
    std::fs::File::options()
        .write(true)
        .open(&three)
        .unwrap()
        .set_modified(t)
        .unwrap();

    assert_eq!(newest_backup_at(&path).unwrap(), Some(three));
}

#[test]
fn unrelated_files_beside_the_config_are_not_backups() {
    let tmp = tempfile::tempdir().unwrap();
    let path = live_file(&tmp);
    let dir = path.parent().unwrap();
    for stray in [
        "keyboard_shortcuts.toml.bak.x",
        "keyboard_shortcuts.toml.bak.1",
        "keyboard_shortcuts.toml.bak.101",
        "other.toml.bak",
    ] {
        std::fs::write(dir.join(stray), b"x").unwrap();
    }
    assert_eq!(newest_backup_at(&path).unwrap(), None);
}

#[test]
fn no_backup_means_none() {
    let tmp = tempfile::tempdir().unwrap();
    let path = live_file(&tmp);
    assert_eq!(newest_backup_at(&path).unwrap(), None);
}

// #1378 — Messages cuts a line at 157 bytes, so the reason must come
// before the (long) config path or it is the reason that gets cut.
#[test]
fn a_full_ladder_error_names_the_reason_before_the_path() {
    let tmp = tempfile::tempdir().unwrap();
    let path = live_file(&tmp);
    for slot in 1..=MAX_ASIDE_SLOTS {
        std::fs::write(slot_path(&path, slot).unwrap(), b"x").unwrap();
    }

    let message = back_up_profile_file_at(&path).unwrap_err().to_string();

    assert!(
        message.starts_with("all 100 .bak slots are taken"),
        "{message}"
    );
    let reason_end = message.find(" beside ").expect("path follows the reason");
    assert!(reason_end < 40, "the reason must be short: {message}");
}

// A source file older than the existing backup (restored from a sync
// folder, say) must not make the new copy rank as the older one: the copy
// is ranked by when it was written, whatever mtime `fs::copy` carried over
// (Windows keeps the source's).
#[test]
fn a_backup_of_an_older_source_still_ranks_newest() {
    let tmp = tempfile::tempdir().unwrap();
    let path = live_file(&tmp);
    let existing = slot_path(&path, 1).unwrap();
    write_at(&existing, 600);
    write_at(&path, 3600);

    let fresh = back_up_profile_file_at(&path).unwrap().unwrap();

    assert_ne!(fresh, existing, "the existing backup is never overwritten");
    assert_eq!(newest_backup_at(&path).unwrap(), Some(fresh));
}
