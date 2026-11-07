//! #661 slice 5b: what the Edit menu says about Undo / Redo (`can_undo`,
//! `can_redo`) is what a dispatched undo / redo does, for every kind of
//! document and every window that can show one.
//!
//! Undo and Redo are not in `explains_when_unavailable`, so a disabled one
//! is dropped as `Disabled` and never reaches a handler: the menu and the
//! dispatch agree when "enabled" means "the outcome is `Ran` and some history
//! moved", and "disabled" means "`Disabled` and nothing moved".

use std::path::PathBuf;

use super::CommandArgs;
use super::CommandOutcome;
use super::enablement::is_enabled;
use crate::app::state::{ModalId, WindowKind};
use crate::app::{FootprintDocument, Signex, SymbolDocument, TabInfo, TabKind};
use crate::keymap::AppCommandId;
use crate::library::state::{ComponentPreviewState, EditorAddress};

type Window = Option<iced::window::Id>;

/// How much history a fixture starts with.
#[derive(Clone, Copy, Debug)]
enum Stage {
    Empty,
    HasUndo,
    HasRedo,
}

const STAGES: [Stage; 3] = [Stage::Empty, Stage::HasUndo, Stage::HasRedo];

fn id(command: &str) -> AppCommandId {
    AppCommandId::new(command).unwrap()
}

fn menu(app: &Signex, window: Window, command: &str) -> bool {
    is_enabled(&id(command), &app.command_context(window))
}

/// Every history length the app holds, so a step anywhere shows.
fn fingerprint(app: &Signex) -> String {
    let ds = &app.document_state;
    let mut parts: Vec<String> = Vec::new();
    let mut engines: Vec<_> = ds.engines.iter().collect();
    engines.sort_by_key(|(p, _)| (*p).clone());
    for (path, e) in engines {
        parts.push(format!(
            "E {} {} {}",
            path.display(),
            e.can_undo(),
            e.can_redo()
        ));
    }
    let mut symbols: Vec<_> = ds.symbol_editors.iter().collect();
    symbols.sort_by_key(|(p, _)| (*p).clone());
    for (path, e) in symbols {
        let h = &e.history;
        parts.push(format!(
            "S {} {} {}",
            path.display(),
            h.undo_len(),
            h.redo_len()
        ));
    }
    let mut footprints: Vec<_> = ds.footprint_editors.iter().collect();
    footprints.sort_by_key(|(p, _)| (*p).clone());
    for (path, e) in footprints {
        let h = &e.history;
        parts.push(format!(
            "F {} {} {}",
            path.display(),
            h.undo_len(),
            h.redo_len()
        ));
    }
    for (address, e) in &app.library.editors {
        let h = &e.history;
        parts.push(format!(
            "R {:?} {} {} buf={} dirty={}",
            address,
            h.can_undo(),
            h.can_redo(),
            e.params_edit_buf.len(),
            e.dirty
        ));
    }
    for (project, h) in &ds.rules_history {
        parts.push(format!("P {project:?} {} {}", h.can_undo(), h.can_redo()));
    }
    for (project, tab) in &ds.project_tabs {
        parts.push(format!(
            "T {project:?} {} {}",
            tab.general_history.can_undo(),
            tab.general_history.can_redo()
        ));
    }
    parts.join("\n")
}

/// Dispatch `command` from `window`; whether it ran and moved some history.
fn ran(app: &mut Signex, window: Window, command: &str) -> (bool, CommandOutcome) {
    let before = fingerprint(app);
    let (outcome, _task) =
        app.dispatch_command_reporting(&id(command), CommandArgs::none(), window);
    let moved = fingerprint(app) != before;
    (matches!(outcome, CommandOutcome::Ran) && moved, outcome)
}

fn push_tab(app: &mut Signex, path: &std::path::Path, kind: TabKind) {
    app.document_state.tabs.push(TabInfo {
        title: "doc".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

fn engine_with_a_step() -> signex_engine::Engine {
    let mut engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    let document = engine.document().clone();
    engine
        .execute(signex_engine::Command::ReplaceDocument { document })
        .unwrap();
    engine
}

/// Bring the fixture to `stage`: HasRedo is HasUndo plus one dispatched undo.
fn settle(app: &mut Signex, window: Window, stage: Stage, give_undo: impl FnOnce(&mut Signex)) {
    if !matches!(stage, Stage::Empty) {
        give_undo(app);
    }
    if matches!(stage, Stage::HasRedo) {
        let (moved, outcome) = ran(app, window, "undo");
        assert!(
            moved,
            "fixture: the undo that fills redo must run: {outcome:?}"
        );
    }
}

fn schematic(stage: Stage) -> (Signex, Window) {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("agree.snxsch");
    app.document_state.active_path = Some(path.clone());
    push_tab(&mut app, &path, TabKind::Schematic);
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(Default::default()).unwrap(),
    );
    settle(&mut app, None, stage, |app| {
        app.document_state
            .engines
            .insert(path, engine_with_a_step());
    });
    (app, None)
}

fn pcb(_stage: Stage) -> (Signex, Window) {
    let (mut app, _t) = Signex::new();
    push_tab(&mut app, &PathBuf::from("agree.snxpcb"), TabKind::Pcb);
    (app, None)
}

fn symbol(stage: Stage) -> (Signex, Window) {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("agree.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    push_tab(&mut app, &path, TabKind::SymbolEditor(path.clone()));
    settle(&mut app, None, stage, |app| {
        let doc = app.document_state.symbol_editors.get_mut(&path).unwrap();
        let snap = doc.snapshot();
        doc.commit_history(snap);
    });
    (app, None)
}

fn footprint(stage: Stage) -> (Signex, Window) {
    let (mut app, _t) = Signex::new();
    let path = PathBuf::from("agree.snxfpt");
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    push_tab(&mut app, &path, TabKind::FootprintEditor(path.clone()));
    settle(&mut app, None, stage, |app| {
        app.document_state
            .footprint_editors
            .get_mut(&path)
            .unwrap()
            .push_history();
    });
    (app, None)
}

/// A Component row open in a tab; `buffer` leaves a half-typed parameter.
fn open_row(app: &mut Signex) -> (EditorAddress, crate::app::ComponentEditorTab) {
    let row = crate::library::component_preview::test_row();
    let library_path = PathBuf::from("/tmp/agree.snxlib");
    let row_id = signex_library::RowId::from_uuid(row.row_id);
    let address = EditorAddress::new(library_path.clone(), "R".into(), row_id);
    app.library.editors.insert(
        address.clone(),
        ComponentPreviewState::from_row(library_path.clone(), "R".into(), row),
    );
    let tab = crate::app::ComponentEditorTab {
        library_path,
        table: "R".into(),
        row_id,
    };
    (address, tab)
}

fn give_row_a_step(app: &mut Signex, address: &EditorAddress) {
    let row = app.library.editors.get_mut(address).unwrap();
    crate::library::component_preview::apply_inline_edit(
        row,
        crate::library::messages::EditorMsg::SetLifecycle(signex_library::LifecycleState::Released),
    );
}

fn component_tab(stage: Stage) -> (Signex, Window) {
    let (mut app, _t) = Signex::new();
    let (address, tab) = open_row(&mut app);
    let path = PathBuf::from("row.tsv#1");
    push_tab(&mut app, &path, TabKind::ComponentEditor(tab));
    settle(&mut app, None, stage, |app| give_row_a_step(app, &address));
    (app, None)
}

fn component_window(stage: Stage) -> (Signex, Window) {
    let (mut app, _t) = Signex::new();
    let (address, tab) = open_row(&mut app);
    let kind = WindowKind::ComponentEditor {
        library_path: tab.library_path.clone(),
        table: tab.table.clone(),
        row_id: tab.row_id,
    };
    push_tab(
        &mut app,
        &PathBuf::from("row.tsv#1"),
        TabKind::ComponentEditor(tab),
    );
    let window = iced::window::Id::unique();
    app.ui_state.windows.insert(window, kind);
    settle(&mut app, Some(window), stage, |app| {
        give_row_a_step(app, &address)
    });
    (app, Some(window))
}

/// A sheet in its own window, with the main window on a footprint editor
/// that has a step of its own (it must never be touched).
fn undocked(stage: Stage) -> (Signex, Window) {
    let (mut app, window) = footprint(Stage::HasUndo);
    let path = PathBuf::from("undocked.snxsch");
    let main_tab = app.document_state.active_tab;
    app.document_state.tabs.insert(
        0,
        TabInfo {
            title: "undocked".into(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: TabKind::Schematic,
        },
    );
    app.document_state.active_tab = main_tab + 1;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(Default::default()).unwrap(),
    );
    let id = iced::window::Id::unique();
    app.ui_state.windows.insert(
        id,
        WindowKind::UndockedTab {
            path: path.clone(),
            title: "undocked".into(),
        },
    );
    app.interaction_state
        .canvases
        .insert(id, crate::canvas::CanvasSlot::new());
    let _ = window;
    settle(&mut app, Some(id), stage, |app| {
        app.document_state
            .engines
            .insert(path, engine_with_a_step());
    });
    (app, Some(id))
}

/// A window whose tab is gone, over a main window that holds a sheet with
/// an undo step: a stroke there must not step the sheet.
fn window_without_a_tab(_stage: Stage) -> (Signex, Window) {
    let (mut app, _w) = schematic(Stage::HasUndo);
    let id = iced::window::Id::unique();
    app.ui_state.windows.insert(
        id,
        WindowKind::UndockedTab {
            path: PathBuf::from("closed.snxsch"),
            title: "closed".into(),
        },
    );
    (app, Some(id))
}

/// A Library Browser tab in front, a sheet's engine still live behind it.
fn library_browser_over_a_live_engine(_stage: Stage) -> (Signex, Window) {
    let (mut app, _w) = schematic(Stage::HasUndo);
    push_tab(
        &mut app,
        &PathBuf::from("browser"),
        TabKind::LibraryBrowser(PathBuf::from("/tmp/agree.snxlib")),
    );
    app.document_state.active_path = None;
    (app, None)
}

fn rules(stage: Stage) -> (Signex, Window) {
    let dir = std::env::temp_dir().join(format!("signex-agree-{}", uuid::Uuid::now_v7()));
    let mut app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir.to_string_lossy(),
        &["a_root.snxsch"],
    );
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    // The sheet behind the dialog has a step of its own.
    let path = dir.join("a_root.snxsch");
    app.document_state
        .engines
        .insert(path.clone(), engine_with_a_step());
    push_tab(&mut app, &path, TabKind::Schematic);
    app.document_state.active_path = Some(path);
    app.ui_state.erc_dialog_open = true;
    let window = iced::window::Id::unique();
    app.ui_state
        .windows
        .insert(window, WindowKind::DetachedModal(ModalId::ErcDialog));
    settle(&mut app, Some(window), stage, |app| {
        let _ = app.handle_store_erc_rules_in_project();
        let _ = app.handle_erc_severity_changed(
            signex_erc::RuleKind::UnusedPin,
            signex_erc::Severity::Info,
        );
    });
    (app, Some(window))
}

type Fixture = fn(Stage) -> (Signex, Window);

const FIXTURES: [(&str, Fixture); 11] = [
    ("schematic", schematic),
    ("pcb", pcb),
    ("symbol", symbol),
    ("footprint", footprint),
    ("component row in a tab", component_tab),
    ("component preview window", component_window),
    ("undocked sheet", undocked),
    ("window without a tab", window_without_a_tab),
    (
        "library browser over a live engine",
        library_browser_over_a_live_engine,
    ),
    ("project rules", rules),
    ("project rules, second pass", rules),
];

#[test]
fn the_menu_and_a_dispatched_undo_or_redo_agree() {
    let _ = crate::diagnostics::init_logging();
    for (name, build) in FIXTURES {
        for stage in STAGES {
            for command in ["undo", "redo"] {
                let (mut app, window) = build(stage);
                let enabled = menu(&app, window, command);
                let (moved, outcome) = ran(&mut app, window, command);
                assert_eq!(
                    enabled, moved,
                    "{name} / {stage:?} / {command}: menu {enabled}, dispatch moved {moved} ({outcome:?})"
                );
                if !enabled {
                    assert!(
                        matches!(outcome, CommandOutcome::Disabled { .. }),
                        "{name} / {stage:?} / {command}: a disabled command is dropped: {outcome:?}"
                    );
                }
            }
        }
    }
}

/// The fixtures are not vacuous: each one that holds history is enabled.
#[test]
fn the_fixtures_with_history_light_the_menu() {
    let _ = crate::diagnostics::init_logging();
    for name in [
        "schematic",
        "symbol",
        "footprint",
        "component row in a tab",
        "component preview window",
        "undocked sheet",
        "project rules",
    ] {
        let (_, build) = FIXTURES.iter().find(|(n, _)| *n == name).unwrap();
        let (app, window) = build(Stage::HasUndo);
        assert!(menu(&app, window, "undo"), "{name}: undo lit");
        assert!(!menu(&app, window, "redo"), "{name}: redo dark");
        let (app, window) = build(Stage::HasRedo);
        assert!(menu(&app, window, "redo"), "{name}: redo lit");
    }
}
