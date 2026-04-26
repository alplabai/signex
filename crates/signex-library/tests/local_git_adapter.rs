//! Integration tests for the local + git library adapter (WS-A).
//!
//! Exercises the v0.9-refactored lifecycle of a `.snxlib/` directory:
//! init → save several revisions → reopen → search → lock contention →
//! review workflow redirect.

#![cfg(feature = "local-git")]

use std::path::Path;

use signex_library::adapter::{FieldSet, LibraryAdapter, LibraryError, LibraryQuery};
use signex_library::adapters::library_set::LibrarySet;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::component::{DatasheetRef, PlmReserved, Revision};
use signex_library::identity::Version;
use signex_library::lifecycle::LifecycleState;
use signex_library::manifest::{LibraryMeta, LibraryMode, Manifest, UsersConfig, WorkflowConfig};
use signex_library::manufacturer::ManufacturerPart;
use signex_library::param::ParamMap;
use signex_library::primitive::{
    Body3D, BodyShape, Footprint, LayerId, Pad, PadKind, PadShape, PinElectricalType,
    PinOrientation, Polygon, PrimitiveKind, PrimitiveRef, SimKind, SimModel, Symbol, SymbolPin,
};
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
        .save_revision(id, fixture_revision(sym_b, fp_a, "MPN-B"), "symbol swap")
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
        .save_revision(id, fixture_revision(sym, fp, "MPN-A"), "submit for review")
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

// ── Primitive CRUD (WS-C step C2) ────────────────────────────────────────

fn fixture_symbol(name: &str) -> Symbol {
    let now = chrono::Utc::now();
    Symbol {
        uuid: Uuid::now_v7(),
        name: name.into(),
        anchor: [0.0, 0.0],
        pins: vec![SymbolPin {
            number: "1".into(),
            name: "OUT".into(),
            electrical: PinElectricalType::Output,
            position: [0.0, 0.0],
            orientation: PinOrientation::Right,
            length: 2.54,
        }],
        graphics: Vec::new(),
        schematic_params: ParamMap::new(),
        created: now,
        updated: now,
    }
}

fn fixture_footprint(name: &str) -> Footprint {
    let now = chrono::Utc::now();
    Footprint {
        uuid: Uuid::now_v7(),
        name: name.into(),
        anchor: [0.0, 0.0],
        pads: vec![Pad {
            number: "1".into(),
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            size: [1.0, 1.4],
            position: [0.0, 0.0],
            rotation: 0.0,
            layers: vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")],
            drill: None,
            solder_mask_margin: None,
            paste_margin: None,
        }],
        courtyard: Polygon::default(),
        silk_f: Vec::new(),
        silk_b: Vec::new(),
        fab_f: Vec::new(),
        fab_b: Vec::new(),
        body_3d: Body3D {
            shape: BodyShape::Extrude,
            height_mm: 1.6,
            offset_z_mm: 0.0,
            top_color: [0.1, 0.1, 0.1, 1.0],
            side_color: [0.2, 0.2, 0.2, 1.0],
            outline: None,
        },
        step_attachment: None,
        pcb_params: ParamMap::new(),
        created: now,
        updated: now,
    }
}

fn fixture_sim(name: &str) -> SimModel {
    SimModel {
        uuid: Uuid::now_v7(),
        name: name.into(),
        kind: SimKind::Spice3,
        body: ".SUBCKT TEST IN OUT\n.ENDS".into(),
        default_node_map: Default::default(),
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
    }
}

/// `library_id()` reflects the manifest's stable UUID so the resolver can key
/// `LibrarySet` mounts off the adapter directly.
#[test]
fn library_id_returns_manifest_id() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Id.snxlib");
    let manifest = empty_manifest("Id", false);
    let expected = manifest.library.library_id;
    let adapter = LocalGitAdapter::init(&root, manifest).unwrap();
    assert_eq!(adapter.library_id(), expected);
}

/// Save a Symbol → reopen → get_symbol → bytes are identical (JSON round-trip
/// covers the on-disk path, plus the `symbols/` directory is auto-created).
#[test]
fn save_then_get_symbol_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Sym.snxlib");
    let adapter = LocalGitAdapter::init(&root, empty_manifest("Sym", false)).unwrap();
    let sym = fixture_symbol("OPAMP-DUAL-8");
    let uuid = sym.uuid;
    adapter
        .save_symbol(sym.clone(), "add OPAMP-DUAL-8")
        .unwrap();

    let on_disk = root.join("symbols").join(format!("{uuid}.snxsym"));
    assert!(on_disk.exists(), "expected {on_disk:?} after save_symbol");

    drop(adapter);
    let reopened = LocalGitAdapter::open(&root).unwrap();
    let got = reopened.get_symbol(uuid).unwrap();
    assert_eq!(got, sym);
}

#[test]
fn get_symbol_missing_uuid_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Miss.snxlib");
    let adapter = LocalGitAdapter::init(&root, empty_manifest("Miss", false)).unwrap();
    let err = adapter.get_symbol(Uuid::now_v7()).unwrap_err();
    assert!(matches!(err, LibraryError::NotFound(_)));
}

#[test]
fn save_then_get_footprint_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Fpt.snxlib");
    let adapter = LocalGitAdapter::init(&root, empty_manifest("Fpt", false)).unwrap();
    let fp = fixture_footprint("SOIC-8");
    let uuid = fp.uuid;
    adapter.save_footprint(fp.clone(), "add SOIC-8").unwrap();

    let on_disk = root.join("footprints").join(format!("{uuid}.snxfpt"));
    assert!(on_disk.exists());

    drop(adapter);
    let reopened = LocalGitAdapter::open(&root).unwrap();
    let got = reopened.get_footprint(uuid).unwrap();
    assert_eq!(got, fp);
}

#[test]
fn save_then_get_sim_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Sim.snxlib");
    let adapter = LocalGitAdapter::init(&root, empty_manifest("Sim", false)).unwrap();
    let sm = fixture_sim("LM358");
    let uuid = sm.uuid;
    adapter.save_sim(sm.clone(), "add LM358").unwrap();

    let on_disk = root.join("sims").join(format!("{uuid}.snxsim"));
    assert!(on_disk.exists());

    drop(adapter);
    let reopened = LocalGitAdapter::open(&root).unwrap();
    let got = reopened.get_sim(uuid).unwrap();
    assert_eq!(got, sm);
}

/// Each `save_*` produces its own commit (so history mirrors edits).
#[test]
fn primitive_saves_each_create_a_commit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Hist.snxlib");
    let adapter = LocalGitAdapter::init(&root, empty_manifest("Hist", false)).unwrap();
    adapter.save_symbol(fixture_symbol("A"), "add A").unwrap();
    adapter.save_symbol(fixture_symbol("B"), "add B").unwrap();
    adapter
        .save_footprint(fixture_footprint("F1"), "add F1")
        .unwrap();
    adapter.save_sim(fixture_sim("S1"), "add S1").unwrap();

    let repo = git2::Repository::open(&root).unwrap();
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    let count = walk.count();
    // 1 (init) + 4 (saves) = 5 commits.
    assert_eq!(count, 5, "expected 5 commits, got {count}");
}

/// `list_symbols` / `list_footprints` / `list_sims` walk the per-kind dir,
/// return one summary per file, alphabetically sorted by name.
#[test]
fn list_primitives_returns_alphabetic_summaries() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("List.snxlib");
    let adapter = LocalGitAdapter::init(&root, empty_manifest("List", false)).unwrap();
    adapter.save_symbol(fixture_symbol("Zeta"), "z").unwrap();
    adapter.save_symbol(fixture_symbol("Alpha"), "a").unwrap();
    adapter.save_symbol(fixture_symbol("Mu"), "m").unwrap();

    let summaries = adapter.list_symbols().unwrap();
    let names: Vec<&str> = summaries.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["Alpha", "Mu", "Zeta"]);
    for s in &summaries {
        assert_eq!(s.kind, PrimitiveKind::Symbol);
    }

    // Empty footprint dir → empty list.
    assert!(adapter.list_footprints().unwrap().is_empty());

    adapter
        .save_footprint(fixture_footprint("SOIC-8"), "add")
        .unwrap();
    let fps = adapter.list_footprints().unwrap();
    assert_eq!(fps.len(), 1);
    assert_eq!(fps[0].kind, PrimitiveKind::Footprint);
    assert_eq!(fps[0].name, "SOIC-8");
}

/// LibrarySet integration test — mount two LocalGit libraries and resolve a
/// `PrimitiveRef` whose `library_id` points at one specific lib. Verifies the
/// cross-library resolver picks the correct adapter and surfaces unresolved
/// refs cleanly.
#[test]
fn library_set_resolves_across_two_local_libs() {
    let dir = tempfile::tempdir().unwrap();
    let root_a = dir.path().join("A.snxlib");
    let root_b = dir.path().join("B.snxlib");
    let manifest_a = empty_manifest("A", false);
    let manifest_b = empty_manifest("B", false);
    let lib_a = manifest_a.library.library_id;
    let lib_b = manifest_b.library.library_id;
    assert_ne!(lib_a, lib_b);

    let adapter_a = LocalGitAdapter::init(&root_a, manifest_a).unwrap();
    let adapter_b = LocalGitAdapter::init(&root_b, manifest_b).unwrap();

    // Each library gets its own symbol with the SAME local UUID — so a
    // resolver that ignores library_id would pick the wrong one. The test
    // forces `library_id` to be load-bearing.
    let shared_uuid = Uuid::now_v7();
    let mut sym_a = fixture_symbol("OPAMP-IN-A");
    sym_a.uuid = shared_uuid;
    let mut sym_b = fixture_symbol("OPAMP-IN-B");
    sym_b.uuid = shared_uuid;
    adapter_a.save_symbol(sym_a.clone(), "in A").unwrap();
    adapter_b.save_symbol(sym_b.clone(), "in B").unwrap();

    // Footprint only lives in B — a ref into A would be unresolved.
    let fp_b = fixture_footprint("SOIC-8");
    let fp_b_uuid = fp_b.uuid;
    adapter_b
        .save_footprint(fp_b.clone(), "soic-8 in B")
        .unwrap();

    let mut set = LibrarySet::new();
    set.mount(Box::new(adapter_a));
    set.mount(Box::new(adapter_b));
    assert_eq!(set.len(), 2);

    // Cross-library resolution: refs disambiguated by library_id.
    let from_a = set
        .resolve_symbol(&PrimitiveRef::new(lib_a, shared_uuid))
        .expect("symbol from lib A resolves");
    assert_eq!(from_a.name, "OPAMP-IN-A");
    let from_b = set
        .resolve_symbol(&PrimitiveRef::new(lib_b, shared_uuid))
        .expect("symbol from lib B resolves");
    assert_eq!(from_b.name, "OPAMP-IN-B");

    // Footprint only resolves through lib B's id; lib A returns None.
    assert!(
        set.resolve_footprint(&PrimitiveRef::new(lib_b, fp_b_uuid))
            .is_some()
    );
    assert!(
        set.resolve_footprint(&PrimitiveRef::new(lib_a, fp_b_uuid))
            .is_none()
    );

    // unresolved_refs filters down to misses across both libs.
    let bogus_lib = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
    let bogus_uuid = PrimitiveRef::new(lib_a, Uuid::now_v7());
    let resolves = PrimitiveRef::new(lib_a, shared_uuid);
    let unresolved = set.unresolved_refs([&bogus_lib, &bogus_uuid, &resolves]);
    assert_eq!(unresolved.len(), 2);
    assert!(unresolved.contains(&bogus_lib));
    assert!(unresolved.contains(&bogus_uuid));
    assert!(!unresolved.contains(&resolves));
}
