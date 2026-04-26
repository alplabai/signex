//! Integration tests for the local + git library adapter (WS-A).
//!
//! Exercises the v0.9-refactored lifecycle of a `.snxlib/` directory:
//! init → save several revisions → reopen → search → lock contention →
//! review workflow redirect.

#![cfg(feature = "local-git")]

use std::path::Path;

use signex_library::adapter::{FieldSet, LibraryAdapter, LibraryError, LibraryQuery};
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::component::{DatasheetRef, PlmReserved, Revision};
use signex_library::identity::Version;
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMeta, LibraryMode, Manifest, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::PrimitiveRef;
use uuid::Uuid;

fn empty_manifest(name: &str, review_required: bool) -> Manifest {
    Manifest {
        library: LibraryMeta {
            name: name.into(),
            library_id: Uuid::now_v7(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig {
            review_required,
            ..Default::default()
        },
        users: UsersConfig::default(),
    }
}

/// Build a fixture revision pointing at `(library_id, sym_uuid)` for the
/// symbol primitive and `(library_id, fp_uuid)` for the footprint. Saving the
/// same revision again with the *same* primitive UUIDs is a metadata-only
/// change (minor bump); swapping a primitive UUID is a major bump.
fn fixture_revision(sym_uuid: Uuid, fp_uuid: Uuid, mpn: &str) -> Revision {
    let lib = Uuid::nil();
    Revision {
        version: Version::new(0, 0), // overwritten by adapter
        state: LifecycleState::Released,
        created: chrono::Utc::now(),
        author: "tester@example.com".into(),
        message: "fixture".into(),
        symbol_ref: PrimitiveRef::new(lib, sym_uuid),
        footprint_ref: Some(PrimitiveRef::new(lib, fp_uuid)),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", mpn),
        alternates: Vec::new(),
        supply: Vec::new(),
        datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        content_hash: [0u8; 32],
    }
}

/// Initialising into a non-existent directory writes manifest + makes a git
/// commit; reopening picks up the same manifest.
#[test]
fn init_open_round_trip_empty_library() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Empty.snxlib");
    let manifest = empty_manifest("Empty", false);
    let adapter = LocalGitAdapter::init(&root, manifest.clone()).unwrap();
    assert_eq!(adapter.manifest().library.name, "Empty");
    assert!(root.join("manifest.toml").exists());
    assert!(root.join(".git").is_dir());

    drop(adapter);
    let reopened = LocalGitAdapter::open(&root).unwrap();
    assert_eq!(
        reopened.manifest().library.library_id,
        manifest.library.library_id
    );
    let hits = reopened.search(&LibraryQuery::default()).unwrap();
    assert!(hits.is_empty(), "fresh library has no parts");
}

/// Re-init over an existing manifest must not silently nuke history.
#[test]
fn init_refuses_existing_library() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("X.snxlib");
    LocalGitAdapter::init(&root, empty_manifest("X", false)).unwrap();
    let err = LocalGitAdapter::init(&root, empty_manifest("X", false)).unwrap_err();
    assert!(matches!(err, LibraryError::Conflict(_)));
}

/// Saving 3 revisions of the same component:
/// - v1.0: first save (sym_a, fp_a, MPN-A).
/// - v1.1: only mpn changes → minor bump.
/// - v2.0: symbol primitive UUID swaps → major bump.
///
/// Reopen the adapter and verify search returns the latest summary, history
/// includes all three, and the on-disk file is `<uuid>.snxprt`.
#[test]
fn three_revisions_auto_bump_minor_then_major() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Hist.snxlib");
    let adapter = LocalGitAdapter::init(&root, empty_manifest("Hist", false)).unwrap();
    let id = Uuid::now_v7();
    let sym_a = Uuid::now_v7();
    let sym_b = Uuid::now_v7();
    let fp_a = Uuid::now_v7();

    // First save → 1.0.
    adapter
        .save_revision(id, fixture_revision(sym_a, fp_a, "MPN-A"), "first release")
        .unwrap();
    let comp = adapter.get_component(id).unwrap();
    assert_eq!(comp.head, Version::new(1, 0));
    assert_eq!(comp.revisions.len(), 1);

    // mpn-only change should be a minor bump (same primitive refs).
    let mut rev_b = fixture_revision(sym_a, fp_a, "MPN-B");
    rev_b.message = "vendor swap".into();
    adapter.save_revision(id, rev_b, "swap mpn").unwrap();
    let comp = adapter.get_component(id).unwrap();
    assert_eq!(comp.head, Version::new(1, 1));
    assert_eq!(comp.revisions.len(), 2);

    // Symbol primitive UUID swap must trigger a major bump.
    adapter
        .save_revision(
            id,
            fixture_revision(sym_b, fp_a, "MPN-B"),
            "symbol swap",
        )
        .unwrap();
    let comp = adapter.get_component(id).unwrap();
    assert_eq!(comp.head, Version::new(2, 0));
    assert_eq!(comp.revisions.len(), 3);

    // Disk layout: parts/<uuid>.snxprt (single file per component now).
    let part_path = root.join("parts").join(format!("{id}.snxprt"));
    assert!(part_path.exists(), "expected on-disk file at {part_path:?}");

    // Drop, reopen, search picks up the latest summary.
    drop(adapter);
    let reopened = LocalGitAdapter::open(&root).unwrap();
    let hits = reopened
        .search(&LibraryQuery {
            text: Some("MPN".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    let hit = &hits[0];
    assert_eq!(hit.head, Version::new(2, 0));
    assert_eq!(hit.mpn, "MPN-B");
}

/// `try_lock` writes a lock file alongside the part. A second adapter handle
/// pointing at the same `.snxlib/` then sees the lock and refuses.
/// `release_lock` removes the file and lets the second handle succeed.
#[test]
fn lock_blocks_sibling_adapter_until_released() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Locks.snxlib");
    let a = LocalGitAdapter::init(&root, empty_manifest("Locks", false)).unwrap();
    let b = LocalGitAdapter::open(&root).unwrap();
    let id = Uuid::now_v7();

    a.try_lock(id, FieldSet::Symbol).unwrap();

    let lock_file = root.join("parts").join(format!("{id}.symbol.lock"));
    assert!(lock_file.exists(), "lock file lives under parts/");

    let err = b.try_lock(id, FieldSet::Symbol).unwrap_err();
    match err {
        LibraryError::Locked { field_set, .. } => assert_eq!(field_set, "symbol"),
        other => panic!("expected Locked, got {other:?}"),
    }

    // Locks for different field-sets must not collide.
    a.try_lock(id, FieldSet::Footprint).unwrap();

    a.release_lock(id, FieldSet::Symbol).unwrap();
    assert!(!lock_file.exists());
    b.try_lock(id, FieldSet::Symbol).unwrap();
}

/// `release_lock` on an absent file is `NotFound` — UI surfaces this as a
/// "no-op" toast rather than an error dialog.
#[test]
fn release_lock_without_holder_errors() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("LR.snxlib");
    let a = LocalGitAdapter::init(&root, empty_manifest("LR", false)).unwrap();
    let id = Uuid::now_v7();
    let err = a.release_lock(id, FieldSet::Symbol).unwrap_err();
    assert!(matches!(err, LibraryError::NotFound(_)));
}

/// With `workflow.review_required = true`, `save_revision` writes the new
/// `.snxprt` on a `review/<uuid>` git branch instead of trunk. Trunk stays
/// empty until a reviewer merges it.
#[test]
fn review_required_redirects_save_to_review_branch() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Review.snxlib");
    let adapter = LocalGitAdapter::init(&root, empty_manifest("Review", true)).unwrap();
    let id = Uuid::now_v7();
    let sym = Uuid::now_v7();
    let fp = Uuid::now_v7();

    adapter
        .save_revision(
            id,
            fixture_revision(sym, fp, "MPN-A"),
            "submit for review",
        )
        .unwrap();

    // The .snxprt should NOT be visible on trunk's working tree after save —
    // the adapter must hop back to the trunk branch when review_required.
    let on_disk = root.join("parts").join(format!("{id}.snxprt"));
    assert!(
        !on_disk.exists(),
        "trunk should be clean after a review-only save"
    );

    // …but the review/<uuid> branch should hold the new revision.
    let repo = git2::Repository::open(&root).unwrap();
    let branch_name = format!("review/{id}");
    let branch = repo
        .find_branch(&branch_name, git2::BranchType::Local)
        .expect("review branch should exist");
    let commit = branch.get().peel_to_commit().unwrap();
    let tree = commit.tree().unwrap();
    let part_entry = tree
        .get_path(Path::new(&format!("parts/{id}.snxprt")))
        .expect("review branch tree contains the new revision");
    assert_eq!(part_entry.kind(), Some(git2::ObjectType::Blob));
}

/// Even with `review_required`, `try_lock` / `release_lock` operate on the
/// working tree directly (never branch-scoped) so two designers see each
/// other's locks immediately.
#[test]
fn lock_files_ignore_review_branch_state() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("RL.snxlib");
    let a = LocalGitAdapter::init(&root, empty_manifest("RL", true)).unwrap();
    let b = LocalGitAdapter::open(&root).unwrap();
    let id = Uuid::now_v7();

    a.try_lock(id, FieldSet::Lifecycle).unwrap();
    let err = b.try_lock(id, FieldSet::Lifecycle).unwrap_err();
    assert!(matches!(err, LibraryError::Locked { .. }));
}
