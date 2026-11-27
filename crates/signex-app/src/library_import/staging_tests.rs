//! A staged preview's folder (#3171): created fresh beside the target,
//! never adopted, discarded without following a link, and moved into
//! place only while the target is as it was when the preview was staged.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::error::ImportError;
use super::events::{Counts, ExitKind, Summary};
use super::output::claim_output;
use super::run::{LibraryRunRequest, ModelsMode, RunMode, RunOutcome};
use super::place::{HeldDir, copy_across, rename_entry, rename_no_replace};
use super::staging::{
    SPLIT_MARK, STAGING_MARK, StagedLibrary, TargetBefore, move_entry, staging_dir_for,
    sweep_stale,
};

fn request(output: PathBuf, mode: RunMode) -> LibraryRunRequest {
    LibraryRunRequest {
        inputs: vec![],
        output,
        name: None,
        db: None,
        search_paths: vec![],
        models: ModelsMode::Include,
        mode,
    }
}

fn run_in(dir: &Path) -> RunOutcome {
    RunOutcome {
        exit: ExitKind::Ok,
        summary: Summary {
            ok: true,
            error: None,
            exit: ExitKind::Ok,
            library: Some(dir.to_path_buf()),
            report: Some(dir.join("import-report.json")),
            files: vec![dir.join("P.snxlib")],
            counts: Counts::default(),
            warnings: Vec::new(),
        },
        library: Some(dir.to_path_buf()),
        report: None,
        report_problem: None,
        stderr_tail: String::new(),
    }
}

/// A staged preview of `target` holding one library file, as a run left it.
fn staged(target: &Path) -> StagedLibrary {
    let target_before = TargetBefore::read(target).unwrap();
    let staging = staging_dir_for(target).unwrap();
    let claimed = claim_output(&request(staging.clone(), RunMode::Stage)).unwrap();
    std::fs::create_dir(staging.join("symbols")).unwrap();
    std::fs::write(staging.join("P.snxlib"), "library").unwrap();
    std::fs::write(staging.join("symbols").join("R.snxsym"), "symbol").unwrap();
    StagedLibrary {
        claimed,
        target: target.to_path_buf(),
        target_before,
        request: request(target.to_path_buf(), RunMode::Stage),
        run: run_in(&staging),
        library_file: staging.join("P.snxlib"),
    }
}

#[test]
fn the_staging_folder_sits_beside_the_target_and_is_never_adopted() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Passives");
    let staging = staging_dir_for(&target).unwrap();
    assert_eq!(staging.parent(), Some(dir.path()), "same parent");
    let name = staging.file_name().unwrap().to_str().unwrap();
    assert!(name.starts_with(".Passives.signex-preview-"), "{name}");
    assert!(name.contains(STAGING_MARK));
    // An empty folder at the name is refused, not adopted.
    std::fs::create_dir(&staging).unwrap();
    assert_eq!(
        claim_output(&request(staging.clone(), RunMode::Stage)),
        Err(ImportError::StagingExists {
            path: staging.clone()
        })
    );
    assert!(staging.is_dir(), "the refusal removes nothing");
}

#[test]
fn a_staged_preview_moves_into_place_with_its_paths() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("libs").join("Passives");
    std::fs::create_dir(dir.path().join("libs")).unwrap();
    let staged = staged(&target);
    let staging = staged.dir().to_path_buf();
    staged.move_into_place().unwrap();
    assert!(!staging.exists(), "moved, not copied");
    assert_eq!(
        std::fs::read_to_string(target.join("symbols").join("R.snxsym")).unwrap(),
        "symbol"
    );
    let run = staged.run_in_target();
    assert_eq!(run.library.as_deref(), Some(target.as_path()));
    assert_eq!(run.summary.report, Some(target.join("import-report.json")));
    assert_eq!(run.summary.files, vec![target.join("P.snxlib")]);
    assert_eq!(staged.library_file_in_target(), target.join("P.snxlib"));
}

/// The identity `claim_output` and `TargetBefore` record for `dir`.
fn id_of(dir: &Path) -> super::empty_dir::FolderId {
    super::empty_dir::folder_id_at(dir).unwrap()
}

/// An absent target is the staging folder itself, renamed.
#[test]
fn an_absent_target_is_the_staging_folder_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Passives");
    let staged = staged(&target);
    let staging_id = id_of(staged.dir());
    staged.move_into_place().unwrap();
    assert_eq!(id_of(&target), staging_id, "the same folder, renamed");
    assert!(!staged.dir().exists());
}

/// An empty folder the user chose is kept as it is, with its identity and
/// permissions; the staged entries are moved into it and the emptied
/// staging folder goes.
#[test]
fn an_empty_target_folder_is_kept_and_receives_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Passives");
    std::fs::create_dir(&target).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::Permissions::from_mode(0o750);
        std::fs::set_permissions(&target, mode).unwrap();
    }
    let before = id_of(&target);
    let staged = staged(&target);
    assert_eq!(staged.target_before, TargetBefore::Empty(before));
    staged.move_into_place().unwrap();
    assert_eq!(id_of(&target), before, "the user's folder is kept");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o750, "with its permissions");
    }
    assert!(target.join("P.snxlib").is_file());
    assert_eq!(
        std::fs::read_to_string(target.join("symbols").join("R.snxsym")).unwrap(),
        "symbol"
    );
    assert!(!staged.dir().exists(), "the emptied staging folder goes");
}

/// A move into an empty target that fails half way moves back what it
/// moved: the target is empty again and the preview whole, and the error
/// names the entry that did not move.
#[test]
fn a_move_that_fails_half_way_leaves_the_target_empty_and_the_preview_whole() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Passives");
    std::fs::create_dir(&target).unwrap();
    let staged = staged(&target);
    let mut calls = 0;
    let mut failing = |from: &HeldDir, to: &HeldDir, name: &OsStr| {
        calls += 1;
        if calls == 2 {
            return Err(std::io::Error::other("injected"));
        }
        move_entry(from, to, name)
    };
    let failure = staged.move_into_place_with(&mut failing).unwrap_err();
    assert!(failure.preview_kept);
    let ImportError::MoveFailed { path, detail } = &failure.error else {
        panic!("{:?}", failure.error);
    };
    assert_eq!(path.parent(), Some(target.as_path()), "names the entry");
    assert!(detail.contains("injected"), "{detail}");
    assert_eq!(
        std::fs::read_dir(&target).unwrap().count(),
        0,
        "empty again"
    );
    assert!(staged.dir().join("P.snxlib").is_file());
    assert_eq!(
        std::fs::read_to_string(staged.dir().join("symbols").join("R.snxsym")).unwrap(),
        "symbol"
    );
    // The preview kept, a later try moves it in.
    staged.move_into_place().unwrap();
    assert!(target.join("P.snxlib").is_file());
}

/// The folder check runs again just before the move: a target that
/// appeared, or got a file, since the preview refuses the move and the
/// preview is kept for another try.
#[test]
fn a_target_created_or_changed_after_the_preview_refuses_the_move() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Passives");
    let appeared = staged(&target);
    std::fs::create_dir(&target).unwrap();
    let failure = appeared.move_into_place().unwrap_err();
    assert_eq!(
        failure.error,
        ImportError::TargetChanged {
            path: target.clone()
        }
    );
    assert!(failure.preview_kept);
    assert!(
        appeared.dir().join("P.snxlib").is_file(),
        "the preview stays"
    );
    assert!(target.is_dir(), "what appeared stays");
    std::fs::remove_dir(&target).unwrap();

    std::fs::create_dir(&target).unwrap();
    let filled = staged(&target);
    std::fs::write(target.join("mine.txt"), "keep").unwrap();
    let failure = filled.move_into_place().unwrap_err();
    assert!(failure.preview_kept);
    assert_eq!(
        std::fs::read_to_string(target.join("mine.txt")).unwrap(),
        "keep"
    );
    assert!(filled.dir().join("P.snxlib").is_file());
    assert!(appeared.discard() && filled.discard());
}

#[test]
fn discard_removes_the_staging_folder_and_the_parents_it_made() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("new").join("Passives");
    let staged = staged(&target);
    assert_eq!(
        staged.claimed.created_parent,
        Some(dir.path().join("new")),
        "the parent was made for the preview"
    );
    assert!(staged.discard());
    assert!(!staged.dir().exists());
    assert!(
        !dir.path().join("new").exists(),
        "its empty parent goes too"
    );
    assert!(staged.discard(), "a second discard finds nothing to do");
}

#[cfg(unix)]
#[test]
fn discard_never_follows_a_link_out_of_the_staging_folder() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep.txt"), "keep").unwrap();
    let staged = staged(&dir.path().join("Passives"));
    std::os::unix::fs::symlink(&outside, staged.dir().join("link")).unwrap();
    assert!(staged.discard());
    assert_eq!(
        std::fs::read_to_string(outside.join("keep.txt")).unwrap(),
        "keep"
    );
}

/// A move that fails half way and whose move back fails too leaves the
/// library split: the preview is not kept for another try, the error names
/// both folders, and the staging folder is kept (renamed out of the
/// staging pattern, so no sweep removes it), never discarded.
#[test]
fn a_move_back_that_fails_reports_a_split_library_and_keeps_both_folders() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Passives");
    std::fs::create_dir(&target).unwrap();
    let staged = staged(&target);
    let mut calls = 0;
    // The first entry moves, the second fails, moving the first back fails.
    let mut failing = |from: &HeldDir, to: &HeldDir, name: &OsStr| {
        calls += 1;
        if calls >= 2 {
            return Err(std::io::Error::other("injected"));
        }
        move_entry(from, to, name)
    };
    let failure = staged.move_into_place_with(&mut failing).unwrap_err();
    assert!(!failure.preview_kept, "no whole preview to try again");
    let ImportError::LibrarySplit {
        target: named,
        staging,
        detail,
    } = &failure.error
    else {
        panic!("{:?}", failure.error);
    };
    assert_eq!(named, &target);
    assert!(detail.contains("injected"), "{detail}");
    let kept_name = staging.file_name().unwrap().to_str().unwrap();
    assert!(kept_name.contains(SPLIT_MARK), "{kept_name}");
    assert_eq!(std::fs::read_dir(&target).unwrap().count(), 1, "one entry moved");
    assert_eq!(std::fs::read_dir(staging).unwrap().count(), 1, "the other kept");
    assert!(!staged.dir().exists(), "renamed, not removed");
    let swept = sweep_stale(&[dir.path().to_path_buf()], far_future());
    assert_eq!(swept, 0, "a split library is never swept");
    assert!(staging.is_dir());
}

/// Across file systems an entry is copied then its source removed; once
/// the copy is whole the entry counts as moved even when the source
/// cannot be removed, so it is not left in both places as a failure.
#[test]
fn a_whole_copy_counts_as_moved_when_its_source_stays() {
    let dir = tempfile::tempdir().unwrap();
    let (from, to) = (dir.path().join("from"), dir.path().join("to"));
    std::fs::create_dir_all(from.join("symbols")).unwrap();
    std::fs::write(from.join("symbols").join("R.snxsym"), "symbol").unwrap();
    let mut stuck = |_: &Path| -> std::io::Result<()> { Err(std::io::Error::other("busy")) };
    copy_across(&from, &to, &mut stuck).unwrap();
    assert_eq!(
        std::fs::read_to_string(to.join("symbols").join("R.snxsym")).unwrap(),
        "symbol"
    );
    assert!(from.join("symbols").join("R.snxsym").is_file(), "left, logged");
    // A file copy never replaces what is at its destination.
    std::fs::write(dir.path().join("a.txt"), "new").unwrap();
    std::fs::write(dir.path().join("b.txt"), "theirs").unwrap();
    let mut remove = |p: &Path| std::fs::remove_file(p);
    assert!(copy_across(&dir.path().join("a.txt"), &dir.path().join("b.txt"), &mut remove).is_err());
    assert_eq!(std::fs::read_to_string(dir.path().join("b.txt")).unwrap(), "theirs");
}

/// No rename replaces what appeared at its destination: an empty folder,
/// or a file, is refused and left as it is.
#[test]
fn a_rename_never_replaces_what_appeared_at_its_destination() {
    let dir = tempfile::tempdir().unwrap();
    let (staging, target) = (dir.path().join("staging"), dir.path().join("target"));
    std::fs::create_dir_all(staging.join("sub")).unwrap();
    std::fs::write(staging.join("sub").join("f"), "x").unwrap();
    std::fs::write(staging.join("P.snxlib"), "library").unwrap();
    std::fs::create_dir_all(target.join("sub")).unwrap();
    std::fs::write(target.join("P.snxlib"), "theirs").unwrap();
    let (from, to) = (HeldDir::open(&staging).unwrap(), HeldDir::open(&target).unwrap());
    for name in ["sub", "P.snxlib"] {
        let refused = rename_entry(&from, &to, OsStr::new(name)).unwrap_err();
        #[cfg(unix)]
        assert_eq!(refused.kind(), std::io::ErrorKind::AlreadyExists, "{name}");
        #[cfg(not(unix))]
        drop(refused);
    }
    assert_eq!(std::fs::read_to_string(target.join("P.snxlib")).unwrap(), "theirs");
    assert_eq!(std::fs::read_dir(target.join("sub")).unwrap().count(), 0);
    assert!(staging.join("sub").join("f").is_file());
    drop((from, to));
    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    assert!(rename_no_replace(&staging, &empty).is_err(), "an empty folder is not replaced");
    assert!(staging.join("P.snxlib").is_file() && empty.is_dir());
}

/// Unix: a link swapped in at the held target's path is never written
/// through; the rename goes into the folder that was held.
#[cfg(unix)]
#[test]
fn a_link_swapped_in_at_the_held_target_is_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let (staging, target) = (dir.path().join("staging"), dir.path().join("target"));
    let (moved_away, outside) = (dir.path().join("moved"), dir.path().join("outside"));
    for d in [&staging, &target, &outside] {
        std::fs::create_dir(d).unwrap();
    }
    std::fs::write(staging.join("P.snxlib"), "library").unwrap();
    let (from, to) = (HeldDir::open(&staging).unwrap(), HeldDir::open(&target).unwrap());
    std::fs::rename(&target, &moved_away).unwrap();
    std::os::unix::fs::symlink(&outside, &target).unwrap();
    rename_entry(&from, &to, OsStr::new("P.snxlib")).unwrap();
    assert!(moved_away.join("P.snxlib").is_file(), "into the held folder");
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0, "never through the link");
    assert!(HeldDir::open(&target).is_err(), "a link is refused as a folder to hold");
}

/// Far enough ahead that every folder made in a test counts as older than
/// the session.
fn far_future() -> SystemTime {
    SystemTime::now() + Duration::from_secs(3600)
}

/// Opening the dialog sweeps the staging folders a crash left: only
/// folders named with the staging mark and older than the session, never
/// a split library's, another folder, a file or a link.
#[test]
fn stale_staging_folders_are_swept_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let stale = root.join(format!(".Passives{STAGING_MARK}0001"));
    std::fs::create_dir_all(stale.join("symbols")).unwrap();
    std::fs::write(stale.join("symbols").join("R.snxsym"), "x").unwrap();
    let marked = root.join(format!(".Caps{STAGING_MARK}0002"));
    std::fs::create_dir(&marked).unwrap();
    std::fs::write(marked.join(super::staging::SPLIT_MARKER), "").unwrap();
    let mine = root.join("Passives");
    std::fs::create_dir(&mine).unwrap();
    let file = root.join(format!(".Leds{STAGING_MARK}0003"));
    std::fs::write(&file, "not a folder").unwrap();
    // This session's previews are newer than its start: none goes.
    assert_eq!(sweep_stale(std::slice::from_ref(&root), SystemTime::UNIX_EPOCH), 0);
    assert!(stale.is_dir());
    assert_eq!(sweep_stale(std::slice::from_ref(&root), far_future()), 1);
    assert!(!stale.exists(), "the stale preview goes");
    assert!(marked.is_dir() && mine.is_dir() && file.is_file(), "the rest stays");
    #[cfg(unix)]
    {
        let outside = root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("keep"), "keep").unwrap();
        let link = root.join(format!(".Link{STAGING_MARK}0004"));
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        assert_eq!(sweep_stale(std::slice::from_ref(&root), far_future()), 0);
        assert!(outside.join("keep").is_file() && link.exists());
    }
}
