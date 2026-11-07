//! #1972 — a draft inside a library nested in a mounted library's folder, the
//! nested one not mounted, is written, committed and version-bumped through
//! the nested library, never through the mounted one. Real libraries on a
//! temp dir, no mocks.

use super::close_primitives_tests::open_dirty_symbol;
use super::*;

/// A git-backed library at `file`.
fn init_library(file: &Path, name: &str) -> LocalGitAdapter {
    LocalGitAdapter::init(
        file,
        SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: Uuid::now_v7(),
            library: LibrarySection {
                name: name.into(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            classes: Vec::new(),
        },
        LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        },
    )
    .expect("init nested library")
}

fn head_of(dir: &Path) -> git2::Oid {
    git2::Repository::open(dir)
        .expect("a repository")
        .head()
        .expect("a head")
        .peel_to_commit()
        .expect("a commit")
        .id()
}

/// The fixture's (mounted) library with an unmounted one inside its folder,
/// and a dirty draft in that one.
struct Nested {
    f: Fixture,
    inner_file: PathBuf,
    symbol: PathBuf,
}

fn nested() -> Nested {
    let mut f = fixture();
    let inner_file = f.file.parent().unwrap().join("inner").join("Inner.snxlib");
    drop(init_library(&inner_file, "Inner"));
    let symbol = inner_file
        .parent()
        .unwrap()
        .join("symbols")
        .join("s.snxsym");
    std::fs::create_dir_all(symbol.parent().unwrap()).unwrap();
    open_dirty_symbol(&mut f, &symbol);
    assert!(
        f.app.library.library_at(&inner_file).is_none(),
        "the nested library is not mounted"
    );
    Nested {
        f,
        inner_file,
        symbol,
    }
}

/// The outer library's HEAD and `.snxlib` bytes: what a save through it
/// would change.
fn outer_state(n: &Nested) -> (git2::Oid, Vec<u8>) {
    (
        head_of(n.f.file.parent().unwrap()),
        std::fs::read(&n.f.file).unwrap(),
    )
}

fn assert_written_through_the_inner_library(n: &Nested, outer_before: &(git2::Oid, Vec<u8>)) {
    assert!(n.symbol.exists(), "the draft was written");
    // Version bump (Personal mode): the file holds the patch bump of the
    // version the draft started at.
    let started = signex_library::Symbol::empty("Sym1").version;
    let written = signex_library::SymbolFile::from_bytes(&std::fs::read(&n.symbol).unwrap())
        .expect("a symbol file");
    assert_eq!(
        written.symbols[0].version,
        signex_library::patch_bump(&started),
        "version bumped by the inner library's workflow"
    );
    // The inner library took the commit.
    let inner = LocalGitAdapter::open(&n.inner_file).unwrap();
    let history = inner.history(&n.symbol).expect("history");
    assert_eq!(
        history.len(),
        1,
        "one commit of the draft in the inner repo"
    );
    assert_eq!(
        head_of(n.inner_file.parent().unwrap()).to_string(),
        history[0].sha,
        "the inner HEAD is the commit of the draft"
    );
    // The outer library is untouched: no commit, no rewritten `.snxlib`.
    assert_eq!(&outer_state(n), outer_before, "outer library untouched");
}

#[test]
fn an_off_thread_save_commits_and_bumps_through_the_unmounted_inner_library() {
    let mut n = nested();
    let outer_before = outer_state(&n);

    let task =
        n.f.app
            .save_primitive_tab_at(&n.symbol)
            .expect("save starts");
    run_task(&mut n.f.app, task);

    assert!(!n.f.app.document_state.is_document_dirty(&n.symbol));
    assert_written_through_the_inner_library(&n, &outer_before);
}

#[test]
fn a_blocking_save_commits_and_bumps_through_the_unmounted_inner_library() {
    let mut n = nested();
    let outer_before = outer_state(&n);

    n.f.app
        .save_primitive_tab_blocking(&n.symbol)
        .expect("saved");

    assert_written_through_the_inner_library(&n, &outer_before);
}

#[test]
fn a_draft_outside_the_inner_library_still_goes_through_the_mounted_one() {
    let mut f = fixture();
    let symbol = f.file.parent().unwrap().join("symbols").join("o.snxsym");
    std::fs::create_dir_all(symbol.parent().unwrap()).unwrap();
    open_dirty_symbol(&mut f, &symbol);
    let before = head_of(f.file.parent().unwrap());

    let task = f.app.save_primitive_tab_at(&symbol).expect("save starts");
    run_task(&mut f.app, task);

    assert!(symbol.exists());
    assert_ne!(head_of(f.file.parent().unwrap()), before, "outer committed");
}
