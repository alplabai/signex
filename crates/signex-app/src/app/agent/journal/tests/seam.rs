//! The folder handle is the seam: files are opened through the `.signex/`
//! folder as it was opened, not through its path, so a swap of the folder
//! after the check changes nothing (#2253).

use super::super::append::append_line_in;
use super::super::safe_fs::SignexDir;
use super::{CAP, Journal, edit};

#[cfg(unix)]
#[test]
fn a_folder_swapped_for_a_link_after_it_was_opened_leaves_the_outside_folder_empty() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let signex = SignexDir::create(project.path()).unwrap();

    // After the folder was checked and opened: move it away and plant a link
    // to the outside folder under its name.
    std::fs::rename(project.path().join(".signex"), project.path().join("moved")).unwrap();
    std::os::unix::fs::symlink(outside.path(), project.path().join(".signex")).unwrap();
    append_line_in(
        &signex,
        Journal::Edits,
        &edit("color_net", 1).to_line(),
        CAP,
    )
    .unwrap();
    append_line_in(
        &signex,
        Journal::Access,
        &super::access("accepted").to_line(),
        CAP,
    )
    .unwrap();

    assert_eq!(
        std::fs::read_dir(outside.path()).unwrap().count(),
        0,
        "something was written through the link"
    );
    let moved = project.path().join("moved");
    assert!(moved.join("agent-edits.jsonl").is_file());
    assert!(moved.join("agent-access.jsonl").is_file());
    assert!(moved.join("agent-journal.lock").is_file());
}

#[cfg(unix)]
#[test]
fn a_rotation_goes_through_the_same_handle() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let signex = SignexDir::create(project.path()).unwrap();
    std::fs::rename(project.path().join(".signex"), project.path().join("moved")).unwrap();
    std::os::unix::fs::symlink(outside.path(), project.path().join(".signex")).unwrap();

    for revision in 0..130 {
        append_line_in(
            &signex,
            Journal::Edits,
            &edit("color_net", revision).to_line(),
            100,
        )
        .unwrap();
    }

    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    let kept = std::fs::read_to_string(project.path().join("moved/agent-edits.jsonl")).unwrap();
    assert!(
        kept.lines().count() <= 100,
        "rotated inside the real folder"
    );
}
