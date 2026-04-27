//! Integration tests for the local + git library adapter.
//!
//! Per `v0.9-refactor-2-plan.md` §7, this exercise covers both flows the
//! adapter ships:
//!
//! * Primitive (`Symbol` / `Footprint` / `SimModel`) round-trip + commit.
//! * Row CRUD over `tables/<category>.tsv` — insert/update/delete by id,
//!   lookup by `internal_pn`, and `iter_rows` across every table.

#![cfg(feature = "local-git")]

use signex_library::adapter::{LibraryAdapter, LibraryError};
use signex_library::adapters::library_set::LibrarySet;
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_library::component::{ComponentRow, DatasheetRef, PlmReserved};
use signex_library::identity::{ComponentClass, InternalPn, RowId};
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
        tables: Vec::new(),
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
    assert!(root.join("library.toml").exists());
    assert!(root.join(".git").is_dir());

    drop(adapter);
    let reopened = LocalGitAdapter::open(&root).unwrap();
    assert_eq!(
        reopened.manifest().library.library_id,
        manifest.library.library_id
    );
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

// ── Primitive CRUD ───────────────────────────────────────────────────────

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

/// Save a Symbol → reopen → get_symbol → bytes are identical.
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

// ── Row CRUD ──────────────────────────────────────────────────────────────

/// Build a fixture row with a given internal PN and class. The `lib_id`
/// argument is the library this row's primitives live in — the adapter's
/// own `library_id` for an in-library row, or some other lib for a row
/// pointing at an external primitive.
fn fixture_row(internal_pn: &str, class: &str, lib_id: Uuid) -> ComponentRow {
    let now = chrono::Utc::now();
    ComponentRow {
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(internal_pn),
        class: ComponentClass::new(class),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(lib_id, Uuid::now_v7()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", internal_pn),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
    }
}

/// `insert_row` → `read_row` round-trip; `update_row` mutates in place;
/// `delete_row` removes the row and `read_row` then returns `NotFound`.
#[test]
fn local_git_round_trip_row() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("RowRT.snxlib");
    let manifest = empty_manifest("RowRT", false);
    let lib_id = manifest.library.library_id;
    let adapter = LocalGitAdapter::init(&root, manifest).unwrap();

    let row = fixture_row("R10K", "resistor", lib_id);
    let row_id = RowId::from_uuid(row.row_id);

    adapter
        .insert_row("resistors", row.clone(), "first")
        .unwrap();
    let on_disk = root.join("tables").join("resistors.tsv");
    assert!(on_disk.exists(), "expected {on_disk:?} after insert_row");

    let got = adapter.read_row("resistors", row_id).unwrap();
    assert_eq!(got, row);

    // Mutate a non-key field; update_row must keep the same row_id.
    let mut modified = row.clone();
    modified.internal_pn = InternalPn::new("R10K_RENAMED");
    adapter
        .update_row("resistors", modified.clone(), "rename")
        .unwrap();
    let after_update = adapter.read_row("resistors", row_id).unwrap();
    assert_eq!(after_update, modified);
    assert_eq!(after_update.internal_pn.as_str(), "R10K_RENAMED");

    // Delete + read returns NotFound.
    adapter.delete_row("resistors", row_id, "drop").unwrap();
    let err = adapter.read_row("resistors", row_id).unwrap_err();
    assert!(matches!(err, LibraryError::NotFound(_)));
}

/// `iter_rows` walks every `.tsv` under `tables/` and pairs each row with
/// its table name.
#[test]
fn local_git_iter_rows_across_tables() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("IterRows.snxlib");
    let manifest = empty_manifest("IterRows", false);
    let lib_id = manifest.library.library_id;
    let adapter = LocalGitAdapter::init(&root, manifest).unwrap();

    let r1 = fixture_row("R10K", "resistor", lib_id);
    let r2 = fixture_row("R47K", "resistor", lib_id);
    let c1 = fixture_row("C100N", "capacitor", lib_id);

    adapter.insert_row("resistors", r1.clone(), "r1").unwrap();
    adapter.insert_row("resistors", r2.clone(), "r2").unwrap();
    adapter.insert_row("capacitors", c1.clone(), "c1").unwrap();

    let mut got = adapter.iter_rows().unwrap();
    assert_eq!(got.len(), 3);

    // Sort by (table, internal_pn) so the order is stable in the assertion.
    got.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.internal_pn.cmp(&b.1.internal_pn))
    });
    assert_eq!(got[0].0, "capacitors");
    assert_eq!(got[0].1.internal_pn.as_str(), "C100N");
    assert_eq!(got[1].0, "resistors");
    assert_eq!(got[1].1.internal_pn.as_str(), "R10K");
    assert_eq!(got[2].0, "resistors");
    assert_eq!(got[2].1.internal_pn.as_str(), "R47K");

    // `list_tables` returns sorted file stems.
    let tables = adapter.list_tables().unwrap();
    assert_eq!(
        tables,
        vec!["capacitors".to_string(), "resistors".to_string()]
    );
}

/// `read_row_by_pn` finds the first matching row across every table.
#[test]
fn local_git_read_row_by_pn() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ByPN.snxlib");
    let manifest = empty_manifest("ByPN", false);
    let lib_id = manifest.library.library_id;
    let adapter = LocalGitAdapter::init(&root, manifest).unwrap();

    let target = fixture_row("R10K", "resistor", lib_id);
    let other = fixture_row("R47K", "resistor", lib_id);
    adapter
        .insert_row("resistors", target.clone(), "target")
        .unwrap();
    adapter.insert_row("resistors", other, "other").unwrap();

    let pn: InternalPn = "R10K".parse().unwrap();
    let (table, row) = adapter.read_row_by_pn(&pn).unwrap();
    assert_eq!(table, "resistors");
    assert_eq!(row, target);

    // Missing PN returns NotFound, not a panic.
    let missing: InternalPn = "Q_NOT_THERE".parse().unwrap();
    let err = adapter.read_row_by_pn(&missing).unwrap_err();
    assert!(matches!(err, LibraryError::NotFound(_)));
}

/// The `msg` argument on `insert_row` lands on the resulting libgit2 commit,
/// so the user-supplied audit trail is preserved.
#[test]
fn local_git_commits_with_message() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Commits.snxlib");
    let manifest = empty_manifest("Commits", false);
    let lib_id = manifest.library.library_id;
    let adapter = LocalGitAdapter::init(&root, manifest).unwrap();

    let row = fixture_row("R10K", "resistor", lib_id);
    adapter.insert_row("resistors", row, "msg-XYZ").unwrap();

    let repo = git2::Repository::open(&root).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.summary(), Some("msg-XYZ"));
}
