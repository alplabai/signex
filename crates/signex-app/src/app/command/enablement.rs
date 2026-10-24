//! Command enablement (#368, slice 3 of the Command Registry #278).
//!
//! "Is this command available right now?" has one answer, [`is_enabled`],
//! read by the menu bar to grey its rows and by
//! [`Signex::dispatch_command`] to drop a keystroke for a command that has
//! nothing to act on. Before this, only the menu asked, so Ctrl+S with no
//! document open still fired while File ▸ Save was greyed.
//!
//! A predicate never disables a shortcut where its handler does something
//! today. Where the menu used to grey a row more narrowly than that, the
//! predicate follows the handler: Undo is enabled in a symbol or footprint
//! editor tab because Ctrl+Z undoes there, though the old menu gate read
//! only the schematic engine. Commands with no entry, including the ones
//! whose handler acts from any tab (Find, the grid toggle, AutoFocus), are
//! always enabled.

use crate::keymap::AppCommandId;

use super::super::*;

/// The facts the enablement predicates read, for one window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CommandContext {
    /// A schematic sheet is shown: the main window's active tab, or the
    /// undocked tab window the command came from.
    pub has_schematic: bool,
    /// A PCB is shown, in the same sense.
    pub has_pcb: bool,
    /// ERC and annotation have sheets to work on: a project is open, or
    /// some schematic is. They assemble the project's sheets, not only the
    /// active tab's.
    pub has_sheets: bool,
    /// The active tab is a `.snxsym` symbol editor.
    pub has_symbol_editor: bool,
    /// The active tab is a `.snxfpt` footprint editor.
    pub has_footprint_editor: bool,
    /// A schematic canvas the command can act on has a selection.
    pub has_selection: bool,
    /// Undo has something to undo: the symbol or footprint editor's own
    /// history in an editor tab, else a net-colour change or the
    /// schematic engine's history.
    pub can_undo: bool,
    /// Redo has something to redo, from the same histories as `can_undo`
    /// (net-colour changes have no redo).
    pub can_redo: bool,
    /// The active project's `.snxprj` has unsaved changes, which Save
    /// writes from any tab.
    pub project_dirty: bool,
}

/// Whether `command` can act in `ctx`. Unlisted commands are enabled.
pub(crate) fn is_enabled(command: &AppCommandId, ctx: &CommandContext) -> bool {
    let editor = ctx.has_symbol_editor || ctx.has_footprint_editor;
    match command.as_str() {
        "save_document" => ctx.has_schematic || editor || ctx.project_dirty,
        "save_document_as" => ctx.has_schematic || editor,
        "undo" => ctx.can_undo,
        "redo" => ctx.can_redo,
        "cut" | "copy" | "duplicate" => ctx.has_selection,
        // The footprint editor deletes its own selection, which is not the
        // schematic canvas's; with none it is a no-op.
        "delete_selection" => ctx.has_selection || ctx.has_footprint_editor,
        "print"
        | "paste"
        | "smart_paste"
        | "select_all"
        | "place_wire"
        | "place_bus"
        | "place_net_label"
        | "open_components_panel" => ctx.has_schematic,
        // These assemble the project's sheets, not only the active tab's.
        "annotate_schematic_quietly" | "force_annotate_all_schematics" | "run_erc" => {
            ctx.has_schematic || ctx.has_sheets
        }
        "zoom_to_fit" => ctx.has_schematic || ctx.has_pcb,
        // The keystroke opens the footprint editor's grid picker; the
        // View menu row cycles the schematic / PCB grid.
        "cycle_snap_grid_forward" => ctx.has_schematic || ctx.has_pcb || ctx.has_footprint_editor,
        _ => true,
    }
}

impl Signex {
    /// The [`CommandContext`] for a command invoked from `window` (`None`:
    /// the main window).
    ///
    /// The handlers act on the main window's active tab, from whichever
    /// window the key came in, so these facts are the main window's. An
    /// undocked tab window adds its own document for the commands routed to
    /// its canvas (Fit All): a fact is true when either shows it. Every
    /// other window (a detached panel or modal, a component editor) adds
    /// nothing, so Ctrl+S typed there still saves the main window's
    /// document.
    pub(crate) fn command_context(&self, window: Option<iced::window::Id>) -> CommandContext {
        let document = &self.document_state;
        let ui = &self.ui_state;
        let undocked_tab = window.filter(|id| {
            ui.main_window_id != Some(*id)
                && matches!(
                    ui.windows.get(id),
                    Some(crate::app::state::WindowKind::UndockedTab { .. })
                )
        });
        let engine = document.active_engine();
        let has_schematic = engine.is_some()
            || undocked_tab.is_some_and(|id| document.engine_for_window(id, ui).is_some());
        let has_pcb = self.has_active_pcb()
            || undocked_tab.is_some_and(|id| self.pcb_for_window(id).is_some());
        let has_selection = !self.interaction_state.active_canvas().selected.is_empty()
            || undocked_tab.is_some_and(|id| {
                !self
                    .interaction_state
                    .canvas_for_window(id)
                    .selected
                    .is_empty()
            });
        let active_kind = document.tabs.get(document.active_tab).map(|t| &t.kind);
        let has_symbol_editor = matches!(active_kind, Some(TabKind::SymbolEditor(_)));
        let has_footprint_editor = matches!(active_kind, Some(TabKind::FootprintEditor(_)));
        let (can_undo, can_redo) = match active_kind {
            Some(TabKind::SymbolEditor(path)) => document
                .symbol_editors
                .get(path)
                .map_or((false, false), |e| {
                    (!e.undo_snapshots.is_empty(), !e.redo_snapshots.is_empty())
                }),
            Some(TabKind::FootprintEditor(path)) => document
                .footprint_editors
                .get(path)
                .map_or((false, false), |e| {
                    (!e.history.is_empty(), !e.redo.is_empty())
                }),
            _ => (
                !ui.net_color_undo.is_empty() || engine.is_some_and(|e| e.can_undo()),
                engine.is_some_and(|e| e.can_redo()),
            ),
        };
        let project_dirty = document.active_project.is_some_and(|id| {
            document
                .projects
                .iter()
                .find(|p| p.id == id)
                .is_some_and(|p| document.dirty_paths.contains(&p.path))
        });
        CommandContext {
            has_schematic,
            has_pcb,
            has_sheets: document.active_project.is_some() || !document.engines.is_empty(),
            has_symbol_editor,
            has_footprint_editor,
            has_selection,
            can_undo,
            can_redo,
            project_dirty,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::super::CommandArgs;
    use super::*;

    fn id(command: &str) -> AppCommandId {
        AppCommandId::new(command).unwrap()
    }

    fn enabled(app: &Signex, command: &str) -> bool {
        is_enabled(&id(command), &app.command_context(None))
    }

    fn push_tab(app: &mut Signex, path: &std::path::Path, kind: TabKind) {
        app.document_state.tabs.push(TabInfo {
            title: "doc".into(),
            path: path.to_path_buf(),
            cached_document: None,
            dirty: false,
            project_id: None,
            kind,
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
    }

    fn with_schematic() -> Signex {
        let (mut app, _task) = Signex::new();
        let path = PathBuf::from("enablement.snxsch");
        let engine =
            signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.active_path = Some(path.clone());
        push_tab(&mut app, &path, TabKind::Schematic);
        app
    }

    fn with_symbol_editor() -> (Signex, PathBuf) {
        let (mut app, _task) = Signex::new();
        let path = PathBuf::from("enablement.snxsym");
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
        app.document_state
            .symbol_editors
            .insert(path.clone(), SymbolEditorState::new(path.clone(), file));
        push_tab(&mut app, &path, TabKind::SymbolEditor(path.clone()));
        (app, path)
    }

    fn with_footprint_editor() -> (Signex, PathBuf) {
        let (mut app, _task) = Signex::new();
        let path = PathBuf::from("enablement.snxfpt");
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
        app.document_state
            .footprint_editors
            .insert(path.clone(), FootprintEditorState::new(path.clone(), file));
        push_tab(&mut app, &path, TabKind::FootprintEditor(path.clone()));
        (app, path)
    }

    #[test]
    fn save_is_disabled_with_no_document_and_enabled_on_every_saveable_tab() {
        let (app, _task) = Signex::new();
        assert!(!enabled(&app, "save_document"));
        assert!(!enabled(&app, "save_document_as"));

        assert!(enabled(&with_schematic(), "save_document"));
        assert!(enabled(&with_symbol_editor().0, "save_document"));
        assert!(enabled(&with_footprint_editor().0, "save_document"));
        assert!(enabled(&with_footprint_editor().0, "save_document_as"));
    }

    #[test]
    fn undo_and_redo_follow_the_schematic_engine_history() {
        let mut app = with_schematic();
        assert!(!enabled(&app, "undo"));
        assert!(!enabled(&app, "redo"));

        let engine = app.document_state.active_engine_mut().unwrap();
        let document = engine.document().clone();
        engine
            .execute(signex_engine::Command::ReplaceDocument { document })
            .unwrap();
        assert!(enabled(&app, "undo"));

        let _ = app.dispatch_command(&id("undo"), CommandArgs::none(), None);
        assert!(enabled(&app, "redo"));
    }

    /// The old menu gate read only the schematic engine. Ctrl+Z undoes in a
    /// footprint editor tab from its own history, so it must stay enabled
    /// there, and the keystroke must still reach the editor.
    #[test]
    fn undo_in_a_footprint_editor_follows_its_own_history_and_still_undoes() {
        let (mut app, path) = with_footprint_editor();
        assert!(!enabled(&app, "undo"));
        app.document_state
            .footprint_editors
            .get_mut(&path)
            .unwrap()
            .push_history();
        assert!(enabled(&app, "undo"));

        let _ = app.dispatch_command(&id("undo"), CommandArgs::none(), None);

        assert!(
            app.document_state.footprint_editors[&path]
                .history
                .is_empty()
        );
        assert!(enabled(&app, "redo"));
    }

    /// A net-colour change is undone by Ctrl+Z without any engine history.
    #[test]
    fn a_net_colour_change_enables_undo() {
        let (mut app, _task) = Signex::new();
        app.ui_state
            .net_color_undo
            .push(std::collections::HashMap::new());
        assert!(enabled(&app, "undo"));
    }

    #[test]
    fn save_is_enabled_from_any_tab_while_the_project_file_is_dirty() {
        let (mut app, _task) = Signex::new();
        let project = app.document_state.mint_project_id();
        let path = PathBuf::from("p.snxprj");
        app.document_state.projects.push(crate::app::LoadedProject {
            id: project,
            path: path.clone(),
            data: signex_types::project::ProjectData {
                name: "p".into(),
                dir: String::new(),
                schematic_root: None,
                pcb_file: None,
                sheets: Vec::new(),
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                unknown: Default::default(),
            },
            pending_libraries: Default::default(),
        });
        app.document_state.active_project = Some(project);
        assert!(!enabled(&app, "save_document"));

        app.document_state.dirty_paths.insert(path);
        assert!(enabled(&app, "save_document"));
    }

    /// #368 review: a key typed in a detached modal or panel window is
    /// handled against the main window's document, so it stays enabled.
    #[test]
    fn a_detached_modal_window_keeps_the_main_documents_commands_enabled() {
        let mut app = with_schematic();
        let modal = iced::window::Id::unique();
        app.ui_state.windows.insert(
            modal,
            crate::app::state::WindowKind::DetachedModal(crate::app::state::ModalId::ErcDialog),
        );
        let ctx = app.command_context(Some(modal));
        for command in ["save_document", "place_wire", "paste", "run_erc"] {
            assert!(is_enabled(&id(command), &ctx), "{command}");
        }
    }

    /// #368 review: ERC and annotation assemble the project's sheets, so
    /// they run with a PCB or editor tab active too.
    #[test]
    fn erc_and_annotation_stay_enabled_with_a_project_but_no_active_schematic() {
        let (mut app, _path) = with_footprint_editor();
        assert!(!enabled(&app, "run_erc"));
        app.document_state.active_project = Some(app.document_state.mint_project_id());
        assert!(enabled(&app, "run_erc"));
        assert!(enabled(&app, "annotate_schematic_quietly"));
    }

    #[test]
    fn a_disabled_command_dispatches_to_a_no_op() {
        let (mut app, _task) = Signex::new();

        let task = app.dispatch_command(&id("place_wire"), CommandArgs::none(), None);

        assert_eq!(task.units(), 0);
        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Select,
            "no schematic: no wire tool"
        );

        let mut app = with_schematic();
        let _ = app.dispatch_command(&id("place_wire"), CommandArgs::none(), None);
        assert_eq!(app.interaction_state.current_tool, Tool::Wire);
    }

    /// Find, the grid toggle and AutoFocus act from any tab, so they stay
    /// enabled although the old menu greyed them without a schematic.
    #[test]
    fn a_command_with_no_gate_stays_enabled() {
        let (app, _task) = Signex::new();
        for command in [
            "cycle_selection_mode",
            "not_a_real_command",
            "find",
            "toggle_visible_grid",
            "toggle_auto_focus",
        ] {
            assert!(enabled(&app, command), "{command}");
        }
    }
}
