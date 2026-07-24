//! #99 — schematic/PCB file-open read+parse is async.
//!
//! `open_schematic_file` / `open_pcb_file` used to `fs::read_to_string`
//! + parse synchronously inside `update()`. They now return a
//! `Task::perform` (`spawn_blocking` body) that completes with
//! `FileMsg::SchematicOpenFinished` / `FileMsg::PcbOpenFinished` —
//! mirrors the `HistoryLoaded` pattern. These tests pin both halves:
//! `update()` must not synchronously open the tab, and the completion
//! message must apply exactly like the old inline path did (success
//! opens the tab, failure logs and opens nothing).

use signex_app::app::{FileMsg, Message, Signex};

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// The general "exercise dispatchers without the iced runtime" methodology
// this whole regression suite follows lives in the parent module doc at
// `tests/regression.rs`.

/// Minimal but valid `SchematicSheet` fixture — same shape as
/// `signex_types::format::tests::empty_sheet`.
fn empty_schematic_sheet() -> signex_types::schematic::SchematicSheet {
    signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: "A4".into(),
        root_sheet_page: "1".into(),
        symbols: vec![],
        wires: vec![],
        junctions: vec![],
        labels: vec![],
        child_sheets: vec![],
        no_connects: vec![],
        text_notes: vec![],
        buses: vec![],
        bus_entries: vec![],
        drawings: vec![],
        no_erc_directives: vec![],
        title_block: Default::default(),
        lib_symbols: Default::default(),
    }
}

#[test]
fn opening_a_schematic_does_not_synchronously_create_a_tab() {
    let tmp = TempDir::new().expect("tempdir");
    let sch_path = tmp.path().join("Async.snxsch");
    let sheet = empty_schematic_sheet();
    let serialised = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise schematic");
    fs::write(&sch_path, serialised).expect("write .snxsch");

    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::File(FileMsg::Opened(Some(sch_path))));

    assert!(
        app.document_state.tabs.is_empty(),
        "the read+parse is now a Task::perform off the UI thread — update() \
         must return before a tab exists, not open it inline"
    );
}

#[test]
fn schematic_open_finished_ok_opens_the_tab_like_the_old_sync_path_did() {
    let path = PathBuf::from("/tmp/does-not-matter/board.snxsch");
    let sheet = empty_schematic_sheet();
    let sheet_uuid = sheet.uuid;

    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: path.clone(),
        title: "board".to_string(),
        result: Ok(Box::new(sheet)),
    }));

    let tab = app
        .document_state
        .tabs
        .iter()
        .find(|t| t.path == path)
        .expect("SchematicOpenFinished(Ok) must open a tab for the completed path");
    assert_eq!(tab.title, "board");
    assert_eq!(
        app.document_state
            .engines
            .get(&path)
            .map(|engine| engine.document().uuid),
        Some(sheet_uuid),
        "the engine keyed by the opened path must hold the schematic carried \
         by the completion message"
    );
}

#[test]
fn schematic_open_finished_err_opens_no_tab_and_does_not_panic() {
    let path = PathBuf::from("/tmp/does-not-matter/broken.snxsch");

    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: path.clone(),
        title: "broken".to_string(),
        result: Err("parse broken.snxsch: TSV block malformed".to_string()),
    }));

    assert!(
        app.document_state.tabs.is_empty(),
        "a failed async parse must not open a tab, matching the old \
         open_document_path Err early-return"
    );
}

/// Minimal but valid `PcbBoard` fixture — same shape as
/// `signex_types::format::tests::empty_board`.
fn empty_pcb_board() -> signex_types::pcb::PcbBoard {
    signex_types::pcb::PcbBoard {
        uuid: uuid::Uuid::new_v4(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

#[test]
fn pcb_open_finished_ok_opens_the_tab_like_the_old_sync_path_did() {
    use signex_app::app::TabDocument;

    let path = PathBuf::from("/tmp/does-not-matter/board.snxpcb");
    let board = empty_pcb_board();
    let board_uuid = board.uuid;

    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        path: path.clone(),
        title: "board".to_string(),
        result: Ok(Box::new(board)),
    }));

    let tab = app
        .document_state
        .tabs
        .iter()
        .find(|t| t.path == path)
        .expect("PcbOpenFinished(Ok) must open a tab for the completed path");
    assert_eq!(tab.title, "board");
    assert_eq!(
        tab.cached_document
            .as_ref()
            .and_then(TabDocument::as_pcb)
            .map(|b| b.uuid),
        Some(board_uuid),
        "the tab's cached document must hold the board carried by the completion message"
    );
}

#[test]
fn pcb_open_finished_err_opens_no_tab_and_does_not_panic() {
    let path = PathBuf::from("/tmp/does-not-matter/broken.snxpcb");

    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        path: path.clone(),
        title: "broken".to_string(),
        result: Err("parse broken.snxpcb: TSV block malformed".to_string()),
    }));

    assert!(
        app.document_state.tabs.is_empty(),
        "a failed async PCB parse must not open a tab either"
    );
}
