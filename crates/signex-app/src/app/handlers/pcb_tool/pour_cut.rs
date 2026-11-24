//! Pour Cutout and Slice Pour (#2216), on the active copper layer, each one
//! undo step:
//! - Pour Cutout takes a corner per click and is closed by a click on its
//!   first corner or a right-click once it has three; it places a cutout
//!   region of its own (`draw::new_pour_cutout`, #2871) that every polygon
//!   pour on the layer leaves empty when it fills. No pour is edited.
//! - Slice Pour takes two clicks; each pour on the layer the segment meets is
//!   cut in two along the line through them (`pour_edit::slice_pours`).
//!   When nothing is cut, Messages says so and the board is unchanged.

use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::draw;
use signex_engine::pcb::pour_edit::slice_pours;
use signex_i18n::{ids, t};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

use super::BoardToolSession;
use super::outline_preview::open_outline;
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

impl Signex {
    /// A Pour Cutout click at `at`: a new corner, or the cutout once it
    /// closes on its first corner.
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
            self.place_pour_cutout(outline);
            return;
        }
        if points.last() != Some(&at)
            && let Some(session) = self.own_board_tool_mut()
        {
            session.points.push(at);
        }
    }

    /// Place a pour cutout over `outline` on the active copper layer and
    /// start the next shape.
    pub(super) fn place_pour_cutout(&mut self, outline: Vec<Point>) {
        if let Some(session) = self.own_board_tool_mut() {
            session.points.clear();
        }
        let tool = BoardPlaceTool::PourCutout;
        let Some(layer) = self.tool_copper_layer(tool) else {
            return;
        };
        let name = t!(self.ui_state.i18n, tool.name()).into_string();
        match draw::new_pour_cutout(Uuid::now_v7(), outline, layer) {
            Some(region) => {
                self.run_placement(&name, Ok(vec![draw::place_zone(region)]));
            }
            None => {
                let note = t!(self.ui_state.i18n, ids::PCB_TOOL_NO_AREA, "tool" => name.as_str());
                crate::diagnostics::log_warning(note.into_string());
            }
        }
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

    /// Run a slice as one step, or say in Messages that nothing was cut.
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
    /// `preview`: the cutout region, the slice as a thin line on the active
    /// copper layer.
    pub(super) fn pour_cut_preview(
        &self,
        session: &BoardToolSession,
        cursor: Point,
        preview: &mut PcbBoard,
    ) {
        let Some(&first) = session.points.first() else {
            return;
        };
        let Some(copper) = self.active_copper_layer() else {
            return;
        };
        let layer = PcbLayer::Copper(copper);
        if session.tool == BoardPlaceTool::SlicePour {
            preview
                .segments
                .extend(open_outline(&[first, cursor], layer));
            return;
        }
        let outline: Vec<Point> = session.points.iter().copied().chain([cursor]).collect();
        match draw::new_pour_cutout(Uuid::nil(), outline.clone(), copper) {
            Some(zone) => preview.zones.push(zone),
            None => preview.segments.extend(open_outline(&outline, layer)),
        }
    }
}
