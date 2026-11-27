//! A staged preview's folder (#3171): created fresh beside the target,
//! never adopted, discarded without following a link, and moved into
//! place only while the target is as it was when the preview was staged.

use std::path::{Path, PathBuf};

use super::error::ImportError;
use super::events::{Counts, ExitKind, Summary};
use super::output::claim_output;
use super::run::{LibraryRunRequest, ModelsMode, RunMode, RunOutcome};
use super::staging::{STAGING_MARK, StagedLibrary, TargetBefore, staging_dir_for};

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
    let mut failing = |from: &Path, to: &Path| {
        calls += 1;
        if calls == 2 {
            return Err(std::io::Error::other("injected"));
        }
        std::fs::rename(from, to)
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
