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

#[test]
fn a_gap_in_the_slots_does_not_hide_a_newer_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let path = live_file(&tmp);
    std::fs::write(slot_path(&path, 1).unwrap(), b"a").unwrap();
    let third = slot_path(&path, 3).unwrap();
    std::fs::write(&third, b"c").unwrap();

    assert_eq!(newest_backup_at(&path).unwrap(), Some(third));
}

#[test]
fn no_backup_means_none() {
    let tmp = tempfile::tempdir().unwrap();
    let path = live_file(&tmp);
    assert_eq!(newest_backup_at(&path).unwrap(), None);
}
