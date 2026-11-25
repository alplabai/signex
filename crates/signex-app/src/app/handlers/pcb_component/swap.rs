//! Swap the footprint of components placed on a board (#2366), from the
//! Component view's Footprint Name button or `change_footprint` (the board
//! menu, the palette). The footprint picker opens with the components' own
//! library footprints first; the pick is read off the update thread into its
//! board form, then every selected footprint is replaced
//! (`signex_engine::pcb::footprint_swap`) and each linked schematic part takes
//! the same footprint (`linked_parts`), as one undo step. Old pads no new pad
//! takes over, and parts whose sheet is closed or read-only, are listed in
//! Messages; a refusal says why and changes nothing.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::pcb::footprint_swap::{self, UnmatchedPad};
use signex_i18n::{ids, t};
use signex_library::{PrimitiveKind, PrimitiveRef};
use signex_types::pcb::{Footprint, PcbBoard, SymbolLink};
use uuid::Uuid;

use super::linked_parts::{LinkedPlan, NotUpdated};
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{FootprintSwapMsg, Message, Signex};
use crate::library::messages::LibraryMessage;
use crate::library::place::resolve_library_footprint;
use crate::library::state::PrimitivePickerTarget;

impl Signex {
    pub(crate) fn handle_footprint_swap(&mut self, msg: FootprintSwapMsg) -> Task<Message> {
        match msg {
            FootprintSwapMsg::Requested => self.open_selection_footprint_swap(),
            FootprintSwapMsg::Picked {
                board,
                footprints,
                footprint,
            } => {
                self.ui_state.command_replay.note_swap_in_transit(true);
                self.read_swap_footprint(board, footprints, footprint)
            }
            FootprintSwapMsg::Resolved {
                board,
                footprints,
                result,
            } => {
                self.ui_state.command_replay.note_swap_in_transit(false);
                if let Ok(template) = &result {
                    self.ui_state
                        .command_replay
                        .note_footprint_swap(footprints.clone(), template.clone());
                }
                let before = self.board_revision(&board);
                self.swap_footprints(&board, &footprints, result);
                self.finish_footprint_swap_record(&board, before);
                Task::none()
            }
        }
    }

    /// `change_footprint`: the picker for the footprints selected on the
    /// board in view.
    fn open_selection_footprint_swap(&mut self) -> Task<Message> {
        let Some(board) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return Task::none();
        };
        let footprints: Vec<Uuid> = self.pcb_selected_footprints().into_iter().collect();
        if footprints.is_empty() {
            return Task::none();
        }
        // The picker opens on the next message; its row waits for it.
        self.ui_state.command_replay.note_swap_in_transit(true);
        self.open_footprint_swap_picker(&board, &footprints)
    }

    /// Open the footprint picker for `footprints` on `board`, listing first
    /// the footprints their library parts offer.
    pub(super) fn open_footprint_swap_picker(
        &mut self,
        board: &Path,
        footprints: &[Uuid],
    ) -> Task<Message> {
        let suggested = self
            .document_state
            .pcb_board(board)
            .map(|b| self.footprint_suggestions(b, footprints))
            .unwrap_or_default();
        Task::done(Message::Library(LibraryMessage::OpenPrimitivePicker {
            kind: PrimitiveKind::Footprint,
            target: PrimitivePickerTarget::BoardFootprint {
                board: board.to_path_buf(),
                footprints: footprints.to_vec(),
                suggested,
            },
        }))
    }

    /// The footprints, default then alternates, of every cached library part
    /// that offers the library footprint one of `footprints` came from.
    fn footprint_suggestions(&self, board: &PcbBoard, footprints: &[Uuid]) -> Vec<PrimitiveRef> {
        let sources: Vec<PrimitiveRef> = board
            .footprints
            .iter()
            .filter(|fp| footprints.contains(&fp.uuid))
            .filter_map(|fp| fp.library_ref.as_ref())
            .map(|source| PrimitiveRef::new(source.library, source.footprint))
            .collect();
        let mut suggested = Vec::new();
        let rows = self
            .library
            .open_libraries
            .iter()
            .flat_map(|lib| lib.tables.values().flatten());
        for row in rows.filter(|row| sources.iter().any(|s| row.binds_footprint(s))) {
            for binding in row.footprint_bindings() {
                if !suggested.contains(&binding.footprint) {
                    suggested.push(binding.footprint);
                }
            }
        }
        suggested
    }

    /// Read the picked footprint in its board form off the update thread.
    fn read_swap_footprint(
        &mut self,
        board: PathBuf,
        footprints: Vec<Uuid>,
        footprint: PrimitiveRef,
    ) -> Task<Message> {
        let open_libraries: Vec<(Uuid, PathBuf)> = self
            .library
            .open_libraries
            .iter()
            .map(|l| (l.library_id, l.root.clone()))
            .collect();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    resolve_library_footprint(&footprint, &open_libraries)
                })
                .await
                .unwrap_or_else(|e| Err(format!("the footprint read failed: {e}")))
            },
            move |result| {
                Message::Pcb(crate::app::PcbMsg::FootprintSwap(
                    FootprintSwapMsg::Resolved {
                        board: board.clone(),
                        footprints: footprints.clone(),
                        result: result.map(Box::new),
                    },
                ))
            },
        )
    }

    /// Replace `footprints` on `board` with the footprint read, and give
    /// their linked schematic parts the same footprint, as one undo step.
    /// The old pads that went with no match, and the parts whose sheet could
    /// not take the footprint, are listed in Messages.
    pub(crate) fn swap_footprints(
        &mut self,
        board: &Path,
        footprints: &[Uuid],
        result: Result<Box<Footprint>, String>,
    ) {
        let what = self.catalog(ids::PCB_COMPONENT_FOOTPRINT_SWAP);
        let template = match result {
            Ok(template) => *template,
            Err(why) => return self.refuse_swap(&what, why),
        };
        let Some(engine) = self.document_state.board_engine(board) else {
            let line =
                t!(self.ui_state.i18n, ids::PCB_FOOTPRINT_BOARD_GONE, "what" => what.as_str());
            crate::diagnostics::log_warning(line.into_string());
            return;
        };
        let revision = engine.revision();
        let ids: BTreeSet<Uuid> = footprints.iter().copied().collect();
        let swap = match footprint_swap::swap_commands(engine.board(), &ids, &template) {
            Ok(swap) if !swap.commands.is_empty() => swap,
            Ok(_) => return,
            Err(refusal) => return self.refuse_swap(&what, refusal.to_string()),
        };
        let links: Vec<(String, SymbolLink)> = engine
            .board()
            .footprints
            .iter()
            .filter(|fp| ids.contains(&fp.uuid))
            .filter_map(|fp| Some((fp.reference.clone(), fp.schematic_link.clone()?)))
            .collect();
        let plan = match &template.library_ref {
            Some(source) if !links.is_empty() => self.plan_linked_footprints(
                &links,
                PrimitiveRef::new(source.library, source.footprint),
            ),
            _ => LinkedPlan::default(),
        };
        let step = CheckedStep {
            label: &what,
            expected_revision: revision,
        };
        let ran = if plan.sheets.is_empty() {
            self.execute_board_commands(board, swap.commands, Some(step))
                .map_err(|error| self.refuse_swap(&what, error.to_string()))
                .is_ok()
        } else {
            let group = self.mint_undo_group();
            let changed = self.run_linked_footprints(plan.sheets, group, &what);
            match self.execute_board_commands_grouped(board, swap.commands, step, group) {
                Ok(_) => {
                    let kind = crate::app::UndoGroupKind::FootprintSwap;
                    self.commit_linked_footprints(changed, group, board.to_path_buf(), kind);
                    true
                }
                Err(error) => {
                    self.roll_back_linked_footprints(&changed, group, &what);
                    self.refuse_swap(&what, error.to_string());
                    false
                }
            }
        };
        if ran {
            for pad in &swap.unmatched {
                self.report_unmatched_pad(pad);
            }
            for (designator, why) in &plan.not_updated {
                self.report_part_not_updated(designator, *why);
            }
        }
        self.sync_pcb_board_summary();
    }

    fn refuse_swap(&self, what: &str, why: String) {
        let line = t!(
            self.ui_state.i18n,
            ids::PCB_FOOTPRINT_REFUSED,
            "what" => what,
            "why" => why
        );
        crate::diagnostics::log_warning(line.into_string());
    }

    fn report_part_not_updated(&self, designator: &str, why: NotUpdated) {
        let id = match why {
            NotUpdated::SheetNotOpen => ids::PCB_COMPONENT_SWAP_SHEET_NOT_OPEN,
            NotUpdated::SheetReadOnly => ids::PCB_COMPONENT_SWAP_SHEET_READ_ONLY,
        };
        let line = t!(self.ui_state.i18n, id, "designator" => designator);
        crate::diagnostics::log_warning(line.into_string());
    }

    fn report_unmatched_pad(&self, pad: &UnmatchedPad) {
        let i18n = &self.ui_state.i18n;
        let (designator, number) = (pad.reference.as_str(), pad.number.as_str());
        let line = match &pad.net {
            Some(net) => t!(
                i18n,
                ids::PCB_COMPONENT_SWAP_PAD_NET_DROPPED,
                "designator" => designator,
                "pad" => number,
                "net" => net.as_str()
            ),
            None => t!(
                i18n,
                ids::PCB_COMPONENT_SWAP_PAD_DROPPED,
                "designator" => designator,
                "pad" => number
            ),
        };
        crate::diagnostics::log_warning(line.into_string());
    }
}
