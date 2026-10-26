//! #1117: Enable Version Control merges a managed block into the user's
//! `.gitignore` / `.gitattributes` instead of replacing them.

#![cfg(feature = "local-git")]

use signex_library::{LibraryError, enable_project_version_control};

const BEGIN: &str = "# >>> signex (managed) >>>";
const END: &str = "# <<< signex (managed) <<<";

/// A fixed git identity through libgit2's config, not the process environment.
fn with_identity() {
    signex_library::git_identity::test_identity::install();
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.snxprj"), "x").unwrap();
    dir
}

fn read(dir: &tempfile::TempDir, name: &str) -> String {
    std::fs::read_to_string(dir.path().join(name)).unwrap()
}

#[test]
fn without_an_existing_gitignore_the_block_comes_first_then_the_unticked_rows() {
    with_identity();
    let dir = project();
    enable_project_version_control(dir.path(), false, true, Some("# Generated\nlibs/\n")).unwrap();
    let text = read(&dir, ".gitignore");
    assert!(text.starts_with(&format!("{BEGIN}\n")), "{text}");
    let end_at = text.find(END).expect("block end");
    let rows_at = text.find("libs/\n").expect("unticked row");
    assert!(
        rows_at > end_at,
        "user rows live outside the block:\n{text}"
    );
    assert!(text.contains(".DS_Store"));
}

#[test]
fn with_no_unticked_rows_the_gitignore_is_exactly_the_block() {
    with_identity();
    let dir = project();
    enable_project_version_control(dir.path(), false, true, None).unwrap();
    let text = read(&dir, ".gitignore");
    assert!(text.starts_with(BEGIN) && text.ends_with(&format!("{END}\n")));
    assert_eq!(text.matches(BEGIN).count(), 1);
}

#[test]
fn an_existing_gitignore_is_merged_never_overwritten() {
    with_identity();
    let dir = project();
    std::fs::write(dir.path().join(".gitignore"), "my-rules\n*.log").unwrap();
    enable_project_version_control(dir.path(), false, true, Some("libs/\n")).unwrap();
    let text = read(&dir, ".gitignore");
    assert!(text.starts_with(BEGIN), "{text}");
    assert!(
        text.contains(&format!("{END}\n\nmy-rules\n*.log\nlibs/\n")),
        "{text}"
    );
}

#[test]
fn an_existing_gitattributes_is_merged_when_lfs_is_ticked() {
    with_identity();
    let dir = project();
    std::fs::write(dir.path().join(".gitattributes"), "*.x text\n").unwrap();
    enable_project_version_control(dir.path(), true, true, None).unwrap();
    let text = read(&dir, ".gitattributes");
    assert!(text.starts_with(BEGIN), "{text}");
    assert!(text.ends_with("\n\n*.x text\n"), "{text}");
    assert!(text.contains(BEGIN) && text.contains("assets/*.png filter=lfs"));
    assert!(!text.contains("svg"));
}

#[test]
fn unbalanced_markers_refuse_enable_and_leave_the_file_untouched() {
    with_identity();
    let dir = project();
    let broken = format!("mine\n{BEGIN}\nno end\n");
    std::fs::write(dir.path().join(".gitignore"), &broken).unwrap();
    let err = enable_project_version_control(dir.path(), false, true, None).unwrap_err();
    assert!(
        matches!(&err, LibraryError::Conflict(m) if m.contains("unbalanced")),
        "{err}"
    );
    assert_eq!(read(&dir, ".gitignore"), broken);
    assert!(!dir.path().join(".git").exists(), "no repo left behind");
}

#[test]
fn library_scope_gets_no_managed_block_but_keeps_the_unticked_rows() {
    with_identity();
    let dir = project();
    enable_project_version_control(dir.path(), false, false, None).unwrap();
    assert!(!dir.path().join(".gitignore").exists());

    let dir = project();
    std::fs::write(dir.path().join(".gitignore"), "mine\n").unwrap();
    enable_project_version_control(dir.path(), false, false, Some("libs/\n")).unwrap();
    assert_eq!(read(&dir, ".gitignore"), "mine\nlibs/\n");
}

#[test]
fn git_itself_agrees_on_what_the_block_ignores() {
    with_identity();
    let dir = project();
    enable_project_version_control(dir.path(), false, true, None).unwrap();
    let repo = git2::Repository::open(dir.path()).unwrap();
    for kept in [
        "Board.snxsch",
        "sub/Board.snxpcb",
        "a.snxprj",
        "parts.snxlib/components.tsv",
        "assets/logo.png",
        "assets/logo.svg",
        "exports/Board.pdf",
        "exports/bom.csv",
    ] {
        assert!(!repo.is_path_ignored(kept).unwrap(), "{kept} must be kept");
    }
    for junk in [
        "Board.snxsch.1-0.tmp",
        ".parts.snxlib.signex-writelock",
        ".DS_Store",
        "sub/Thumbs.db",
        "desktop.ini",
    ] {
        assert!(
            repo.is_path_ignored(junk).unwrap(),
            "{junk} must be ignored"
        );
    }
}

#[test]
fn a_project_with_only_managed_files_is_refused_and_everything_is_put_back() {
    with_identity();
    // Nothing but the user's own ignore/attributes files: nothing real to commit.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".gitignore"), "mine\r\n").unwrap();
    std::fs::write(dir.path().join(".gitattributes"), "*.x text").unwrap();
    let err = enable_project_version_control(dir.path(), true, true, None).unwrap_err();
    assert!(err.to_string().contains("save the project first"), "{err}");
    assert_eq!(read(&dir, ".gitignore"), "mine\r\n");
    assert_eq!(read(&dir, ".gitattributes"), "*.x text");
    assert!(
        !dir.path().join(".git").exists(),
        "the repo this call made is removed"
    );

    // No pre-existing files: the ones we created are removed too.
    let empty = tempfile::tempdir().unwrap();
    assert!(enable_project_version_control(empty.path(), true, true, None).is_err());
    assert!(!empty.path().join(".gitignore").exists());
    assert!(!empty.path().join(".gitattributes").exists());
    assert!(!empty.path().join(".git").exists());
}

#[cfg(unix)]
#[test]
fn symlinked_files_are_refused_and_never_followed() {
    with_identity();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret");
    std::fs::write(&secret, "TOP SECRET\n").unwrap();

    for (name, use_lfs) in [(".gitignore", false), (".gitattributes", true)] {
        let dir = project();
        std::os::unix::fs::symlink(&secret, dir.path().join(name)).unwrap();
        let err = enable_project_version_control(dir.path(), use_lfs, true, None).unwrap_err();
        assert!(matches!(err, LibraryError::Conflict(_)), "{name}: {err}");
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "TOP SECRET\n");
        assert!(
            std::fs::symlink_metadata(dir.path().join(name))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(!dir.path().join(".git").exists());
    }
}

#[cfg(unix)]
#[test]
fn a_dangling_git_link_counts_as_an_existing_git_entry() {
    with_identity();
    let dir = project();
    std::os::unix::fs::symlink(dir.path().join("nowhere"), dir.path().join(".git")).unwrap();
    let err = enable_project_version_control(dir.path(), false, true, None).unwrap_err();
    assert!(matches!(err, LibraryError::Conflict(_)), "{err}");
    assert!(
        std::fs::symlink_metadata(dir.path().join(".git"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn a_bad_gitattributes_is_refused_before_git_init_even_with_lfs_off() {
    with_identity();
    let dir = project();
    let broken = format!("{BEGIN}\nx\n");
    std::fs::write(dir.path().join(".gitattributes"), &broken).unwrap();
    std::fs::write(dir.path().join(".gitignore"), "mine\n").unwrap();

    let err = enable_project_version_control(dir.path(), false, true, None).unwrap_err();

    assert!(
        matches!(&err, LibraryError::Conflict(m) if m.contains("unbalanced")),
        "{err}"
    );
    assert_eq!(read(&dir, ".gitattributes"), broken);
    assert_eq!(read(&dir, ".gitignore"), "mine\n", "ignore file untouched");
    assert!(!dir.path().join(".git").exists(), "no repo left behind");
}

#[test]
fn project_scope_with_lfs_off_still_writes_the_managed_gitattributes() {
    with_identity();
    let dir = project();
    enable_project_version_control(dir.path(), false, true, None).unwrap();
    let text = read(&dir, ".gitattributes");
    assert!(text.starts_with(BEGIN), "{text}");
    assert!(!text.contains("filter=lfs"), "{text}");
}

#[test]
fn a_user_negation_in_gitignore_still_wins_over_the_managed_block() {
    with_identity();
    let dir = project();
    std::fs::write(dir.path().join(".gitignore"), "!keep/*.tmp\n").unwrap();
    enable_project_version_control(dir.path(), false, true, None).unwrap();
    let text = read(&dir, ".gitignore");
    assert!(text.starts_with(BEGIN), "block goes above the user's lines");
    assert!(text.ends_with("!keep/*.tmp\n"), "{text}");

    let repo = git2::Repository::open(dir.path()).unwrap();
    assert!(!repo.is_path_ignored("keep/a.tmp").unwrap());
    assert!(repo.is_path_ignored("other/a.tmp").unwrap());
}
