//! Pins for #1618: showing the sheet hierarchy in the Projects tree must not
//! change what the project-level outputs read from the `.snxprj` `sheets`
//! list. One hierarchical fixture, listed in a deliberate order
//! (`top`, `other`, `mid`, `leaf`): `mid` and `leaf` are placed by sheet
//! symbols AND listed, `other` is listed and placed by nothing.

use std::path::PathBuf;

use signex_output::{BomOptions, bom};

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::handlers::menu::export::{build_bom_export_context, build_export_scope};

pub(crate) const DIR: &str = "/w/tree-pins";
pub(crate) const LISTED: [&str; 4] = ["top.snxsch", "other.snxsch", "mid.snxsch", "leaf.snxsch"];

/// `top` -> `mid` -> `leaf`, plus the loose `other`; every sheet open.
pub(crate) fn hierarchical_app() -> crate::app::Signex {
    let mut app = app_workspace(DIR, &LISTED);
    let path = |name: &str| PathBuf::from(DIR).join(name);
    for (name, reference, children) in [
        ("top.snxsch", "R_TOP", &["mid.snxsch"][..]),
        ("other.snxsch", "R_OTHER", &[][..]),
        ("mid.snxsch", "R_MID", &["leaf.snxsch"][..]),
        ("leaf.snxsch", "R_LEAF", &[][..]),
    ] {
        open_with(
            &mut app.document_state,
            &path(name),
            sheet_with_net(reference, &format!("NET_{reference}"), children),
        );
    }
    app.document_state.active_path = Some(path("top.snxsch"));
    app
}

#[test]
fn numbering_and_pdf_order_follow_the_walk_not_the_listed_order() {
    let app = hierarchical_app();
    let (ctx, _) = build_export_scope(&app.document_state).expect("export scope");
    let printed: Vec<(String, usize, usize)> = ctx
        .sheets
        .iter()
        .map(|page| (page.sheet_name.clone(), page.sheet_number, page.sheet_count))
        .collect();
    assert_eq!(
        printed,
        vec![
            ("top.snxsch".to_string(), 1, 4),
            ("mid.snxsch".to_string(), 2, 4),
            ("leaf.snxsch".to_string(), 3, 4),
            ("other.snxsch".to_string(), 4, 4),
        ]
    );
}

#[test]
fn the_declared_pages_and_the_pages_outside_the_hierarchy_are_unchanged() {
    let app = hierarchical_app();
    let project = &app.document_state.projects[0];
    let declared: Vec<PathBuf> = crate::app::project_sheets::project_pages(Some(project));
    let expected: Vec<PathBuf> = LISTED.iter().map(|f| PathBuf::from(DIR).join(f)).collect();
    assert_eq!(declared, expected, "the `.snxprj` list keeps its order");

    let root = crate::app::project_sheets::project_root_sheet_path(project).expect("root");
    let set = crate::app::project_sheets::assemble_project_sheets(
        &app.document_state,
        &declared,
        &root,
        project.case_fold,
    );
    assert_eq!(
        set.pages_outside_the_hierarchy,
        vec![PathBuf::from(DIR).join("other.snxsch")]
    );
}

#[test]
fn the_bom_counts_every_listed_and_placed_sheet_once() {
    let app = hierarchical_app();
    let ctx = build_bom_export_context(&app.document_state).expect("scope");
    let table = bom::rollup(&ctx, &BomOptions::default());
    let mut refs: Vec<String> = table
        .rows
        .iter()
        .flat_map(|row| row.references.clone())
        .collect();
    refs.sort();
    assert_eq!(refs, ["R_LEAF", "R_MID", "R_OTHER", "R_TOP"]);
}
