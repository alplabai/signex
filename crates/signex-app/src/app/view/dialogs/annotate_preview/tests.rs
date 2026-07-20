//! The Annotate preview and the Annotate action must describe the same sheets.
//!
//! Round 5 repointed `handle_annotate` at the shared assembler and left the
//! preview on its own rule. The dialog the user approves and the operation that
//! runs then described *different sheet sets*, in both directions: the preview
//! hid the unlisted hierarchical children the action renumbers and saves to
//! disk, and listed loose tabs the action refuses to touch (#406).
//!
//! These drive the real preview against the real handler over one fixture, so a
//! future change to either alone goes red.

use std::path::PathBuf;

use signex_types::format::SnxSchematic;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::documents::{TabInfo, TabKind};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};

/// A project listing only `top.snxsch`, which is open and references
/// `child.snxsch` — a hierarchical child sitting unlisted and unopened on disk.
/// Both hold one unannotated `R?`. Plus a loose tab from nowhere, also `R?`.
///
/// Returns the app and the temp dir (caller cleans up).
fn fixture() -> (Signex, PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-annotate-preview-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");

    let child = SnxSchematic::new(sheet_with_net("R?", "CHILD_NET", &[]))
        .write_string()
        .expect("serialize child");
    std::fs::write(dir.join("child.snxsch"), child).expect("write child");

    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let top = dir.join("top.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R?", "TOP_NET", &["child.snxsch"]),
    );
    let loose = PathBuf::from("/w/loose/scratch.snxsch");
    open_with(
        &mut app.document_state,
        &loose,
        sheet_with_net("R?", "LOOSE_NET", &[]),
    );
    // Tabs, not just engines: the old preview seeded from `tabs`, and
    // `handle_annotate`'s cached-tab pass keys off the active tab index — so
    // the loose tab must be a non-active tab for its exclusion to mean
    // anything.
    app.document_state.tabs.push(tab("top", top.clone()));
    app.document_state.tabs.push(tab("scratch", loose));
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(top);
    (app, dir)
}

fn tab(title: &str, path: PathBuf) -> TabInfo {
    TabInfo {
        title: title.to_string(),
        path,
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    }
}

/// The proposed designator the preview promises for the sheet titled `sheet`.
fn proposed_for(app: &Signex, sheet: &str) -> Option<String> {
    app.preview_project_annotations()
        .into_iter()
        .find(|e| e.sheet == sheet)
        .map(|e| e.proposed)
}

#[test]
fn the_preview_covers_every_sheet_the_action_writes() {
    let (mut app, dir) = fixture();

    let promised_child = proposed_for(&app, "child");
    let promised_top = proposed_for(&app, "top");

    let _ = app.handle_annotate(signex_engine::AnnotateMode::Incremental);

    let written = std::fs::read_to_string(dir.join("child.snxsch")).expect("child still on disk");
    let child_after: Vec<String> = SnxSchematic::parse(&written)
        .expect("child parses")
        .sheet
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect();
    let top_after: Vec<String> = app
        .document_state
        .engines
        .get(&dir.join("top.snxsch"))
        .expect("top engine")
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect();
    std::fs::remove_dir_all(&dir).ok();

    // The action renumbers the unlisted child and saves the file. The preview
    // must have shown that row — otherwise Signex writes a schematic the user
    // was never shown.
    assert_eq!(
        promised_child.as_deref(),
        child_after.first().map(String::as_str),
        "the preview must promise exactly what the action writes to the child \
         file on disk (preview said {promised_child:?}, disk holds {child_after:?})"
    );
    assert_eq!(
        promised_top.as_deref(),
        top_after.first().map(String::as_str),
        "…and the same for the active sheet (preview said {promised_top:?}, \
         engine holds {top_after:?})"
    );
    assert!(
        child_after.first().is_some_and(|r| !r.ends_with('?')),
        "precondition: the action does renumber the child: {child_after:?}"
    );
}

#[test]
fn the_preview_omits_tabs_the_action_refuses_to_touch() {
    let (mut app, dir) = fixture();

    let rows = app.preview_project_annotations();
    let sheets: Vec<&str> = rows.iter().map(|e| e.sheet.as_str()).collect();
    assert!(
        !sheets.contains(&"scratch"),
        "a loose tab is not part of this project — the action leaves it alone, \
         so the preview must not offer to renumber it: {sheets:?}"
    );

    let _ = app.handle_annotate(signex_engine::AnnotateMode::Incremental);
    let loose_after = app
        .document_state
        .engines
        .get(&PathBuf::from("/w/loose/scratch.snxsch"))
        .expect("loose engine")
        .document()
        .symbols
        .first()
        .map(|s| s.reference.clone());
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(
        loose_after.as_deref(),
        Some("R?"),
        "precondition: the action really does leave the loose tab untouched"
    );
}
