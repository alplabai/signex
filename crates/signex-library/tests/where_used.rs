//! Integration tests for the where-used reverse index.
//!
//! The index is keyed by [`RowId`] (component-table row) under the
//! DBLib model — not by a `(uuid, Version)` pair. Verifies the
//! public contract:
//!   - `ingest_sheet` registers row references for a (project, sheet) pair.
//!   - `where_used(row_id)` returns every site.
//!   - `drop_project(p)` removes every site under that project root.
//!   - Re-ingesting a sheet replaces (not appends) its previous entries.

use std::path::PathBuf;

use signex_library::{RowId, WhereUsedIndex};

#[test]
fn where_used_returns_all_sites_for_a_row_across_sheets() {
    let project = PathBuf::from("/projects/alpha.snxprj");
    let sheet1 = PathBuf::from("/projects/alpha/main.snxsch");
    let sheet2 = PathBuf::from("/projects/alpha/power.snxsch");
    let sheet3 = PathBuf::from("/projects/alpha/decoupling.snxsch");

    let row_a = RowId::new();
    let row_b = RowId::new();

    let mut idx = WhereUsedIndex::new();
    idx.ingest_sheet(&project, &sheet1, &[(row_a, "R1".into())]);
    idx.ingest_sheet(&project, &sheet2, &[(row_a, "R2".into())]);
    idx.ingest_sheet(&project, &sheet3, &[(row_b, "C1".into())]);

    let a_sites = idx.where_used(row_a);
    assert_eq!(a_sites.len(), 2, "expected 2 sites for row_a");
    let mut a_instances: Vec<_> = a_sites.iter().map(|s| s.instance_id.clone()).collect();
    a_instances.sort();
    assert_eq!(a_instances, vec!["R1".to_string(), "R2".to_string()]);
    assert!(a_sites.iter().all(|s| s.project_path == project));

    let b_sites = idx.where_used(row_b);
    assert_eq!(b_sites.len(), 1, "expected 1 site for row_b");
    assert_eq!(b_sites[0].instance_id, "C1");
    assert_eq!(b_sites[0].sheet_path, sheet3);
}

#[test]
fn drop_project_removes_all_sites_under_that_project() {
    let project_a = PathBuf::from("/projects/alpha.snxprj");
    let project_b = PathBuf::from("/projects/beta.snxprj");
    let sheet_a = PathBuf::from("/projects/alpha/main.snxsch");
    let sheet_b = PathBuf::from("/projects/beta/main.snxsch");

    let row_x = RowId::new();

    let mut idx = WhereUsedIndex::new();
    idx.ingest_sheet(&project_a, &sheet_a, &[(row_x, "R1".into())]);
    idx.ingest_sheet(&project_b, &sheet_b, &[(row_x, "R2".into())]);
    assert_eq!(idx.where_used(row_x).len(), 2);

    idx.drop_project(&project_a);

    let after = idx.where_used(row_x);
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

    let row_a = RowId::new();
    let row_b = RowId::new();

    let mut idx = WhereUsedIndex::new();
    idx.ingest_sheet(&project, &sheet, &[(row_a, "R1".into())]);
    assert_eq!(idx.where_used(row_a).len(), 1);

    // Sheet now references a different row; the old reference must vanish.
    idx.ingest_sheet(&project, &sheet, &[(row_b, "C1".into())]);
    assert_eq!(idx.where_used(row_a).len(), 0);
    assert_eq!(idx.where_used(row_b).len(), 1);
}
