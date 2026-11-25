//! #2651 — Save As re-keys the active schematic onto the new path, and a
//! panic while it runs leaves the engine where it was (ADR-0006).

use std::path::{Path, PathBuf};

use crate::app::state::{RememberedPageSetup, WindowKind};
use crate::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use crate::persist::{DocumentSaveDone, SaveOutcome};

/// An active, dirty schematic tab at `dir/a.snxsch` with a remembered page
/// setup. Returns the app, the old and the new path, and the revision.
fn app_with_dirty_sheet(dir: &Path) -> (Signex, PathBuf, PathBuf, u64) {
    let (mut app, _boot) = Signex::new();
    let from = dir.join("a.snxsch");
    let to = dir.join("b.snxsch");
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        ..Default::default()
    };
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(from.clone()));
    let revision = engine.revision();
    app.document_state.engines.insert(from.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "a".to_string(),
        path: from.clone(),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(from.clone());
    app.document_state.mark_document_dirty(&from);
    app.document_state.page_setup_memory.insert(
        from.clone(),
        RememberedPageSetup::capture(&signex_output::PdfOptions::default()),
    );
    (app, from, to, revision)
}

fn active_tab(app: &Signex) -> &TabInfo {
    &app.document_state.tabs[app.document_state.active_tab]
}

#[test]
fn a_save_as_rekey_moves_engine_tab_dirty_mark_and_windows_together() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, from, to, revision) = app_with_dirty_sheet(dir.path());
    // An undocked window shows the sheet, so the completion runs in it.
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: from.clone(),
            title: "a".into(),
        },
    );
    app.interaction_state
        .canvases
        .insert(window, crate::canvas::CanvasSlot::new());

    let _ = app.update(Message::File(FileMsg::DocumentSaved(DocumentSaveDone {
        path: to.clone(),
        revision,
        save_as_from: Some(from.clone()),
        bytes: std::sync::Arc::new(b"saved".to_vec()),
        sequence: 0,
        result: Ok(SaveOutcome::Written),
        recovery_hold: None,
    })));

    let engines = &app.document_state.engines;
    assert!(
        !engines.contains_key(&from),
        "no engine is left at the old path"
    );
    assert_eq!(engines.get(&to).and_then(|e| e.path()), Some(to.as_path()));
    assert_eq!(active_tab(&app).path, to);
    assert_eq!(active_tab(&app).title, "b");
    assert!(!app.document_state.is_document_dirty(&from));
    assert!(
        !app.document_state.is_document_dirty(&to),
        "the written revision is the saved one"
    );
    assert!(app.ui_state.windows.values().all(|kind| matches!(
        kind,
        WindowKind::UndockedTab { path, .. } if *path == to
    )));
    assert!(app.document_state.page_setup_memory.contains_key(&to));
    assert!(!app.document_state.page_setup_memory.contains_key(&from));
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(to.as_path())
    );
}

#[test]
fn a_panicking_save_as_keeps_the_engine_at_its_old_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, from, to, _revision) = app_with_dirty_sheet(dir.path());

    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app.rekey_active_schematic(to.clone(), |_| panic!("injected"))
    }));

    assert!(unwound.is_err(), "the injected panic must reach the caller");
    let engines = &app.document_state.engines;
    assert_eq!(
        engines.get(&from).and_then(|e| e.path()),
        Some(from.as_path()),
        "the engine stays in the map at its old path"
    );
    assert!(!engines.contains_key(&to), "no engine exists at the target");
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(from.as_path())
    );
    assert_eq!(active_tab(&app).path, from);
    assert_eq!(active_tab(&app).title, "a");
    assert!(app.document_state.is_document_dirty(&from));
}
