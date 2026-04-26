//! Integration tests for the where-used reverse index (Phase 1 WS-G).
//!
//! Verifies the public contract:
//!   - `ingest_sheet` registers component references for a (project, sheet) pair.
//!   - `where_used(uuid, None)` returns every site (any version pinned).
//!   - `where_used(uuid, Some(version))` filters to a specific pinned version.
//!   - `drop_project(p)` removes every site under that project root.
//!   - Re-ingesting a sheet replaces (not appends) its previous entries.

use std::path::PathBuf;

use signex_library::{Version, WhereUsedIndex};
use uuid::Uuid;

#[test]
fn where_used_returns_all_sites_for_a_uuid_across_sheets() {
    let project = PathBuf::from("/projects/alpha.snxprj");
    let sheet1 = PathBuf::from("/projects/alpha/main.snxsch");
    let sheet2 = PathBuf::from("/projects/alpha/power.snxsch");
    let sheet3 = PathBuf::from("/projects/alpha/decoupling.snxsch");

    let uuid_a = Uuid::now_v7();
    let uuid_b = Uuid::now_v7();
    let v = Version::new(1, 0);

    let mut idx = WhereUsedIndex::new();
    idx.ingest_sheet(&project, &sheet1, &[(uuid_a, "R1".into(), v)]);
    idx.ingest_sheet(&project, &sheet2, &[(uuid_a, "R2".into(), v)]);
    idx.ingest_sheet(&project, &sheet3, &[(uuid_b, "C1".into(), v)]);

    let a_sites = idx.where_used(uuid_a, None);
    assert_eq!(a_sites.len(), 2, "expected 2 sites for uuid_a");
    let mut a_instances: Vec<_> = a_sites.iter().map(|s| s.instance_id.clone()).collect();
    a_instances.sort();
    assert_eq!(a_instances, vec!["R1".to_string(), "R2".to_string()]);
    assert!(a_sites.iter().all(|s| s.project_path == project));

    let b_sites = idx.where_used(uuid_b, None);
    assert_eq!(b_sites.len(), 1, "expected 1 site for uuid_b");
    assert_eq!(b_sites[0].instance_id, "C1");
    assert_eq!(b_sites[0].sheet_path, sheet3);
}

#[test]
fn where_used_filters_by_pinned_version_when_specified() {
    let project = PathBuf::from("/projects/beta.snxprj");
    let sheet_v1 = PathBuf::from("/projects/beta/old.snxsch");
    let sheet_v2 = PathBuf::from("/projects/beta/new.snxsch");

    let uuid_a = Uuid::now_v7();
    let v1 = Version::new(1, 0);
    let v2 = Version::new(2, 0);

    let mut idx = WhereUsedIndex::new();
    idx.ingest_sheet(&project, &sheet_v1, &[(uuid_a, "U1".into(), v1)]);
    idx.ingest_sheet(&project, &sheet_v2, &[(uuid_a, "U1".into(), v2)]);

    // Specific version → only matching sites.
    let v1_sites = idx.where_used(uuid_a, Some(v1));
    assert_eq!(v1_sites.len(), 1);
    assert_eq!(v1_sites[0].sheet_path, sheet_v1);
    assert_eq!(v1_sites[0].version_pinned, v1);

    let v2_sites = idx.where_used(uuid_a, Some(v2));
    assert_eq!(v2_sites.len(), 1);
    assert_eq!(v2_sites[0].sheet_path, sheet_v2);
    assert_eq!(v2_sites[0].version_pinned, v2);

    // None → both, regardless of version.
    let any_sites = idx.where_used(uuid_a, None);
    assert_eq!(any_sites.len(), 2);
}

#[test]
fn drop_project_removes_all_sites_under_that_project() {
    let project_a = PathBuf::from("/projects/alpha.snxprj");
    let project_b = PathBuf::from("/projects/beta.snxprj");
    let sheet_a = PathBuf::from("/projects/alpha/main.snxsch");
    let sheet_b = PathBuf::from("/projects/beta/main.snxsch");

    let uuid_x = Uuid::now_v7();
    let v = Version::new(1, 0);

    let mut idx = WhereUsedIndex::new();
    idx.ingest_sheet(&project_a, &sheet_a, &[(uuid_x, "R1".into(), v)]);
    idx.ingest_sheet(&project_b, &sheet_b, &[(uuid_x, "R2".into(), v)]);
    assert_eq!(idx.where_used(uuid_x, None).len(), 2);

    idx.drop_project(&project_a);

    let after = idx.where_used(uuid_x, None);
    assert_eq!(after.len(), 1, "drop_project should remove project_a sites");
    assert_eq!(after[0].project_path, project_b);
    assert_eq!(after[0].sheet_path, sheet_b);
}

#[test]
fn re_ingesting_a_sheet_replaces_its_previous_entries() {
    // Spec: "Replace all entries for a sheet (called when a sheet is opened/saved)."
    // Re-ingest must not append duplicates.
    let project = PathBuf::from("/projects/gamma.snxprj");
    let sheet = PathBuf::from("/projects/gamma/main.snxsch");

    let uuid_a = Uuid::now_v7();
    let uuid_b = Uuid::now_v7();
    let v = Version::new(1, 0);

    let mut idx = WhereUsedIndex::new();
    idx.ingest_sheet(&project, &sheet, &[(uuid_a, "R1".into(), v)]);
    assert_eq!(idx.where_used(uuid_a, None).len(), 1);

    // Sheet now references a different component; the old reference must vanish.
    idx.ingest_sheet(&project, &sheet, &[(uuid_b, "C1".into(), v)]);
    assert_eq!(idx.where_used(uuid_a, None).len(), 0);
    assert_eq!(idx.where_used(uuid_b, None).len(), 1);
}
