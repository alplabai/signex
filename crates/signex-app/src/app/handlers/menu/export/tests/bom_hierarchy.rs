//! The BOM on a hierarchical project (#730), driven from the real export scope:
//! every part on every child sheet is counted once per placement, and a flat
//! project's BOM is unchanged.
//!
//! Per-instance designators do not exist yet, so a child placed N times lists
//! its designator N times (see `SharedReferenceAcrossInstances`).

use std::path::PathBuf;

use signex_output::{BomOptions, BomTable, bom};

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::handlers::menu::export::build_bom_export_context;

/// Every designator the BOM lists, one entry per part, sorted.
fn listed(table: &BomTable) -> Vec<String> {
    let mut refs: Vec<String> = table
        .rows
        .iter()
        .flat_map(|row| row.references.clone())
        .collect();
    refs.sort();
    refs
}

fn bom_of(app: &crate::app::Signex) -> BomTable {
    let ctx = build_bom_export_context(&app.document_state).expect("scope");
    bom::rollup(&ctx, &BomOptions::default())
}

/// `top` places `top_children`; `mid` (if given) places `mid_children`;
/// `leaf` is a leaf. Only `top` is a listed page, so every other sheet is a
/// hierarchical child that is never a printed page.
fn three_level(dir: &str, top_children: &[&str], mid_children: &[&str]) -> crate::app::Signex {
    let mut app = app_workspace(dir, &["top.snxsch"]);
    let path = |name: &str| PathBuf::from(dir).join(name);
    open_with(
        &mut app.document_state,
        &path("top.snxsch"),
        sheet_with_net("R_TOP", "TOP_NET", top_children),
    );
    open_with(
        &mut app.document_state,
        &path("mid.snxsch"),
        sheet_with_net("R_MID", "MID_NET", mid_children),
    );
    open_with(
        &mut app.document_state,
        &path("leaf.snxsch"),
        sheet_with_net("R_LEAF", "LEAF_NET", &[]),
    );
    app.document_state.active_path = Some(path("top.snxsch"));
    app
}

#[test]
fn a_three_level_hierarchy_lists_every_part_once() {
    let app = three_level("/w/b3", &["mid.snxsch"], &["leaf.snxsch"]);
    assert_eq!(listed(&bom_of(&app)), ["R_LEAF", "R_MID", "R_TOP"]);
}

#[test]
fn a_child_placed_twice_lists_both_placements() {
    let app = three_level("/w/b2", &["mid.snxsch", "mid.snxsch"], &["leaf.snxsch"]);
    // `mid` twice, and so `leaf` twice (once under each `mid`).
    assert_eq!(
        listed(&bom_of(&app)),
        ["R_LEAF", "R_LEAF", "R_MID", "R_MID", "R_TOP"]
    );
    let qty: u32 = bom_of(&app).rows.iter().map(|r| r.qty).sum();
    assert_eq!(qty, 5);
}

#[test]
fn a_flat_projects_bom_is_unchanged() {
    let mut app = app_workspace("/w/bf", &["a.snxsch", "b.snxsch"]);
    for (name, reference) in [("a.snxsch", "R_A"), ("b.snxsch", "R_B")] {
        open_with(
            &mut app.document_state,
            &PathBuf::from("/w/bf").join(name),
            sheet_with_net(reference, &format!("NET_{reference}"), &[]),
        );
    }
    app.document_state.active_path = Some(PathBuf::from("/w/bf").join("b.snxsch"));

    let mut ctx = build_bom_export_context(&app.document_state).expect("scope");
    assert!(!ctx.netlist_sheets.is_empty(), "the hierarchy was derived");
    let with_hierarchy = bom::rollup(&ctx, &BomOptions::default());
    // The pre-#730 input: printed pages only.
    ctx.netlist_sheets.clear();
    ctx.netlist_sheet_placements.clear();
    let pages_only = bom::rollup(&ctx, &BomOptions::default());

    assert_eq!(listed(&with_hierarchy), ["R_A", "R_B"]);
    assert_eq!(
        serde_json::to_string(&with_hierarchy).unwrap(),
        serde_json::to_string(&pages_only).unwrap(),
        "golden: the flat BOM must equal the pages-only BOM"
    );
}

#[test]
fn a_loose_documents_bom_keeps_every_unowned_open_tab() {
    // Two unrelated loose sheets, X focused: the BOM keeps the PDF's page
    // scope (both), not just X's hierarchy.
    let mut app = app_workspace("/w/bl", &["a.snxsch"]);
    let x = PathBuf::from("/w/loose").join("x.snxsch");
    let y = PathBuf::from("/w/loose").join("y.snxsch");
    open_with(
        &mut app.document_state,
        &x,
        sheet_with_net("R_X", "NX", &[]),
    );
    open_with(
        &mut app.document_state,
        &y,
        sheet_with_net("R_Y", "NY", &[]),
    );
    app.document_state.active_path = Some(x);

    assert_eq!(listed(&bom_of(&app)), ["R_X", "R_Y"]);
}

#[test]
fn a_loose_documents_written_bom_matches_the_preview() {
    // The file the user saves must list what the preview lists: both loose
    // tabs, not only the focused sheet's hierarchy.
    let mut app = app_workspace("/w/bw", &["a.snxsch"]);
    let x = PathBuf::from("/w/loosew").join("x.snxsch");
    let y = PathBuf::from("/w/loosew").join("y.snxsch");
    open_with(
        &mut app.document_state,
        &x,
        sheet_with_net("R_X", "NX", &[]),
    );
    open_with(
        &mut app.document_state,
        &y,
        sheet_with_net("R_Y", "NY", &[]),
    );
    app.document_state.active_path = Some(x);
    assert_eq!(listed(&bom_of(&app)), ["R_X", "R_Y"]);

    let dir = std::env::temp_dir().join(format!("signex_bom_loose_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let out = dir.join("bom.csv");
    let _ = app.handle_export_bom_finished(
        app.document_state.active_path.clone().expect("active"),
        Default::default(),
        Ok(out.clone()),
    );
    let written = std::fs::read_to_string(&out).expect("the BOM was written");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(written.contains("R_X"), "written BOM: {written}");
    assert!(written.contains("R_Y"), "written BOM: {written}");
}

#[test]
fn a_project_whose_hierarchy_cannot_be_derived_says_so_in_bom_terms() {
    // The declared root sheet is neither open nor on disk, so no hierarchy is
    // derived and the BOM falls back to the printed pages. A BOM user must be
    // told child-sheet parts are missing, in BOM terms.
    let _ = crate::diagnostics::init_logging();
    let dir = std::env::temp_dir().join(format!("signex-bom-noroot-{}", uuid::Uuid::new_v4()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    app.document_state.projects[0].data.schematic_root = Some("gone-root.snxsch".to_string());
    let top = dir.join("top.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R_TOP", "TOP_NET", &[]),
    );
    app.document_state.active_path = Some(top);

    let out = dir.join("bom.csv");
    let _ = app.handle_export_bom_finished(
        app.document_state.active_path.clone().expect("active"),
        Default::default(),
        Ok(out),
    );

    let warned = crate::diagnostics::recent_entries().iter().any(|e| {
        e.level == crate::diagnostics::DiagnosticLevel::Warning
            && e.message
                .contains("BOM: project hierarchy could not be derived")
    });
    assert!(warned, "the BOM warning must reach the Messages panel");
    std::fs::remove_dir_all(&dir).ok();
}
