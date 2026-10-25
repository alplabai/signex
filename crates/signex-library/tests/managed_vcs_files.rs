//! #1117: Enable Version Control merges a managed block into the user's
//! `.gitignore` / `.gitattributes` instead of replacing them.

#![cfg(feature = "local-git")]

use signex_library::{LibraryError, enable_project_version_control};

const BEGIN: &str = "# >>> signex (managed) >>>";
const END: &str = "# <<< signex (managed) <<<";

fn with_identity() {
    // SAFETY: set before any other thread in this test binary reads it; every
    // test sets the same values.
    unsafe {
        std::env::set_var("GIT_AUTHOR_NAME", "Ada Lovelace");
        std::env::set_var("GIT_AUTHOR_EMAIL", "ada@example.org");
    }
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
    enable_project_version_control(dir.path(), false, Some("# Generated\nlibs/\n")).unwrap();
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
    enable_project_version_control(dir.path(), false, None).unwrap();
    let text = read(&dir, ".gitignore");
    assert!(text.starts_with(BEGIN) && text.ends_with(&format!("{END}\n")));
    assert_eq!(text.matches(BEGIN).count(), 1);
}

#[test]
fn an_existing_gitignore_is_merged_never_overwritten() {
    with_identity();
    let dir = project();
    std::fs::write(dir.path().join(".gitignore"), "my-rules\n*.log").unwrap();
    enable_project_version_control(dir.path(), false, Some("libs/\n")).unwrap();
    let text = read(&dir, ".gitignore");
    assert!(text.starts_with("my-rules\n*.log\n"), "{text}");
    assert!(text.contains(BEGIN) && text.contains("libs/\n"));
}

#[test]
fn an_existing_gitattributes_is_merged_when_lfs_is_ticked() {
    with_identity();
    let dir = project();
    std::fs::write(dir.path().join(".gitattributes"), "*.x text\n").unwrap();
    enable_project_version_control(dir.path(), true, None).unwrap();
    let text = read(&dir, ".gitattributes");
    assert!(text.starts_with("*.x text\n"), "{text}");
    assert!(text.contains(BEGIN) && text.contains("assets/*.png filter=lfs"));
    assert!(!text.contains("svg"));
}

#[test]
fn unbalanced_markers_refuse_enable_and_leave_the_file_untouched() {
    with_identity();
    let dir = project();
    let broken = format!("mine\n{BEGIN}\nno end\n");
    std::fs::write(dir.path().join(".gitignore"), &broken).unwrap();
    let err = enable_project_version_control(dir.path(), false, None).unwrap_err();
    assert!(
        matches!(&err, LibraryError::Conflict(m) if m.contains("unbalanced")),
        "{err}"
    );
    assert_eq!(read(&dir, ".gitignore"), broken);
    assert!(!dir.path().join(".git").exists(), "no repo left behind");
}
