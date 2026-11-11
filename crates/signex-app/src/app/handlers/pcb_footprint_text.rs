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
use signex_i18n::{ids, t};

/// Text typed into a footprint's fields and not yet applied, one draft per
/// field, so typing in one field never loses the other's text.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbFootprintDraft {
    pub board: PathBuf,
    pub footprint: Uuid,
    pub designator: Option<String>,
    pub value: Option<String>,
}

impl PcbFootprintDraft {
    /// Whether this draft is for `footprint` on `board`.
    pub(crate) fn is_for(&self, board: &Path, footprint: Uuid) -> bool {
        self.board == board && self.footprint == footprint
    }

    /// The typed text of `field`, if any.
    pub(crate) fn text(&self, field: FootprintTextField) -> Option<&String> {
        match field {
            FootprintTextField::Designator => self.designator.as_ref(),
            FootprintTextField::Value => self.value.as_ref(),
        }
    }

    fn slot(&mut self, field: FootprintTextField) -> &mut Option<String> {
        match field {
            FootprintTextField::Designator => &mut self.designator,
            FootprintTextField::Value => &mut self.value,
        }
    }
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
                let slot = &mut self.interaction_state.pcb_footprint_draft;
                if !slot.as_ref().is_some_and(|d| d.is_for(board, *footprint)) {
                    *slot = Some(PcbFootprintDraft {
                        board: board.clone(),
                        footprint: *footprint,
                        designator: None,
                        value: None,
                    });
                }
                if let Some(draft) = slot.as_mut() {
                    *draft.slot(*field) = Some(text.clone());
                }
            }
            PcbFootprintTextMsg::Submitted {
                board,
                footprint,
                field,
            } => {
                // Only this field's text is taken; the other field's stays.
                let typed = self
                    .interaction_state
                    .pcb_footprint_draft
                    .as_mut()
                    .filter(|d| d.is_for(board, *footprint))
                    .and_then(|d| d.slot(*field).take());
                if self
                    .interaction_state
                    .pcb_footprint_draft
                    .as_ref()
                    .is_some_and(|d| d.designator.is_none() && d.value.is_none())
                {
                    self.interaction_state.pcb_footprint_draft = None;
                }
                if let Some(text) = typed {
                    self.apply_footprint_text(board, *footprint, *field, &text);
                }
            }
        }
        self.sync_pcb_board_summary();
    }

    /// Run `text` into `field` of `footprint` on `board` as one labelled
    /// step, or say why not.
    fn apply_footprint_text(
        &mut self,
        board: &Path,
        footprint: Uuid,
        field: FootprintTextField,
        text: &str,
    ) {
        let what = t!(
            self.ui_state.i18n,
            match field {
                FootprintTextField::Designator => ids::PCB_FOOTPRINT_DESIGNATOR,
                FootprintTextField::Value => ids::PCB_FOOTPRINT_VALUE,
            }
        )
        .into_string();
        let refused = |app: &Self, why: String| {
            let line = t!(
                app.ui_state.i18n,
                ids::PCB_FOOTPRINT_REFUSED,
                "what" => what.as_str(),
                "why" => why
            );
            crate::diagnostics::log_warning(line.into_string());
        };
        let Some(engine) = self.document_state.board_engine(board) else {
            let line =
                t!(self.ui_state.i18n, ids::PCB_FOOTPRINT_BOARD_GONE, "what" => what.as_str());
            crate::diagnostics::log_warning(line.into_string());
            return;
        };
        let revision = engine.revision();
        let built = designators::text_edit_command(engine.board(), footprint, field, text);
        let command = match built {
            Ok(Some(command)) => command,
            Ok(None) => return,
            Err(refusal) => {
                refused(self, refusal.to_string());
                return;
            }
        };
        let step = CheckedStep {
            label: &what,
            expected_revision: revision,
        };
        if let Err(error) = self.execute_board_commands(board, vec![command], Some(step)) {
            refused(self, error.to_string());
            return;
        }
        if field == FootprintTextField::Designator {
            self.warn_shared_designator(board, footprint);
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
            let line = t!(
                self.ui_state.i18n,
                ids::PCB_FOOTPRINT_SHARED_NOTICE,
                "designator" => fp.reference.as_str(),
                "count" => others
            );
            crate::diagnostics::log_warning(line.into_string());
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
            .filter(|d| d.is_for(path, id));
        Some(PcbFootprintFacts {
            board: path.to_path_buf(),
            uuid: id,
            designator: fp.reference.clone(),
            value: fp.value.clone(),
            footprint_name: fp.footprint_id.clone(),
            designator_draft: draft.and_then(|d| d.text(FootprintTextField::Designator).cloned()),
            value_draft: draft.and_then(|d| d.text(FootprintTextField::Value).cloned()),
            editability,
            also_on: designators::designator_holders(board, &fp.reference, id).len(),
        })
    }
}

impl Signex {
    /// The pass after every update: an unapplied draft goes once its board
    /// is closed or no window shows it (a tab switch), or its footprint is
    /// gone, so Enter can never apply text typed for a board left behind.
    pub(crate) fn prune_pcb_footprint_draft(&mut self) {
        let Some(draft) = self.interaction_state.pcb_footprint_draft.as_ref() else {
            return;
        };
        let docs = &self.document_state;
        let in_main = docs
            .tabs
            .get(docs.active_tab)
            .is_some_and(|tab| tab.path == draft.board);
        let in_window = self.ui_state.windows.values().any(|kind| {
            matches!(
                kind,
                crate::app::state::WindowKind::UndockedTab { path, .. } if *path == draft.board
            )
        });
        let footprint_there = docs.has_pcb_tab(&draft.board)
            && docs
                .pcb_board(&draft.board)
                .is_some_and(|b| b.footprints.iter().any(|fp| fp.uuid == draft.footprint));
        if !(footprint_there && (in_main || in_window)) {
            self.interaction_state.pcb_footprint_draft = None;
        }
    }

    /// An undo or redo moved the board at `path`: the text a draft was typed
    /// over may have changed under it, so its draft goes.
    pub(crate) fn drop_pcb_footprint_draft_of(&mut self, path: &Path) {
        if self
            .interaction_state
            .pcb_footprint_draft
            .as_ref()
            .is_some_and(|d| d.board == path)
        {
            self.interaction_state.pcb_footprint_draft = None;
        }
    }
}
