//! The board Component view's edits (#2354, #2365, #2369). Each edit builds
//! its commands with `signex_engine::pcb::component` or `component_edit`,
//! which refuse a locked footprint for everything but an unlock, and runs
//! them as one labelled board undo step through `execute_board_commands`. A
//! refusal changes nothing and says why in the Messages panel. Typed numbers
//! and parameter values are drafts until Enter, kept per field for the
//! footprints they were typed for.

mod edits;
mod facts;
mod identity;
mod linked_parts;
mod part_swap;
pub(crate) mod part_swap_operands;
#[cfg(test)]
pub(crate) mod part_swap_tests;
mod swap;

pub(crate) use swap::SwapRefusal;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::component::{self, ComponentRefusal, LocationField};
use signex_engine::pcb::component_edit;
use signex_engine::pcb::designators::FootprintTextField;
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb::PcbBoard;
use signex_widgets::property_illustration::IllustrationHover;
use uuid::Uuid;

use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::panels::{ComponentField, PcbComponentFacts, PcbComponentMsg};

/// The command the mismatch note's Update PCB button runs: Design ▸ Update
/// PCB Document, worded Import Changes From on a board.
const UPDATE_PCB_COMMAND: &str = "update_pcb_from_schematic";

/// The Component view's own state: the header's row filter, the text typed
/// and not applied yet, and the field or illustration part under the
/// pointer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PcbComponentPanelState {
    pub search: String,
    pub draft: Option<PcbComponentDraft>,
    pub illustration_hover: IllustrationHover,
}

/// Text typed into the Component view's fields for one set of footprints.
#[derive(Debug, Clone, PartialEq)]
pub struct PcbComponentDraft {
    pub board: PathBuf,
    /// The footprints the text was typed for, sorted.
    pub footprints: Vec<Uuid>,
    pub texts: BTreeMap<ComponentField, String>,
}

impl PcbComponentDraft {
    /// Whether this draft is for `footprints` on `board`.
    pub(crate) fn is_for(&self, board: &Path, footprints: &[Uuid]) -> bool {
        self.board == board && self.footprints == footprints
    }
}

impl Signex {
    pub(crate) fn handle_pcb_component(&mut self, msg: &PcbComponentMsg) -> Task<Message> {
        match msg {
            PcbComponentMsg::FootprintPickRequested { board, footprints } => {
                return self.open_footprint_swap_picker(board, footprints);
            }
            PcbComponentMsg::UpdatePcbPressed => {
                // The Design menu's command, so it is gated and worded the
                // same way.
                return match AppCommandId::new(UPDATE_PCB_COMMAND) {
                    Ok(command) => self.run_menu_command(&command, None),
                    Err(_) => Task::none(),
                };
            }
            // Only the hover changes: patch the shown facts instead of
            // rebuilding the board summary on every pointer crossing.
            PcbComponentMsg::IllustrationHovered(hover) => {
                let state = &mut self.interaction_state.pcb_component;
                state.illustration_hover.update(hover.clone());
                let hover = state.illustration_hover.clone();
                let shown = self.document_view.panel_ctx.pcb_board.as_mut();
                if let Some(facts) = shown.and_then(|b| b.component.as_mut()) {
                    facts.illustration_hover = hover;
                }
                return Task::none();
            }
            PcbComponentMsg::SearchChanged(query) => {
                self.interaction_state
                    .pcb_component
                    .search
                    .clone_from(query);
            }
            PcbComponentMsg::FieldTyped {
                board,
                footprints,
                field,
                text,
            } => {
                let slot = &mut self.interaction_state.pcb_component.draft;
                if !slot.as_ref().is_some_and(|d| d.is_for(board, footprints)) {
                    *slot = Some(PcbComponentDraft {
                        board: board.clone(),
                        footprints: footprints.clone(),
                        texts: BTreeMap::new(),
                    });
                }
                if let Some(draft) = slot.as_mut() {
                    draft.texts.insert(field.clone(), text.clone());
                }
            }
            PcbComponentMsg::FieldSubmitted {
                board,
                footprints,
                field,
            } => {
                // Only this field's text is taken; the others' stay.
                let slot = &mut self.interaction_state.pcb_component.draft;
                let typed = slot
                    .as_mut()
                    .filter(|d| d.is_for(board, footprints))
                    .and_then(|d| d.texts.remove(field));
                if slot.as_ref().is_some_and(|d| d.texts.is_empty()) {
                    *slot = None;
                }
                if let Some(text) = typed {
                    self.apply_component_field(board, footprints, field, &text);
                }
            }
            PcbComponentMsg::LockToggled {
                board,
                footprints,
                locked,
            } => {
                let what = self.catalog(ids::PCB_COMPONENT_LOCKED);
                self.run_component_edit(board, footprints, &what, |b, ids| {
                    component::lock_commands(b, ids, *locked)
                });
            }
            PcbComponentMsg::SidePicked {
                board,
                footprints,
                side,
            } => {
                let what = self.catalog(ids::PCB_COMPONENT_LAYER);
                self.run_component_edit(board, footprints, &what, |b, ids| {
                    component::side_commands(b, ids, *side)
                });
            }
            PcbComponentMsg::TextShownToggled {
                board,
                footprints,
                field,
                shown,
            } => {
                let what = self.catalog(match field {
                    FootprintTextField::Designator => ids::PCB_FOOTPRINT_DESIGNATOR,
                    FootprintTextField::Value => ids::PCB_COMPONENT_COMMENT,
                });
                self.run_component_edit(board, footprints, &what, |b, ids| {
                    component::text_shown_commands(b, ids, *field, *shown)
                });
            }
            PcbComponentMsg::SettingPicked {
                board,
                footprints,
                setting,
            } => {
                let what = self.catalog(edits::setting_label(setting));
                self.run_component_edit(board, footprints, &what, |b, ids| {
                    component_edit::setting_commands(b, ids, setting)
                });
            }
            PcbComponentMsg::ParameterAdded { board, footprints } => {
                self.add_component_parameter(board, footprints);
            }
            PcbComponentMsg::ParameterRemoved {
                board,
                footprints,
                key,
            } => {
                let what = t!(
                    self.ui_state.i18n,
                    ids::PCB_COMPONENT_PARAMETER_REMOVED,
                    "key" => key.as_str()
                )
                .into_string();
                self.run_component_edit(board, footprints, &what, |b, ids| {
                    component_edit::remove_parameter_commands(b, ids, key)
                });
            }
        }
        self.sync_pcb_board_summary();
        Task::none()
    }

    fn catalog(&self, id: MessageId) -> String {
        t!(self.ui_state.i18n, id).into_string()
    }

    /// Run typed `text` into `field` of `footprints` on `board`.
    fn apply_component_field(
        &mut self,
        board: &Path,
        footprints: &[Uuid],
        field: &ComponentField,
        text: &str,
    ) {
        match field {
            ComponentField::Location(location) => {
                let what = self.catalog(match location {
                    LocationField::X => ids::PCB_COMPONENT_X,
                    LocationField::Y => ids::PCB_COMPONENT_Y,
                    LocationField::Rotation => ids::PCB_COMPONENT_ROTATION,
                });
                self.run_component_edit(board, footprints, &what, |b, ids| {
                    let typed = component::parse_location(*location, text)?;
                    // X and Y are typed from the board origin, Y up (#2384).
                    let origin = b.options.origin;
                    let value = match location {
                        LocationField::X => origin.board_x(typed),
                        LocationField::Y => origin.board_y(typed),
                        LocationField::Rotation => typed,
                    };
                    component::location_commands(b, ids, *location, value)
                });
            }
            ComponentField::Parameter(key) => {
                self.run_component_edit(board, footprints, key, |b, ids| {
                    component::parameter_commands(b, ids, key, text)
                });
            }
            ComponentField::Setting(typed) => {
                let what = self.catalog(edits::typed_label(*typed));
                self.run_component_edit(board, footprints, &what, |b, ids| {
                    let setting = component_edit::parse_setting(*typed, text)?;
                    component_edit::setting_commands(b, ids, &setting)
                });
            }
            ComponentField::Comment => {
                let what = self.catalog(ids::PCB_COMPONENT_COMMENT);
                self.run_component_edit(board, footprints, &what, |b, ids| {
                    component_edit::comment_commands(b, ids, text)
                });
            }
            // A new parameter's fields are applied together by the Add
            // button (`ParameterAdded`), never one at a time.
            ComponentField::NewParameterName | ComponentField::NewParameterValue => {}
        }
    }

    /// Build the commands of one edit from the board at `board` and run them
    /// as one step labelled `what`, or say why not. Whether the step ran.
    fn run_component_edit(
        &mut self,
        board: &Path,
        footprints: &[Uuid],
        what: &str,
        build: impl FnOnce(&PcbBoard, &BTreeSet<Uuid>) -> Result<Vec<BoardCommand>, ComponentRefusal>,
    ) -> bool {
        let refused = |app: &Self, why: String| {
            let line = t!(
                app.ui_state.i18n,
                ids::PCB_FOOTPRINT_REFUSED,
                "what" => what,
                "why" => why
            );
            crate::diagnostics::log_warning(line.into_string());
        };
        let Some(engine) = self.document_state.board_engine(board) else {
            let line = t!(self.ui_state.i18n, ids::PCB_FOOTPRINT_BOARD_GONE, "what" => what);
            crate::diagnostics::log_warning(line.into_string());
            return false;
        };
        let revision = engine.revision();
        let ids: BTreeSet<Uuid> = footprints.iter().copied().collect();
        let commands = match build(engine.board(), &ids) {
            Ok(commands) => commands,
            Err(refusal) => {
                refused(self, refusal.to_string());
                return false;
            }
        };
        if commands.is_empty() {
            return false;
        }
        let step = CheckedStep {
            label: what,
            expected_revision: revision,
        };
        match self.execute_board_commands(board, commands, Some(step)) {
            Ok(_) => true,
            Err(error) => {
                refused(self, error.to_string());
                false
            }
        }
    }

    /// The pass after every update: a draft goes once its board is closed or
    /// no window shows it, or one of its footprints is gone.
    pub(crate) fn prune_pcb_component_draft(&mut self) {
        let Some(draft) = self.interaction_state.pcb_component.draft.as_ref() else {
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
        let footprints_there = docs.has_pcb_tab(&draft.board)
            && docs.pcb_board(&draft.board).is_some_and(|b| {
                draft
                    .footprints
                    .iter()
                    .all(|id| b.footprints.iter().any(|fp| fp.uuid == *id))
            });
        if !(footprints_there && (in_main || in_window)) {
            self.interaction_state.pcb_component.draft = None;
        }
    }

    /// The Component view of the board in view now shows `shown`: a draft
    /// typed for other footprints on that board goes, as the selection moved
    /// on. A draft of a board in another window stays.
    pub(crate) fn drop_moved_pcb_component_draft(&mut self, shown: Option<&PcbComponentFacts>) {
        let in_view = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|tab| tab.path.clone());
        let slot = &mut self.interaction_state.pcb_component.draft;
        if slot.as_ref().is_some_and(|d| {
            in_view.as_deref() == Some(d.board.as_path())
                && !shown.is_some_and(|s| d.is_for(&s.board, &s.footprints))
        }) {
            *slot = None;
        }
    }
}
