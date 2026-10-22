//! Issue #141 — no git entry point may author a commit as a generic
//! Signex identity. Real git2 repos in temp dirs.
//!
//! One `#[test]` only: it rewrites process env and libgit2's global
//! config search paths, so it must own this test binary.

#![cfg(feature = "local-git")]

use signex_library::adapter::{LibraryAdapter, LibraryError};
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::enable_project_version_control;
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use uuid::Uuid;

fn manifest() -> SnxlibManifest {
    SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    }
}

fn head_author(dir: &std::path::Path) -> (String, String) {
    let repo = git2::Repository::open(dir).unwrap();
    let commit = repo.head().unwrap().peel_to_commit().unwrap();
    let a = commit.author();
    (
        a.name().unwrap().to_string(),
        a.email().unwrap().to_string(),
    )
}

#[test]
fn commits_use_the_user_identity_and_refuse_without_one() {
    let empty_cfg = tempfile::tempdir().unwrap();
    // SAFETY: this binary holds a single test, so nothing races the
    // env / libgit2 global-option writes below.
    unsafe {
        for level in [
            git2::ConfigLevel::System,
            git2::ConfigLevel::XDG,
            git2::ConfigLevel::Global,
            git2::ConfigLevel::ProgramData,
        ] {
            git2::opts::set_search_path(level, empty_cfg.path()).unwrap();
        }
        for k in [
            "GIT_AUTHOR_NAME",
            "GIT_AUTHOR_EMAIL",
            "GIT_COMMITTER_NAME",
            "GIT_COMMITTER_EMAIL",
            "USER",
            "USERNAME",
            "EMAIL",
        ] {
            std::env::remove_var(k);
        }
    }

    // OS-user / host hints must never stand in for an identity.
    unsafe {
        std::env::set_var("USER", "osuser");
        std::env::set_var("USERNAME", "osuser");
        std::env::set_var("EMAIL", "osuser@example.org");
        std::env::set_var("HOSTNAME", "somehost");
        std::env::set_var("COMPUTERNAME", "somehost");
    }

    // --- Existing user files survive a refused Enable Version Control. ---
    let precious = tempfile::tempdir().unwrap();
    std::fs::write(precious.path().join("a.snxprj"), "x").unwrap();
    std::fs::write(
        precious.path().join(".gitignore"),
        "my-precious-rules
",
    )
    .unwrap();
    std::fs::write(
        precious.path().join(".gitattributes"),
        "*.x text
",
    )
    .unwrap();
    let err = enable_project_version_control(
        precious.path(),
        true,
        Some(
            "*.tmp
",
        ),
    )
    .unwrap_err();
    assert!(matches!(&err, LibraryError::Backend(m) if m.contains("identity not configured")));
    assert_eq!(
        std::fs::read_to_string(precious.path().join(".gitignore")).unwrap(),
        "my-precious-rules
",
        "a refusal must not touch the user's .gitignore"
    );
    assert_eq!(
        std::fs::read_to_string(precious.path().join(".gitattributes")).unwrap(),
        "*.x text
",
        "a refusal must not touch the user's .gitattributes"
    );
    assert!(!precious.path().join(".git").exists());

    // --- No identity: project VC refuses, leaves nothing behind. ---
    let proj = tempfile::tempdir().unwrap();
    std::fs::write(proj.path().join("a.snxprj"), "x").unwrap();
    let err = enable_project_version_control(proj.path(), true, Some("*.tmp\n")).unwrap_err();
    assert!(matches!(&err, LibraryError::Backend(m) if m.contains("identity not configured")));
    assert!(!proj.path().join(".git").exists(), "no repo left behind");
    assert!(!proj.path().join(".gitattributes").exists());
    assert!(!proj.path().join(".gitignore").exists());

    // --- No identity: library init with git refuses before writing. ---
    let lib_dir = tempfile::tempdir().unwrap();
    let file = lib_dir.path().join("lib").join("lib.snxlib");
    let Err(err) = LocalGitAdapter::init(
        &file,
        manifest(),
        LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        },
    ) else {
        panic!("init must refuse");
    };
    assert!(matches!(&err, LibraryError::Backend(m) if m.contains("identity not configured")));
    assert!(!file.exists(), "no .snxlib written on refusal");

    // --- No identity: re-init refuses before `git init` touches disk. ---
    let rec_dir = tempfile::tempdir().unwrap();
    let rec_file = rec_dir.path().join("rec.snxlib");
    std::fs::write(&rec_file, "x").unwrap();
    let Err(err) = LocalGitAdapter::recover_init(&rec_file) else {
        panic!("recover_init must refuse");
    };
    assert!(matches!(&err, LibraryError::Backend(m) if m.contains("identity not configured")));
    assert!(
        !rec_dir.path().join(".git").exists(),
        "no unborn .git/ left behind by a refused re-init"
    );

    // --- With an identity: commits carry it, never a generic one. ---
    unsafe {
        std::env::set_var("GIT_AUTHOR_NAME", "Ada Lovelace");
        std::env::set_var("GIT_AUTHOR_EMAIL", "ada@example.org");
    }
    enable_project_version_control(proj.path(), false, None).unwrap();
    assert_eq!(
        head_author(proj.path()),
        ("Ada Lovelace".into(), "ada@example.org".into())
    );

    let adapter = LocalGitAdapter::init(
        &file,
        manifest(),
        LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        },
    )
    .unwrap();
    assert_eq!(
        head_author(file.parent().unwrap()),
        ("Ada Lovelace".into(), "ada@example.org".into())
    );

    // --- Identity lost later: a write-then-commit path changes nothing. ---
    unsafe {
        std::env::remove_var("GIT_AUTHOR_NAME");
        std::env::remove_var("GIT_AUTHOR_EMAIL");
    }
    let before = std::fs::read(&file).unwrap();
    let head_before = git2::Repository::open(file.parent().unwrap())
        .unwrap()
        .head()
        .unwrap()
        .target()
        .unwrap();
    let err = adapter
        .create_empty_table("fresh_table", "")
        .expect_err("no identity must refuse");
    assert!(err.to_string().contains("identity not configured"), "{err}");
    assert_eq!(std::fs::read(&file).unwrap(), before, "file untouched");
    assert_eq!(
        git2::Repository::open(file.parent().unwrap())
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap(),
        head_before,
        "no commit made"
    );
    assert!(
        !adapter
            .list_tables()
            .map(|t| t.iter().any(|n| n == "fresh_table"))
            .unwrap_or(false),
        "in-memory library must not carry the refused edit"
    );

    // --- A repo-local identity is honoured by re-init of an existing repo. ---
    let repo = git2::Repository::open(file.parent().unwrap()).unwrap();
    let mut cfg = repo.config().unwrap();
    cfg.set_str("user.name", "Local User").unwrap();
    cfg.set_str("user.email", "local@example.org").unwrap();
    LocalGitAdapter::recover_init(&file).expect("repo-local identity is enough");
    assert_eq!(
        head_author(file.parent().unwrap()),
        ("Local User".into(), "local@example.org".into())
    );
}
