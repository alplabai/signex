//! The window-layout commands on the agent channel (#2249): Tile
//! Horizontally, Tile Vertically and Untile, open to agents by the
//! maintainer's rule that everything is AI-controllable. They name no
//! document, run the same handler as the menu row and answer with the layout
//! they left. See `docs/AGENT_SOCKET.md`, "Allowlist".

use iced::Task;
use serde_json::{Value, json};

use super::handler::Verdict;
use super::reply::outcome_verdict;
use crate::app::command::CommandOutcome;
use crate::app::command::enablement::disabled_reason;
use crate::app::{DocumentKey, Message, Signex, TileLayout};
use crate::keymap::AppCommandId;

impl Signex {
    /// Run the window-layout `command` as its menu row does, refused with
    /// the row's reason when it is greyed.
    pub(super) fn run_agent_window_layout(
        &mut self,
        command: &AppCommandId,
    ) -> (Verdict, Task<Message>) {
        // A greyed row's refusal is the command's own, not an access event.
        self.agent.reached_handler = true;
        let context = self.command_context(None);
        if let Some(reason) = disabled_reason(command, &context) {
            let outcome = CommandOutcome::Disabled {
                reason: reason.into(),
            };
            return (
                outcome_verdict(command.as_str(), outcome, None),
                Task::none(),
            );
        }
        let task = match command.as_str() {
            "tile_windows_horizontally" => self.tile(TileLayout::Stacked),
            "tile_windows_vertically" => self.tile(TileLayout::SideBySide),
            _ => self.untile_documents(),
        };
        self.agent.serving = true;
        let layout = self.window_layout_json();
        self.agent.serving = false;
        let outcome = CommandOutcome::Answered(layout);
        (outcome_verdict(command.as_str(), outcome, None), task)
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
