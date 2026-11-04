//! #2004 — a primitive save also moves the rows of the other open
//! libraries that bind it, each under its own workflow mode.

use super::*;
use crate::library::component_preview::test_row;
use signex_library::adapters::local_git::LibraryInitOptions;
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{PrimitiveRef, WorkflowMode};
use std::path::{Path, PathBuf};

const TABLE: &str = "generics";

fn library(dir: &Path, name: &str, mode: WorkflowMode) -> (LocalGitAdapter, PathBuf, Uuid) {
    let library_id = Uuid::now_v7();
    let mut workflow = WorkflowConfig::default();
    workflow.mode = mode;
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: name.into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow,
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let root = dir.join(name).join(format!("{name}.snxlib"));
    let adapter = LocalGitAdapter::init(
        root.clone(),
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .unwrap();
    (adapter, root, library_id)
}

/// A row of `adapter` that binds footprint `footprint` of `home`.
fn bound_row(adapter: &LocalGitAdapter, home: Uuid, footprint: Uuid, released: bool) -> RowId {
    let mut row = test_row();
    row.internal_pn = signex_library::InternalPn::new(format!("R-{}", row.row_id));
    row.footprint_ref = Some(PrimitiveRef::new(home, footprint));
    row.footprint_version = "0.0.1".into();
    row.released = released;
    adapter.insert_row(TABLE, row.clone(), "seed").unwrap();
    RowId::from_uuid(row.row_id)
}

struct World {
    _dir: tempfile::TempDir,
    home: SaveLibrary,
    home_id: Uuid,
    other: PathBuf,
    other_adapter: LocalGitAdapter,
    footprint: Uuid,
}

fn world(other_mode: WorkflowMode) -> World {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let (home, _, home_id) = library(dir.path(), "home", WorkflowMode::Personal);
    let (other_adapter, other, _) = library(dir.path(), "other", other_mode);
    World {
        home: SaveLibrary::Mounted(Box::new(home)),
        home_id,
        other,
        other_adapter,
        footprint: Uuid::now_v7(),
        _dir: dir,
    }
}

fn save_footprint(w: &World, others: &[OtherLibrary<'_>]) -> CascadeOutcome {
    let saved = [SavedPrimitive {
        uuid: w.footprint,
        name: "SOT-23".into(),
        version: "0.0.2".into(),
    }];
    cascade_after_container_save(
        &w.home,
        ContainerWrite::Merge,
        SavedKind::Footprint,
        &saved,
        None,
        others,
    )
}

fn other_library(w: &World, ready: bool) -> OtherLibrary<'_> {
    OtherLibrary {
        root: &w.other,
        own: None,
        ready: Box::new(move || ready),
    }
}

#[test]
fn a_row_of_another_library_that_binds_the_footprint_is_moved() {
    let w = world(WorkflowMode::Personal);
    let row = bound_row(&w.other_adapter, w.home_id, w.footprint, false);
    let outcome = save_footprint(&w, &[other_library(&w, true)]);
    let stored = w.other_adapter.read_row(TABLE, row).unwrap();
    assert_eq!(stored.footprint_version, "0.0.2");
    assert_ne!(stored.version, "0.0.1", "the row's own version moved");
    assert_eq!(outcome.bumped_elsewhere, vec![(w.other.clone(), vec![row])]);
    assert!(outcome.owed.is_empty());
}

#[test]
fn a_released_row_of_a_team_library_follows_the_in_library_rule() {
    let w = world(WorkflowMode::Team);
    let released = bound_row(&w.other_adapter, w.home_id, w.footprint, true);
    let open = bound_row(&w.other_adapter, w.home_id, w.footprint, false);
    let outcome = save_footprint(&w, &[other_library(&w, true)]);
    assert_eq!(
        w.other_adapter
            .read_row(TABLE, released)
            .unwrap()
            .footprint_version,
        "0.0.1",
        "a released row keeps its pin"
    );
    assert_eq!(
        w.other_adapter
            .read_row(TABLE, open)
            .unwrap()
            .footprint_version,
        "0.0.2"
    );
    assert_eq!(outcome.bumped_elsewhere, vec![(w.other.clone(), vec![open])]);
}

#[test]
fn a_library_whose_write_order_did_not_come_is_left_alone_and_owed() {
    let w = world(WorkflowMode::Personal);
    let row = bound_row(&w.other_adapter, w.home_id, w.footprint, false);
    let outcome = save_footprint(&w, &[other_library(&w, false)]);
    assert_eq!(
        w.other_adapter
            .read_row(TABLE, row)
            .unwrap()
            .footprint_version,
        "0.0.1"
    );
    assert_eq!(outcome.owed, vec![w.footprint]);
}

#[test]
fn a_library_that_cannot_be_opened_does_not_keep_the_save_owed() {
    let w = world(WorkflowMode::Personal);
    let missing = w.other.with_file_name("missing.snxlib");
    let broken = OtherLibrary {
        root: &missing,
        own: None,
        ready: Box::new(|| true),
    };
    let outcome = save_footprint(&w, &[broken]);
    assert!(outcome.owed.is_empty());
    assert!(outcome.bumped_elsewhere.is_empty());
}
