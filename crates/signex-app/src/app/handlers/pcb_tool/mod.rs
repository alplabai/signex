//! The board placement tools (#2373): a tool picked from Place, the PCB
//! Active Bar, a key or the palette is armed on the board in view of the
//! window it was picked in. The pointer's point lands on an object point or
//! the grid; each click (or finished shape) is one board undo step through
//! `run_placement`, built by `signex_engine::pcb::draw`. What the next click
//! adds follows the pointer, drawn by the board renderer in its layer's
//! colour and dimmed. Esc or a right-click ends the shape being drawn, and
//! then the tool; a tab switch leaves it behind. Properties shows what the
//! next click places, and Tab pauses placing to edit it (`form`, #2393).

use std::path::PathBuf;

use signex_engine::pcb::{BoardCommand, BoardItem, draw};
use signex_i18n::{ids, t};
use signex_renderer::pcb::PcbSnapshot;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::pcb_canvas::{carried_board, snap_point};

mod area;
mod form;
mod route;
mod string;

pub(crate) use form::has_form;
pub use form::{BoardToolDefaults, PlacementPause};

#[cfg(test)]
mod area_tests;
#[cfg(test)]
mod form_tests;
#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod string_tests;
#[cfg(test)]
mod tests;

/// A placement tool armed on one board in one window, and the shape it is
/// drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardToolSession {
    pub tool: BoardPlaceTool,
    /// The board the tool is armed on; another tab leaves it behind.
    pub path: PathBuf,
    /// The undocked window it was armed in (`None`: the main window).
    pub window: Option<iced::window::Id>,
    /// The points placed so far of the shape being drawn.
    pub points: Vec<Point>,
    /// Where the next click lands; `None` until the pointer is over the
    /// board.
    pub cursor: Option<Point>,
    /// The layer, width and net of the track run being drawn.
    pub route: Option<draw::TrackStyle>,
    /// Tab paused placing for the values to be edited in Properties (#2393);
    /// what Esc puts back.
    pub paused: Option<PlacementPause>,
}

impl BoardToolSession {
    /// Whether the tool belongs to the board at `path` shown in `window`.
    pub fn is_for(&self, path: Option<&std::path::Path>, window: Option<iced::window::Id>) -> bool {
        path == Some(self.path.as_path()) && window == self.window
    }

    /// Whether a track run is being drawn: Space and Shift+Space bend it.
    pub fn is_routing(&self) -> bool {
        self.tool == BoardPlaceTool::Track && !self.points.is_empty()
    }
}

impl Signex {
    /// Arm `tool` on the board in view, ending a Move, paste or part on the
    /// cursor and a net pick. Nothing without a board in view or for a tool
    /// with no command yet.
    pub(crate) fn arm_board_tool(&mut self, tool: BoardPlaceTool) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return;
        };
        if tool.command().is_none() {
            return;
        }
        self.end_pcb_carry();
        self.interaction_state.pcb_primitive.net_pick = None;
        // A guide armed by the board panel's Place would take the first
        // click ahead of the tool (#2396).
        self.interaction_state.pcb_guide_place = None;
        self.interaction_state.pcb_tool = Some(BoardToolSession {
            tool,
            path,
            window: self.interaction_state.acting_window,
            points: Vec::new(),
            cursor: None,
            route: None,
            paused: None,
        });
        crate::diagnostics::log_info(t!(self.ui_state.i18n, tool.texts().1).into_string());
        self.refresh_pcb_highlight();
    }

    /// The tool armed on the board this window shows.
    pub(crate) fn own_board_tool(&self) -> Option<&BoardToolSession> {
        let path = self.active_tab_path();
        let window = self.interaction_state.acting_window;
        self.interaction_state
            .pcb_tool
            .as_ref()
            .filter(|session| session.is_for(path.as_deref(), window))
    }

    fn own_board_tool_mut(&mut self) -> Option<&mut BoardToolSession> {
        let path = self.active_tab_path();
        let window = self.interaction_state.acting_window;
        self.interaction_state
            .pcb_tool
            .as_mut()
            .filter(|session| session.is_for(path.as_deref(), window))
    }

    /// End the tool armed on the board this window shows.
    pub(crate) fn end_board_tool(&mut self) {
        if self.own_board_tool().is_some() {
            self.interaction_state.pcb_tool = None;
            self.refresh_pcb_highlight();
        }
    }

    /// Esc or a right-click: undo the edits of a pause and go on placing,
    /// else drop the shape being drawn, or end the tool when none is.
    /// Whether a tool took it.
    pub(crate) fn step_back_board_tool(&mut self) -> bool {
        if self.revert_board_tool_pause() {
            return true;
        }
        let Some(session) = self.own_board_tool_mut() else {
            return false;
        };
        if session.points.is_empty() {
            self.end_board_tool();
        } else {
            session.points.clear();
            session.route = None;
            self.refresh_pcb_highlight();
        }
        true
    }

    /// The active layer of the board in view, when it is copper: where a
    /// track or a copper area is drawn.
    fn active_copper_layer(&self) -> Option<CopperLayer> {
        let path = self.active_tab_path()?;
        self.pcb_layer_view(&path).active().copper()
    }

    /// [`Self::active_copper_layer`], or `None` after saying in the Messages
    /// panel that `tool` draws on copper only.
    fn tool_copper_layer(&self, tool: BoardPlaceTool) -> Option<CopperLayer> {
        let layer = self.active_copper_layer();
        if layer.is_none() {
            let i18n = &self.ui_state.i18n;
            let name = t!(i18n, tool.name()).into_string();
            let note = t!(i18n, ids::PCB_TOOL_NEEDS_COPPER, "tool" => name.as_str());
            crate::diagnostics::log_warning(note.into_string());
        }
        layer
    }

    /// Where a tool click at `at` lands on the board in view.
    fn board_tool_point(&self, board: &PcbBoard, at: Point) -> Point {
        snap_point(board, at, &self.ui_state.pcb_snap, self.pcb_active_layer())
    }

    /// The pointer moved over the board: what the next click adds follows.
    pub(crate) fn board_tool_followed(&mut self, at: Point) {
        let Some(board) = self.own_board_tool().and(self.active_pcb()) else {
            return;
        };
        let at = self.board_tool_point(board, at);
        if let Some(session) = self.own_board_tool_mut()
            && session.cursor != Some(at)
        {
            session.cursor = Some(at);
            self.refresh_pcb_highlight();
        }
    }

    /// A click on the board while a tool is armed: add what it places.
    /// Whether the click belonged to the tool.
    pub(crate) fn board_tool_clicked(&mut self, at: Point) -> bool {
        let Some(tool) = self.own_board_tool().map(|s| s.tool) else {
            return false;
        };
        if self.is_board_tool_paused() {
            // A paused tool places nothing; the net pick of its form still
            // takes the click.
            return self.interaction_state.pcb_primitive.net_pick.is_none();
        }
        let Some(board) = self.active_pcb() else {
            return false;
        };
        let at = self.board_tool_point(board, at);
        match tool {
            BoardPlaceTool::Via => {
                let defaults = &self.interaction_state.pcb_tool_defaults;
                let via = defaults.via(board, Uuid::now_v7(), at);
                let command = BoardCommand::AddItems {
                    items: vec![BoardItem::Via(via)],
                };
                self.run_placement("Via", Ok(vec![command]));
            }
            BoardPlaceTool::Track => self.route_clicked(at),
            BoardPlaceTool::TextString => self.string_clicked(at),
            BoardPlaceTool::Fill
            | BoardPlaceTool::SolidRegion
            | BoardPlaceTool::PolygonPour
            | BoardPlaceTool::Keepout => {
                self.area_clicked(tool, at);
            }
            _ => return false,
        }
        if let Some(session) = self.own_board_tool_mut() {
            session.cursor = Some(at);
        }
        self.refresh_pcb_highlight();
        true
    }

    /// What the canvas draws on the cursor for the tool armed on the board
    /// in view: the board holding only what the next click adds.
    pub(crate) fn board_tool_ghost(&self) -> Option<PcbSnapshot> {
        let session = self.own_board_tool()?;
        let preview = self.board_tool_preview()?;
        Some(self.pcb_snapshot_for(&session.path, &preview))
    }

    /// The board holding only what the next click of the tool armed on the
    /// board in view adds; `None` without a cursor over the board.
    fn board_tool_preview(&self) -> Option<PcbBoard> {
        let session = self.own_board_tool()?;
        let board = self.document_state.pcb_board(&session.path)?;
        let cursor = session.cursor?;
        let mut preview = carried_board(board, &Default::default(), &[], Point::new(0.0, 0.0));
        match session.tool {
            BoardPlaceTool::Via => {
                let defaults = &self.interaction_state.pcb_tool_defaults;
                preview.vias.push(defaults.via(board, Uuid::nil(), cursor));
            }
            BoardPlaceTool::Track => self.route_preview(session, board, cursor, &mut preview),
            BoardPlaceTool::TextString => self.string_preview(cursor, &mut preview),
            BoardPlaceTool::Fill
            | BoardPlaceTool::SolidRegion
            | BoardPlaceTool::PolygonPour
            | BoardPlaceTool::Keepout => {
                self.area_preview(session, board, cursor, &mut preview);
            }
            _ => return None,
        }
        Some(preview)
    }
}
