//! #1840 — a cascade step is recorded only for a row the cascade alone
//! changed; a change by another writer, before or after the cascade's
//! write, keeps the row refused (#1072).

use super::*;
use crate::library::component_preview::test_row;
use crate::library::writes::OwnRevisions;
use signex_library::adapters::local_git::LibraryInitOptions;
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{PrimitiveRef, WorkflowMode};

const TABLE: &str = "generics";

enum Foreign {
    Nobody,
    BeforeTheCascade,
    AfterTheCascade,
}

struct Run {
    own: OwnRevisions,
    row: RowId,
    base: Option<u64>,
    stored: Option<u64>,
}

fn edit_description(adapter: &LocalGitAdapter, row: RowId, text: &str) {
    let mut stored = adapter.read_row(TABLE, row).unwrap();
    stored.description = text.into();
    adapter.update_row(TABLE, stored, "another writer").unwrap();
}

fn run(foreign: Foreign) -> Run {
    signex_library::git_identity::test_identity::install();
    let tmp = tempfile::tempdir().unwrap();
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "cells".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = LocalGitAdapter::init(
        tmp.path().join("cells").join("cells.snxlib"),
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .unwrap();
    let symbol = Uuid::now_v7();
    let mut seed = test_row();
    seed.symbol_ref = PrimitiveRef::new(library_id, symbol);
    adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();
    let row = RowId::from_uuid(seed.row_id);
    let base = adapter.read_row(TABLE, row).unwrap().server_revision;

    let before = rows_before(&adapter);
    if matches!(foreign, Foreign::BeforeTheCascade) {
        edit_description(&adapter, row, "theirs");
    }
    let save = PrimitiveSave {
        kind: CascadeKind::Symbol,
        library_id,
        uuid: symbol,
        version: "0.0.2".into(),
    };
    let report =
        signex_library::cascade_after_saves(&adapter, &[save], WorkflowMode::Personal).unwrap();
    assert_eq!(report.auto_bumped, vec![row], "the cascade moved the row");
    if matches!(foreign, Foreign::AfterTheCascade) {
        edit_description(&adapter, row, "theirs");
    }
    let own = OwnRevisions::default();
    record_cascade_steps(&own, &adapter, &before, &report.auto_bumped);
    let stored = adapter.read_row(TABLE, row).unwrap().server_revision;
    Run {
        own,
        row,
        base,
        stored,
    }
}

#[test]
fn a_row_only_the_cascade_changed_is_recorded() {
    let r = run(Foreign::Nobody);
    assert!(r.own.only_cascaded_since(r.row, r.base, r.stored));
}

#[test]
fn a_change_by_another_writer_before_the_cascade_is_not_recorded() {
    let r = run(Foreign::BeforeTheCascade);
    assert!(!r.own.only_cascaded_since(r.row, r.base, r.stored));
}

#[test]
fn a_change_by_another_writer_after_the_cascade_is_not_recorded() {
    let r = run(Foreign::AfterTheCascade);
    assert!(!r.own.only_cascaded_since(r.row, r.base, r.stored));
}

#[test]
fn two_cascades_in_a_row_are_one_step_from_the_first_base() {
    let own = OwnRevisions::default();
    let row = RowId::from_uuid(Uuid::now_v7());
    own.record_cascade(row, Some(1), Some(2));
    own.record_cascade(row, Some(2), Some(3));
    assert!(own.only_cascaded_since(row, Some(1), Some(3)));
    assert!(!own.only_cascaded_since(row, Some(2), Some(3)));
}

#[test]
fn a_cascade_that_does_not_continue_the_last_one_starts_over() {
    let own = OwnRevisions::default();
    let row = RowId::from_uuid(Uuid::now_v7());
    own.record_cascade(row, Some(1), Some(2));
    own.record_cascade(row, Some(5), Some(6));
    assert!(own.only_cascaded_since(row, Some(5), Some(6)));
    assert!(!own.only_cascaded_since(row, Some(1), Some(6)));
}
