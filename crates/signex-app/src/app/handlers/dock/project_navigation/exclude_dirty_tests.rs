//! #738 / #694 — the Exclude prompt keys paths by the dialog project's probed
//! volume rule, not by the host family. Unit tests because `CaseFold` is
//! crate-private, so an integration test cannot build a folding project.

use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::scope::CaseFold;
use crate::app::{ExcludeDirtyChoice, Message, RemoveChoice, RemoveDialogState, RemoveMsg, Signex};

const ORIGINAL: &[u8] = b"original-bytes";

/// A project on a case-insensitive volume listing `X.snxsch`, with that sheet
/// held dirty under the spelling `x.snxsch`, and an Exclude dialog opened on
/// `X.snxsch`. The probe says "folds" on every host, so a host-family fold
/// would disagree on macOS and Linux.
fn folding_project_with_dirty_lowercase_sheet() -> (Signex, tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_str().expect("utf8 tempdir").to_string();
    let mut app = app_workspace(&dir, &["X.snxsch"]);
    app.document_state.projects[0].case_fold = CaseFold::from_probe(true);
    let held = tmp.path().join("x.snxsch");
    std::fs::write(&held, ORIGINAL).expect("write sheet");
    let mut engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default())
        .expect("engine");
    engine.set_path(Some(held.clone()));
    app.document_state.engines.insert(held.clone(), engine);
    app.document_state.dirty_paths.insert(held.clone());
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: tmp.path().join("X.snxsch"),
        tree_path: vec![0, 0],
        project_id: Some(app.document_state.projects[0].id),
        display_name: "X.snxsch".to_string(),
        is_outside_project: false,
        unsaved_prompt: false,
    });
    (app, tmp, held)
}

fn exclude_then(app: &mut Signex, choice: ExcludeDirtyChoice) {
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::ExcludeFromProject,
    )));
    assert!(
        app.ui_state
            .remove_dialog
            .as_ref()
            .is_some_and(|d| d.unsaved_prompt),
        "a dirty sheet held under a case variant must still prompt"
    );
    let _ = app.update(Message::Remove(RemoveMsg::ExcludeDirty(choice)));
}

fn project_lists_sheet(app: &Signex) -> bool {
    app.document_state.projects[0]
        .data
        .sheets
        .iter()
        .any(|s| s.filename == "X.snxsch")
}

#[test]
fn save_finds_a_dirty_sheet_under_a_case_variant_on_a_folding_volume() {
    let (mut app, _tmp, held) = folding_project_with_dirty_lowercase_sheet();

    exclude_then(&mut app, ExcludeDirtyChoice::Save);

    assert_ne!(std::fs::read(&held).unwrap(), ORIGINAL, "Save must write");
    assert!(!app.document_state.dirty_paths.contains(&held));
    assert!(app.ui_state.remove_dialog.is_none());
    assert!(!project_lists_sheet(&app));
}

#[test]
fn discard_drops_a_dirty_sheet_under_a_case_variant_on_a_folding_volume() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _tmp, held) = folding_project_with_dirty_lowercase_sheet();

    exclude_then(&mut app, ExcludeDirtyChoice::Discard);

    assert_eq!(std::fs::read(&held).unwrap(), ORIGINAL, "must not write");
    assert!(!app.document_state.dirty_paths.contains(&held));
    assert!(!app.document_state.engines.contains_key(Path::new(&held)));
    assert!(!project_lists_sheet(&app));
}
