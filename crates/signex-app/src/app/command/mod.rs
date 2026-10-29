//! Home of the Command Registry (#278): the id->[`crate::app::Message`] bridge
//! (`bridge.rs`), the dispatch entry point ([`Signex::dispatch_command`]) and
//! its argument type ([`CommandArgs`]), and the enablement gate
//! ([`enablement::is_enabled`]) the menu bar reads too. The palette and the
//! keymap run commands through [`Signex::dispatch_command`]. Of the menu items
//! only Undo and Redo reach it, and only from an undocked window's menu or the
//! native menu; the main window's menu emits its own `Message` variants
//! (`handlers/menu/editing.rs`).

pub(crate) mod active_bar;
mod args;
pub(crate) mod bridge;
pub(crate) mod enablement;

pub(crate) use args::CommandArgs;
use bridge::core_to_message;
pub(crate) use enablement::CommandContext;
pub(crate) use enablement::is_enabled;

use iced::Task;

use super::*;

impl Signex {
    /// The Command Registry's dispatch entry point — resolve a stable
    /// [`crate::keymap::AppCommandId`] and run it exactly as if the
    /// resolved [`Message`] had been sent directly.
    ///
    /// Deliberately takes an id + args, never a [`Message`] — the whole
    /// point of the registry is that a caller (keyboard today; menu,
    /// command palette, and a future CLI later) names *what* to run
    /// without knowing the app's internal message shape.
    ///
    /// `args` is accepted but unused: [`core_to_message`] ignores it,
    /// because every catalog command is nullary today. It is here only
    /// so this signature is stable ahead of a consumer that needs it —
    /// do not thread it through `core_to_message` speculatively.
    ///
    /// An id can fail to resolve two ways, and both are a silent no-op,
    /// never a panic: it isn't in the catalog at all, or it's a real
    /// catalog id with no live arm in `core_to_message` yet. Ids come
    /// from user-editable TOML keymap profiles today and a CLI later —
    /// both are untrusted input, not a programmer error.
    ///
    /// `window` is the window the command was invoked from, if any. A canvas
    /// event goes to the canvas of the undocked window it was typed in; the
    /// plain [`Message::CanvasEvent`] always targets the main window (#525).
    ///
    /// A command [`is_enabled`] says has nothing to act on in that window is
    /// dropped here, the same rule the menu bar greys its row by (#368),
    /// unless its handler tells the user why it did nothing
    /// ([`enablement::explains_when_unavailable`]).
    pub(crate) fn dispatch_command(
        &mut self,
        command: &crate::keymap::AppCommandId,
        _args: CommandArgs,
        window: Option<iced::window::Id>,
    ) -> Task<Message> {
        // Undo / Redo typed in an undocked tab step that window's document
        // and are gated by its history, not the main window's active tab
        // (#1546).
        if let Some(window_id) = window
            && matches!(command.as_str(), "undo" | "redo")
            && self.undocked_tab_has_canvas(window)
        {
            return self.run_in_window(window_id, true, |app| {
                app.dispatch_command(command, _args, None)
            });
        }
        if !is_enabled(command, &self.command_context(window))
            && !enablement::explains_when_unavailable(command)
        {
            return Task::none();
        }
        // Undo / Redo follow the window the stroke came from: in the ERC
        // dialog they step the project's rule history and never reach the
        // sheet, whatever that history holds.
        let resolved = if self.erc_dialog_takes_input(window) {
            match command.as_str() {
                "undo" => Some(Message::Erc(ErcMsg::RulesUndone)),
                "redo" => Some(Message::Erc(ErcMsg::RulesRedone)),
                _ => core_to_message(command),
            }
        } else {
            core_to_message(command)
        };
        match (resolved, window) {
            (Some(Message::CanvasEvent(event)), Some(window_id))
                if self.interaction_state.canvases.contains_key(&window_id) =>
            {
                self.dispatch_update(Message::CanvasEventInWindow { window_id, event })
            }
            (Some(message), _) => self.dispatch_update(message),
            (None, _) => Task::none(),
        }
    }
}

#[cfg(test)]
mod arrange_tests;

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
    /// programmer error — it must resolve to a no-op, never panic.
    #[test]
    fn unknown_id_is_a_no_op_and_does_not_panic() {
        let (mut app, _task) = Signex::new();

        let command = AppCommandId::new("not_a_real_command").unwrap();
        let task = app.dispatch_command(&command, CommandArgs::none(), None);

        assert_eq!(task.units(), 0);
    }

    /// `break_wire` is a real catalog entry
    /// (`keymap/catalog`) with no arm in `core_to_message`
    /// yet — it must resolve to a no-op too, not panic.
    #[test]
    fn catalog_id_with_no_bridge_arm_is_a_no_op_and_does_not_panic() {
        let (mut app, _task) = Signex::new();

        let command = AppCommandId::new("break_wire").unwrap();
        assert!(
            crate::keymap::metadata_for(&command).is_some(),
            "test fixture drifted: break_wire is no longer a catalog id"
        );

        let task = app.dispatch_command(&command, CommandArgs::none(), None);

        assert_eq!(task.units(), 0);
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
                dirty: false,
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
        bridge::core_to_message(command).expect("the command resolves")
    }
}
