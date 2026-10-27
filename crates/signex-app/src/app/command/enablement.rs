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
//!
//! Ids that resolve to the same message share one arm (`place_net_label`
//! and `place_local_net_label`, …), so an alias cannot bypass the gate; a
//! test checks every such pair in the catalog.

use crate::keymap::AppCommandId;

use super::super::*;

/// The facts the enablement predicates read, for one window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub(crate) struct CommandContext {
    /// The main window's active tab is a schematic sheet.
    pub has_schematic: bool,
    /// The main window's active tab is a PCB.
    pub has_pcb: bool,
    /// The undocked tab window the command came from shows a schematic or
    /// a PCB. Only Fit All acts on that window's own canvas; every other
    /// handler acts on the main window's active tab.
    pub undocked_has_canvas: bool,
    /// ERC and annotation have sheets to work on: a project is open, or
    /// some schematic is. They assemble the project's sheets, not only the
    /// active tab's.
    pub has_sheets: bool,
    /// The active tab is a `.snxsym` symbol editor.
    pub has_symbol_editor: bool,
    /// The active tab is a `.snxfpt` footprint editor.
    pub has_footprint_editor: bool,
    /// The main window's schematic canvas has a selection.
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
        // E edits the selected object in the schematic and in the
        // footprint / symbol editors, whose selections `has_selection`
        // does not read, so a keystroke must not be dropped there.
        "edit_selected_object_properties" => {
            ctx.has_selection || ctx.has_footprint_editor || ctx.has_symbol_editor
        }
        // The Export menu row is gated on `has_schematic`; without an
        // engine the handler does nothing.
        "report_manager_bom" => ctx.has_schematic,
        // The footprint editor deletes its own selection, which is not the
        // schematic canvas's; with none it is a no-op.
        "delete_selection" | "remove_last_vertex" => ctx.has_selection || ctx.has_footprint_editor,
        "print"
        | "paste"
        | "smart_paste"
        | "paste_special"
        | "select_all"
        | "place_wire"
        | "place_bus"
        | "place_net_label"
        | "place_local_net_label"
        | "open_components_panel"
        | "place_symbol" => ctx.has_schematic,
        // These assemble the project's sheets, not only the active tab's.
        "annotate_schematic_quietly" | "force_annotate_all_schematics" | "run_erc" => {
            ctx.has_schematic || ctx.has_sheets
        }
        "zoom_to_fit" | "zoom_to_all_objects" | "show_all_design_objects" => {
            ctx.has_schematic || ctx.has_pcb || ctx.undocked_has_canvas
        }
        // The keystroke opens the footprint editor's grid picker and does
        // nothing elsewhere. The View menu row shares the label but cycles
        // the schematic / PCB grid; see [`cycle_grid_menu_enabled`].
        "cycle_snap_grid_forward" | "open_grid_picker" => ctx.has_footprint_editor,
        _ => true,
    }
}

/// View ▸ Cycle Grid Size: the menu row sends `MenuMessage::CycleGrid`,
/// which steps the schematic / PCB grid, not the command its label is
/// taken from.
pub(crate) fn cycle_grid_menu_enabled(ctx: &CommandContext) -> bool {
    ctx.has_schematic || ctx.has_pcb
}

/// Commands whose handler explains to the user why it did nothing. The
/// menu greys them by [`is_enabled`], but a keystroke still reaches the
/// handler so the explanation is not lost (Save As in a PCB tab, #671;
/// Print with no schematic).
pub(crate) fn explains_when_unavailable(command: &AppCommandId) -> bool {
    matches!(command.as_str(), "save_document_as" | "print")
}

impl Signex {
    /// The [`CommandContext`] for a command invoked from `window` (`None`:
    /// the main window).
    ///
    /// The handlers act on the main window's active tab, from whichever
    /// window the key came in, so the facts are the main window's. The
    /// only exception is `undocked_has_canvas`, for Fit All, which an
    /// undocked tab routes to its own canvas. Any other window (a detached
    /// panel or modal, a component editor) adds nothing, so Ctrl+S typed
    /// there still saves the main window's document.
    pub(crate) fn command_context(&self, window: Option<iced::window::Id>) -> CommandContext {
        let document = &self.document_state;
        let active_kind = document.tabs.get(document.active_tab).map(|t| &t.kind);
        let (can_undo, can_redo) = if self.erc_dialog_takes_input(window) {
            self.rules_history_available()
        } else {
            self.history_available(active_kind)
        };
        CommandContext {
            has_schematic: document.active_engine().is_some(),
            has_pcb: self.has_active_pcb(),
            undocked_has_canvas: self.undocked_tab_has_canvas(window),
            has_sheets: document.active_project.is_some() || !document.engines.is_empty(),
            has_symbol_editor: matches!(active_kind, Some(TabKind::SymbolEditor(_))),
            has_footprint_editor: matches!(active_kind, Some(TabKind::FootprintEditor(_))),
            has_selection: !self.interaction_state.active_canvas().selected.is_empty(),
            can_undo,
            can_redo,
            project_dirty: self.active_project_is_dirty(),
        }
    }

    /// Whether `window` is an undocked tab window showing a schematic or a
    /// PCB.
    fn undocked_tab_has_canvas(&self, window: Option<iced::window::Id>) -> bool {
        let ui = &self.ui_state;
        window.is_some_and(|id| {
            ui.main_window_id != Some(id)
                && matches!(
                    ui.windows.get(&id),
                    Some(crate::app::state::WindowKind::UndockedTab { .. })
                )
                && (self.document_state.engine_for_window(id, ui).is_some()
                    || self.pcb_for_window(id).is_some())
        })
    }

    /// Whether a stroke typed in `window` belongs to the ERC dialog, which
    /// always opens in its own OS window. A stroke in the main window while
    /// the dialog is open belongs to the sheet.
    pub(crate) fn erc_dialog_takes_input(&self, window: Option<iced::window::Id>) -> bool {
        use crate::app::dispatch::input::InputTarget;
        use crate::app::state::ModalId;
        self.ui_state.erc_dialog_open
            && self.input_target(window) == InputTarget::DetachedModal(ModalId::ErcDialog)
    }

    /// `(can_undo, can_redo)` of the active project's stored rule set.
    fn rules_history_available(&self) -> (bool, bool) {
        let document = &self.document_state;
        document
            .active_document_project()
            .and_then(|p| document.rules_history.get(&p.id))
            .map_or((false, false), |h| (h.can_undo(), h.can_redo()))
    }

    /// `(can_undo, can_redo)` for the active tab: an editor tab's own
    /// history, else net-colour changes and the schematic engine.
    fn history_available(&self, active_kind: Option<&TabKind>) -> (bool, bool) {
        let document = &self.document_state;
        match active_kind {
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
            _ => {
                let engine = document.active_engine();
                let has_steps = |steps: &std::collections::HashMap<_, Vec<_>>| {
                    document
                        .active_path
                        .as_ref()
                        .and_then(|p| steps.get(p))
                        .is_some_and(|s| !s.is_empty())
                };
                (
                    has_steps(&self.ui_state.net_color_undo)
                        || engine.is_some_and(|e| e.can_undo()),
                    has_steps(&self.ui_state.net_color_redo)
                        || engine.is_some_and(|e| e.can_redo()),
                )
            }
        }
    }

    /// Whether the active project's `.snxprj` has unsaved changes.
    fn active_project_is_dirty(&self) -> bool {
        let document = &self.document_state;
        document.active_project.is_some_and(|id| {
            document
                .projects
                .iter()
                .find(|p| p.id == id)
                .is_some_and(|p| document.dirty_paths.contains(&p.path))
        })
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
            .insert(path.clone(), SymbolDocument::new(path.clone(), file));
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
            .insert(path.clone(), FootprintDocument::new(path.clone(), file));
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
        let path = std::path::PathBuf::from("colour.snxsch");
        app.document_state.active_path = Some(path.clone());
        app.ui_state.net_color_undo.entry(path).or_default().push(
            crate::app::state::NetColorStep {
                colors: std::collections::HashMap::new(),
                at: 0,
            },
        );
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
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                unknown: Default::default(),
            },
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
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

    /// #907 review: ids that resolve to the same message are the same
    /// command, so they must get the same answer. An alias left to the
    /// default arm would let a second key bypass the gate.
    #[test]
    fn ids_that_resolve_to_the_same_message_share_their_enablement() {
        let facts: [fn(&mut CommandContext); 10] = [
            |c| c.has_schematic = true,
            |c| c.has_pcb = true,
            |c| c.undocked_has_canvas = true,
            |c| c.has_sheets = true,
            |c| c.has_symbol_editor = true,
            |c| c.has_footprint_editor = true,
            |c| c.has_selection = true,
            |c| c.can_undo = true,
            |c| c.can_redo = true,
            |c| c.project_dirty = true,
        ];
        let mut contexts = vec![CommandContext::default()];
        contexts.extend(facts.iter().map(|set| {
            let mut ctx = CommandContext::default();
            set(&mut ctx);
            ctx
        }));
        let resolved: Vec<(AppCommandId, String)> = crate::keymap::all_command_ids()
            .filter_map(|name| {
                let command = id(name);
                super::super::bridge::resolve(&command).map(|m| (command, format!("{m:?}")))
            })
            .collect();
        for (a, message_a) in &resolved {
            for (b, message_b) in &resolved {
                if message_a != message_b {
                    continue;
                }
                for ctx in &contexts {
                    assert_eq!(
                        is_enabled(a, ctx),
                        is_enabled(b, ctx),
                        "{} and {} resolve to one message but differ in {ctx:?}",
                        a.as_str(),
                        b.as_str()
                    );
                }
            }
        }
    }

    #[test]
    fn undo_and_redo_in_a_symbol_editor_follow_its_own_history() {
        let (mut app, path) = with_symbol_editor();
        assert!(!enabled(&app, "undo"));
        let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
        let snapshot = editor.snapshot();
        editor.commit_history(snapshot);
        assert!(enabled(&app, "undo"));

        let _ = app.dispatch_command(&id("undo"), CommandArgs::none(), None);
        assert!(enabled(&app, "redo"));
    }

    /// #907 review: an undocked tab window counts only for Fit All, which
    /// it routes to its own canvas. Copy and the rest act on the main
    /// window's tab, so they stay disabled with no document there.
    #[test]
    fn an_undocked_schematic_enables_fit_all_only() {
        let (mut app, _task) = Signex::new();
        let path = PathBuf::from("undocked.snxsch");
        let engine =
            signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
        app.document_state.engines.insert(path.clone(), engine);
        let window = iced::window::Id::unique();
        app.ui_state.windows.insert(
            window,
            crate::app::state::WindowKind::UndockedTab {
                path,
                title: "undocked".into(),
            },
        );

        let ctx = app.command_context(Some(window));

        assert!(is_enabled(&id("zoom_to_fit"), &ctx));
        assert!(!is_enabled(&id("paste"), &ctx));
        assert!(!is_enabled(&id("save_document_as"), &ctx));
    }

    /// #907 review: Save As and Print explain in the Messages panel why
    /// they did nothing, so the keystroke still reaches them.
    /// #1152 review: E must still reach the footprint / symbol editors,
    /// whose selection `has_selection` does not see.
    #[test]
    fn edit_properties_keystroke_stays_enabled_in_the_editors() {
        let (start, _task) = Signex::new();
        assert!(!enabled(&start, "edit_selected_object_properties"));
        assert!(enabled(
            &with_footprint_editor().0,
            "edit_selected_object_properties"
        ));
        assert!(enabled(
            &with_symbol_editor().0,
            "edit_selected_object_properties"
        ));
    }

    /// #1152 review: Bill of Materials needs an active schematic, as its
    /// Export menu row does.
    #[test]
    fn bill_of_materials_needs_a_schematic() {
        let (start, _task) = Signex::new();
        assert!(!enabled(&start, "report_manager_bom"));
        assert!(!enabled(&with_footprint_editor().0, "report_manager_bom"));
        assert!(enabled(&with_schematic(), "report_manager_bom"));
    }

    #[test]
    fn save_as_and_print_still_explain_why_they_did_nothing() {
        assert!(explains_when_unavailable(&id("save_document_as")));
        assert!(explains_when_unavailable(&id("print")));
        assert!(!explains_when_unavailable(&id("save_document")));
    }
}
