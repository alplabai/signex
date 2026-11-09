//! Placement edits on the board in view (#2240): rotate, flip, delete and
//! move the selected footprints, by key, menu, Active Bar action, palette,
//! drag or Edit ▸ Move. Each edit is one `execute_board_commands` step; the
//! commands are built by `signex_engine::pcb::placement`, which also refuses
//! a locked footprint. A refusal changes nothing and says why in the
//! Messages panel. A selected pad stands for its footprint.

use std::collections::BTreeSet;

use pcb_geom::ItemRef;
use signex_engine::pcb::placement::{self, PlacementRefusal, PlacementTransform};
use signex_engine::pcb::{BoardCommand, BoardItemId};
use signex_types::pcb::Point;
use uuid::Uuid;

use crate::app::Signex;
use crate::pcb_canvas::{CarryKind, PcbCarry, carry_delta};

impl Signex {
    /// Rotate or mirror the board selection when the active tab is a PCB.
    /// Whether it was one (the schematic path is then skipped).
    pub(crate) fn transform_pcb_selection(&mut self, transform: PlacementTransform) -> bool {
        if !self.has_active_pcb() {
            return false;
        }
        let footprints = self.pcb_selected_footprints();
        let Some(board) = self.active_pcb() else {
            return false;
        };
        let built = placement::transform_commands(board, &footprints, transform);
        let what = match transform {
            PlacementTransform::Rotate { .. } => "Rotate",
            PlacementTransform::FlipLeftRight | PlacementTransform::FlipTopBottom => "Flip",
        };
        self.run_placement(what, built);
        true
    }

    /// Delete the board selection when the active tab is a PCB: its
    /// footprints (a selected pad's included), tracks and vias.
    pub(crate) fn delete_pcb_selection(&mut self) -> bool {
        if !self.has_active_pcb() {
            return false;
        }
        let items = self
            .with_pcb_selection(|state, _| deletable_items(state.selection()))
            .unwrap_or_default();
        let Some(board) = self.active_pcb() else {
            return false;
        };
        let built = placement::delete_commands(board, items);
        if self.run_placement("Delete", built) {
            self.interaction_state.pcb_selection.clear_selection();
            self.refresh_pcb_highlight();
        }
        true
    }

    /// Edit ▸ Move on a board: the selected footprints follow the pointer
    /// from the first click to the second. With nothing selected and
    /// `pick_first`, the first click picks the footprint under it.
    pub(crate) fn arm_pcb_move(&mut self, pick_first: bool) -> bool {
        if !self.has_active_pcb() {
            return false;
        }
        let footprints = self.pcb_selected_footprints();
        let (Some(board), Some(path)) = (self.active_pcb(), self.active_tab_path()) else {
            return false;
        };
        if footprints.is_empty() && !pick_first {
            crate::diagnostics::log_warning("Move: select a footprint to move first");
            return true;
        }
        if !footprints.is_empty()
            && let Err(refusal) = placement::editable_footprints(board, &footprints)
        {
            refuse("Move", &refusal);
            return true;
        }
        crate::diagnostics::log_info(if footprints.is_empty() {
            "Move: click a footprint, then click where it should go (Esc to cancel)"
        } else {
            "Move: click a reference point, then click where it should go (Esc to cancel)"
        });
        self.interaction_state.pcb_carry = Some(PcbCarry::new(CarryKind::Move, path, footprints));
        true
    }

    /// A click while Edit ▸ Move is armed: pick and anchor, or drop.
    /// Whether the click belonged to the move.
    pub(crate) fn pcb_move_clicked(&mut self, at: Point, tolerance_mm: f64) -> bool {
        let Some(carry) = self.live_carry(CarryKind::Move) else {
            return false;
        };
        let Some(anchor) = carry.anchor else {
            let mut footprints = carry.footprints;
            if footprints.is_empty() {
                // The click picks, and selects, what it lands on.
                self.with_pcb_selection(|state, board| {
                    state.click(board, at, tolerance_mm, pcb_geom::ClickMode::Replace)
                });
                footprints = self.pcb_selected_footprints();
                if footprints.is_empty() {
                    // Nothing under the pointer: keep waiting for a footprint.
                    self.refresh_pcb_highlight();
                    return true;
                }
            }
            let refused = self
                .active_pcb()
                .map(|board| placement::editable_footprints(board, &footprints).err());
            if let Some(Some(refusal)) = refused {
                refuse("Move", &refusal);
                self.end_pcb_carry();
                return true;
            }
            if let Some(carry) = self.interaction_state.pcb_carry.as_mut() {
                carry.footprints = footprints;
                carry.anchor = Some(at);
            }
            self.refresh_pcb_highlight();
            return true;
        };
        self.drop_pcb_carry(anchor, at);
        true
    }

    /// The pointer moved while a Move is anchored: follow it.
    pub(crate) fn pcb_move_followed(&mut self, at: Point) {
        if let Some(anchor) = self.live_carry(CarryKind::Move).and_then(|c| c.anchor) {
            self.carry_to(anchor, at);
        }
    }

    /// A drag of the selected footprints reached `to`: preview it.
    pub(crate) fn pcb_footprints_dragged(&mut self, from: Point, to: Point) {
        if self.live_carry(CarryKind::Drag).is_none() {
            let footprints = self.pcb_selected_footprints();
            let Some(path) = self.active_tab_path() else {
                return;
            };
            let mut carry = PcbCarry::new(CarryKind::Drag, path, footprints);
            carry.anchor = Some(from);
            self.interaction_state.pcb_carry = Some(carry);
            self.refresh_pcb_highlight();
        }
        self.carry_to(from, to);
    }

    /// The drag was released at `to`: move as one step.
    pub(crate) fn pcb_footprints_dropped(&mut self, from: Point, to: Point) {
        if self.live_carry(CarryKind::Drag).is_none() {
            self.pcb_footprints_dragged(from, to);
        }
        self.drop_pcb_carry(from, to);
    }

    /// Drop a drag or a Move without moving anything (Esc, pointer gone).
    pub(crate) fn end_pcb_carry(&mut self) {
        self.interaction_state.pcb_canvas.cancel_marquee();
        if self.interaction_state.pcb_carry.take().is_some() {
            self.refresh_pcb_highlight();
        }
    }

    /// The carry of `kind`, if it belongs to the board in view.
    fn live_carry(&self, kind: CarryKind) -> Option<PcbCarry> {
        let carry = self.interaction_state.pcb_carry.as_ref()?;
        let shown = self.active_tab_path();
        (carry.kind == kind && shown.as_ref() == Some(&carry.path)).then(|| carry.clone())
    }

    fn carry_to(&mut self, from: Point, to: Point) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let Some(carry) = self.interaction_state.pcb_carry.as_ref() else {
            return;
        };
        let delta = carry_delta(board, &carry.footprints, from, to);
        if let Some(carry) = self.interaction_state.pcb_carry.as_mut()
            && carry.delta != delta
        {
            carry.delta = delta;
            self.refresh_pcb_highlight();
        }
    }

    fn drop_pcb_carry(&mut self, from: Point, to: Point) {
        let Some(carry) = self.interaction_state.pcb_carry.clone() else {
            return;
        };
        let built = self.active_pcb().map(|board| {
            let delta = carry_delta(board, &carry.footprints, from, to);
            placement::move_commands(board, &carry.footprints, delta)
        });
        self.end_pcb_carry();
        if let Some(built) = built {
            self.run_placement("Move", built);
        }
    }

    /// Run a built placement edit on the board in view as one step, or say
    /// why not. Whether the board changed.
    fn run_placement(
        &mut self,
        what: &str,
        built: Result<Vec<BoardCommand>, PlacementRefusal>,
    ) -> bool {
        let commands = match built {
            Ok(commands) if commands.is_empty() => return false,
            Ok(commands) => commands,
            Err(refusal) => {
                refuse(what, &refusal);
                return false;
            }
        };
        let Some(path) = self.active_tab_path() else {
            return false;
        };
        match self.execute_board_commands(&path, commands, None) {
            Ok(patches) => {
                self.refresh_pcb_highlight();
                !patches.is_empty()
            }
            Err(error) => {
                crate::diagnostics::log_warning(format!("{what}: {error}; nothing changed"));
                false
            }
        }
    }

    /// The footprints a placement command would act on in the board in view.
    pub(crate) fn pcb_selected_footprints(&mut self) -> BTreeSet<Uuid> {
        self.with_pcb_selection(|state, _| state.selection().footprints())
            .unwrap_or_default()
    }
}

/// What the placement commands could act on in a window, for enablement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub(crate) struct PcbTargets {
    /// The window shows a board.
    pub shown: bool,
    /// Something on it is selected.
    pub any: bool,
    /// The selection names footprints and none is locked.
    pub placeable: bool,
    /// The selection names something to delete and no locked footprint.
    pub deletable: bool,
}

impl Signex {
    /// [`PcbTargets`] for a command from `window` (`None`: the main one).
    pub(crate) fn pcb_targets(&self, window: Option<iced::window::Id>) -> PcbTargets {
        use crate::app::state::WindowKind;
        let undocked = window.filter(|id| self.ui_state.main_window_id != Some(*id));
        let (board, path, state) = match undocked {
            Some(id) => {
                let Some(WindowKind::UndockedTab { path, .. }) = self.ui_state.windows.get(&id)
                else {
                    // A detached panel or modal acts on the main window's board.
                    return self.pcb_targets(None);
                };
                (
                    self.pcb_for_window(id),
                    Some(path.clone()),
                    self.interaction_state.pcb_selections.get(&id),
                )
            }
            None => (
                self.active_pcb(),
                self.active_tab_path(),
                Some(&self.interaction_state.pcb_selection),
            ),
        };
        let Some(board) = board else {
            return PcbTargets::default();
        };
        let selection = state
            .filter(|s| s.document().is_some() && s.document() == path.as_deref())
            .map(|s| s.selection());
        let Some(selection) = selection.filter(|s| !s.is_empty()) else {
            return PcbTargets {
                shown: true,
                ..PcbTargets::default()
            };
        };
        let footprints = selection.footprints();
        let unlocked = placement::editable_footprints(board, &footprints).is_ok();
        PcbTargets {
            shown: true,
            any: true,
            placeable: unlocked,
            deletable: (footprints.is_empty() || unlocked)
                && !deletable_items(selection).is_empty(),
        }
    }
}

/// What Delete removes for `selection`: its footprints (a selected pad's
/// included), tracks and vias.
pub(crate) fn deletable_items(selection: &pcb_geom::Selection) -> Vec<BoardItemId> {
    let mut items: Vec<BoardItemId> = selection
        .footprints()
        .into_iter()
        .map(BoardItemId::Footprint)
        .collect();
    items.extend(selection.items().filter_map(|item| match item {
        ItemRef::Track(id) => Some(BoardItemId::Segment(id)),
        ItemRef::Via(id) => Some(BoardItemId::Via(id)),
        ItemRef::Pad { .. } | ItemRef::Footprint(_) => None,
    }));
    items
}

fn refuse(what: &str, refusal: &PlacementRefusal) {
    crate::diagnostics::log_warning(format!("{what}: {refusal}; nothing changed"));
}
