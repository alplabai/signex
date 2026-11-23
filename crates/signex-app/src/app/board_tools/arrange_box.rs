//! Arrange in Box on the board in view (#2409): `arrange_in_box` arms the
//! box for the selected unlocked components, two clicks on the board draw
//! it, and the components are packed inside it as one undo step by the
//! command's operand handler. Esc drops it; arming a placement tool does too.

use std::collections::BTreeSet;
use std::path::PathBuf;

use iced::Task;
use signex_engine::pcb::placement::editable_footprints;
use signex_i18n::{ids, t};
use signex_types::pcb::Point;
use signex_types::pcb_options::GridUse;
use uuid::Uuid;

use crate::app::command::OperandValue;
use crate::app::{Message, Signex};

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
    /// corner, the second the opposite one and runs `arrange_in_box`'s
    /// operand handler on the armed components and the two corners. `None`
    /// when the click did not belong to it.
    pub(crate) fn pcb_arrange_box_clicked(&mut self, at: Point) -> Option<Task<Message>> {
        let armed = self.interaction_state.pcb_arrange_box.clone()?;
        let main_window = self.interaction_state.acting_window.is_none();
        if !main_window || self.active_tab_path().as_deref() != Some(armed.path.as_path()) {
            return None;
        }
        let Some(board) = self.active_pcb() else {
            return Some(Task::none());
        };
        let at = self.board_grid_point(board, GridUse::NonComponents, at);
        let Some(first) = armed.first else {
            if let Some(armed) = self.interaction_state.pcb_arrange_box.as_mut() {
                armed.first = Some(at);
            }
            return Some(Task::none());
        };
        self.interaction_state.pcb_arrange_box = None;
        let mut operands = vec![
            ("start", OperandValue::Point(first.x, first.y)),
            ("end", OperandValue::Point(at.x, at.y)),
        ];
        if !armed.footprints.is_empty() {
            let ids = armed.footprints.into_iter().collect();
            operands.push(("ids", OperandValue::Targets(ids)));
        }
        Some(self.run_translated_tool(COMMAND, &armed.path, operands))
    }
}
