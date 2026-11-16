//! Pour Cutout and Slice Pour (#2216), on the active copper layer, each one
//! undo step built by `signex_engine::pcb::pour_edit`:
//! - Pour Cutout takes a corner per click and is closed by a click on its
//!   first corner or a right-click once it has three; the area is cut out of
//!   every polygon pour on the layer it overlaps (a hole, a notch, or pieces
//!   that become pours of their own).
//! - Slice Pour takes two clicks; each pour on the layer the segment meets is
//!   cut in two along the line through them.
//!
//! When nothing is cut, Messages says so and the board is unchanged.

use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::draw;
use signex_engine::pcb::pour_edit::{cut_pours, slice_pours};
use signex_i18n::{ids, t};
use signex_types::format::pcb_layer_token;
use signex_types::pcb::{PcbBoard, Point, Segment};
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

use super::BoardToolSession;
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

/// The width, in mm, the slice line is previewed at.
const SLICE_PREVIEW_WIDTH_MM: f64 = 0.1;

impl Signex {
    /// A Pour Cutout click at `at`: a new corner, or the cut once it closes
    /// on its first corner.
    pub(super) fn pour_cutout_clicked(&mut self, at: Point) {
        if self.tool_copper_layer(BoardPlaceTool::PourCutout).is_none() {
            return;
        }
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let points = &session.points;
        if points.len() >= 3 && points.first() == Some(&at) {
            let outline = points.clone();
            self.cut_pours_with(outline);
            return;
        }
        if points.last() != Some(&at)
            && let Some(session) = self.own_board_tool_mut()
        {
            session.points.push(at);
        }
    }

    /// Cut `outline` out of the pours on the active copper layer and start
    /// the next shape.
    pub(super) fn cut_pours_with(&mut self, outline: Vec<Point>) {
        if let Some(session) = self.own_board_tool_mut() {
            session.points.clear();
        }
        let tool = BoardPlaceTool::PourCutout;
        let layer = self.tool_copper_layer(tool);
        let (Some(board), Some(layer)) = (self.active_pcb(), layer) else {
            return;
        };
        let commands = cut_pours(board, layer, &outline, Uuid::now_v7);
        self.run_pour_edit(tool, commands);
        self.refresh_pcb_highlight();
    }

    /// A Slice Pour click at `at`: the line's first point, or the slice from
    /// it to `at`.
    pub(super) fn slice_pour_clicked(&mut self, at: Point) {
        let tool = BoardPlaceTool::SlicePour;
        let Some(layer) = self.tool_copper_layer(tool) else {
            return;
        };
        let first = self
            .own_board_tool()
            .and_then(|session| session.points.first().copied());
        let Some(first) = first else {
            if let Some(session) = self.own_board_tool_mut() {
                session.points.push(at);
            }
            return;
        };
        if first == at {
            return;
        }
        if let Some(session) = self.own_board_tool_mut() {
            session.points.clear();
        }
        let Some(board) = self.active_pcb() else {
            return;
        };
        let commands = slice_pours(board, layer, first, at, Uuid::now_v7);
        self.run_pour_edit(tool, commands);
    }

    /// Run a pour edit as one step, or say in Messages that nothing was cut.
    fn run_pour_edit(&mut self, tool: BoardPlaceTool, commands: Vec<BoardCommand>) {
        let name = t!(self.ui_state.i18n, tool.name()).into_string();
        if commands.is_empty() {
            let note = t!(self.ui_state.i18n, ids::PCB_TOOL_POUR_MISSED, "tool" => name.as_str());
            crate::diagnostics::log_warning(note.into_string());
            return;
        }
        self.run_placement(&name, Ok(commands));
    }

    /// What the next click of Pour Cutout or Slice Pour closes, drawn into
    /// `preview`: the cutout as a keepout-styled area, the slice as a thin
    /// line on the active copper layer.
    pub(super) fn pour_cut_preview(
        &self,
        session: &BoardToolSession,
        cursor: Point,
        preview: &mut PcbBoard,
    ) {
        let Some(&first) = session.points.first() else {
            return;
        };
        let Some(layer) = self.active_copper_layer() else {
            return;
        };
        if session.tool == BoardPlaceTool::SlicePour {
            preview.segments.push(Segment {
                locked: false,
                attributes: Default::default(),
                uuid: Uuid::nil(),
                start: first,
                end: cursor,
                width: SLICE_PREVIEW_WIDTH_MM,
                layer: pcb_layer_token(PcbLayer::Copper(layer)),
                net: 0,
                unknown_columns: Default::default(),
            });
            return;
        }
        let outline: Vec<Point> = session.points.iter().copied().chain([cursor]).collect();
        preview.zones.extend(draw::new_keepout(
            Uuid::nil(),
            outline,
            PcbLayer::Copper(layer),
        ));
    }
}
