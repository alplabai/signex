//! Length tuning from the track panel (#2389): Enter in Tune Net To turns
//! the one selected track into an accordion that brings its net to the
//! typed length (`signex_engine::pcb::route::tune_length`), keeping the
//! rule clearance from other nets, as one board undo step. The panel's
//! net length readout then shows the new length.

use pcb_geom::{CopperItem, ItemRef, nm_to_mm};
use signex_engine::pcb::route::{Meander, tune_length};
use signex_i18n::{ids, t};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::state::pcb_edit::CheckedStep;
use crate::panels::pcb_primitive_properties::PrimitiveTarget;

/// How tall a bump may grow, in accordion pitches.
const AMPLITUDE_PITCHES: f64 = 4.0;

impl Signex {
    /// Tune the target's one track so its net reaches `target_mm`, or say
    /// why not.
    pub(super) fn tune_track_length(&mut self, target: &PrimitiveTarget, target_mm: f64) {
        let what = ids::PCB_PRIM_TUNE_LENGTH;
        let [ItemRef::Track(track)] = target.items[..] else {
            let why = t!(self.ui_state.i18n, ids::PCB_PRIM_TUNE_ONE_TRACK).into_string();
            self.refuse_primitive_edit(what, why);
            return;
        };
        let Some(engine) = self.document_state.board_engine(&target.board) else {
            let name = t!(self.ui_state.i18n, what).into_string();
            let line = t!(self.ui_state.i18n, ids::PCB_PRIM_BOARD_GONE, "what" => name);
            crate::diagnostics::log_warning(line.into_string());
            return;
        };
        let board = engine.board();
        if board.unknown.newer_minor_source.is_some() {
            let why = t!(self.ui_state.i18n, ids::PCB_PRIM_READ_ONLY).into_string();
            self.refuse_primitive_edit(what, why);
            return;
        }
        let revision = engine.revision();
        let rules = match self.board_design_rules(&target.board, board) {
            Ok((rules, _)) => rules,
            Err(why) => {
                self.refuse_primitive_edit(what, why);
                return;
            }
        };
        let width = board
            .segments
            .iter()
            .find(|s| s.uuid == track)
            .map_or(0.0, |s| s.width.abs());
        // Legs a track width and a clearance apart, on either side.
        let pitch_mm = 2.0 * (width + nm_to_mm(rules.clearance));
        let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
        let meander = Meander {
            clearance: &clearance,
            reach: rules.widest_clearance(),
            pitch_mm,
            amplitude_mm: AMPLITUDE_PITCHES * pitch_mm,
        };
        let commands = match tune_length(board, track, target_mm, &meander, Uuid::now_v7) {
            Ok(commands) => commands,
            Err(refusal) => {
                self.refuse_primitive_edit(what, refusal.to_string());
                return;
            }
        };
        let label = t!(self.ui_state.i18n, what).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: revision,
        };
        if let Err(error) = self.execute_board_commands(&target.board, commands, Some(step)) {
            self.refuse_primitive_edit(what, error.to_string());
        }
    }
}
