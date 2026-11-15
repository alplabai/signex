//! The area tools (#2373), each added as one undo step:
//! - Fill and Solid Region: solid copper on the active copper layer, on the
//!   net of the copper under its first point;
//! - Keepout: an area on the active copper or keep-out layer that keeps all
//!   copper off that copper layer (the design rule check reads it).
//!
//! A Fill is the rectangle between two clicks. A Solid Region or Keepout
//! takes a corner per click and is closed by a click on its first corner or
//! a right-click once it has three; Esc drops it.

use signex_engine::pcb::draw;
use signex_i18n::{ids, t};
use signex_types::pcb::{PcbBoard, Point, Zone};
use signex_types::pcb_layer::PcbLayer;
use signex_types::zone_keepout::keepout_copper;
use uuid::Uuid;

use super::BoardToolSession;
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

/// The outline the next click at `at` would close: the Fill's rectangle, or
/// the polygon's corners with `at`.
fn outline_to(session: &BoardToolSession, at: Point) -> Vec<Point> {
    match (session.tool, session.points.first()) {
        (BoardPlaceTool::Fill, Some(&corner)) => draw::rectangle(corner, at),
        _ => session.points.iter().copied().chain([at]).collect(),
    }
}

/// The area `tool` makes of `outline` on the active layer `active`; `None`
/// for a layer the tool cannot draw on or an outline with no area.
fn area_zone(
    tool: BoardPlaceTool,
    board: &PcbBoard,
    id: Uuid,
    outline: Vec<Point>,
    active: PcbLayer,
) -> Option<Zone> {
    match tool {
        BoardPlaceTool::Keepout => draw::new_keepout(id, outline, active),
        _ => draw::new_copper_area(board, id, outline, active.copper()?),
    }
}

impl Signex {
    /// The active layer of the board in view when `tool` can draw on it,
    /// else `None` after saying so in the Messages panel.
    fn area_layer(&self, tool: BoardPlaceTool) -> Option<PcbLayer> {
        if tool != BoardPlaceTool::Keepout {
            return self.tool_copper_layer(tool).map(PcbLayer::Copper);
        }
        let active = self
            .active_tab_path()
            .map(|p| self.pcb_layer_view(&p).active());
        let layer = active.filter(|layer| keepout_copper(*layer).is_some());
        if layer.is_none() {
            let note = t!(self.ui_state.i18n, ids::PCB_TOOL_KEEPOUT_NEEDS_LAYER);
            crate::diagnostics::log_warning(note.into_string());
        }
        layer
    }

    /// A Fill, Solid Region or Keepout click at `at`.
    pub(super) fn area_clicked(&mut self, tool: BoardPlaceTool, at: Point) {
        if self.area_layer(tool).is_none() {
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
            self.add_area(tool, outline);
            return;
        }
        if points.last() != Some(&at)
            && let Some(session) = self.own_board_tool_mut()
        {
            session.points.push(at);
        }
    }

    /// A right-click while a Solid Region or Keepout has three corners:
    /// close it. Whether it did.
    pub(crate) fn close_board_region(&mut self) -> bool {
        let Some(session) = self.own_board_tool() else {
            return false;
        };
        let polygon = matches!(
            session.tool,
            BoardPlaceTool::SolidRegion | BoardPlaceTool::Keepout
        );
        if !polygon || session.points.len() < 3 {
            return false;
        }
        let (tool, outline) = (session.tool, session.points.clone());
        self.add_area(tool, outline);
        true
    }

    /// Add `tool`'s area over `outline` on the active layer and start the
    /// next shape.
    fn add_area(&mut self, tool: BoardPlaceTool, outline: Vec<Point>) {
        if let Some(session) = self.own_board_tool_mut() {
            session.points.clear();
        }
        let active = self.area_layer(tool);
        let (Some(board), Some(active)) = (self.active_pcb(), active) else {
            return;
        };
        let name = t!(self.ui_state.i18n, tool.name()).into_string();
        match area_zone(tool, board, Uuid::now_v7(), outline, active) {
            Some(zone) => {
                self.run_placement(&name, Ok(vec![draw::place_zone(zone)]));
            }
            None => {
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
        let Some(path) = self.active_tab_path() else {
            return;
        };
        if session.points.is_empty() {
            return;
        }
        let active = self.pcb_layer_view(&path).active();
        let outline = outline_to(session, cursor);
        preview
            .zones
            .extend(area_zone(session.tool, board, Uuid::nil(), outline, active));
    }
}
