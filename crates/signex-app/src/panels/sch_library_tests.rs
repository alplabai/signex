//! #2262: the SCH Library panel does not repeat its dock tab's title, and
//! its breadcrumb counts symbols in the right plural.

use super::*;
use crate::app::{Signex, SymbolDocument, TabInfo, TabKind};
use crate::layout_probe::lay_out_texts;

/// An app with a one-symbol `.snxsym` open in a symbol editor tab.
fn app_with_one_symbol() -> Signex {
    let (mut app, _boot) = Signex::new();
    let path = std::path::PathBuf::from("power.snxsym");
    app.document_state.tabs.push(TabInfo {
        title: "power".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("+3V3"));
    let editor = SymbolDocument::new(path.clone(), file);
    app.document_state.symbol_editors.insert(path, editor);
    app.refresh_panel_ctx();
    app
}

fn texts(ctx: &PanelContext) -> Vec<String> {
    lay_out_texts(view_sch_library(ctx), iced::Size::new(240.0, 600.0))
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

#[test]
fn the_panel_has_no_title_row_and_counts_one_symbol_in_the_singular() {
    let app = app_with_one_symbol();
    let ctx = &app.document_state.panel_ctx;
    assert!(
        ctx.symbol_editor.is_some(),
        "the symbol editor reaches the panel"
    );
    let shown = texts(ctx);
    assert!(
        shown.iter().all(|t| t != "SCH Library"),
        "the dock tab already names the panel: {shown:#?}"
    );
    assert!(
        shown.iter().any(|t| t.ends_with("(1 symbol)")),
        "singular count: {shown:#?}"
    );
    assert!(shown.iter().any(|t| t == "+3V3"), "{shown:#?}");
}
