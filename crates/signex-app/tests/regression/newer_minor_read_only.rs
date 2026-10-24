//! #104 — a schematic written by a newer minor format loads for viewing
//! but Save is refused: the document stays dirty, the file is untouched,
//! and the refusal reaches the Save Failed card and the Messages panel.

use signex_app::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use signex_types::schematic::SchematicSheet;
use tempfile::TempDir;

#[test]
fn saving_a_newer_minor_schematic_is_refused_and_stays_dirty() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Newer.snxsch");
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise");
    let newer = text.replacen("format = \"snxsch/1\"", "format = \"snxsch/1.7\"", 1);
    assert_ne!(newer, text, "fixture must carry the newer token");
    std::fs::write(&path, &newer).expect("write fixture");

    let engine = signex_engine::Engine::open(&path).expect("a newer minor still loads");
    let (mut app, _t) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "Newer".into(),
        path: path.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.document_state.dirty_paths.insert(path.clone());

    let _ = app.update(Message::File(FileMsg::Save));

    assert_eq!(std::fs::read_to_string(&path).unwrap(), newer, "untouched");
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(app.document_state.tabs[0].dirty);
    assert!(app.document_state.error_notice.is_some());
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("newer Signex") && e.message.contains("snxsch/1.7")),
        "refusal must reach the Messages panel"
    );
}
