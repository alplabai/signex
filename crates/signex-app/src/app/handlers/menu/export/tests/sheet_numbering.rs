//! Project sheet numbering in the export scope (#1081): one number per sheet
//! placement, from the hierarchy, the same whichever pages an export prints.

use std::path::PathBuf;

use signex_types::schematic::SheetInstance;

use super::protel_hierarchy::placed_app;
use super::{open_with, sheet_with_net};
use crate::app::handlers::menu::export::build_export_scope;

/// `(page name, sheet number, sheet count)` for every printed page.
fn printed(app: &crate::app::Signex) -> Vec<(String, usize, usize)> {
    let (ctx, _) = build_export_scope(&app.document_state).expect("export scope");
    ctx.sheets
        .iter()
        .map(|page| (page.sheet_name.clone(), page.sheet_number, page.sheet_count))
        .collect()
}

/// The export's `Sheet numbering:` Messages lines (the stitcher's own lines,
/// such as the twice-placed child's shared designator, are left out).
fn numbering_lines(app: &crate::app::Signex) -> Vec<String> {
    let (_, issues) = build_export_scope(&app.document_state).expect("export scope");
    issues
        .messages()
        .into_iter()
        .filter(|line| line.starts_with("Sheet numbering:"))
        .collect()
}

/// Put `entry` on `top.snxsch`'s sheet symbol `symbol`.
fn with_top_entry(app: &mut crate::app::Signex, dir: &str, symbol: usize, entry: SheetInstance) {
    let top_path = PathBuf::from(dir).join("top.snxsch");
    let mut top = app.document_state.engines[&top_path].document().clone();
    top.child_sheets[symbol].instances.push(entry);
    open_with(&mut app.document_state, &top_path, top);
}

fn root_entry(page: &str) -> SheetInstance {
    SheetInstance {
        project: String::new(),
        path: "/".to_string(),
        page: page.to_string(),
    }
}

#[test]
fn pages_take_their_number_from_the_hierarchy_not_the_declared_list() {
    let dir = "/w/numbering-order";
    let mut app = placed_app(
        dir,
        &["top.snxsch", "other.snxsch", "child.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    open_with(
        &mut app.document_state,
        &PathBuf::from(dir).join("other.snxsch"),
        sheet_with_net("R_OTHER", "OTHER_NET", &[]),
    );
    assert_eq!(
        printed(&app),
        vec![
            ("top.snxsch".to_string(), 1, 4),
            ("other.snxsch".to_string(), 4, 4),
            ("child.snxsch (A)".to_string(), 2, 4),
            ("child.snxsch (B)".to_string(), 3, 4),
        ]
    );
}

#[test]
fn the_count_includes_placements_that_are_not_printed_pages() {
    let app = placed_app(
        "/w/numbering-count",
        &["top.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    assert_eq!(printed(&app), vec![("top.snxsch".to_string(), 1, 3)]);
}

#[test]
fn a_subset_export_keeps_the_project_numbers() {
    let app = placed_app(
        "/w/numbering-subset",
        &["top.snxsch", "child.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    let (mut ctx, _) = build_export_scope(&app.document_state).expect("export scope");
    ctx.sheets
        .retain(|page| page.sheet_name == "child.snxsch (B)");
    let only: Vec<(usize, usize)> = ctx
        .sheets
        .iter()
        .map(|page| (page.sheet_number, page.sheet_count))
        .collect();
    assert_eq!(only, [(3, 3)]);
}

#[test]
fn a_manual_number_is_printed_and_reported() {
    let dir = "/w/numbering-manual";
    let mut app = placed_app(
        dir,
        &["top.snxsch", "child.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    with_top_entry(&mut app, dir, 1, root_entry("7"));
    assert_eq!(
        printed(&app),
        vec![
            ("top.snxsch".to_string(), 1, 3),
            ("child.snxsch (A)".to_string(), 2, 3),
            ("child.snxsch (B)".to_string(), 7, 3),
        ]
    );
    assert_eq!(
        numbering_lines(&app),
        vec![
            "Sheet numbering: page B is numbered 7, above the page count 3".to_string(),
            "Sheet numbering: no page prints number 3".to_string(),
        ]
    );
}

#[test]
fn an_unusable_manual_number_is_reported_and_ignored() {
    let dir = "/w/numbering-invalid";
    let mut app = placed_app(
        dir,
        &["top.snxsch", "child.snxsch"],
        &["A", "B"],
        Vec::new(),
    );
    with_top_entry(&mut app, dir, 0, root_entry("abc"));
    assert_eq!(printed(&app)[1], ("child.snxsch (A)".to_string(), 2, 3));
    assert_eq!(
        numbering_lines(&app),
        vec![
            "Sheet numbering: page A has the manual number 'abc', which is not a positive whole \
             number; it is numbered 2"
                .to_string()
        ]
    );
}
