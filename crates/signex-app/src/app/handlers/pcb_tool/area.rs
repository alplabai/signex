//! The Fill and Solid Region tools (#2373): solid copper on the active
//! copper layer, on the net of the copper under its first point, added as
//! one undo step. A Fill is the rectangle between two clicks. A Solid Region
//! takes a corner per click and is closed by a click on its first corner or
//! a right-click once it has three; Esc drops it.

use signex_engine::pcb::draw;
use signex_i18n::{ids, t};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::BoardToolSession;
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

/// The outline the next click at `at` would close: the Fill's rectangle, or
/// the Region's corners with `at`.
fn outline_to(session: &BoardToolSession, at: Point) -> Vec<Point> {
    match (session.tool, session.points.first()) {
        (BoardPlaceTool::Fill, Some(&corner)) => draw::rectangle(corner, at),
        _ => session.points.iter().copied().chain([at]).collect(),
    }
}

impl Signex {
    /// A Fill or Solid Region click at `at`.
    pub(super) fn area_clicked(&mut self, tool: BoardPlaceTool, at: Point) {
        if self.tool_copper_layer(tool).is_none() {
            return;
        }
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let points = &session.points;
        let closes = match tool {
            BoardPlaceTool::Fill => !points.is_empty(),
            _ => points.len() >= 3 && points.first() == Some(&at),
        };
        if closes {
            let outline = match tool {
                BoardPlaceTool::Fill => outline_to(session, at),
                _ => points.clone(),
            };
            self.add_copper_area(tool, outline);
            return;
        }
        if points.last() != Some(&at)
            && let Some(session) = self.own_board_tool_mut()
        {
            session.points.push(at);
        }
    }

    /// A right-click while a Solid Region has three corners: close it.
    /// Whether it did.
    pub(crate) fn close_board_region(&mut self) -> bool {
        let Some(session) = self.own_board_tool() else {
            return false;
        };
        if session.tool != BoardPlaceTool::SolidRegion || session.points.len() < 3 {
            return false;
        }
        let outline = session.points.clone();
        self.add_copper_area(BoardPlaceTool::SolidRegion, outline);
        true
    }

    /// Add solid copper over `outline` on the active copper layer and start
    /// the next shape.
    fn add_copper_area(&mut self, tool: BoardPlaceTool, outline: Vec<Point>) {
        if let Some(session) = self.own_board_tool_mut() {
            session.points.clear();
        }
        let (Some(board), Some(layer)) = (self.active_pcb(), self.active_copper_layer()) else {
            return;
        };
        let what = if tool == BoardPlaceTool::Fill {
            "Fill"
        } else {
            "Solid Region"
        };
        match draw::new_copper_area(board, Uuid::now_v7(), outline, layer) {
            Some(zone) => {
                self.run_placement(what, Ok(vec![draw::place_zone(zone)]));
            }
            None => {
                let name = t!(self.ui_state.i18n, tool.name()).into_string();
                let note = t!(self.ui_state.i18n, ids::PCB_TOOL_NO_AREA, "tool" => name.as_str());
                crate::diagnostics::log_warning(note.into_string());
            }
        }
        self.refresh_pcb_highlight();
    }

    /// The area the next click closes, drawn into `preview`.
    pub(super) fn area_preview(
        &self,
        session: &BoardToolSession,
        board: &PcbBoard,
        cursor: Point,
        preview: &mut PcbBoard,
    ) {
        let Some(layer) = self.active_copper_layer() else {
            return;
        };
        if session.points.is_empty() {
            return;
        }
        let outline = outline_to(session, cursor);
        preview
            .zones
            .extend(draw::new_copper_area(board, Uuid::nil(), outline, layer));
    }
}
