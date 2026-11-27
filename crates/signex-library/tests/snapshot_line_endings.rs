//! An import repository restores files byte for byte even when the user's
//! global git config converts line endings (#3190). This binary holds one
//! test, because it points libgit2's global config at a file of its own.

use signex_library::import_snapshot::{commit_snapshot, prepare_snapshot_repo, restore_to_head};

const LIBRARY: &str = "format = \"snxlib/2\"\n";
const IMPORT: &str = "Import Lib\n\nSignex-Import: example-import 1.0\n";

#[test]
fn a_restore_keeps_lf_files_under_a_global_autocrlf() {
    let config = tempfile::tempdir().unwrap();
    std::fs::write(
        config.path().join(".gitconfig"),
        "[user]\n\tname = Signex Test\n\temail = test@example.org\n[core]\n\tautocrlf = true\n",
    )
    .unwrap();
    // SAFETY: this binary holds a single test, so nothing races libgit2's
    // global search-path writes.
    unsafe {
        for level in [
            git2::ConfigLevel::System,
            git2::ConfigLevel::XDG,
            git2::ConfigLevel::ProgramData,
        ] {
            git2::opts::set_search_path(level, tempfile::tempdir().unwrap().path()).unwrap();
        }
        git2::opts::set_search_path(git2::ConfigLevel::Global, config.path()).unwrap();
    }

    // A repository Signex prepares.
    let fresh = tempfile::tempdir().unwrap();
    std::fs::write(fresh.path().join("Lib.snxlib"), LIBRARY).unwrap();
    prepare_snapshot_repo(fresh.path()).unwrap();
    commit_snapshot(fresh.path(), IMPORT).unwrap();
    let attributes = std::fs::read_to_string(fresh.path().join(".gitattributes")).unwrap();
    assert!(attributes.lines().any(|l| l == "* -text"), "{attributes}");
    std::fs::write(fresh.path().join("Lib.snxlib"), "half written").unwrap();
    restore_to_head(fresh.path()).unwrap();
    assert_eq!(
        std::fs::read(fresh.path().join("Lib.snxlib")).unwrap(),
        LIBRARY.as_bytes()
    );

    // A repository an older build prepared: no attribute, no local setting.
    let older = tempfile::tempdir().unwrap();
    std::fs::write(older.path().join("Lib.snxlib"), LIBRARY).unwrap();
    let repo = git2::Repository::init(older.path()).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("Lib.snxlib")).unwrap();
    index.write().unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let who = git2::Signature::now("Signex Test", "test@example.org").unwrap();
    repo.commit(Some("HEAD"), &who, &who, IMPORT, &tree, &[])
        .unwrap();
    std::fs::write(older.path().join("Lib.snxlib"), "half written").unwrap();
    restore_to_head(older.path()).unwrap();
    assert_eq!(
        std::fs::read(older.path().join("Lib.snxlib")).unwrap(),
        LIBRARY.as_bytes()
    );
    assert!(
        !older.path().join(".gitattributes").exists(),
        "a restore adds no file"
    );
}
