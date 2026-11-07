//! Home of the Command Registry (#278): the id->[`crate::app::Message`] bridge
//! (`bridge.rs`), the dispatch entry point ([`Signex::dispatch_command`]) and
//! its argument type ([`CommandArgs`]), and the enablement gate
//! ([`enablement::is_enabled`]) the menu bar reads too. The palette and the
//! keymap run commands through [`Signex::dispatch_command`], and so do the
//! context menus' command rows (`ContextAction::Command`, #1727), which pass
//! the click point and project in [`InvocationContext`]. The menu bar's rows
//! that carry a command id run it here too (`MenuMessage::Command`, #1922):
//! the main window's, an undocked window's and the native menu's.

pub(crate) mod active_bar;
mod args;
pub(crate) mod bridge;
pub(crate) mod enablement;
mod needs;
mod outcome;
#[cfg(test)]
mod undo_agreement_tests;
mod window_document;

pub use args::CommandArgs;
pub(crate) use args::InvocationContext;
use bridge::core_to_message;
pub(crate) use enablement::CommandContext;
pub(crate) use enablement::is_enabled;
pub use outcome::CommandOutcome;

use iced::Task;

use super::document::{DocumentKey, UndoTarget};

use super::*;

impl Signex {
    /// The Command Registry's dispatch entry point — resolve a stable
    /// [`crate::keymap::AppCommandId`] and run it exactly as if the
    /// resolved [`Message`] had been sent directly.
    ///
    /// Deliberately takes an id + args, never a [`Message`] — the whole
    /// point of the registry is that a caller (keyboard, menu, command
    /// palette, context menu, and a future CLI) names *what* to run
    /// without knowing the app's internal message shape.
    ///
    /// Keymap, palette and menu callers have no use for the result; this is
    /// [`Self::dispatch_command_reporting`] without it.
    pub(crate) fn dispatch_command(
        &mut self,
        command: &crate::keymap::AppCommandId,
        args: CommandArgs,
        window: Option<iced::window::Id>,
    ) -> Task<Message> {
        self.dispatch_command_reporting(command, args, window).1
    }

    /// A menu row's command, from the window `window` (`None`: the main one).
    /// The row was live, so it normally runs; if it does not, the Messages
    /// panel says why: a command the window cannot act on is named here, and
    /// an unknown or unmapped id is logged by the bridge.
    pub(crate) fn run_menu_command(
        &mut self,
        command: &crate::keymap::AppCommandId,
        window: Option<iced::window::Id>,
    ) -> Task<Message> {
        let (outcome, task) = self.dispatch_command_reporting(command, CommandArgs::none(), window);
        if let CommandOutcome::Disabled { reason } = outcome {
            crate::diagnostics::log_info(format!(
                "Menu: {} did nothing: {reason}",
                command.as_str()
            ));
        }
        task
    }

    /// [`Self::dispatch_command`], reporting what happened (#1928).
    ///
    /// `args` carries what the caller knows that the id does not (see
    /// [`CommandArgs`]); the bridge reads it for the commands that take it.
    ///
    /// An id can fail to resolve two ways, and both are a no-op that is
    /// reported, never a panic: it isn't in the catalog at all
    /// ([`CommandOutcome::UnknownId`]), or it's a real catalog id with no
    /// live arm in `core_to_message` yet ([`CommandOutcome::NoBridgeArm`]).
    /// Ids come from user-editable TOML keymap profiles today and a CLI
    /// later — both are untrusted input, not a programmer error.
    ///
    /// `window` is the window the command was invoked from, if any. A canvas
    /// event goes to the canvas of the undocked window it was typed in; the
    /// plain [`Message::CanvasEvent`] always targets the main window (#525).
    ///
    /// A command [`is_enabled`] says has nothing to act on in that window is
    /// dropped here, the same rule the menu bar greys its row by (#368), and
    /// reported as [`CommandOutcome::Disabled`] with the reason, unless its
    /// handler tells the user why it did nothing
    /// ([`enablement::explains_when_unavailable`]): that one still runs, and
    /// is reported as `Disabled` too, so a caller is never told `Ran` for a
    /// command that had nothing to act on.
    pub(crate) fn dispatch_command_reporting(
        &mut self,
        command: &crate::keymap::AppCommandId,
        args: CommandArgs,
        window: Option<iced::window::Id>,
    ) -> (CommandOutcome, Task<Message>) {
        // Undo / Redo typed in an undocked tab step that window's document
        // and are gated by its history, not the main window's active tab
        // (#1546).
        if let Some(window_id) = window
            && matches!(command.as_str(), "undo" | "redo")
            && self.undocked_tab_has_canvas(window)
        {
            let mut outcome = CommandOutcome::Ran;
            let task = self.run_in_window(window_id, true, |app| {
                let (inner, task) = app.dispatch_command_reporting(command, args, None);
                outcome = inner;
                task
            });
            return (outcome, task);
        }
        // Undo / Redo typed in a window that shows a document of its own
        // (a Component Preview, an undocked editor) step that document, gated
        // by its history, never the main window's tab (#1854). A sheet is
        // stepped through the swap above; the ERC dialog through its message.
        if matches!(command.as_str(), "undo" | "redo")
            && self.undo_window_is_own(window)
            && let target = self.undo_target(window)
            && !matches!(
                target,
                UndoTarget::ProjectRules(_)
                    | UndoTarget::Document(DocumentKey::Schematic(_) | DocumentKey::Pcb(_))
            )
        {
            let undo = command.as_str() == "undo";
            let (can_undo, can_redo) = self.undo_availability(&target);
            if !(if undo { can_undo } else { can_redo }) {
                let reason = if undo {
                    "Nothing to undo"
                } else {
                    "Nothing to redo"
                };
                return (CommandOutcome::disabled(reason), Task::none());
            }
            self.step_undo_target(&target, undo);
            return (CommandOutcome::Ran, Task::none());
        }
        // Close Document follows the window the stroke came from (#1787).
        if command.as_str() == "close_active_document"
            && let Some(result) = self.close_document_in_window(window)
        {
            return result;
        }
        // Save, Save As, Print and the tab stepper act on the document the
        // window shows (#1794).
        if let Some(result) = self.window_document_command(command, &args, window) {
            return result;
        }
        let unmet = enablement::disabled_reason(command, &self.command_context(window));
        if unmet.is_some() && !enablement::explains_when_unavailable(command) {
            return (
                CommandOutcome::disabled(unmet.unwrap_or_default()),
                Task::none(),
            );
        }
        // Undo / Redo follow the window the stroke came from: in the ERC
        // dialog they step the project's rule history and never reach the
        // sheet, whatever that history holds.
        let resolved = if self.erc_dialog_takes_input(window) {
            match command.as_str() {
                "undo" => Some(Message::Erc(ErcMsg::RulesUndone)),
                "redo" => Some(Message::Erc(ErcMsg::RulesRedone)),
                _ => core_to_message(command, &args),
            }
        } else {
            core_to_message(command, &args)
        };
        let outcome = match (unmet, &resolved) {
            (Some(reason), _) => CommandOutcome::disabled(reason),
            (None, Some(_)) => CommandOutcome::Ran,
            (None, None) if crate::keymap::metadata_for(command).is_some() => {
                CommandOutcome::NoBridgeArm
            }
            (None, None) => CommandOutcome::UnknownId,
        };
        let task = match (resolved, window) {
            (Some(Message::CanvasEvent(event)), Some(window_id))
                if self.interaction_state.canvases.contains_key(&window_id) =>
            {
                self.dispatch_update(Message::CanvasEventInWindow { window_id, event })
            }
            // Any other command typed in an undocked document window acts on
            // the sheet that window shows (#1966).
            (Some(message), Some(window_id)) if self.is_document_window(window_id) => {
                self.run_command_in_window(window_id, message)
            }
            // Find Next / Previous from main (F3, menu, palette: `None` is
            // main) take a Sheet-scope Find owned by an undocked window over.
            (Some(message), w)
                if is_find_step(&message)
                    && w.is_none_or(|w| self.ui_state.main_window_id == Some(w)) =>
            {
                match self.ui_state.main_window_id {
                    Some(main_id) => self.step_find_in_window(main_id, message),
                    None => self.dispatch_update(message),
                }
            }
            (Some(message), _) => self.dispatch_update(message),
            (None, _) => Task::none(),
        };
        (outcome, task)
    }
}

/// F3 / Shift+F3 resolved to a Find step.
fn is_find_step(message: &Message) -> bool {
    use crate::find_replace::FindReplaceMsg::{NextRequested, PreviousRequested};
    matches!(
        message,
        Message::FindReplaceMsg(NextRequested | PreviousRequested)
    )
}

#[cfg(test)]
mod arrange_tests;

#[cfg(test)]
mod place_part_tests;

#[cfg(test)]
mod shortcut_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::AppCommandId;
    use crate::schematic_runtime::hit_test::SelectionMode;

    /// A known id resolves through the bridge and reaches the right
    /// handler — observed here via its state mutation rather than by
    /// comparing `Message`/`Task` values.
    #[test]
    fn known_id_dispatches_to_the_right_message() {
        let (mut app, _task) = Signex::new();
        app.ui_state.selection_mode = SelectionMode::Touching;

        let command = AppCommandId::new("cycle_selection_mode").unwrap();
        let _ = app.dispatch_command(&command, CommandArgs::none(), None);

        assert_eq!(app.ui_state.selection_mode, SelectionMode::Inside);
    }

    /// #791 — the `next_grid` command (the `N` key in the Classic profile) steps the grid through the
    /// same presets as View > Cycle Grid Size.
    #[test]
    fn next_grid_command_steps_the_grid() {
        let (mut app, _task) = Signex::new();
        app.ui_state.grid_size_mm = 1.27;
        app.ui_state.visible_grid_mm = 1.27;

        let command = AppCommandId::new("next_grid").unwrap();
        let _ = app.dispatch_command(&command, CommandArgs::none(), None);

        assert_eq!(app.ui_state.grid_size_mm, 2.54);
        assert_eq!(app.ui_state.visible_grid_mm, 2.54);
    }

    /// #1057 - the click snap lands on an exact nm multiple of the grid
    /// (100 steps of 1.27 mm is 127000000 nm, not 126999998).
    #[test]
    fn snap_to_grid_is_exact_in_nanometres() {
        let (mut app, _task) = Signex::new();
        app.ui_state.snap_enabled = true;
        app.ui_state.grid_size_mm = 1.27;

        let (x, y) = app.snap_to_grid(127.0 + 0.2, 254.0 - 0.3);

        assert_eq!((x * 1e6).round() as i64, 127_000_000);
        assert_eq!((y * 1e6).round() as i64, 254_000_000);
    }

    /// An id with no catalog entry at all is untrusted input, not a
    /// programmer error — it must resolve to a no-op, never panic, and say
    /// it was unknown (#1928).
    #[test]
    fn unknown_id_is_a_no_op_and_does_not_panic() {
        let (mut app, _task) = Signex::new();

        let command = AppCommandId::new("not_a_real_command").unwrap();
        let (outcome, task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);

        assert_eq!(outcome, CommandOutcome::UnknownId);
        assert_eq!(task.units(), 0);
    }

    /// `center_on_cursor` is a real catalog entry
    /// (`keymap/catalog`) with no arm in `core_to_message`
    /// yet — it must resolve to a no-op too, not panic.
    #[test]
    fn catalog_id_with_no_bridge_arm_is_a_no_op_and_does_not_panic() {
        let (mut app, _task) = Signex::new();

        let command = AppCommandId::new("center_on_cursor").unwrap();
        assert!(
            crate::keymap::metadata_for(&command).is_some(),
            "test fixture drifted: center_on_cursor is no longer a catalog id"
        );

        let (outcome, task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);

        assert_eq!(outcome, CommandOutcome::NoBridgeArm);
        assert_eq!(task.units(), 0);
    }

    /// A command with nothing to act on is dropped and says why (#1928).
    #[test]
    fn a_disabled_command_reports_why_it_did_nothing() {
        let (mut app, _task) = Signex::new();

        let command = AppCommandId::new("place_wire").unwrap();
        let (outcome, task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);

        assert_eq!(
            outcome,
            CommandOutcome::Disabled {
                reason: "No schematic is open".to_string()
            }
        );
        assert_eq!(task.units(), 0);
        assert_eq!(app.interaction_state.current_tool, Tool::Select);
    }

    /// A command whose handler explains itself still runs while disabled,
    /// and is reported as disabled rather than as having run (#1928).
    #[test]
    fn an_explaining_command_runs_but_is_reported_disabled() {
        let _ = crate::diagnostics::init_logging();
        let (mut app, _task) = Signex::new();

        let command = AppCommandId::new("place_diff_pair").unwrap();
        let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);

        assert_eq!(
            outcome,
            CommandOutcome::Disabled {
                reason: "Planned, not available yet".to_string()
            }
        );
        assert!(
            crate::diagnostics::recent_entries().iter().any(|e| e
                .message
                .starts_with("Planned, not available yet: Differential Pair")),
            "the handler ran and explained"
        );
    }

    #[test]
    fn an_enabled_known_command_reports_that_it_ran() {
        let (mut app, _task) = Signex::new();

        let command = AppCommandId::new("cycle_selection_mode").unwrap();
        let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);

        assert_eq!(outcome, CommandOutcome::Ran);
    }

    /// Close Document typed in a detached modal or panel does nothing; the
    /// outcome says so instead of `Ran` (#1792, #1928).
    #[test]
    fn close_document_in_a_detached_modal_is_reported_disabled() {
        let (mut app, _task) = Signex::new();
        let window = iced::window::Id::unique();
        app.ui_state.windows.insert(
            window,
            crate::app::state::WindowKind::DetachedModal(crate::app::state::ModalId::ErcDialog),
        );

        let command = AppCommandId::new("close_active_document").unwrap();
        let (outcome, task) =
            app.dispatch_command_reporting(&command, CommandArgs::none(), Some(window));

        assert!(matches!(outcome, CommandOutcome::Disabled { .. }));
        assert_eq!(task.units(), 0);
    }

    /// #1922: a menu click that does not run says why in the Messages panel.
    #[test]
    fn a_menu_click_that_cannot_run_is_logged() {
        let _ = crate::diagnostics::init_logging();
        let (mut app, _task) = Signex::new();
        let click = |app: &mut Signex, id: &str| {
            let _ = app.update(Message::Menu(crate::menu_bar::MenuMessage::command(id)));
        };
        let count = |needle: &str| crate::diagnostics::logged_count(needle);

        let disabled = count("Menu: place_wire did nothing: No schematic is open");
        click(&mut app, "place_wire");
        assert_eq!(
            count("Menu: place_wire did nothing: No schematic is open"),
            disabled + 1
        );

        let unmapped = count("command 'center_on_cursor'");
        click(&mut app, "center_on_cursor");
        assert!(
            count("command 'center_on_cursor'") > unmapped,
            "no bridge arm"
        );

        let unknown = count("command 'no_such_menu_command'");
        click(&mut app, "no_such_menu_command");
        assert!(
            count("command 'no_such_menu_command'") > unknown,
            "unknown id"
        );
    }

    /// #103 - `unselect_all` empties the active canvas selection. It used
    /// to be a catalog id with no bridge arm, so Ctrl+Shift+A did nothing.
    #[test]
    fn unselect_all_command_clears_the_selection() {
        use signex_types::schematic::{SelectedItem, SelectedKind};

        let (mut app, _task) = Signex::new();
        app.interaction_state.active_canvas_mut().selected =
            vec![SelectedItem::new(uuid::Uuid::now_v7(), SelectedKind::Wire)];

        let command = AppCommandId::new("unselect_all").unwrap();
        let _ = app.dispatch_command(&command, CommandArgs::none(), None);

        assert!(
            app.interaction_state
                .active_canvas_mut()
                .selected
                .is_empty()
        );
    }

    fn app_with_tabs(kinds: Vec<TabKind>) -> Signex {
        let (mut app, _task) = Signex::new();
        for (i, kind) in kinds.into_iter().enumerate() {
            app.document_state.tabs.push(crate::app::TabInfo {
                title: format!("t{i}"),
                path: std::path::PathBuf::from(format!("t{i}.snxsch")),
                cached_document: None,
                project_id: None,
                kind,
            });
        }
        app.document_state.active_tab = 0;
        app
    }

    /// #103 - `next_sheet` / `previous_sheet` walk the schematic tabs,
    /// wrap at both ends, and step over a non-schematic tab in between.
    #[test]
    fn sheet_commands_cycle_the_schematic_tabs_and_wrap() {
        let mut app = app_with_tabs(vec![TabKind::Schematic, TabKind::Pcb, TabKind::Schematic]);
        let next = AppCommandId::new("next_sheet").unwrap();
        let previous = AppCommandId::new("previous_sheet").unwrap();

        let _ = app.update(core_message(&next));
        assert_eq!(app.document_state.active_tab, 2, "skips the PCB tab");
        let _ = app.update(core_message(&next));
        assert_eq!(app.document_state.active_tab, 0, "wraps forward");
        let _ = app.update(core_message(&previous));
        assert_eq!(app.document_state.active_tab, 2, "wraps backward");
    }

    #[test]
    fn sheet_commands_do_nothing_from_a_non_schematic_tab() {
        let mut app = app_with_tabs(vec![TabKind::Schematic, TabKind::Pcb]);
        app.document_state.active_tab = 1;
        let next = AppCommandId::new("next_sheet").unwrap();

        let _ = app.update(core_message(&next));

        assert_eq!(app.document_state.active_tab, 1);
    }

    fn run(app: &mut Signex, id: &str) {
        let _ = app.update(core_message(&AppCommandId::new(id).unwrap()));
    }

    /// #103 - `next_document_tab` / `previous_document_tab` step through
    /// every tab kind, wrapping at both ends.
    #[test]
    fn document_tab_commands_cycle_all_tabs_and_wrap() {
        let mut app = app_with_tabs(vec![TabKind::Schematic, TabKind::Pcb, TabKind::Schematic]);
        run(&mut app, "next_document_tab");
        assert_eq!(app.document_state.active_tab, 1, "lands on the PCB tab");
        run(&mut app, "next_document_tab");
        run(&mut app, "next_document_tab");
        assert_eq!(app.document_state.active_tab, 0, "wraps forward");
        run(&mut app, "previous_document_tab");
        assert_eq!(app.document_state.active_tab, 2, "wraps backward");
    }

    /// #103 - `previous_grid` and `cycle_snap_grid_backward` undo `next_grid`.
    #[test]
    fn previous_grid_commands_step_the_grid_back() {
        let sizes = crate::canvas::grid::GRID_SIZES_MM;
        for id in ["previous_grid", "cycle_snap_grid_backward"] {
            let (mut app, _task) = Signex::new();
            app.ui_state.grid_size_mm = sizes[1];
            run(&mut app, id);
            assert_eq!(app.ui_state.grid_size_mm, sizes[0], "{id}");
            run(&mut app, id);
            assert_eq!(
                app.ui_state.grid_size_mm,
                sizes[sizes.len() - 1],
                "{id} wraps"
            );
        }
    }

    fn core_message(command: &AppCommandId) -> Message {
        bridge::core_to_message(command, &CommandArgs::none()).expect("the command resolves")
    }
}
