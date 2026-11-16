//! Arrange in Box on the board in view (#2409): `arrange_in_box` arms the
//! box for the selected unlocked components, two clicks on the board draw
//! it, and the components are packed inside it as one undo step. Esc drops
//! it; arming a placement tool does too.

use std::collections::BTreeSet;
use std::path::PathBuf;

use signex_engine::pcb::board_tools::arrange_in_box;
use signex_engine::pcb::placement::{editable_footprints, grid_for};
use signex_i18n::{ids, t};
use signex_types::pcb::Point;
use signex_types::pcb_options::GridUse;
use uuid::Uuid;

use crate::app::Signex;

const COMMAND: &str = "arrange_in_box";

/// Arrange in Box waiting for its corners.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrangeBox {
    /// The board it was armed on.
    pub path: PathBuf,
    /// The components it moves, as selected when it was armed.
    pub footprints: BTreeSet<Uuid>,
    /// The first corner, once clicked.
    pub first: Option<Point>,
}

impl Signex {
    /// Arm the box for the components selected on the board in view, or
    /// say why not (nothing selected, a locked one).
    pub(crate) fn arm_arrange_box(&mut self) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return;
        };
        let footprints = self.pcb_selected_footprints();
        let Some(board) = self.active_pcb() else {
            return;
        };
        if let Err(refusal) = editable_footprints(board, &footprints) {
            self.say_board_tool(COMMAND, &Err(refusal.to_string()));
            return;
        }
        self.end_pcb_carry();
        self.interaction_state.pcb_tool = None;
        self.interaction_state.pcb_guide_place = None;
        self.interaction_state.pcb_arrange_box = Some(ArrangeBox {
            path,
            footprints,
            first: None,
        });
        crate::diagnostics::log_info(
            t!(self.ui_state.i18n, ids::PCB_ARRANGE_BOX_HINT).into_string(),
        );
        self.refresh_pcb_highlight();
    }

    /// A click on the board while the box is armed: the first sets a
    /// corner, the second the opposite one and runs the arrange. Whether
    /// the click belonged to it.
    pub(crate) fn pcb_arrange_box_clicked(&mut self, at: Point) -> bool {
        let Some(armed) = self.interaction_state.pcb_arrange_box.clone() else {
            return false;
        };
        let main_window = self.interaction_state.acting_window.is_none();
        if !main_window || self.active_tab_path().as_deref() != Some(armed.path.as_path()) {
            return false;
        }
        let Some(board) = self.active_pcb() else {
            return true;
        };
        let at = grid_for(board, GridUse::NonComponents).snap(at);
        let Some(first) = armed.first else {
            if let Some(armed) = self.interaction_state.pcb_arrange_box.as_mut() {
                armed.first = Some(at);
            }
            return true;
        };
        let built = arrange_in_box(board, &armed.footprints, first, at);
        self.interaction_state.pcb_arrange_box = None;
        let ran = self.run_board_tool(&armed.path, built);
        self.say_board_tool(COMMAND, &ran);
        true
    }
}
