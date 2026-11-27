//! The window-layout commands on the agent channel (#2249, #2470): Tile
//! Horizontally, Tile Vertically, Untile, Fit (`zoom_to_fit`) and the dock's
//! `set_dock_panel`, open to agents by the maintainer's rule that everything
//! is AI-controllable. They name no document, run the same handler as the
//! menu row (the operand handler for the dock) and answer with what they
//! left. See `docs/AGENT_SOCKET.md`, "Allowlist".

use iced::Task;
use serde_json::{Value, json};

use crate::app::command::enablement::disabled_reason;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{DocumentKey, Message, Signex, TileLayout};
use crate::keymap::AppCommandId;

impl Signex {
    /// Run the window-layout `command` as its menu row does, refused with
    /// the row's reason when it is greyed. The agent runner of
    /// `run_command_request` calls it (#2495, "One request path").
    pub(crate) fn run_agent_window_layout(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        // A greyed row's refusal is the command's own, not an access event.
        self.agent.reached_handler = true;
        let context = self.command_context(None);
        if let Some(reason) = disabled_reason(command, &context) {
            let outcome = CommandOutcome::Disabled {
                reason: reason.into(),
            };
            return (outcome, Task::none());
        }
        let task = match command.as_str() {
            "set_dock_panel" => {
                return crate::app::handlers::dock::panel_command::set_dock_panel(self, args);
            }
            "zoom_to_fit" => self.update(Message::CanvasEvent(crate::canvas::CanvasEvent::FitAll)),
            "tile_windows_horizontally" => self.tile(TileLayout::Stacked),
            "tile_windows_vertically" => self.tile(TileLayout::SideBySide),
            _ => self.untile_documents(),
        };
        self.agent.serving = true;
        let answer = if command.as_str() == "zoom_to_fit" {
            self.view_json()
        } else {
            self.window_layout_json()
        };
        self.agent.serving = false;
        (CommandOutcome::Answered(answer), task)
    }

    /// What Fit framed: the main window's active document, by id and name
    /// when it belongs to an open project, as a tiled pane is named.
    fn view_json(&self) -> Value {
        let key = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(DocumentKey::of);
        let entries = self.document_entries();
        let entry = entries
            .iter()
            .find(|entry| key.is_some() && entry.key == key);
        json!({
            "fitted": {
                "document": entry.and_then(|e| e.id).map(|id| id.to_string()),
                "name": entry.map(|e| e.name.clone()),
            }
        })
    }

    /// The main window's layout: `tiled`, the `layout` (`stacked`,
    /// `side_by_side` or null) and the panes in order. A pane names its
    /// document by id and name when it belongs to an open project, as the
    /// queries do; another pane says only whether it is focused.
    fn window_layout_json(&self) -> Value {
        let Some((layout, panes)) = self.tiled_panes() else {
            return json!({ "tiled": false, "layout": null, "panes": [] });
        };
        let entries = self.document_entries();
        let active = self.document_state.active_tab;
        let panes: Vec<Value> = panes
            .iter()
            .map(|&index| {
                let key = self
                    .document_state
                    .tabs
                    .get(index)
                    .and_then(DocumentKey::of);
                let entry = entries
                    .iter()
                    .find(|entry| key.is_some() && entry.key == key);
                json!({
                    "focused": index == active,
                    "document": entry.and_then(|e| e.id).map(|id| id.to_string()),
                    "name": entry.map(|e| e.name.clone()),
                })
            })
            .collect();
        json!({
            "tiled": true,
            "layout": match layout {
                TileLayout::Stacked => "stacked",
                TileLayout::SideBySide => "side_by_side",
            },
            "panes": panes,
        })
    }
}
