//! The Graphics Line and Arc on a copper layer (Caner, 2026-11-24, #2869):
//! copper is a track. Line draws a run of straight tracks, any angle, on
//! the net of the copper under its first click, one undo step per track; a
//! click on the run's end ends it. Arc draws a track arc by its centre,
//! start and end (clockwise on screen), on the net under its start. Their
//! width and net come from the Track and Arc tools' values, which
//! Properties shows while the active layer is copper (`placing_tool`).

use signex_engine::pcb::BoardItem;
use signex_engine::pcb::draw::{self, TrackStyle};
use signex_i18n::{ids, t};
use signex_types::corner::{CornerMode, CornerRule};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use uuid::Uuid;

use super::BoardToolSession;
use super::outline_preview::open_outline;
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

/// A copper line is one straight track per click.
const STRAIGHT: CornerRule = CornerRule {
    mode: CornerMode::FreeAngle,
    is_flipped: false,
};

impl Signex {
    /// The tool `tool` places as on the active layer: the Graphics Line and
    /// Arc on copper place a track and a track arc, so their values and
    /// form are the Track and copper Arc tools'. Any other tool is itself.
    pub(crate) fn placing_tool(&self, tool: BoardPlaceTool) -> BoardPlaceTool {
        let on_copper = self.active_copper_layer().is_some();
        match tool {
            BoardPlaceTool::Line if on_copper => BoardPlaceTool::Track,
            BoardPlaceTool::GraphicArc if on_copper => BoardPlaceTool::Arc,
            _ => tool,
        }
    }

    /// A Line or Arc click at `at` with a copper layer active. Whether it
    /// was one.
    pub(super) fn copper_graphic_clicked(&mut self, tool: BoardPlaceTool, at: Point) -> bool {
        let Some(layer) = self.active_copper_layer() else {
            return false;
        };
        match tool {
            BoardPlaceTool::Line => self.copper_line_clicked(layer, at),
            BoardPlaceTool::GraphicArc => self.copper_arc_clicked(PcbLayer::Copper(layer), at),
            _ => return false,
        }
        true
    }

    fn set_copper_run(&mut self, points: Vec<Point>, style: Option<TrackStyle>) {
        if let Some(session) = self.own_board_tool_mut() {
            session.points = points;
            session.route = style;
        }
    }

    /// The first click starts the run on the net under it; each later one
    /// adds a straight track from the run's end. A click on the end, or on
    /// another copper layer than the run's, ends the run (the latter starts
    /// a new one there).
    fn copper_line_clicked(&mut self, layer: CopperLayer, at: Point) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let run = (session.points.last().copied())
            .zip(session.route)
            .filter(|(_, style)| style.layer == layer);
        let Some((anchor, style)) = run else {
            let defaults = &self.interaction_state.pcb_tool_defaults;
            let mut style = defaults.track_style(board, at, layer);
            if defaults.track_width_mm.is_none() {
                style = style.with_class_width(board, self.track_tool_net_classes());
            }
            self.set_copper_run(vec![at], Some(style));
            return;
        };
        if self.clicks_run_end(at, anchor) {
            self.set_copper_run(Vec::new(), None);
            return;
        }
        let tracks = draw::track_run(anchor, at, STRAIGHT, style, Uuid::now_v7);
        if let Some(command) = draw::place_route(tracks, None) {
            let name = t!(self.ui_state.i18n, BoardPlaceTool::Line.name()).into_string();
            self.run_placement(&name, Ok(vec![command]));
        }
        self.set_copper_run(vec![at], Some(style));
    }

    /// The centre, the start, then the end, which places the track arc.
    fn copper_arc_clicked(&mut self, layer: PcbLayer, at: Point) {
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let mut points = session.points.clone();
        if points.last() == Some(&at) {
            return;
        }
        points.push(at);
        if points.len() < 3 {
            self.set_copper_run(points, None);
            return;
        }
        let (centre, start, end) = (points[0], points[1], points[2]);
        let Some(board) = self.active_pcb() else {
            return;
        };
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let arc = draw::arc_points_about(centre, start, end)
            .and_then(|three| defaults.arc(board, Uuid::now_v7(), three, layer));
        self.set_copper_run(Vec::new(), None);
        match arc {
            Some(arc) => {
                let name = t!(self.ui_state.i18n, BoardPlaceTool::GraphicArc.name()).into_string();
                self.run_placement(&name, Ok(vec![draw::place_item(BoardItem::Arc(arc))]));
            }
            None => {
                let note = t!(self.ui_state.i18n, ids::PCB_TOOL_NO_ARC);
                self.note_board_tool(note.into_string());
            }
        }
    }

    /// What the next Line or Arc click on copper adds, drawn into
    /// `preview`. Whether the active layer is copper and the tool one of
    /// them.
    pub(super) fn copper_graphic_preview(
        &self,
        session: &BoardToolSession,
        cursor: Point,
        preview: &mut PcbBoard,
    ) -> bool {
        let Some(layer) = self.active_copper_layer() else {
            return false;
        };
        match session.tool {
            BoardPlaceTool::Line => {
                if let (Some(&anchor), Some(style)) = (session.points.last(), session.route) {
                    let legs = draw::track_run(anchor, cursor, STRAIGHT, style, Uuid::nil);
                    preview.segments.extend(legs);
                }
            }
            BoardPlaceTool::GraphicArc => {
                let Some(board) = self.document_state.pcb_board(&session.path) else {
                    return true;
                };
                let clicks: Vec<Point> = session.points.iter().copied().chain([cursor]).collect();
                let layer = PcbLayer::Copper(layer);
                let defaults = &self.interaction_state.pcb_tool_defaults;
                match clicks.as_slice() {
                    &[centre, start, end] => preview.free.arcs.extend(
                        draw::arc_points_about(centre, start, end)
                            .and_then(|three| defaults.arc(board, Uuid::nil(), three, layer)),
                    ),
                    // Before the end, a line from the centre.
                    [_, _, ..] => preview.segments.extend(open_outline(&clicks, layer)),
                    _ => {}
                }
            }
            _ => return false,
        }
        true
    }
}
