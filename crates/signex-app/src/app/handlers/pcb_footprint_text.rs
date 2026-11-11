//! A board footprint's designator and value, edited in the Properties panel
//! (#1312). Typing keeps a draft; Enter applies it as one board undo step
//! through `execute_board_commands`. The command comes from
//! `signex_engine::pcb::designators`, which refuses a locked footprint and
//! an empty designator; a refusal changes nothing and says why in the
//! Messages panel. A designator another footprint already has is applied
//! and warned about, as the schematic's Properties apply one.

use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use signex_engine::pcb::designators::{self, FootprintTextField};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::state::pcb_edit::CheckedStep;
use crate::panels::{FootprintEditability, PcbFootprintFacts, PcbFootprintTextMsg};

/// Text typed into a footprint's field and not yet applied.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbFootprintDraft {
    pub board: PathBuf,
    pub footprint: Uuid,
    pub field: FootprintTextField,
    pub text: String,
}

impl Signex {
    pub(crate) fn handle_pcb_footprint_text(&mut self, msg: &PcbFootprintTextMsg) {
        match msg {
            PcbFootprintTextMsg::Typed {
                board,
                footprint,
                field,
                text,
            } => {
                self.interaction_state.pcb_footprint_draft = Some(PcbFootprintDraft {
                    board: board.clone(),
                    footprint: *footprint,
                    field: *field,
                    text: text.clone(),
                });
            }
            PcbFootprintTextMsg::Submitted {
                board,
                footprint,
                field,
            } => {
                let draft = self.interaction_state.pcb_footprint_draft.take();
                if let Some(draft) = draft
                    .filter(|d| d.board == *board && d.footprint == *footprint && d.field == *field)
                {
                    self.apply_footprint_text(&draft);
                }
            }
        }
        self.sync_pcb_board_summary();
    }

    /// Run `draft` on its board as one labelled step, or say why not.
    fn apply_footprint_text(&mut self, draft: &PcbFootprintDraft) {
        let what = match draft.field {
            FootprintTextField::Designator => "Designator",
            FootprintTextField::Value => "Value",
        };
        let Some(engine) = self.document_state.board_engine(&draft.board) else {
            crate::diagnostics::log_warning(format!("{what}: the board is no longer open"));
            return;
        };
        let revision = engine.revision();
        let built = designators::text_edit_command(
            engine.board(),
            draft.footprint,
            draft.field,
            &draft.text,
        );
        let command = match built {
            Ok(Some(command)) => command,
            Ok(None) => return,
            Err(refusal) => {
                crate::diagnostics::log_warning(format!("{what}: {refusal}; nothing changed"));
                return;
            }
        };
        let step = CheckedStep {
            label: what,
            expected_revision: revision,
        };
        if let Err(error) = self.execute_board_commands(&draft.board, vec![command], Some(step)) {
            crate::diagnostics::log_warning(format!("{what}: {error}; nothing changed"));
            return;
        }
        if draft.field == FootprintTextField::Designator {
            self.warn_shared_designator(&draft.board, draft.footprint);
        }
    }

    /// Say in the Messages panel when the footprint's new designator is on
    /// other footprints too.
    fn warn_shared_designator(&self, path: &Path, footprint: Uuid) {
        let Some(board) = self.document_state.pcb_board(path) else {
            return;
        };
        let Some(fp) = board.footprints.iter().find(|fp| fp.uuid == footprint) else {
            return;
        };
        let others = designators::designator_holders(board, &fp.reference, footprint).len();
        if others > 0 {
            crate::diagnostics::log_warning(format!(
                "Designator: {} is also on {others} other footprint(s) of this board",
                fp.reference
            ));
        }
    }

    /// The Properties panel's facts of the one footprint selected on the
    /// board at `path`, with this footprint's draft if one is pending.
    pub(crate) fn pcb_footprint_facts(
        &self,
        path: &Path,
        board: &PcbBoard,
    ) -> Option<PcbFootprintFacts> {
        let selection = &self.interaction_state.pcb_selection;
        if selection.document() != Some(path) {
            return None;
        }
        let mut items = selection.selection().items();
        let (Some(ItemRef::Footprint(id)), None) = (items.next(), items.next()) else {
            return None;
        };
        let fp = board.footprints.iter().find(|fp| fp.uuid == id)?;
        let editability = if board.unknown.newer_minor_source.is_some() {
            FootprintEditability::ReadOnly
        } else if fp.locked {
            FootprintEditability::Locked
        } else {
            FootprintEditability::Editable
        };
        let draft = self
            .interaction_state
            .pcb_footprint_draft
            .as_ref()
            .filter(|d| d.board == path && d.footprint == id)
            .map(|d| (d.field, d.text.clone()));
        Some(PcbFootprintFacts {
            board: path.to_path_buf(),
            uuid: id,
            designator: fp.reference.clone(),
            value: fp.value.clone(),
            footprint_name: fp.footprint_id.clone(),
            draft,
            editability,
            also_on: designators::designator_holders(board, &fp.reference, id).len(),
        })
    }
}
