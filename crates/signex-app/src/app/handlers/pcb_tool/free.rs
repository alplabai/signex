//! The Pad, Arc and Dimension tools (#2373), each placing as one undo step:
//! - Pad: each click places a free through-hole pad on the net under it;
//! - Arc: on the active layer, by centre, by edge or at any angle
//!   (`cycle_arc_mode`); a copper arc takes the net under its start;
//! - Dimension: two clicks for the points it measures, a third for where
//!   its line stands.
//!
//! What each places follows the values set in Properties (#2393,
//! `BoardToolDefaults`).

use signex_engine::pcb::BoardItem;
use signex_engine::pcb::draw::{self, ArcMode};
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb::{BoardText, Pad, PadNet, PcbBoard, Point};
use signex_types::pcb_arc::TrackArc;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

use super::{BoardToolDefaults, BoardToolSession};
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

/// The sentence of the Arc hint that says how `mode` draws.
pub(crate) fn arc_mode_hint(mode: ArcMode) -> MessageId {
    match mode {
        ArcMode::Centre => ids::PCB_ARC_MODE_CENTRE,
        ArcMode::Edge => ids::PCB_ARC_MODE_EDGE,
        ArcMode::AnyAngle => ids::PCB_ARC_MODE_ANY,
    }
}

impl BoardToolDefaults {
    /// The free pad the Pad tool places at `at` on `board`.
    pub fn pad(&self, board: &PcbBoard, id: Uuid, at: Point) -> Pad {
        let number = self.pad_designator.clone().unwrap_or_else(|| "1".into());
        let mut pad = draw::new_free_pad(board, id, at, number);
        if let Some(shape) = self.pad_shape {
            pad.shape = shape;
        }
        pad.size.x = self.pad_size_x_mm.unwrap_or(pad.size.x);
        pad.size.y = self.pad_size_y_mm.unwrap_or(pad.size.y);
        pad.rotation = self.pad_rotation.unwrap_or(pad.rotation);
        if let (Some(hole), Some(drill)) = (self.pad_hole_mm, pad.drill.as_mut()) {
            drill.diameter = hole;
        }
        match self.pad_net {
            Some(0) => pad.net = None,
            Some(number) => {
                let name = board
                    .nets
                    .iter()
                    .find(|n| n.number == number)
                    .map(|n| n.name.clone())
                    .unwrap_or_default();
                pad.net = Some(PadNet { number, name });
            }
            None => {}
        }
        pad
    }

    /// The arc through `points` the Arc tool places on `layer`.
    pub fn arc(
        &self,
        board: &PcbBoard,
        id: Uuid,
        points: (Point, Point, Point),
        layer: PcbLayer,
    ) -> Option<TrackArc> {
        let width = self
            .arc_width_mm
            .unwrap_or_else(|| draw::default_track_width(board));
        let mut arc = draw::new_arc(board, id, points, layer, width)?;
        if let Some(net) = self.arc_net.filter(|_| layer.is_copper()) {
            arc.net = net;
        }
        Some(arc)
    }

    /// The string the String tool places at `at` on `layer`; `None` on a
    /// layer that takes no text. `fallback` is the catalog's new-string text.
    pub fn text(
        &self,
        id: Uuid,
        at: Point,
        layer: PcbLayer,
        fallback: String,
    ) -> Option<BoardText> {
        let words = self.text.clone().unwrap_or(fallback);
        let mut text = draw::new_text(id, at, layer, words)?;
        text.font_size = self.text_height_mm.unwrap_or(text.font_size);
        text.rotation = self.text_rotation.unwrap_or(text.rotation);
        Some(text)
    }

    /// The dimension the Dimension tool places from `start` to `end`, its
    /// line through `through`.
    pub fn dimension(
        &self,
        id: Uuid,
        (start, end, through): (Point, Point, Point),
        layer: PcbLayer,
    ) -> Option<LinearDimension> {
        let mut dimension = draw::new_dimension(id, start, end, through, layer)?;
        dimension.line_width = self.dimension_line_width_mm.unwrap_or(dimension.line_width);
        dimension.text_height = self
            .dimension_text_height_mm
            .unwrap_or(dimension.text_height);
        dimension.precision = self.dimension_precision.unwrap_or(dimension.precision);
        Some(dimension)
    }
}

impl Signex {
    /// The layer the board in view has active.
    fn active_board_layer(&self) -> Option<PcbLayer> {
        let path = self.active_tab_path()?;
        Some(self.pcb_layer_view(&path).active())
    }

    /// A Pad click at `at`: place a free pad there.
    pub(super) fn pad_clicked(&mut self, at: Point) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let pad = self
            .interaction_state
            .pcb_tool_defaults
            .pad(board, Uuid::now_v7(), at);
        let name = t!(self.ui_state.i18n, ids::PCB_TOOL_PAD).into_string();
        self.run_placement(&name, Ok(vec![draw::place_item(BoardItem::FreePad(pad))]));
    }

    /// An Arc click at `at`: one more point of the arc, or its last, which
    /// places it.
    pub(super) fn arc_clicked(&mut self, at: Point) {
        let mode = self.interaction_state.pcb_tool_defaults.arc_mode;
        let Some(clicks) = self.next_clicks(at, mode.clicks()) else {
            return;
        };
        let (Some(board), Some(layer)) = (self.active_pcb(), self.active_board_layer()) else {
            return;
        };
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let arc = draw::arc_points(mode, &clicks)
            .and_then(|points| defaults.arc(board, Uuid::now_v7(), points, layer));
        match arc {
            Some(arc) => {
                let name = t!(self.ui_state.i18n, ids::PCB_TOOL_ARC).into_string();
                self.run_placement(&name, Ok(vec![draw::place_item(BoardItem::Arc(arc))]));
            }
            None => {
                let note = t!(self.ui_state.i18n, ids::PCB_TOOL_NO_ARC);
                crate::diagnostics::log_warning(note.into_string());
            }
        }
    }

    /// A Dimension click at `at`: a measured point, or where the line
    /// stands, which places it.
    pub(super) fn dimension_clicked(&mut self, at: Point) {
        let Some(clicks) = self.next_clicks(at, 3) else {
            return;
        };
        let Some(layer) = self.active_board_layer() else {
            return;
        };
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let points = (clicks[0], clicks[1], clicks[2]);
        match defaults.dimension(Uuid::now_v7(), points, layer) {
            Some(dimension) => {
                let name = t!(self.ui_state.i18n, ids::PCB_TOOL_DIMENSION).into_string();
                let command = draw::place_item(BoardItem::Dimension(dimension));
                self.run_placement(&name, Ok(vec![command]));
            }
            None => {
                let note = t!(self.ui_state.i18n, ids::PCB_TOOL_NO_DIMENSION);
                crate::diagnostics::log_warning(note.into_string());
            }
        }
    }

    /// The clicks of the shape with `at` added once it has `needed` of
    /// them, which starts the next shape; `None` while it needs more, after
    /// keeping `at` (a click on the last point adds nothing).
    fn next_clicks(&mut self, at: Point, needed: usize) -> Option<Vec<Point>> {
        let session = self.own_board_tool_mut()?;
        if session.points.len() + 1 < needed {
            if session.points.last() != Some(&at) {
                session.points.push(at);
            }
            return None;
        }
        let mut clicks = std::mem::take(&mut session.points);
        clicks.push(at);
        Some(clicks)
    }

    /// What the next click of the Pad, Arc or Dimension tool adds, drawn
    /// into `preview`.
    pub(super) fn free_preview(
        &self,
        session: &BoardToolSession,
        board: &PcbBoard,
        cursor: Point,
        preview: &mut PcbBoard,
    ) {
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let Some(layer) = self.active_board_layer() else {
            return;
        };
        let clicks: Vec<Point> = session.points.iter().copied().chain([cursor]).collect();
        match session.tool {
            BoardPlaceTool::Pad => {
                preview
                    .free
                    .pads
                    .push(defaults.pad(board, Uuid::nil(), cursor));
            }
            BoardPlaceTool::Arc => {
                let arc = draw::arc_points(defaults.arc_mode, &clicks)
                    .and_then(|points| defaults.arc(board, Uuid::nil(), points, layer));
                preview.free.arcs.extend(arc);
            }
            BoardPlaceTool::Dimension => {
                // Before the second point the line lies on the measured one.
                let through = clicks.get(2).copied().unwrap_or(clicks[0]);
                let Some(&end) = clicks.get(1) else {
                    return;
                };
                let points = (clicks[0], end, through);
                preview
                    .free
                    .dimensions
                    .extend(defaults.dimension(Uuid::nil(), points, layer));
            }
            _ => {}
        }
    }

    /// `cycle_arc_mode`: the next arc is drawn the next way; Messages says
    /// how.
    pub(crate) fn step_arc_mode(&mut self) {
        let defaults = &mut self.interaction_state.pcb_tool_defaults;
        defaults.arc_mode = defaults.arc_mode.next();
        let mode = defaults.arc_mode;
        let how = t!(self.ui_state.i18n, arc_mode_hint(mode)).into_string();
        let line = t!(self.ui_state.i18n, ids::PCB_TOOL_HINT_ARC, "mode" => how);
        crate::diagnostics::log_info(line.into_string());
        // The clicks of an arc begun another way do not carry over.
        if let Some(session) = self
            .own_board_tool_mut()
            .filter(|s| s.tool == BoardPlaceTool::Arc)
        {
            session.points.clear();
        }
        self.refresh_pcb_highlight();
    }

    /// The pad the Pad tool places next, at the pointer, for its form.
    pub(crate) fn next_board_pad(&self, board: &PcbBoard) -> Pad {
        let at = self.board_tool_cursor();
        self.interaction_state
            .pcb_tool_defaults
            .pad(board, Uuid::nil(), at)
    }

    /// The arc the Arc tool places next, for its form: the arc being drawn
    /// when the pointer completes one, else a stand-in quarter circle at
    /// the pointer with the tool's values.
    pub(crate) fn next_board_arc(&self, board: &PcbBoard) -> Option<TrackArc> {
        let at = self.board_tool_cursor();
        let layer = self.active_board_layer()?;
        let defaults = &self.interaction_state.pcb_tool_defaults;
        let drawn = self.own_board_tool().and_then(|s| {
            let clicks: Vec<Point> = s.points.iter().copied().chain([at]).collect();
            draw::arc_points(defaults.arc_mode, &clicks)
        });
        let stand_in = draw::arc_points(ArcMode::Edge, &[at, Point::new(at.x + 1.0, at.y)]);
        defaults.arc(board, Uuid::nil(), drawn.or(stand_in)?, layer)
    }

    /// The string the String tool places next, for its form; on a layer
    /// that takes no text, as it would read on the top overlay.
    pub(crate) fn next_board_text(&self) -> Option<BoardText> {
        let at = self.board_tool_cursor();
        let layer = self
            .active_board_layer()
            .filter(|l| l.takes_text())
            .unwrap_or(PcbLayer::Technical(
                signex_types::pcb_layer::TechnicalLayer::SilkscreenTop,
            ));
        let fallback = t!(self.ui_state.i18n, ids::PCB_TOOL_STRING_TEXT).into_string();
        let defaults = &self.interaction_state.pcb_tool_defaults;
        defaults.text(Uuid::nil(), at, layer, fallback)
    }

    /// The dimension the Dimension tool places next, for its form: a
    /// stand-in 10 mm long at the pointer with the tool's values.
    pub(crate) fn next_board_dimension(&self) -> Option<LinearDimension> {
        let at = self.board_tool_cursor();
        let layer = self.active_board_layer()?;
        let end = Point::new(at.x + 10.0, at.y);
        let defaults = &self.interaction_state.pcb_tool_defaults;
        defaults.dimension(Uuid::nil(), (at, end, at), layer)
    }
}
