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
fn opening_the_same_schematic_twice_before_it_completes_spawns_only_one_task() {
    // #478 review — the read+parse is a `Task::perform` round-trip, so
    // firing `Opened(path)` twice for the same path before the first
    // completes used to spawn two independent Tasks; both completions
    // would each push a tab aliasing one `document_state.engines`
    // entry, and closing one orphaned the other. The in-flight guard
    // (`document_state.pending_opens`) must make the second call a
    // no-op instead.
    //
    // `iced::Task::units()` counts the concurrent actions a `Task`
    // carries — `Task::none()` is 0, a single `Task::perform` is 1 —
    // so it's a reliable, executor-free way to tell whether the
    // second call actually spawned another read+parse.
    let tmp = TempDir::new().expect("tempdir");
    let sch_path = tmp.path().join("Dup.snxsch");
    let serialised = signex_types::format::SnxSchematic::new(empty_schematic_sheet())
        .write_string()
        .expect("serialise schematic");
    fs::write(&sch_path, serialised).expect("write .snxsch");

    let (mut app, _t) = Signex::new();
    let first = app.update(Message::File(FileMsg::Opened(Some(sch_path.clone()))));
    let second = app.update(Message::File(FileMsg::Opened(Some(sch_path.clone()))));

    assert!(
        first.units() > 0,
        "the first Opened(path) call must spawn the async read+parse Task"
    );
    assert_eq!(
        second.units(),
        first.units() - 1,
        "a second Opened(path) for the same path fired before the first \
         completes must be exactly one Task unit lighter than the first — \
         i.e. it must not spawn a duplicate read+parse Task"
    );
    assert!(
        app.document_state.tabs.is_empty(),
        "still async — neither call may synchronously open a tab"
    );

    // Only one real Task was ever spawned, so only one completion ever
    // arrives in production. Simulate it and confirm exactly one tab
    // results for the path.
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: sch_path.clone(),
        title: "Dup".to_string(),
        result: Ok(Box::new(empty_schematic_sheet())),
    }));

    assert_eq!(
        app.document_state
            .tabs
            .iter()
            .filter(|t| t.path == sch_path)
            .count(),
        1,
        "exactly one tab must exist for the path once the sole in-flight \
         open completes"
    );
    assert!(
        !app.document_state.pending_opens.contains(&sch_path),
        "the in-flight guard must be cleared once the completion arrives"
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

#[test]
fn reopening_an_already_open_schematic_tab_activates_it_instead_of_duplicating() {
    // #478 review — mirrors `handle_open_primitive`'s activate-existing
    // convention: opening a path that already has a tab must switch to
    // that tab, not push a second one aliasing the same engine.
    let first_path = PathBuf::from("/tmp/does-not-matter/first.snxsch");
    let second_path = PathBuf::from("/tmp/does-not-matter/second.snxsch");

    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: first_path.clone(),
        title: "first".to_string(),
        result: Ok(Box::new(empty_schematic_sheet())),
    }));
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: second_path.clone(),
        title: "second".to_string(),
        result: Ok(Box::new(empty_schematic_sheet())),
    }));
    assert_eq!(app.document_state.tabs.len(), 2);
    assert_eq!(app.document_state.active_tab, 1);

    // Re-open the first path — it already has a tab.
    let _ = app.update(Message::File(FileMsg::Opened(Some(first_path))));

    assert_eq!(
        app.document_state.tabs.len(),
        2,
        "reopening an already-open path must not push a duplicate tab"
    );
    assert_eq!(
        app.document_state.active_tab, 0,
        "reopening an already-open path must activate its existing tab"
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
fn opening_the_same_pcb_twice_before_it_completes_spawns_only_one_task() {
    // #478 review — same in-flight race as the schematic case above,
    // for `open_pcb_file`.
    let pcb_path = PathBuf::from("/tmp/does-not-matter/Dup.snxpcb");

    let (mut app, _t) = Signex::new();
    let first = app.update(Message::File(FileMsg::Opened(Some(pcb_path.clone()))));
    let second = app.update(Message::File(FileMsg::Opened(Some(pcb_path.clone()))));

    assert!(
        first.units() > 0,
        "the first Opened(path) call must spawn the async read+parse Task"
    );
    assert_eq!(
        second.units(),
        first.units() - 1,
        "a second Opened(path) for the same PCB path fired before the first \
         completes must not spawn a duplicate read+parse Task"
    );

    // Only one real Task was ever spawned — simulate that sole
    // completion and confirm exactly one tab results.
    let _ = app.update(Message::File(FileMsg::PcbOpenFinished {
        path: pcb_path.clone(),
        title: "Dup".to_string(),
        result: Ok(Box::new(empty_pcb_board())),
    }));

    assert_eq!(
        app.document_state
            .tabs
            .iter()
            .filter(|t| t.path == pcb_path)
            .count(),
        1,
        "exactly one tab must exist for the PCB path once the sole \
         in-flight open completes"
    );
    assert!(
        !app.document_state.pending_opens.contains(&pcb_path),
        "the in-flight guard must be cleared once the completion arrives"
    );
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
