//! The board placement tools (#2373): a tool picked from Place, the PCB
//! Active Bar, a key or the palette is armed on the board in view of the
//! window it was picked in. The pointer's point lands on an object point or
//! the grid; each click (or finished shape) is one board undo step through
//! `run_placement`, built by `signex_engine::pcb::draw`. What the next click
//! adds follows the pointer, drawn by the board renderer in its layer's
//! colour and dimmed. Esc or a right-click ends the shape being drawn, and
//! then the tool; a tab switch leaves it behind. Properties shows what the
//! next click places, and Tab pauses placing to edit it (`form`, #2393).
//! The Graphics tools draw on every layer but the keep-outs (`graphic`,
//! #2424, #2883).

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

/// With Snap off, how near the run's last point (screen px) a click counts
/// as on it and ends the run.
const END_RUN_PX: f64 = 2.0;

mod area;
mod form;
mod free;
mod free_form;
mod graphic;
mod graphic_copper;
mod outline_preview;
mod pour_cut;
mod quick;
mod route;
mod route_end;
mod route_rats;
mod router;
mod side_by_side;
mod string;

pub(crate) use form::has_form;
pub use form::{BoardToolDefaults, PlacementPause};
pub(crate) use free_form::precision_of;
pub use router::RouterState;

#[cfg(test)]
mod area_form_tests;
#[cfg(test)]
mod area_tests;
#[cfg(test)]
mod dimension_defaults_tests;
#[cfg(test)]
mod form_tests;
#[cfg(test)]
mod free_tests;
#[cfg(test)]
mod graphic_copper_tests;
#[cfg(test)]
mod graphic_tests;
#[cfg(test)]
mod layer_tests;
#[cfg(test)]
mod outline_preview_tests;
#[cfg(test)]
mod pause_tests;
#[cfg(test)]
mod pour_cut_tests;
#[cfg(test)]
mod pour_tests;
#[cfg(test)]
mod route_rats_tests;
#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod router_tests;
#[cfg(test)]
mod side_by_side_tests;
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
    /// The route of a routing tool not yet on the board, and its rules.
    pub router: Option<RouterState>,
    /// Tab paused placing for the values to be edited in Properties (#2393);
    /// what Esc puts back.
    pub paused: Option<PlacementPause>,
    /// Why the last click added nothing, for the status bar; the next click
    /// clears it.
    pub note: Option<String>,
}

impl BoardToolSession {
    /// Whether the tool belongs to the board at `path` shown in `window`.
    pub fn is_for(&self, path: Option<&std::path::Path>, window: Option<iced::window::Id>) -> bool {
        path == Some(self.path.as_path()) && window == self.window
    }

    /// Whether a track run or a route is being drawn: Space and
    /// Shift+Space bend it.
    pub fn is_routing(&self) -> bool {
        matches!(
            self.tool,
            BoardPlaceTool::Track
                | BoardPlaceTool::InteractiveRouting
                | BoardPlaceTool::DiffPairRouting
                | BoardPlaceTool::QuickRoute
                | BoardPlaceTool::MultiRoute
        ) && !self.points.is_empty()
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
        // A route drawn on any board, this one or another window's, lands
        // (or is discarded, said in Messages) before the tool is replaced.
        self.finish_board_route(true);
        self.interaction_state.pcb_primitive.net_pick = None;
        // A guide armed by the board panel's Place would take the first
        // click ahead of the tool (#2396).
        self.interaction_state.pcb_guide_place = None;
        // A tool takes the clicks Measure would (#2403).
        self.interaction_state.pcb_query.measure = None;
        self.interaction_state.pcb_arrange_box = None;
        self.interaction_state.pcb_tool = Some(BoardToolSession {
            tool,
            path,
            window: self.interaction_state.acting_window,
            points: Vec::new(),
            cursor: None,
            route: None,
            router: None,
            paused: None,
            note: None,
        });
        crate::diagnostics::log_info(self.board_tool_hint(tool));
        self.sync_route_ratsnest();
        self.refresh_pcb_highlight();
    }

    /// What the Messages panel says when `tool` is armed; the Arc hint
    /// names how its clicks make an arc.
    fn board_tool_hint(&self, tool: BoardPlaceTool) -> String {
        let i18n = &self.ui_state.i18n;
        if tool == BoardPlaceTool::Arc {
            let mode = self.interaction_state.pcb_tool_defaults.arc_mode;
            let how = t!(i18n, free::arc_mode_hint(mode)).into_string();
            return t!(i18n, tool.texts().1, "mode" => how).into_string();
        }
        t!(i18n, tool.texts().1).into_string()
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
            self.release_board_tool();
            self.refresh_pcb_highlight();
        }
    }

    /// End whatever tool is armed, on any board, for a Move, paste or part
    /// on the cursor: a route being drawn lands first, or is discarded with
    /// a Messages line when it no longer fits the board.
    pub(crate) fn release_board_tool(&mut self) {
        self.finish_board_route(true);
        self.interaction_state.pcb_tool = None;
        self.sync_route_ratsnest();
    }

    /// Esc or a right-click: undo the edits of a pause and go on placing,
    /// else drop the shape being drawn (a route lands on the board as one
    /// step, or stays drawn once when it no longer fits), or end the tool
    /// when none is. Whether a tool took it.
    pub(crate) fn step_back_board_tool(&mut self) -> bool {
        if self.revert_board_tool_pause() {
            return true;
        }
        let Some(session) = self.own_board_tool() else {
            return false;
        };
        if session.points.is_empty() {
            self.end_board_tool();
        } else {
            self.end_route_shape();
        }
        self.sync_route_ratsnest();
        true
    }

    /// Forget the shape or route being drawn; the tool stays armed.
    fn drop_board_shape(&mut self) {
        if let Some(session) = self.own_board_tool_mut() {
            session.points.clear();
            session.route = None;
            session.router = None;
        }
        self.sync_route_ratsnest();
        self.refresh_pcb_highlight();
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

    /// Say why a click added nothing: in the Messages panel, and in the
    /// status bar until the next click.
    pub(super) fn note_board_tool(&mut self, line: String) {
        crate::diagnostics::log_warning(&line);
        if let Some(session) = self.own_board_tool_mut() {
            session.note = Some(line);
        }
    }

    /// What the status bar says while a tool is armed on the board this
    /// window shows: paused, why the leg on the pointer or the last click
    /// adds nothing, that the tool needs a copper layer, or how to use it.
    pub(crate) fn board_tool_status(&self) -> Option<String> {
        let session = self.own_board_tool()?;
        let i18n = &self.ui_state.i18n;
        let name = t!(i18n, session.tool.name()).into_string();
        if session.paused.is_some() {
            return Some(t!(i18n, ids::PCB_TOOL_PAUSED, "tool" => name).into_string());
        }
        if let Some(why) = self.router_preview_refusal() {
            return Some(why.to_string());
        }
        if let Some(note) = &session.note {
            return Some(note.clone());
        }
        if draws_on_copper(session.tool) && self.active_copper_layer().is_none() {
            let note = t!(i18n, ids::PCB_TOOL_NEEDS_COPPER, "tool" => name.as_str());
            return Some(note.into_string());
        }
        // Mid-route the corner rule is said instead (`corner_status`).
        (!session.is_routing()).then(|| self.board_tool_hint(session.tool))
    }

    /// Whether a click at `at` is on the run's last point `anchor`, which
    /// ends the run. With Snap off a hand's click never repeats a point
    /// exactly, so [`END_RUN_PX`] of the screen counts as on it.
    fn clicks_run_end(&self, at: Point, anchor: Point) -> bool {
        if at == anchor {
            return true;
        }
        !self.ui_state.snap_enabled
            && (at.x - anchor.x).hypot(at.y - anchor.y) * self.pcb_snap_view().px_per_mm
                <= END_RUN_PX
    }

    /// Where a tool click at `at` lands on the board in view. A running
    /// pair or multi-track route places its centre line, which runs between
    /// its tracks: an object point would put one member on that object, so
    /// only the grid, guides and axes hold it. A running single route with
    /// a net snaps to that net only; one with no net snaps to any pad and
    /// takes its net (Caner, 2026-11-21). Its first click still finds its
    /// net from whatever is there.
    fn board_tool_point(&self, board: &PcbBoard, at: Point) -> Point {
        let mut view = self.pcb_snap_view();
        if let Some(session) = self
            .own_board_tool()
            .filter(|session| !session.points.is_empty())
        {
            match session.tool {
                BoardPlaceTool::DiffPairRouting | BoardPlaceTool::MultiRoute => {
                    view = view.without_objects();
                }
                BoardPlaceTool::Track
                | BoardPlaceTool::InteractiveRouting
                | BoardPlaceTool::QuickRoute => {
                    view.net = session
                        .route
                        .as_ref()
                        .map(|style| style.net)
                        .filter(|net| *net != 0);
                }
                _ => {}
            }
        }
        snap_point(board, at, view)
    }

    /// The pointer moved over the board: what the next click adds follows,
    /// unless Tab paused the tool, which holds it where it was.
    pub(crate) fn board_tool_followed(&mut self, at: Point) {
        let Some(board) = self
            .own_board_tool()
            .filter(|s| s.paused.is_none())
            .and(self.active_pcb())
        else {
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
        if let Some(session) = self.own_board_tool_mut() {
            session.note = None;
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
            BoardPlaceTool::InteractiveRouting => self.router_clicked(at),
            BoardPlaceTool::QuickRoute => self.quick_route_clicked(at),
            BoardPlaceTool::DiffPairRouting | BoardPlaceTool::MultiRoute => {
                self.bus_route_clicked(tool, at);
            }
            BoardPlaceTool::TextString => self.string_clicked(at),
            BoardPlaceTool::Pad => self.pad_clicked(at),
            BoardPlaceTool::Arc => self.arc_clicked(at),
            BoardPlaceTool::Dimension => self.dimension_clicked(at),
            BoardPlaceTool::PourCutout => self.pour_cutout_clicked(at),
            BoardPlaceTool::SlicePour => self.slice_pour_clicked(at),
            BoardPlaceTool::Fill
            | BoardPlaceTool::SolidRegion
            | BoardPlaceTool::PolygonPour
            | BoardPlaceTool::Keepout => {
                self.area_clicked(tool, at);
            }
            BoardPlaceTool::Line
            | BoardPlaceTool::GraphicArc
            | BoardPlaceTool::FullCircle
            | BoardPlaceTool::Rectangle
            | BoardPlaceTool::TextFrame => self.graphic_clicked(tool, at),
            // #2457: the first click is a corner; the second places the room
            // and ends the tool, so Properties shows the new room.
            BoardPlaceTool::Room => {
                let first = self
                    .own_board_tool()
                    .and_then(|s| s.points.first().copied());
                match first {
                    None => {
                        if let Some(session) = self.own_board_tool_mut() {
                            session.points.push(at);
                        }
                    }
                    Some(first) => {
                        if self.place_drawn_room(first, at) {
                            self.end_board_tool();
                            return true;
                        }
                    }
                }
            }
            _ => return false,
        }
        if let Some(session) = self.own_board_tool_mut() {
            session.cursor = Some(at);
        }
        self.sync_route_ratsnest();
        self.refresh_pcb_highlight();
        true
    }

    /// What the canvas draws on the cursor for the tool armed on the board
    /// in view: the board holding only what the next click adds, and the
    /// routed net's ratsnest line from the route's end (`route_rats`).
    pub(crate) fn board_tool_ghost(&self) -> Option<PcbSnapshot> {
        let session = self.own_board_tool()?;
        let preview = self.board_tool_preview()?;
        let line = self.route_ratsnest_line(session, &preview);
        let snapshot = self.pcb_ghost_snapshot_for(&session.path, &preview);
        Some(match line {
            Some(line) => snapshot.with_ratsnest(&[line], &Default::default()),
            None => snapshot,
        })
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
            BoardPlaceTool::InteractiveRouting
            | BoardPlaceTool::QuickRoute
            | BoardPlaceTool::DiffPairRouting
            | BoardPlaceTool::MultiRoute => {
                self.router_preview(session, &mut preview);
            }
            BoardPlaceTool::TextString => self.string_preview(cursor, &mut preview),
            BoardPlaceTool::Pad | BoardPlaceTool::Arc | BoardPlaceTool::Dimension => {
                self.free_preview(session, board, cursor, &mut preview);
            }
            BoardPlaceTool::PourCutout | BoardPlaceTool::SlicePour => {
                self.pour_cut_preview(session, cursor, &mut preview);
            }
            BoardPlaceTool::Fill
            | BoardPlaceTool::SolidRegion
            | BoardPlaceTool::PolygonPour
            | BoardPlaceTool::Keepout => {
                self.area_preview(session, board, cursor, &mut preview);
            }
            BoardPlaceTool::Line
            | BoardPlaceTool::GraphicArc
            | BoardPlaceTool::FullCircle
            | BoardPlaceTool::Rectangle
            | BoardPlaceTool::TextFrame => self.graphic_preview(session, cursor, &mut preview),
            _ => return None,
        }
        Some(preview)
    }
}

/// Whether `tool` draws on the active copper layer only.
fn draws_on_copper(tool: BoardPlaceTool) -> bool {
    matches!(
        tool,
        BoardPlaceTool::Track
            | BoardPlaceTool::InteractiveRouting
            | BoardPlaceTool::DiffPairRouting
            | BoardPlaceTool::QuickRoute
            | BoardPlaceTool::MultiRoute
            | BoardPlaceTool::Fill
            | BoardPlaceTool::SolidRegion
            | BoardPlaceTool::PolygonPour
            | BoardPlaceTool::PourCutout
            | BoardPlaceTool::SlicePour
    )
}
