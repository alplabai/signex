//! The Annotate preview and the Annotate action must describe the same sheets
//! AND promise the same designator on each.
//!
//! Round 5 repointed `handle_annotate` at the shared assembler and left the
//! preview on its own rule. The dialog the user approves and the operation that
//! runs then described *different sheet sets*, in both directions: the preview
//! hid the unlisted hierarchical children the action renumbers and saves to
//! disk, and listed loose tabs the action refuses to touch (#406).
//!
//! Fixing the sheet *set* left the *order* still disagreeing: the preview
//! walked sorted-path order while the action walked cached tabs in tab order,
//! then unopened sheets sorted, then the active engine unconditionally last —
//! so a project whose active sheet doesn't happen to sort last got a preview
//! that promised one designator and an action that assigned another (#435).
//! [`crate::app::project_sheets::ordered_project_sheet_paths`] is now the one
//! walk both sides use.
//!
//! These drive the real preview against the real handler over one fixture, so a
//! future change to either alone goes red.

use std::path::PathBuf;

use signex_types::format::SnxSchematic;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::documents::{TabInfo, TabKind};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};

/// A project listing only `a_root.snxsch`, which is open (and active) and
/// references `z_child.snxsch` — a hierarchical child sitting unlisted and
/// unopened on disk. Both hold one unannotated `R?`. Plus a loose tab from
/// nowhere, also `R?`.
///
/// The names are deliberately adversarial to ordering: `a_root` sorts BEFORE
/// `z_child`, the opposite of what the old action's walk order gave the
/// active sheet (unopened sheets first, active engine unconditionally last).
/// A fixture where the active sheet already sorted last (as `top`/`child` did
/// before #435) can't tell sorted-path order and "active-last" apart — this
/// one can.
///
/// Returns the app and the temp dir (caller cleans up).
fn fixture() -> (Signex, PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-annotate-preview-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");

    let child = SnxSchematic::new(sheet_with_net("R?", "CHILD_NET", &[]))
        .write_string()
        .expect("serialize child");
    std::fs::write(dir.join("z_child.snxsch"), child).expect("write child");

    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    let root = dir.join("a_root.snxsch");
    open_with(
        &mut app.document_state,
        &root,
        sheet_with_net("R?", "ROOT_NET", &["z_child.snxsch"]),
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
    app.document_state.tabs.push(tab("a_root", root.clone()));
    app.document_state.tabs.push(tab("scratch", loose));
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root);
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
fn the_preview_promises_what_the_action_assigns() {
    let (mut app, dir) = fixture();

    let promised_child = proposed_for(&app, "z_child");
    let promised_root = proposed_for(&app, "a_root");

    let _ = app.handle_annotate(signex_engine::AnnotateMode::Incremental);

    let written = std::fs::read_to_string(dir.join("z_child.snxsch")).expect("child still on disk");
    let child_after: Vec<String> = SnxSchematic::parse(&written)
        .expect("child parses")
        .sheet
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect();
    let root_after: Vec<String> = app
        .document_state
        .engines
        .get(&dir.join("a_root.snxsch"))
        .expect("root engine")
        .document()
        .symbols
        .iter()
        .map(|s| s.reference.clone())
        .collect();
    std::fs::remove_dir_all(&dir).ok();

    // The action renumbers the unlisted child and saves the file. The preview
    // must have shown that row — otherwise Signex writes a schematic the user
    // was never shown (#406) — AND it must be the SAME number the action
    // assigns, not just a row with the right sheet name (#435): `a_root`
    // sorts before `z_child`, so a preview/action pair that disagree on walk
    // order swap these two numbers rather than merely omitting one.
    assert_eq!(
        promised_child.as_deref(),
        child_after.first().map(String::as_str),
        "the preview must promise exactly what the action writes to the child \
         file on disk (preview said {promised_child:?}, disk holds {child_after:?})"
    );
    assert_eq!(
        promised_root.as_deref(),
        root_after.first().map(String::as_str),
        "…and the same for the active sheet (preview said {promised_root:?}, \
         engine holds {root_after:?})"
    );
    assert!(
        child_after.first().is_some_and(|r| !r.ends_with('?')),
        "precondition: the action does renumber the child: {child_after:?}"
    );
    assert_ne!(
        promised_child, promised_root,
        "precondition: root and child must land on distinct numbers for this \
         fixture to be able to catch a swap"
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
