//! Windows: an adopted output folder is held from the claim on and emptied
//! without ever deleting through a junction (#3123). The Unix twins are in
//! `output_tests.rs`.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use super::empty_dir::{FolderHold, empty_folder};
use super::output::{ClaimedOutput, claim_output};
use super::run::{LibraryRunRequest, ModelsMode, RunMode};

fn claim(output: &Path) -> ClaimedOutput {
    claim_output(&LibraryRunRequest {
        inputs: vec![],
        output: output.to_path_buf(),
        name: None,
        db: None,
        search_paths: vec![],
        models: ModelsMode::Include,
        mode: RunMode::Import,
    })
    .unwrap()
}

/// `mklink /J link target`; false when the system would not make it.
fn junction(link: &Path, target: &Path) -> bool {
    #[allow(
        clippy::disallowed_methods,
        reason = "a test makes a junction with the system's own tool"
    )]
    let mut command = std::process::Command::new("cmd");
    let status = command
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(status, Ok(status) if status.success())
}

/// A victim folder holding `precious.txt`, and the path of that file.
fn victim(dir: &Path) -> (PathBuf, PathBuf) {
    let victim = dir.join("victim");
    std::fs::create_dir(&victim).unwrap();
    let precious = victim.join("precious.txt");
    std::fs::write(&precious, "keep").unwrap();
    (victim, precious)
}

/// While the run holds the adopted folder nobody can rename it; once the
/// run lets go, they can.
#[test]
fn the_held_folder_cannot_be_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("Mine");
    std::fs::create_dir(&out).unwrap();
    let claimed = claim(&out);
    let moved = dir.path().join("moved");
    assert!(
        std::fs::rename(&out, &moved).is_err(),
        "held, so not renamed"
    );
    drop(claimed);
    std::fs::rename(&out, &moved).unwrap();
}

/// A junction at the folder's path after the hold ended is refused, and so
/// is another real folder there: nothing in either is deleted. (While the
/// hold lasts the swap itself fails, `the_held_folder_cannot_be_renamed`.)
#[test]
fn a_folder_swapped_after_the_hold_deletes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (victim, precious) = victim(dir.path());
    let out = dir.path().join("Mine");
    std::fs::create_dir(&out).unwrap();
    let id = FolderHold::open(&out).unwrap().id();
    std::fs::write(out.join("half.snxsym"), "x").unwrap();
    std::fs::rename(&out, dir.path().join("moved")).unwrap();
    // Another real folder at the path is not the one recorded.
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join("theirs.txt"), "keep").unwrap();
    assert!(empty_folder(&out, &id).is_err());
    assert!(out.join("theirs.txt").exists());
    std::fs::remove_file(out.join("theirs.txt")).unwrap();
    std::fs::remove_dir(&out).unwrap();
    if !junction(&out, &victim) {
        eprintln!("skipped the junction half: mklink /J failed");
        return;
    }
    assert!(empty_folder(&out, &id).is_err());
    assert!(precious.exists(), "nothing deleted through the junction");
    assert!(dir.path().join("moved/half.snxsym").exists());
}

/// A junction the converter left inside the folder is removed as a
/// junction; its target and what is in it stay.
#[test]
fn a_junction_inside_the_folder_is_removed_and_its_target_kept() {
    let dir = tempfile::tempdir().unwrap();
    let (victim, precious) = victim(dir.path());
    let out = dir.path().join("Mine");
    std::fs::create_dir(&out).unwrap();
    let claimed = claim(&out);
    std::fs::write(out.join("half.snxsym"), "x").unwrap();
    std::fs::create_dir(out.join("sub")).unwrap();
    std::fs::write(out.join("sub/part.snxsym"), "x").unwrap();
    if !junction(&out.join("link"), &victim) {
        eprintln!("skipped: mklink /J failed");
        return;
    }
    assert!(claimed.release_after_failure());
    assert!(out.is_dir(), "the adopted folder is kept");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0, "emptied");
    assert!(precious.exists(), "the junction's target is untouched");
}
