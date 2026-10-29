//! #1070 — the command palette offers only commands that can act in the
//! active editor, not the whole registry.
//!
//! It listed every dispatchable command regardless of the active tab, so
//! the start screen offered "Place wire" and the footprint editor offered
//! schematic placement commands that did nothing there. The menu bar was
//! already context-aware; the palette now reads the same facts.

use signex_app::app::command_palette::{CommandAction, CommandEntry, build_catalog, catalog_stamp};
use signex_app::app::{
    CommandPaletteMsg, FileMsg, FootprintDocument, Message, Signex, TabInfo, TabKind,
};
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
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "f".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

/// Commands only a schematic editor can act on. Several are bound under
/// `global` in the built-in profiles (or not bound at all), so the filter
/// must come from the command metadata, not the binding sections.
const SCHEMATIC_ONLY: &[&str] = &[
    "place_wire",
    "annotate_schematic",
    "place_no_erc",
    "place_compile_mask",
    "break_wire",
    "draw_graphic_line",
];

fn open_schematic_tab(app: &mut Signex) {
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    let _ = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: PathBuf::from("palette-context.snxsch"),
        title: "s".into(),
        result: Ok(Box::new(sheet)),
    }));
}

#[test]
fn schematic_only_commands_follow_the_active_editor() {
    let (mut start, _t) = Signex::new();
    let ids = command_ids(&build_catalog(&start));
    for id in SCHEMATIC_ONLY {
        assert!(!ids.iter().any(|i| i == id), "{id} offered on start screen");
    }

    open_footprint_tab(&mut start);
    let ids = command_ids(&build_catalog(&start));
    for id in SCHEMATIC_ONLY {
        assert!(
            !ids.iter().any(|i| i == id),
            "{id} offered in footprint editor"
        );
    }

    let (mut sch, _t) = Signex::new();
    open_schematic_tab(&mut sch);
    let ids = command_ids(&build_catalog(&sch));
    for id in ["place_wire", "annotate_schematic"] {
        assert!(
            ids.iter().any(|i| i == id),
            "{id} missing in schematic editor"
        );
    }
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
        !ids.iter().any(|id| id == "report_manager_bom"),
        "Bill of Materials needs a schematic"
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

/// Editor-only `General` commands. They carry no editor context in a
/// keymap profile, so before the catalog treated `General` as
/// editor-scoped by default they leaked onto the start screen.
const EDITOR_ONLY_GENERAL: &[&str] = &[
    "placement_accept",
    "placement_properties",
    "center_view_at_cursor",
    "cancel_current_tool",
    "cycle_selection_mode",
    "next_grid",
    "toggle_electrical_grid",
    "edit_selected_object_properties",
];

#[test]
fn editor_only_general_commands_are_hidden_on_the_start_screen() {
    let (start, _t) = Signex::new();
    let ids = command_ids(&build_catalog(&start));
    for id in EDITOR_ONLY_GENERAL {
        assert!(!ids.iter().any(|i| i == id), "{id} offered on start screen");
    }
}

#[test]
fn properties_edit_needs_a_selection_even_in_a_schematic() {
    let (mut sch, _t) = Signex::new();
    open_schematic_tab(&mut sch);
    let ids = command_ids(&build_catalog(&sch));
    assert!(
        !ids.iter().any(|i| i == "edit_selected_object_properties"),
        "offered with nothing selected"
    );
    assert!(
        ids.iter().any(|i| i == "next_grid"),
        "editor-scoped General commands stay in an editor"
    );
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
