//! #104 — a schematic written by a newer minor format loads for viewing
//! but Save is refused: the document stays dirty, the file is untouched,
//! and the refusal reaches the Save Failed card and the Messages panel.

use signex_app::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use signex_i18n::RenderTranslatable;
use signex_types::schematic::SchematicSheet;
use tempfile::TempDir;

#[test]
fn saving_a_newer_minor_schematic_is_refused_and_stays_dirty() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Newer.snxsch");
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise");
    let newer = text.replacen("format = \"snxsch/1.1\"", "format = \"snxsch/1.7\"", 1);
    assert_ne!(newer, text, "fixture must carry the newer token");
    std::fs::write(&path, &newer).expect("write fixture");

    let engine = signex_engine::Engine::open(&path).expect("a newer minor still loads");
    let (mut app, _t) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "Newer".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.document_state.mark_document_dirty(&path);

    let _ = app.update(Message::File(FileMsg::Save));

    assert_eq!(std::fs::read_to_string(&path).unwrap(), newer, "untouched");
    assert!(app.document_state.is_document_dirty(&path));
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.tabs[0].path)
    );
    // The card carries the full cause: the operation as its sentence and
    // the error chain in its system block (mockup row 11). The Messages panel line is
    // trimmed to 160 bytes, so only its leading context is matched.
    let notice = app.document_state.error_notice.as_ref().expect("card");
    assert_eq!(
        notice.detail.render_en_us(),
        "Failed to save active document"
    );
    let system = notice.system.as_ref().map(|s| s.as_str()).unwrap_or("");
    assert!(
        system.contains("newer Signex") && system.contains("snxsch/1.7"),
        "{notice:?}"
    );
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Failed to save active document")),
        "refusal must reach the Messages panel"
    );
}

/// A project with nothing in it, as this build writes it.
fn bare_project() -> signex_types::project::ProjectData {
    signex_types::project::ProjectData {
        document_id: None,
        description: Some("as saved".to_string()),
        name: "Newer".to_string(),
        dir: String::new(),
        schematic_root: None,
        pcb_file: None,
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        variant_settings: Default::default(),
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// Write `Newer.snxprj` into `dir` with the format token `snxprj/1.2`, a
/// newer minor than this build writes, and return the bytes on disk.
fn write_newer_minor_project(dir: &std::path::Path) -> (std::path::PathBuf, Vec<u8>) {
    let path = dir.join("Newer.snxprj");
    signex_types::project::write_project(&path, &bare_project()).expect("write project");
    let mut body: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).expect("project JSON");
    body["format"] = serde_json::json!("snxprj/1.2");
    let bytes = serde_json::to_vec_pretty(&body).expect("encode");
    std::fs::write(&path, &bytes).expect("write fixture");
    (path, bytes)
}

fn file_names(dir: &std::path::Path) -> Vec<std::ffi::OsString> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .expect("list")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    names.sort();
    names
}

/// A project read from a newer minor opens, but its Save is refused before
/// anything is written: the `.snxprj` keeps its bytes, the pending library
/// is not created, the project stays modified and the refusal is shown.
#[test]
fn saving_a_newer_minor_project_writes_nothing_and_creates_no_library() {
    use signex_app::library::messages::LibraryMessage;
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (file, before) = write_newer_minor_project(tmp.path());
    let (mut app, _boot) = Signex::new();
    crate::settle_support::settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(file.clone()))),
    );
    let prj = app
        .document_state
        .projects
        .first()
        .expect("a newer minor project still opens")
        .path
        .clone();
    assert!(app.document_state.projects[0].data.is_from_newer_minor());

    app.document_state.projects[0].data.description = Some("typed".to_string());
    app.document_state.mark_document_dirty(&prj);
    let lib = tmp.path().join("NewerParts.snxlib");
    let _ = app.update(Message::Library(LibraryMessage::CreateLibraryAtPath {
        project_path: prj.clone(),
        lib_path: lib.clone(),
    }));
    let _ = app.update(Message::Library(
        LibraryMessage::LibraryCreateOptionsConfirm,
    ));
    assert_eq!(
        app.document_state.projects[0].pending_libraries.len(),
        1,
        "the library waits for the save"
    );
    let listed = file_names(tmp.path());
    app.document_state.error_notice = None;

    crate::settle_support::settle(&mut app, Message::File(FileMsg::Save));

    assert_eq!(std::fs::read(&file).unwrap(), before, "byte-identical");
    assert!(!lib.exists(), "no library is created");
    assert_eq!(file_names(tmp.path()), listed, "no file was created");
    assert_eq!(
        app.document_state.projects[0].pending_libraries.len(),
        1,
        "the library is still waiting"
    );
    assert!(app.document_state.is_document_dirty(&prj));
    let notice = app.document_state.error_notice.as_ref().expect("card");
    let system = notice.system.as_ref().map(|s| s.as_str()).unwrap_or("");
    assert!(
        system.contains("read-only") && system.contains("snxprj/1.2"),
        "{notice:?}"
    );
}
