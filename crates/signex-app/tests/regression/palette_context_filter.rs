//! #1070 — the command palette offers only commands that can act in the
//! active editor, not the whole registry.
//!
//! It listed every dispatchable command regardless of the active tab, so
//! the start screen offered "Place wire" and the footprint editor offered
//! schematic placement commands that did nothing there. The menu bar was
//! already context-aware; the palette now reads the same facts.

use signex_app::app::command_palette::{CommandAction, CommandEntry, build_catalog, catalog_stamp};
use signex_app::app::{CommandPaletteMsg, FootprintEditorState, Message, Signex, TabInfo, TabKind};
use signex_app::tab_bar::TabMessage;
use signex_library::{Footprint, FootprintFile};
use std::path::PathBuf;

fn command_ids(catalog: &[CommandEntry]) -> Vec<String> {
    catalog
        .iter()
        .filter_map(|entry| match &entry.action {
            CommandAction::Command(id) => Some(id.as_str().to_string()),
            _ => None,
        })
        .collect()
}

fn open_footprint_tab(app: &mut Signex) {
    let path = PathBuf::from("palette-context.snxfpt");
    let file = FootprintFile::from_footprint(Footprint::empty("f"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintEditorState::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "f".into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

#[test]
fn start_screen_has_globals_but_no_editor_commands() {
    let (app, _t) = Signex::new();
    let catalog = build_catalog(&app);
    let ids = command_ids(&catalog);

    assert!(ids.iter().any(|id| id == "open_preferences"), "global kept");
    assert!(
        !ids.iter().any(|id| id == "place_wire"),
        "schematic-only command must be hidden with no document"
    );
    assert!(
        !ids.iter().any(|id| id == "open_grid_picker"),
        "footprint-only command must be hidden with no document"
    );
    assert!(
        catalog
            .iter()
            .any(|e| matches!(e.action, CommandAction::Panel(_))),
        "Open Panel rows are global"
    );
}

#[test]
fn footprint_editor_shows_its_own_commands_and_hides_schematic_only() {
    let (mut app, _t) = Signex::new();
    open_footprint_tab(&mut app);
    let ids = command_ids(&build_catalog(&app));

    assert!(ids.iter().any(|id| id == "open_grid_picker"));
    assert!(ids.iter().any(|id| id == "open_preferences"));
    assert!(!ids.iter().any(|id| id == "place_wire"));
}

#[test]
fn tab_switch_rebuilds_the_open_palette() {
    let (mut app, _t) = Signex::new();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    // Tab 0 is a PCB, tab 1 the footprint editor.
    app.document_state.tabs.push(TabInfo {
        title: "board".into(),
        path: PathBuf::from("palette-context.snxpcb"),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Pcb,
    });
    open_footprint_tab(&mut app);
    app.document_state.active_tab = 0;

    let _ = app.update(Message::CommandPalette(CommandPaletteMsg::Open));
    let _ = app.update(Message::CommandPalette(CommandPaletteMsg::QueryChanged(
        "grid picker".into(),
    )));
    assert_eq!(app.ui_state.command_palette.total_matches, 0);
    let before = catalog_stamp(&app);

    let _ = app.update(Message::Tab {
        window_id: window,
        msg: TabMessage::Select(1),
    });
    assert_ne!(before, catalog_stamp(&app), "stamp must see the tab kind");
    assert!(
        app.ui_state.command_palette.total_matches > 0,
        "the footprint tab's command must appear without retyping"
    );
}
