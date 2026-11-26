//! The window-layout commands on the agent channel (#2249): Tile
//! Horizontally, Tile Vertically and Untile, and the panes of mockup row 207
//! (Tile Grid, Split Right, Split Down, Open Second View, Move Tab to Other
//! Pane, Close Pane, Sync Zoom with Other Pane, Equal Pane Sizes), open to
//! agents by the maintainer's rule that everything is AI-controllable. They
//! name no document, run the same handler as the menu row and answer with
//! the layout they left. See `docs/AGENT_SOCKET.md`, "Allowlist".

use iced::Task;
use serde_json::{Value, json};

use super::handler::Verdict;
use super::reply::outcome_verdict;
use crate::app::command::CommandOutcome;
use crate::app::command::enablement::disabled_reason;
use crate::app::handlers::document_panes::refusal_line;
use crate::app::{DocumentKey, Message, Signex, TileLayout};
use crate::document_panes::{DropEdge, SplitRefusal};
use crate::keymap::AppCommandId;

/// The split commands: where the new pane goes and whether it is a second
/// view.
fn pane_split(command: &str) -> Option<(DropEdge, bool)> {
    match command {
        "split_pane_right" => Some((DropEdge::Right, false)),
        "split_pane_down" => Some((DropEdge::Bottom, false)),
        "open_second_view" => Some((DropEdge::Right, true)),
        _ => None,
    }
}

/// The token a reply names `layout` by.
fn layout_key(layout: TileLayout) -> &'static str {
    match layout {
        TileLayout::Stacked => "stacked",
        TileLayout::SideBySide => "side_by_side",
        TileLayout::Grid => "grid",
    }
}

impl Signex {
    /// Run the window-layout `command` as its menu row does, refused with
    /// the row's reason when it is greyed, or with the pane's reason when a
    /// pane command cannot act.
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
        let task = match self.run_window_layout_command(command.as_str()) {
            Ok(task) => task,
            Err(why) => {
                let outcome = CommandOutcome::Disabled {
                    reason: refusal_line(why).into(),
                };
                return (
                    outcome_verdict(command.as_str(), outcome, None),
                    Task::none(),
                );
            }
        };
        self.agent.serving = true;
        let layout = self.window_layout_json();
        self.agent.serving = false;
        let outcome = CommandOutcome::Answered(layout);
        (outcome_verdict(command.as_str(), outcome, None), task)
    }

    /// The handler of window-layout `command`, as its menu row runs it.
    fn run_window_layout_command(&mut self, command: &str) -> Result<Task<Message>, SplitRefusal> {
        if let Some((edge, second_view)) = pane_split(command) {
            let tab = self.document_state.active_tab;
            return self.split_and_keep(tab, edge, second_view);
        }
        match command {
            "tile_windows_horizontally" => Ok(self.tile(TileLayout::Stacked)),
            "tile_windows_vertically" => Ok(self.tile(TileLayout::SideBySide)),
            "tile_windows_grid" => Ok(self.tile(TileLayout::Grid)),
            "move_tab_to_other_pane" => self.move_tab_to_other_pane(),
            "close_pane" => self.close_focused_pane(),
            "sync_pane_zoom" => self.sync_pane_zoom(),
            "equalize_panes" => self.equalize_panes(),
            _ => Ok(self.untile_documents()),
        }
    }

    /// The main window's layout: `tiled`, the `layout` (`stacked`,
    /// `side_by_side`, `grid` or null), while tiled the pane sizes after a
    /// splitter drag (`shares`, null when equal), and the panes in order. A pane names its
    /// document by id and name when it belongs to an open project, as the
    /// queries do; another pane says only whether it is focused. Each pane
    /// lists the names of its tabs and its zoom in percent (null for a pane
    /// that shows no sheet or board). A second view names its document in
    /// both panes; one of them is focused.
    fn window_layout_json(&self) -> Value {
        let Some(view) = self.tiled_pane_view() else {
            return json!({ "tiled": false, "layout": null, "panes": [] });
        };
        let entries = self.document_entries();
        let name_of = |index: usize| {
            let key = self
                .document_state
                .tabs
                .get(index)
                .and_then(DocumentKey::of);
            entries
                .iter()
                .find(|entry| key.is_some() && entry.key == key)
        };
        let lists = self.pane_tab_lists();
        let panes: Vec<Value> = view
            .tabs
            .iter()
            .enumerate()
            .map(|(n, &index)| {
                let entry = name_of(index);
                let tabs: Vec<Value> = lists
                    .get(n)
                    .map(|tabs| {
                        tabs.iter()
                            .map(|&tab| json!(name_of(tab).map(|e| e.name.clone())))
                            .collect()
                    })
                    .unwrap_or_default();
                json!({
                    "focused": n == view.focused,
                    "document": entry.and_then(|e| e.id).map(|id| id.to_string()),
                    "name": entry.map(|e| e.name.clone()),
                    "tabs": tabs,
                    "zoom": self.pane_zoom_percent(&view, n),
                })
            })
            .collect();
        let shares = self.pane_shares_for(&view);
        json!({
            "tiled": true,
            "layout": layout_key(view.layout),
            "shares": (!shares.is_empty()).then(|| shares.to_vec()),
            "panes": panes,
        })
    }
}
