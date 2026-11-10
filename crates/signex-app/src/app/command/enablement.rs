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

pub(crate) use crate::app::handlers::pcb_placement::PcbTargets;
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
    /// a PCB. Fit All acts on that window's own canvas; the per-window
    /// document commands read [`Self::document_is_schematic`] and
    /// [`Self::document_is_editor`] instead.
    pub undocked_has_canvas: bool,
    /// The document Save, Save As and Print act on is a schematic sheet:
    /// the window's own in an undocked tab window (#1813), else the main
    /// window's active tab.
    pub document_is_schematic: bool,
    /// That document is a `.snxsym` or `.snxfpt` editor tab. Both fields are
    /// false in a Component Preview window, which has no file of its own.
    pub document_is_editor: bool,
    /// The window's tab is a board with unsaved changes or a live sketch,
    /// which Save writes (a new board is written only on Save).
    pub document_is_dirty_board: bool,
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
    /// The selection holds at least one schematic drawing, which the
    /// Arrange commands act on (#1583).
    pub has_drawing_selection: bool,
    /// A wire or polyline draw has uncommitted vertices, which
    /// Backspace / Delete pops even with nothing selected.
    pub drawing: bool,
    /// Undo has something to undo: the symbol or footprint editor's own
    /// history in an editor tab, else a net-colour change or the
    /// schematic engine's history.
    pub can_undo: bool,
    /// Redo has something to redo, from the same histories as `can_undo`
    /// (net-colour changes have no redo).
    pub can_redo: bool,
    /// The active project's `.snxprj` has unsaved changes, which Save
    /// writes from any tab of a window that has a document of its own.
    pub project_dirty: bool,
    /// The window holds a document with a file of its own for Save to run
    /// against. False in a Component Preview and in an undocked window whose
    /// tab is gone, which refuse Save whatever the project holds.
    pub document_has_file: bool,
    /// Some document or Component Preview row has unsaved edits, which
    /// File ▸ Save All writes.
    pub has_dirty_documents: bool,
    /// A document tab is open, which Close Document closes.
    pub has_document: bool,
    /// A project is open, which Close Project closes.
    pub has_open_project: bool,
    /// The active sheet is placed by a sheet symbol of another sheet in the
    /// Navigator's hierarchy, which Up Hierarchy opens. A top-level page has
    /// none.
    pub has_parent_sheet: bool,
    /// Some document is the active one (it has a path), which the annotate
    /// commands run from.
    pub has_active_path: bool,
    /// The active tab is a symbol or footprint editor inside an open library,
    /// which Tools ▸ Document Options opens the options of.
    pub has_library_editor: bool,
    /// A library is open or has a Library Browser tab, which Tools ▸ Library
    /// Editor opens or focuses.
    pub has_open_library: bool,
    /// The active symbol editor's current part can be removed: a symbol
    /// keeps its first part, so Remove Part refuses part 1.
    pub can_remove_part: bool,
    /// The board the window shows and what its selection lets the
    /// placement commands do (#2240).
    pub pcb: PcbTargets,
    /// Rotate and mirror have something on a sheet besides the selection:
    /// a part on the cursor, or the Bus Entry tool's next entry.
    pub turn_in_hand: bool,
}

/// Whether `command` can act in `ctx`. Unlisted commands are enabled.
pub(crate) fn is_enabled(command: &AppCommandId, ctx: &CommandContext) -> bool {
    super::needs::needs(command).is_none_or(|needs| needs.is_met(ctx))
}

/// Why `command` cannot act in `ctx`, or `None` when it can.
pub(crate) fn disabled_reason(
    command: &AppCommandId,
    ctx: &CommandContext,
) -> Option<&'static str> {
    super::needs::needs(command)
        .filter(|needs| !needs.is_met(ctx))
        .map(super::needs::Needs::reason)
}

/// Tools ▸ New Part: the handler acts only on a `.snxsym` editor tab.
pub(crate) fn new_part_menu_enabled(ctx: &CommandContext) -> bool {
    ctx.has_symbol_editor
}

/// Tools ▸ Remove Part: a symbol editor on a part the handler accepts.
pub(crate) fn remove_part_menu_enabled(ctx: &CommandContext) -> bool {
    ctx.has_symbol_editor && ctx.can_remove_part
}

/// Commands whose handler explains to the user why it did nothing. The
/// menu greys them by [`is_enabled`], but a keystroke still reaches the
/// handler so the explanation is not lost (Save As in a PCB tab, #671;
/// Print with no schematic; Up Hierarchy on a top-level sheet, whose handler
/// also assembles the sheets itself when the Navigator cache is cold).
pub(crate) fn explains_when_unavailable(command: &AppCommandId) -> bool {
    matches!(
        command.as_str(),
        "save_document_as"
            | "print"
            | "leave_sheet"
            | "navigate_up_hierarchy"
            | "annotate_schematic_quietly"
            | "force_annotate_all_schematics"
    ) || is_planned_command(command)
}

/// A board placement command greyed because a selected footprint is locked
/// (#2240): the menu greys it, but its keystroke still reaches the handler,
/// which says which footprint is locked instead of doing nothing silently.
pub(crate) fn explains_refusal_in(command: &AppCommandId, ctx: &CommandContext) -> bool {
    ctx.pcb.shown && ctx.pcb.any && super::needs::is_placement_command(command)
}

/// An Active Bar command whose tool is a planned placeholder (#1249). It is
/// never enabled, but its keystroke still reaches the handler so the user
/// is told it is not built yet instead of getting silence.
pub(crate) fn is_planned_command(command: &AppCommandId) -> bool {
    super::active_bar::action_for_id(command.as_str())
        .is_some_and(|action| crate::active_bar::is_planned(&action))
}

/// The document the per-window commands (Save, Save As, Print) act on.
#[derive(Debug, Clone, Copy, Default)]
struct WindowDocumentKind {
    is_schematic: bool,
    is_editor: bool,
    is_dirty_board: bool,
    /// The window holds a document with a file of its own, so Save has
    /// something to run against: false in a Component Preview and in a
    /// window whose tab is gone, which refuse Save whatever the project holds.
    has_file: bool,
}

impl Signex {
    /// The [`CommandContext`] for a command invoked from `window` (`None`:
    /// the main window).
    ///
    /// Most handlers act on the main window's active tab, from whichever
    /// window the key came in, so most facts are the main window's. The
    /// per-window commands are the exception (#1800): Save, Save As and Print
    /// act on the document the window shows, so `document_is_schematic` /
    /// `document_is_editor` come from that window's own tab, and so do Undo /
    /// Redo's history and Fit All's `undocked_has_canvas`. A detached panel
    /// or modal shows no document of its own, so Ctrl+S typed there still
    /// saves the main window's. Close Document is resolved before this, in
    /// `close_document_in_window` (#1787).
    pub(crate) fn command_context(&self, window: Option<iced::window::Id>) -> CommandContext {
        let document = &self.document_state;
        let active_kind = document.tabs.get(document.active_tab).map(|t| &t.kind);
        let (can_undo, can_redo) = self.undo_availability(&self.undo_target(window));
        // A command typed in an undocked sheet window runs against that
        // window's selection, so it is enabled by it (#1966).
        let shown_canvas = match window {
            Some(id) if self.is_document_window(id) => self.interaction_state.canvas_for_window(id),
            _ => self.interaction_state.active_canvas(),
        };
        let main_has_schematic = document.active_engine().is_some();
        let has_symbol_editor = matches!(active_kind, Some(TabKind::SymbolEditor(_)));
        let has_footprint_editor = matches!(active_kind, Some(TabKind::FootprintEditor(_)));
        let own = self.window_document_kind(
            window,
            WindowDocumentKind {
                is_schematic: main_has_schematic,
                is_editor: has_symbol_editor || has_footprint_editor,
                is_dirty_board: document
                    .tabs
                    .get(document.active_tab)
                    .is_some_and(|t| self.board_has_unsaved_work(t)),
                has_file: true,
            },
        );
        // An undocked sheet window is enabled by the sheet it shows, not by
        // whatever tab the main window has active.
        let has_schematic = if window.is_some_and(|id| self.is_document_window(id)) {
            own.is_schematic
        } else {
            main_has_schematic
        };
        CommandContext {
            has_schematic,
            // An undocked PCB window is enabled by the board it shows (#2223).
            has_pcb: self.has_active_pcb()
                || window.is_some_and(|id| self.undocked_pcb_path(id).is_some()),
            undocked_has_canvas: self.undocked_tab_has_canvas(window),
            document_is_schematic: own.is_schematic,
            document_is_editor: own.is_editor,
            document_is_dirty_board: own.is_dirty_board,
            has_sheets: document.active_project.is_some() || !document.engines.is_empty(),
            has_symbol_editor,
            has_footprint_editor,
            has_selection: !shown_canvas.selected.is_empty(),
            has_drawing_selection: shown_canvas
                .selected
                .iter()
                .any(|item| item.kind == signex_types::schematic::SelectedKind::Drawing),
            drawing: self.interaction_state.wire_drawing
                || !self.interaction_state.polyline_points.is_empty(),
            can_undo,
            can_redo,
            project_dirty: self.active_project_is_dirty(),
            document_has_file: own.has_file,
            has_dirty_documents: !self.unsaved_set().is_empty(),
            has_document: !document.tabs.is_empty(),
            has_open_project: document.active_project.is_some(),
            has_parent_sheet: self.active_sheet_has_parent(),
            has_active_path: document.active_path.is_some(),
            has_library_editor: self.active_editor_library_root().is_some(),
            has_open_library: self.library_editor_target().is_some(),
            can_remove_part: self.active_symbol_part_is_removable(),
            pcb: self.pcb_targets(window),
            turn_in_hand: self.interaction_state.placement.is_some()
                || self.interaction_state.current_tool == Tool::BusEntry,
        }
    }

    /// What the per-window commands act on in `window`: the window's own tab
    /// in an undocked or Component Preview window, else `main`, the main
    /// window's active-tab facts.
    fn window_document_kind(
        &self,
        window: Option<iced::window::Id>,
        main: WindowDocumentKind,
    ) -> WindowDocumentKind {
        use super::window_document::WindowDocument;
        let none = WindowDocumentKind::default();
        let document = &self.document_state;
        // The main window, or a detached panel or modal, keeps `main`; a
        // window whose tab is gone has nothing.
        let index = match self.document_of_window(window) {
            None => return main,
            Some(WindowDocument::Tab(index)) => index,
            Some(WindowDocument::NoTab) => return none,
        };
        let Some(tab) = document.tabs.get(index) else {
            return none;
        };
        WindowDocumentKind {
            is_schematic: matches!(tab.kind, TabKind::Schematic)
                && document.engines.contains_key(&tab.path),
            is_editor: matches!(
                tab.kind,
                TabKind::SymbolEditor(_) | TabKind::FootprintEditor(_)
            ),
            is_dirty_board: self.board_has_unsaved_work(tab),
            has_file: !tab.kind.is_component_editor(),
        }
    }

    /// `tab` is a board with something Save writes: unsaved changes, or a
    /// live sketch not yet applied to it.
    fn board_has_unsaved_work(&self, tab: &crate::app::documents::TabInfo) -> bool {
        matches!(tab.kind, TabKind::Pcb)
            && (self.document_state.is_document_dirty(&tab.path)
                || self.board_sketch_owed().as_deref() == Some(tab.path.as_path()))
    }

    /// Whether the Navigator's hierarchy places the active tab's sheet
    /// below another sheet: one set lookup, since this runs on every menu
    /// draw.
    fn active_sheet_has_parent(&self) -> bool {
        let document = &self.document_state;
        document.tabs.get(document.active_tab).is_some_and(|tab| {
            document
                .panel_ctx
                .navigator
                .is_placed_below_a_sheet(&tab.path)
        })
    }

    /// Whether the active symbol editor is on a part Remove Part accepts:
    /// the symbol has several parts and the current one is not the first.
    fn active_symbol_part_is_removable(&self) -> bool {
        let document = &self.document_state;
        let Some(TabKind::SymbolEditor(path)) =
            document.tabs.get(document.active_tab).map(|t| &t.kind)
        else {
            return false;
        };
        document.symbol_editors.get(path).is_some_and(|editor| {
            crate::library::editor::symbol::state::max_part_number(editor.primitive()) > 1
                && editor.active_part > 1
        })
    }

    /// Whether `window` is an undocked tab window showing a schematic or a
    /// PCB.
    pub(super) fn undocked_tab_has_canvas(&self, window: Option<iced::window::Id>) -> bool {
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

    /// Whether the active project's `.snxprj` has unsaved changes.
    fn active_project_is_dirty(&self) -> bool {
        let document = &self.document_state;
        document.active_project.is_some_and(|id| {
            document
                .projects
                .iter()
                .find(|p| p.id == id)
                .is_some_and(|p| document.is_document_dirty(&p.path))
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
            project_id: None,
            kind,
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
    }

    fn with_board() -> Signex {
        let (mut app, _task) = Signex::new();
        let path = PathBuf::from("enablement.snxpcb");
        push_tab(&mut app, &path, TabKind::Pcb);
        app
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
        assert!(
            !enabled(&with_board(), "save_document"),
            "a clean board has nothing to save"
        );
        let mut board = with_board();
        let path = board.document_state.tabs[0].path.clone();
        board.document_state.mark_document_dirty(&path);
        assert!(
            enabled(&board, "save_document"),
            "a board with changes saves"
        );
        assert!(!enabled(&board, "save_document_as"), "PCB has no Save As");
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
            !app.document_state.footprint_editors[&path]
                .history
                .can_undo()
        );
        assert!(enabled(&app, "redo"));
    }

    /// A net-colour change is an engine step, so Ctrl+Z has something to undo.
    #[test]
    fn a_net_colour_change_enables_undo() {
        use crate::app::handlers::editing_commands::net_color_driver as colours;
        let (mut app, path) = colours::app_with_sheet("colour.snxsch");
        assert!(!enabled(&app, "undo"));
        let wire = colours::wire_of(&app, &path);
        colours::paint(&mut app, &[wire], colours::color(1));
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
                document_id: None,
                description: None,
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
                schematic_defaults: Default::default(),
                project_defaults: Default::default(),
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
        app.document_state.active_project = Some(project);
        assert!(!enabled(&app, "save_document"));

        app.document_state.mark_document_dirty(&path);
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

    /// #368 review: ERC assembles the project's sheets, so it runs with a PCB
    /// or editor tab active too. The quiet annotate commands start from the
    /// active sheet's path and only logged "no sheet is open" without one, so
    /// they are off until a sheet is open (#1922); their keystroke still
    /// reaches the handler, which says so.
    #[test]
    fn erc_stays_enabled_with_a_project_but_annotation_needs_an_open_sheet() {
        let (mut app, _path) = with_footprint_editor();
        assert!(!enabled(&app, "run_erc"));
        app.document_state.active_project = Some(app.document_state.mint_project_id());
        assert!(enabled(&app, "run_erc"));
        for name in [
            "annotate_schematic_quietly",
            "force_annotate_all_schematics",
        ] {
            assert!(!enabled(&app, name), "{name} with no sheet open");
            assert!(explains_when_unavailable(&id(name)), "{name}");
        }
        let app = with_schematic();
        assert!(enabled(&app, "annotate_schematic_quietly"));
        assert!(enabled(&app, "force_annotate_all_schematics"));
    }

    /// The quiet annotate commands are greyed with a project but no sheet, but
    /// the key still reaches the handler, which says there is nothing to do
    /// (#1922), and the outcome reports it as disabled.
    #[test]
    fn quiet_annotate_with_no_sheet_still_reaches_the_handler_and_says_so() {
        let _ = crate::diagnostics::init_logging();
        for name in [
            "annotate_schematic_quietly",
            "force_annotate_all_schematics",
        ] {
            let (mut app, _task) = Signex::new();
            app.document_state.active_project = Some(app.document_state.mint_project_id());
            let since = crate::diagnostics::newest_entry_id().unwrap_or(0);

            let (outcome, _task) =
                app.dispatch_command_reporting(&id(name), CommandArgs::none(), None);

            assert!(
                matches!(outcome, super::super::CommandOutcome::Disabled { .. }),
                "{name}"
            );
            let said = crate::diagnostics::recent_entries()
                .iter()
                .any(|e| e.id > since && e.message.contains("no sheet is open, nothing to do"));
            assert!(said, "{name}: the handler did not run and explain");
        }
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

    /// #1256 - Ctrl+M in a PCB, footprint or empty window does not arm a tool
    /// the schematic canvas it belongs to is not there to show.
    #[test]
    fn measure_distance_needs_a_schematic() {
        let (mut app, _task) = Signex::new();
        assert!(!enabled(&app, "measure_distance"));
        let _ = app.dispatch_command(&id("measure_distance"), CommandArgs::none(), None);
        assert_eq!(app.interaction_state.current_tool, Tool::Select);

        let mut app = with_schematic();
        assert!(enabled(&app, "measure_distance"));
        let _ = app.dispatch_command(&id("measure_distance"), CommandArgs::none(), None);
        assert_eq!(app.interaction_state.current_tool, Tool::Measure);
    }

    /// #1254 - Break Wire arms its tool only with a schematic open.
    #[test]
    fn break_wire_needs_a_schematic() {
        let (mut app, _task) = Signex::new();
        assert!(!enabled(&app, "break_wire"));
        let _ = app.dispatch_command(&id("break_wire"), CommandArgs::none(), None);
        assert_eq!(app.interaction_state.current_tool, Tool::Select);

        let mut app = with_schematic();
        assert!(enabled(&app, "break_wire"));
        let _ = app.dispatch_command(&id("break_wire"), CommandArgs::none(), None);
        assert_eq!(app.interaction_state.current_tool, Tool::BreakWire);
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
    /// The placement shortcuts act on a schematic sheet, so each is off
    /// without one and on with one (the Place menu rows share the gate).
    #[test]
    fn schematic_placement_shortcuts_need_a_schematic() {
        let off = CommandContext::default();
        let on = CommandContext {
            has_schematic: true,
            ..CommandContext::default()
        };
        for name in [
            "place_power_symbol",
            "place_global_label",
            "place_hierarchical_label",
            "place_junction",
            "place_port",
            "place_off_sheet_connector",
        ] {
            assert!(!is_enabled(&id(name), &off), "{name} without a schematic");
            assert!(is_enabled(&id(name), &on), "{name} with a schematic");
        }
    }

    #[test]
    fn ids_that_resolve_to_the_same_message_share_their_enablement() {
        let facts: [fn(&mut CommandContext); 14] = [
            |c| c.has_schematic = true,
            |c| c.has_pcb = true,
            |c| c.undocked_has_canvas = true,
            |c| c.has_sheets = true,
            |c| c.has_symbol_editor = true,
            |c| c.has_footprint_editor = true,
            |c| c.has_selection = true,
            |c| c.has_drawing_selection = true,
            |c| c.can_undo = true,
            |c| c.can_redo = true,
            |c| c.project_dirty = true,
            |c| c.document_has_file = true,
            |c| c.has_parent_sheet = true,
            |c| c.can_remove_part = true,
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
                super::super::bridge::resolve(&command, &CommandArgs::none())
                    .map(|m| (command, format!("{m:?}")))
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

    /// #1922: the alias pairs run one message and share one gate, so the
    /// second id of a pair cannot arm a tool the first refuses.
    #[test]
    fn alias_pairs_have_one_route_and_one_gate() {
        let pairs = [
            ("place_wire", "draw_wire"),
            ("place_bus", "draw_bus"),
            ("place_text", "place_text_string"),
            ("draw_graphic_line", "draw_line"),
            ("draw_hierarchical_sheet", "place_sheet_symbol"),
        ];
        let without = CommandContext::default();
        let with = CommandContext {
            has_schematic: true,
            ..without
        };
        for (a, b) in pairs {
            let message = |name: &str| {
                format!(
                    "{:?}",
                    super::super::bridge::resolve(&id(name), &CommandArgs::none())
                )
            };
            assert_eq!(message(a), message(b), "{a} / {b} resolve differently");
            for ctx in [&without, &with] {
                assert_eq!(
                    is_enabled(&id(a), ctx),
                    is_enabled(&id(b), ctx),
                    "{a} / {b}"
                );
            }
        }
        // The two wiring pairs need a schematic, both ids of each.
        for name in ["place_wire", "draw_wire", "place_bus", "draw_bus"] {
            assert!(!is_enabled(&id(name), &without), "{name}");
            assert!(is_enabled(&id(name), &with), "{name}");
        }
    }

    /// A command is enabled exactly when it has no reason to be disabled,
    /// for every catalog id (#1928).
    #[test]
    fn the_disabled_reason_agrees_with_is_enabled_for_every_command() {
        let facts: [fn(&mut CommandContext); 6] = [
            |c| c.has_schematic = true,
            |c| c.has_selection = true,
            |c| c.has_footprint_editor = true,
            |c| c.can_undo = true,
            |c| c.has_document = true,
            |c| c.has_sheets = true,
        ];
        let mut contexts = vec![CommandContext::default()];
        contexts.extend(facts.iter().map(|set| {
            let mut ctx = CommandContext::default();
            set(&mut ctx);
            ctx
        }));
        for name in crate::keymap::all_command_ids() {
            let command = id(name);
            for ctx in &contexts {
                let reason = disabled_reason(&command, ctx);
                assert_eq!(
                    reason.is_none(),
                    is_enabled(&command, ctx),
                    "{name} in {ctx:?}"
                );
                assert!(reason.is_none_or(|r| !r.is_empty()), "{name}");
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

    /// An undocked tab window counts for Fit All (its own canvas) and for
    /// the per-window document commands, Save, Save As and Print (#1813).
    /// Copy and the rest act on the main window's tab, so they stay disabled
    /// with no document there.
    #[test]
    fn an_undocked_schematic_enables_fit_all_and_its_file_rows() {
        let (mut app, _task) = Signex::new();
        let path = PathBuf::from("undocked.snxsch");
        let engine =
            signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
        app.document_state.engines.insert(path.clone(), engine);
        push_tab(&mut app, &path, TabKind::Schematic);
        app.document_state.active_tab = usize::MAX;
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
        assert!(is_enabled(&id("save_document"), &ctx));
        assert!(is_enabled(&id("save_document_as"), &ctx));
        assert!(is_enabled(&id("print"), &ctx));
        assert!(!is_enabled(&id("paste"), &ctx));
        assert!(
            !enabled(&app, "save_document_as"),
            "the main window has no sheet"
        );
    }

    /// An undocked schematic window whose sheet has one undo step, its tab
    /// placed before the main window's active tab.
    fn undock_sheet_with_a_step(app: &mut Signex) -> (iced::window::Id, PathBuf) {
        let main_tab = app.document_state.active_tab;
        let path = PathBuf::from("undocked.snxsch");
        let mut engine =
            signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
        let document = engine.document().clone();
        engine
            .execute(signex_engine::Command::ReplaceDocument { document })
            .unwrap();
        app.document_state.engines.insert(path.clone(), engine);
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
        let window = iced::window::Id::unique();
        app.ui_state.windows.insert(
            window,
            crate::app::state::WindowKind::UndockedTab {
                path: path.clone(),
                title: "undocked".into(),
            },
        );
        app.interaction_state
            .canvases
            .insert(window, crate::canvas::CanvasSlot::new());
        (window, path)
    }

    /// #1546: Ctrl+Z typed in an undocked schematic window steps that
    /// window's sheet, even while the main window shows another document
    /// with history of its own.
    #[test]
    fn undo_in_an_undocked_window_steps_that_windows_sheet() {
        let (mut app, footprint) = with_footprint_editor();
        app.document_state
            .footprint_editors
            .get_mut(&footprint)
            .unwrap()
            .push_history();
        let (window, path) = undock_sheet_with_a_step(&mut app);
        let main_tab = app.document_state.active_tab;

        let _ = app.dispatch_command(&id("undo"), CommandArgs::none(), Some(window));

        let engine = &app.document_state.engines[&path];
        assert!(engine.can_redo(), "the undocked sheet was undone");
        assert!(
            app.document_state.footprint_editors[&footprint]
                .history
                .can_undo(),
            "the main window's footprint editor kept its step"
        );
        assert_eq!(app.document_state.active_tab, main_tab);
        assert_eq!(app.document_state.active_path, None);
    }

    /// #1550: an undocked window's Edit menu greys Undo / Redo by that
    /// window's sheet, and a click there steps that sheet.
    #[test]
    fn the_undocked_windows_edit_menu_follows_its_own_sheet() {
        let (mut app, _task) = Signex::new();
        let (window, path) = undock_sheet_with_a_step(&mut app);

        assert!(!enabled(&app, "undo"), "the main window has no history");
        let ctx = app.command_context(Some(window));
        assert!(is_enabled(&id("undo"), &ctx));
        assert!(!is_enabled(&id("redo"), &ctx));

        let _ = app.update(Message::Ui(UiMsg::MenuInWindow {
            window,
            msg: MenuMessage::command("undo"),
        }));

        assert!(app.document_state.engines[&path].can_redo());
        let ctx = app.command_context(Some(window));
        assert!(!is_enabled(&id("undo"), &ctx));
        assert!(is_enabled(&id("redo"), &ctx));
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

    /// Command ids of every planned Active Bar tool that has one.
    fn planned_ids() -> Vec<&'static str> {
        super::super::active_bar::ACTIVE_BAR_COMMANDS
            .iter()
            .filter(|(_, action)| crate::active_bar::is_planned(action))
            .map(|(command, _)| *command)
            .collect()
    }

    #[test]
    fn planned_tool_commands_are_never_enabled_but_always_explain() {
        let ids = planned_ids();
        assert!(ids.contains(&"place_diff_pair"), "{ids:?}");
        let with_command = crate::active_bar::PLANNED_TOOLS
            .iter()
            .filter(|(planned, _)| {
                super::super::active_bar::ACTIVE_BAR_COMMANDS
                    .iter()
                    .any(|(_, action)| action == planned)
            })
            .count();
        assert_eq!(ids.len(), with_command, "{ids:?}");
        let (app, _t) = Signex::new();
        for command in ids {
            assert!(!enabled(&app, command), "{command} must not be enabled");
            assert!(
                explains_when_unavailable(&id(command)),
                "{command} must reach the handler so it can say why"
            );
        }
    }

    #[test]
    fn the_palette_does_not_list_planned_tools_as_runnable() {
        let (app, _t) = Signex::new();
        let listed: Vec<String> = crate::app::command_palette::build_catalog(&app)
            .into_iter()
            .filter_map(|entry| match entry.action {
                crate::app::command_palette::CommandAction::Command(c) => {
                    Some(c.as_str().to_string())
                }
                _ => None,
            })
            .collect();
        for command in planned_ids() {
            assert!(!listed.iter().any(|l| l == command), "{command} is listed");
        }
    }

    #[test]
    fn a_planned_tool_keystroke_still_logs_the_refusal() {
        let _ = crate::diagnostics::init_logging();
        let (mut app, _t) = Signex::new();
        let _ = app.dispatch_command(&id("place_diff_pair"), CommandArgs::default(), None);
        assert!(
            crate::diagnostics::recent_entries().iter().any(|e| e
                .message
                .starts_with("Planned, not available yet: Differential Pair")),
            "the refusal must reach the Messages panel, not be dropped"
        );
    }

    #[test]
    fn save_as_and_print_still_explain_why_they_did_nothing() {
        assert!(explains_when_unavailable(&id("save_document_as")));
        assert!(explains_when_unavailable(&id("print")));
        assert!(!explains_when_unavailable(&id("save_document")));
    }

    /// #1772: a root sheet and a sheet placed by a symbol on it, with the
    /// Navigator's hierarchy built over them and `active` focused.
    fn with_hierarchy(active_is_child: bool) -> Signex {
        use crate::app::handlers::menu::export::tests::sheet_with_net;
        use crate::app::sheet_hierarchy::build_sheet_hierarchy;
        let root = PathBuf::from("/p/root.snxsch");
        let child = PathBuf::from("/p/child.snxsch");
        let sheets = std::collections::HashMap::from([
            (root.clone(), sheet_with_net("R1", "N", &["child.snxsch"])),
            (child.clone(), sheet_with_net("R2", "N", &[])),
        ]);
        let hierarchy = build_sheet_hierarchy(
            &sheets,
            &[],
            &root,
            &[],
            crate::app::state::scope::CaseFold::host_default(),
        );
        let (mut app, _task) = Signex::new();
        app.document_state
            .panel_ctx
            .navigator
            .replace_hierarchy(hierarchy, None, &[]);
        let active = if active_is_child { &child } else { &root };
        push_tab(&mut app, active, TabKind::Schematic);
        app
    }

    /// The answer follows the active tab with no hierarchy rebuild between.
    #[test]
    fn up_hierarchy_follows_the_active_tab_without_a_rebuild() {
        let mut app = with_hierarchy(false);
        let child = PathBuf::from("/p/child.snxsch");
        assert!(!enabled(&app, "navigate_up_hierarchy"));
        push_tab(&mut app, &child, TabKind::Schematic);
        assert!(enabled(&app, "navigate_up_hierarchy"));
        app.document_state.active_tab = 0;
        assert!(!enabled(&app, "navigate_up_hierarchy"));
    }

    #[test]
    fn up_hierarchy_shortcut_is_explained_not_dropped() {
        assert!(explains_when_unavailable(&id("navigate_up_hierarchy")));
        assert!(explains_when_unavailable(&id("leave_sheet")));
    }

    #[test]
    fn up_hierarchy_needs_a_parent_sheet() {
        let top = with_hierarchy(false);
        let child = with_hierarchy(true);
        for name in ["navigate_up_hierarchy", "leave_sheet"] {
            assert!(!enabled(&top, name), "{name} on a top-level sheet");
            assert!(enabled(&child, name), "{name} on a child sheet");
        }
    }

    #[test]
    fn up_hierarchy_is_off_without_a_hierarchy() {
        let app = with_schematic();
        assert!(!enabled(&app, "navigate_up_hierarchy"));
    }

    #[test]
    fn part_commands_need_a_symbol_editor() {
        let schematic = with_schematic();
        let ctx = schematic.command_context(None);
        assert!(!new_part_menu_enabled(&ctx));
        assert!(!remove_part_menu_enabled(&ctx));
        let (editor, _) = with_symbol_editor();
        assert!(new_part_menu_enabled(&editor.command_context(None)));
    }

    #[test]
    fn remove_part_needs_a_second_part_to_be_current() {
        let (mut app, path) = with_symbol_editor();
        assert!(!remove_part_menu_enabled(&app.command_context(None)));
        let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
        editor.primitive_mut().part_count = 2;
        assert!(
            !remove_part_menu_enabled(&app.command_context(None)),
            "on part 1"
        );
        app.document_state
            .symbol_editors
            .get_mut(&path)
            .unwrap()
            .active_part = 2;
        assert!(remove_part_menu_enabled(&app.command_context(None)));
    }
}
