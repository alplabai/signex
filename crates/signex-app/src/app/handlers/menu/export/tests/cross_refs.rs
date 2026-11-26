//! #1273: the export scope carries the off-sheet cross references the PDF
//! and its preview print, numbered as the pages are.

use std::path::PathBuf;

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::handlers::menu::export::build_page_scope;

/// Two flat pages, each with a Global VCC label: each names the other page.
#[test]
fn the_scope_names_the_other_page_of_a_global_label() {
    let dir = "/w/xref";
    let mut app = app_workspace(dir, &["top.snxsch", "power.snxsch"]);
    let top = PathBuf::from(dir).join("top.snxsch");
    let power = PathBuf::from(dir).join("power.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R1", "VCC", &[]),
    );
    open_with(
        &mut app.document_state,
        &power,
        sheet_with_net("R2", "VCC", &[]),
    );
    app.document_state.active_path = Some(top.clone());

    let (ctx, issues) = build_page_scope(&app.document_state).expect("scope");
    let mut numbers: Vec<(usize, PathBuf)> = ctx
        .sheets
        .iter()
        .map(|page| (page.sheet_number, page.path.clone()))
        .collect();
    numbers.sort();
    assert_eq!(numbers, [(1, top.clone()), (2, power.clone())]);

    let refs = issues
        .sheet_numbering
        .as_ref()
        .expect("a project numbering")
        .cross_refs();
    let pages_of = |path: &PathBuf| -> Vec<usize> {
        let page = refs.for_page(path, None).expect("a page with a mark");
        assert_eq!(page.labels.len(), 1, "{page:?}");
        page.labels[0].1.iter().map(|target| target.page).collect()
    };
    assert_eq!(pages_of(&top), [2]);
    assert_eq!(pages_of(&power), [1]);
}
