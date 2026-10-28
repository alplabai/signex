//! #768 — a sheet whose objects together carry more foreign TSV columns
//! than the cap cannot be saved; the refusal must name the objects to
//! delete, as lines of their own in the Messages panel (each entry is
//! trimmed to 160 bytes, so one long line would lose them).

use signex_app::app::{
    FileMsg, Message, ProjectCloseChoice, ProjectMsg, Signex, TabInfo, TabKind, WindowMsg,
};
use signex_types::schematic::{Junction, SchematicSheet};
use tempfile::TempDir;

fn junction_with_columns(uuid: uuid::Uuid, range: std::ops::Range<usize>) -> Junction {
    let columns: Vec<(String, String)> =
        range.map(|i| (format!("c{i}"), "v".to_string())).collect();
    serde_json::from_value(serde_json::json!({
        "uuid": uuid,
        "position": { "x": 0.0, "y": 0.0 },
        "unknown_columns": columns,
    }))
    .expect("junction fixture")
}

/// An app with one dirty sheet whose two junctions together carry 400
/// foreign columns (cap 256); only the second holds columns past it.
fn app_with_over_cap_sheet(
    path: &std::path::Path,
    first: uuid::Uuid,
    second: uuid::Uuid,
) -> Signex {
    // Each junction alone is within the 256 cap; together they are 400.
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        junctions: vec![
            junction_with_columns(first, 0..200),
            junction_with_columns(second, 200..400),
        ],
        ..Default::default()
    };
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(path.to_path_buf()));
    let (mut app, _t) = Signex::new();
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "Merged".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.to_path_buf());
    app.document_state.dirty_paths.insert(path.to_path_buf());
    app
}

/// Messages entries naming `id` within the 160-byte entry cap.
fn names(id: uuid::Uuid) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(&id.to_string()) && e.message.len() <= 160)
}

fn mentions(id: uuid::Uuid) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(&id.to_string()))
}

#[test]
fn over_cap_save_names_the_offending_objects_in_messages() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Merged.snxsch");
    let first = uuid::Uuid::new_v4();
    let second = uuid::Uuid::new_v4();
    let mut app = app_with_over_cap_sheet(&path, first, second);

    let _ = app.update(Message::File(FileMsg::Save));

    assert!(!path.exists(), "a refused save writes nothing");
    assert!(app.document_state.dirty_paths.contains(&path));
    // Only the second junction holds a column past the cap (c256..c399).
    assert!(names(second), "Messages must name the object to delete");
    assert!(
        !mentions(first),
        "the first junction carries no column over the cap"
    );
}

#[test]
fn save_all_on_exit_names_the_offending_objects_in_messages() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Exit.snxsch");
    let first = uuid::Uuid::new_v4();
    let second = uuid::Uuid::new_v4();
    let mut app = app_with_over_cap_sheet(&path, first, second);

    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    // Save All probes the volume off-thread first, then writes (#1216).
    crate::settle_support::settle(
        &mut app,
        Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::SaveAll)),
    );

    assert!(!path.exists(), "a refused save writes nothing");
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(names(second), "Save All must name the object to delete");
    assert!(
        !mentions(first),
        "the first junction carries no column over the cap"
    );
}
