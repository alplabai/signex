//! The area tools (#2373), each added as one undo step:
//! - Fill and Solid Region: solid copper on the active copper layer, on the
//!   net of the copper under its first point;
//! - Polygon Pour: an area on the active copper layer, on the net of the
//!   copper under its first point, whose copper is computed clear of other
//!   nets (#2216);
//! - Keepout: an area on the active copper or keep-out layer that keeps all
//!   copper off that copper layer (the design rule check reads it).
//!
//! A Fill is the rectangle between two clicks. A Solid Region, Polygon Pour or Keepout
//! takes a corner per click and is closed by a click on its first corner or
//! a right-click once it has three; Esc drops it. The zone is built by
//! `signex_engine::pcb::area`, as the operand form builds it (#2470).

use signex_engine::pcb::area::new_area;
use signex_engine::pcb::draw;
use signex_engine::pcb::placement::grid_step_mm;
use signex_i18n::{ids, t};
use signex_types::pcb::{PcbBoard, Point, Zone};
use signex_types::pcb_layer::PcbLayer;
use signex_types::zone_keepout::keepout_copper;
use uuid::Uuid;

use super::outline_preview::open_outline;
use super::{BoardToolDefaults, BoardToolSession};
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::panels::pcb_primitive_properties::unfilled_reason;

/// Whether `b` lies within `reach` mm of `a`.
fn near(a: Point, b: Point, reach: f64) -> bool {
    (a.x - b.x).hypot(a.y - b.y) <= reach
}

/// The outline the next click at `at` would close: the Fill's rectangle, or
/// the polygon's corners with `at`.
fn outline_to(session: &BoardToolSession, at: Point) -> Vec<Point> {
    match (session.tool, session.points.first()) {
        (BoardPlaceTool::Fill, Some(&corner)) => draw::rectangle(corner, at),
        _ => session.points.iter().copied().chain([at]).collect(),
    }
}

/// The area `tool` makes of `outline` on the active layer `active`, with
/// the values set for the tool (#2393); `None` for a layer the tool cannot
/// draw on or an outline with no area.
fn area_zone(
    tool: BoardPlaceTool,
    board: &PcbBoard,
    defaults: &BoardToolDefaults,
    id: Uuid,
    outline: Vec<Point>,
    active: PcbLayer,
) -> Option<Zone> {
    let zone = new_area(board, tool.area_kind()?, id, outline, active)?;
    Some(defaults.area(board, zone))
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
        // A click within half a grid step of the first corner closes the
        // shape, though an object snap moved it off that corner.
        let reach = self
            .active_pcb()
            .map_or(0.0, |board| grid_step_mm(board) / 2.0);
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let points = &session.points;
        let first = points.first().copied();
        let closes = match tool {
            BoardPlaceTool::Fill => !points.is_empty(),
            _ => points.len() >= 3 && first.is_some_and(|first| near(first, at, reach)),
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

    /// A right-click while a Solid Region, Polygon Pour, Keepout or Pour
    /// Cutout has three corners: close it. Whether it did.
    pub(crate) fn close_board_region(&mut self) -> bool {
        let Some(session) = self.own_board_tool() else {
            return false;
        };
        let polygon = matches!(
            session.tool,
            BoardPlaceTool::SolidRegion
                | BoardPlaceTool::PolygonPour
                | BoardPlaceTool::Keepout
                | BoardPlaceTool::PourCutout
        );
        if !polygon || session.points.len() < 3 {
            return false;
        }
        let (tool, outline) = (session.tool, session.points.clone());
        if tool == BoardPlaceTool::PourCutout {
            self.place_pour_cutout(outline);
        } else {
            self.add_area(tool, outline);
        }
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
        let defaults = &self.interaction_state.pcb_tool_defaults;
        match area_zone(tool, board, defaults, Uuid::now_v7(), outline, active) {
            Some(zone) => {
                let (id, pour, netless) = (zone.uuid, zone.is_pour(), zone.net == 0);
                self.run_placement(&name, Ok(vec![draw::place_zone(zone)]));
                if pour {
                    self.say_new_pour(id, &name, netless);
                }
            }
            None => {
                let note = t!(self.ui_state.i18n, ids::PCB_TOOL_NO_AREA, "tool" => name.as_str());
                crate::diagnostics::log_warning(note.into_string());
            }
        }
        self.refresh_pcb_highlight();
    }

    /// A pour just placed: say in the status bar and Messages when it has
    /// no net, or why it has no fill.
    fn say_new_pour(&mut self, id: Uuid, name: &str, netless: bool) {
        let unfilled = self.active_pcb().and_then(|board| {
            let index = board.zones.iter().position(|zone| zone.uuid == id)?;
            pcb_geom::zone_fills(board).unfilled(index)
        });
        let i18n = &self.ui_state.i18n;
        let line = match unfilled {
            Some(why) => {
                let why = t!(i18n, unfilled_reason(why)).into_string();
                t!(i18n, ids::PCB_POUR_NOT_FILLED, "pour" => name, "why" => why)
            }
            None if netless => t!(i18n, ids::PCB_POUR_NO_NET, "tool" => name),
            None => return,
        };
        self.note_board_tool(line.into_string());
    }

    /// The area the next click closes, drawn into `preview`; while the
    /// corners enclose no area yet, the open outline to the pointer
    /// (`outline_preview`).
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
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let zone = area_zone(
            session.tool,
            board,
            defaults,
            Uuid::nil(),
            outline.clone(),
            active,
        );
        let drawable = match session.tool {
            BoardPlaceTool::Keepout => keepout_copper(active).is_some(),
            _ => active.is_copper(),
        };
        match zone {
            Some(zone) => preview.zones.push(zone),
            None if drawable => preview.segments.extend(open_outline(&outline, active)),
            None => {}
        }
    }

    /// The area the tool armed on the board in view places next, for its
    /// Properties form: its settings over a stand-in outline at the pointer.
    /// The layer is the active one, or the tool's nearest layer while the
    /// active one cannot take it.
    pub(crate) fn next_board_area(&self, board: &PcbBoard) -> Option<Zone> {
        let session = self
            .own_board_tool()
            .filter(|s| super::form::is_area(s.tool))?;
        let at = session.cursor.unwrap_or(Point::new(0.0, 0.0));
        let outline = draw::rectangle(at, Point::new(at.x + 1.0, at.y + 1.0));
        let active = self
            .active_tab_path()
            .map(|p| self.pcb_layer_view(&p).active())
            .filter(|layer| match session.tool {
                BoardPlaceTool::Keepout => keepout_copper(*layer).is_some(),
                _ => layer.is_copper(),
            })
            .unwrap_or(PcbLayer::Copper(self.pcb_active_layer()));
        let defaults = &self.interaction_state.pcb_tool_defaults;
        area_zone(session.tool, board, defaults, Uuid::nil(), outline, active)
    }
}
