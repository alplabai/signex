//! The Layer Stack manager's events (#2401). A typed cell keeps a draft
//! until Enter; Enter, a Type pick, Add, Remove and Move are each one board
//! undo step: the edit is made on a copy of the stack
//! (`signex_engine::pcb::stack_edit`) and the result goes to the board as one
//! `SetLayerStack`. A refusal changes nothing and says why in Messages.
//! `set_layer_stack` (an agent, the CLI) runs the same step with a whole
//! stack. The Materials and Finish sub-tabs (mockup rows 176 and 177) edit
//! the stack the same way; the user material library is read and written
//! in tasks, off the update thread.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::pcb::drill_pairs::{drill_pairs, via_type_counts};
use signex_engine::pcb::stack_edit::{StackEdit, edit_stack};
use signex_engine::pcb::via_types::{ViaTypeDef, via_types};
use signex_engine::pcb::{BoardCommand, items_on_layer};
use signex_i18n::{I18n, ids, t};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, ViaType};
use signex_types::pcb_stack::{LayerStack, clean_stack_name};
use signex_types::project::{ProfileRef, resolve_profile};

use crate::app::new_project::profile_name_id;
use crate::app::state::ProjectId;
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex, TabKind};
use crate::panels::layer_stack::{
    CopperUse, DrillPairRow, FinishFacts, MaterialDraft, MaterialPick, StackSubTab, StackUnit,
    UserLibrary, ViaTypeRow, kind_label,
};
use crate::panels::pcb_primitive_properties::parse_number;
use crate::panels::{LayerStackFacts, LayerStackMsg, StackField};
use crate::pcb_canvas::layer_style;

mod materials;

use materials::materials_facts;

/// The manager's own state: the selected row, unapplied cell text and the
/// sub-tab, for the board they were made on, and the unit lengths read in.
/// View state: nothing here is saved.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerStackPanel {
    pub board: Option<PathBuf>,
    pub selected: Option<usize>,
    pub drafts: BTreeMap<(usize, StackField), String>,
    /// The stackup name as typed, until Enter.
    pub name_draft: Option<String>,
    pub unit: StackUnit,
    pub sub_tab: StackSubTab,
    /// The user material library, read once; not tied to a board.
    pub library: UserLibrary,
    pub library_write: LibraryWrite,
    /// The library entry picked in the Materials sub-tab.
    pub material: Option<MaterialPick>,
    pub material_draft: MaterialDraft,
    /// The mask thickness over laminate as typed, until Enter.
    pub finish_draft: Option<String>,
}

/// Writes of the user library, one at a time: an edit made while one is
/// running is owed and written when it ends, so the file ends up as the
/// library is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LibraryWrite {
    pub running: bool,
    pub owed: bool,
    /// Why the last write failed.
    pub error: Option<String>,
}

impl LayerStackPanel {
    /// Start over for `board` when the state was made for another one. The
    /// unit and the user library are the person's and carry over.
    fn follow(&mut self, board: &Path) {
        if self.board.as_deref() != Some(board) {
            *self = Self {
                board: Some(board.to_path_buf()),
                unit: self.unit,
                library: std::mem::take(&mut self.library),
                library_write: std::mem::take(&mut self.library_write),
                ..Self::default()
            };
        }
    }

    fn is_for(&self, board: &Path) -> bool {
        self.board.as_deref() == Some(board)
    }
}

impl Signex {
    /// One Layer Stack event; reading or writing the user material library
    /// comes back as a task.
    pub(crate) fn handle_layer_stack(&mut self, msg: &LayerStackMsg) -> Task<Message> {
        let task = match msg {
            LayerStackMsg::Materials(m) => self.handle_stack_materials(m),
            LayerStackMsg::Finish(f) => {
                self.handle_stack_finish(f);
                Task::none()
            }
            table => {
                self.handle_stack_table(table);
                self.read_user_library_when_shown()
            }
        };
        self.document_view.panel_ctx.layer_stack = self.layer_stack_facts();
        task
    }

    fn handle_stack_table(&mut self, msg: &LayerStackMsg) {
        use LayerStackMsg as M;
        match msg {
            M::RowSelected { board, row } => {
                let panel = &mut self.interaction_state.pcb_layer_stack;
                panel.follow(board);
                panel.selected = (panel.selected != Some(*row)).then_some(*row);
            }
            M::FieldTyped {
                board,
                row,
                field,
                text,
            } => {
                let panel = &mut self.interaction_state.pcb_layer_stack;
                panel.follow(board);
                panel.drafts.insert((*row, *field), text.clone());
            }
            M::FieldSubmitted { board, row, field } => {
                let panel = &mut self.interaction_state.pcb_layer_stack;
                let typed = (panel.board.as_deref() == Some(board.as_path()))
                    .then(|| panel.drafts.remove(&(*row, *field)))
                    .flatten();
                if let Some(text) = typed {
                    self.submit_stack_text(board, *row, *field, &text);
                }
            }
            M::KindPicked { board, row, kind } => {
                self.edit_layer_stack(
                    board,
                    &StackEdit::SetKind {
                        row: *row,
                        kind: *kind,
                    },
                );
            }
            M::LayerAdded { board, kind } => {
                let below = self.selected_stack_row(board);
                self.edit_layer_stack(board, &StackEdit::Add { below, kind: *kind });
            }
            M::RowRemoved { board } => {
                if let Some(row) = self.selected_stack_row(board)
                    && self.edit_layer_stack(board, &StackEdit::Remove { row })
                {
                    self.interaction_state.pcb_layer_stack.selected = None;
                }
            }
            M::RowMoved { board, up } => {
                if let Some(row) = self.selected_stack_row(board)
                    && self.edit_layer_stack(board, &StackEdit::Move { row, up: *up })
                {
                    let moved = if *up { row - 1 } else { row + 1 };
                    self.interaction_state.pcb_layer_stack.selected = Some(moved);
                }
            }
            M::StackupNameTyped { board, text } => {
                let panel = &mut self.interaction_state.pcb_layer_stack;
                panel.follow(board);
                panel.name_draft = Some(text.clone());
            }
            M::StackupNameSubmitted { board } => {
                let panel = &mut self.interaction_state.pcb_layer_stack;
                let typed = (panel.board.as_deref() == Some(board.as_path()))
                    .then(|| panel.name_draft.take())
                    .flatten();
                if let Some(text) = typed {
                    self.submit_stackup_name(board, &text);
                }
            }
            M::UnitPicked { board, unit } => {
                let panel = &mut self.interaction_state.pcb_layer_stack;
                panel.follow(board);
                panel.unit = *unit;
                // A typed length was in the old unit.
                panel
                    .drafts
                    .retain(|(_, field), _| *field != StackField::Thickness);
                panel.finish_draft = None;
            }
            M::SubTabPicked { board, tab } => {
                let panel = &mut self.interaction_state.pcb_layer_stack;
                panel.follow(board);
                panel.sub_tab = *tab;
            }
            // Routed by `handle_layer_stack`.
            M::Materials(_) | M::Finish(_) => {}
        }
    }

    fn selected_stack_row(&self, board: &Path) -> Option<usize> {
        let panel = &self.interaction_state.pcb_layer_stack;
        (panel.board.as_deref() == Some(board))
            .then_some(panel.selected)
            .flatten()
    }

    /// Enter in a cell: read the typed text and apply it.
    fn submit_stack_text(&mut self, board: &Path, row: usize, field: StackField, text: &str) {
        let number = || parse_number(text);
        let edit = match field {
            StackField::Name => Some(StackEdit::SetName {
                row,
                text: text.to_string(),
            }),
            StackField::Material => Some(StackEdit::SetMaterial {
                row,
                text: text.to_string(),
            }),
            StackField::Thickness => {
                let unit = self.interaction_state.pcb_layer_stack.unit;
                number().map(|value| StackEdit::SetThickness {
                    row,
                    mm: unit.to_mm(value),
                })
            }
            StackField::DielectricConstant => {
                number().map(|value| StackEdit::SetDielectricConstant { row, value })
            }
            StackField::CopperWeight => number().map(|oz| StackEdit::SetCopperWeight { row, oz }),
            StackField::LossTangent => {
                number().map(|value| StackEdit::SetLossTangent { row, value })
            }
        };
        match edit {
            Some(edit) => {
                self.edit_layer_stack(board, &edit);
            }
            None => {
                let why = t!(self.ui_state.i18n, ids::PCB_STACK_NOT_A_NUMBER, "text" => text);
                self.refuse_stack_edit(why.into_string());
            }
        }
    }

    /// Enter in the stackup name: one undo step, none when it is unchanged.
    fn submit_stackup_name(&mut self, board: &Path, text: &str) {
        let name = clean_stack_name(text);
        let current = self
            .document_state
            .pcb_board(board)
            .and_then(|b| b.stackup.as_ref())
            .and_then(|stack| stack.name.clone());
        if name == current {
            return;
        }
        let label = t!(self.ui_state.i18n, ids::PCB_STACK_EDIT).into_string();
        let command = BoardCommand::SetStackupName { name };
        if let Err(why) = self.run_board_step(board, vec![command], &label) {
            self.refuse_stack_edit(why);
        }
    }

    /// Make `edit` on the stack the manager shows for `board` and store the
    /// result as one step. Whether the board took it.
    fn edit_layer_stack(&mut self, board: &Path, edit: &StackEdit) -> bool {
        let Some(current) = self.document_state.pcb_board(board).map(shown_stack) else {
            return false;
        };
        let stack = match edit_stack(&current, edit) {
            Ok(stack) => stack,
            Err(refusal) => {
                self.refuse_stack_edit(refusal.to_string());
                return false;
            }
        };
        match self.apply_layer_stack(board, stack) {
            Ok(_) => true,
            Err(why) => {
                self.refuse_stack_edit(why);
                false
            }
        }
    }

    /// Store `stack` on the board at `path` as one undo step; the stack the
    /// board then holds, or why it was refused. The one path the manager and
    /// `set_layer_stack` share.
    pub(crate) fn apply_layer_stack(
        &mut self,
        path: &Path,
        stack: LayerStack,
    ) -> Result<LayerStack, String> {
        let label = t!(self.ui_state.i18n, ids::PCB_STACK_EDIT).into_string();
        let command = BoardCommand::SetLayerStack {
            stack: Box::new(stack),
        };
        self.run_board_step(path, vec![command], &label)?;
        self.document_state
            .pcb_board(path)
            .and_then(|b| b.stackup.clone())
            .ok_or_else(|| label.clone())
    }

    /// Run `commands` on the board at `path` as one step labelled `label`,
    /// all or nothing. Refused for a board that is not open or opened
    /// read-only; the refusal reads as the reason.
    pub(crate) fn run_board_step(
        &mut self,
        path: &Path,
        commands: Vec<BoardCommand>,
        label: &str,
    ) -> Result<(), String> {
        let i18n = &self.ui_state.i18n;
        let Some(engine) = self.document_state.board_engine(path) else {
            return Err(
                t!(i18n, ids::PCB_PRIM_BOARD_GONE, "what" => label.to_string()).into_string(),
            );
        };
        if engine.board().unknown.newer_minor_source.is_some() {
            return Err(t!(i18n, ids::PCB_PRIM_READ_ONLY).into_string());
        }
        let step = CheckedStep {
            label,
            expected_revision: engine.revision(),
        };
        self.execute_board_commands(path, commands, Some(step))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn refuse_stack_edit(&self, why: String) {
        let line = t!(self.ui_state.i18n, ids::PCB_STACK_REFUSED, "why" => why);
        crate::diagnostics::log_warning(line.into_string());
    }

    /// The manager's facts for the board in view, on its PCB tab or its
    /// Layer Stack tab; `None` without one.
    pub(crate) fn layer_stack_facts(&self) -> Option<LayerStackFacts> {
        let docs = &self.document_state;
        let tab = docs.tabs.get(docs.active_tab)?;
        if !matches!(tab.kind, TabKind::Pcb | TabKind::LayerStack) {
            return None;
        }
        let path = tab.path.clone();
        let board = docs.pcb_board(&path)?;
        let stack = shown_stack(board);
        let i18n = &*self.ui_state.i18n;
        let names = row_names(i18n, &stack);
        let panel = &self.interaction_state.pcb_layer_stack;
        let ours = panel.board.as_deref() == Some(path.as_path());
        Some(LayerStackFacts {
            stored: board.stackup.is_some(),
            read_only: board.unknown.newer_minor_source.is_some(),
            selected: panel
                .selected
                .filter(|row| ours && *row < stack.layers.len()),
            drafts: if ours {
                panel.drafts.clone()
            } else {
                BTreeMap::new()
            },
            name_draft: panel.name_draft.clone().filter(|_| ours),
            unit: panel.unit,
            sub_tab: if ours {
                panel.sub_tab
            } else {
                StackSubTab::default()
            },
            drill_pairs: drill_pair_rows(i18n, board, &stack, &names),
            via_types: via_type_rows(i18n, board),
            has_project: tab.project_id.is_some(),
            copper_use: copper_use(board, &stack),
            fab_profile: tab.project_id.and_then(|id| self.fab_profile_name(id)),
            materials: materials_facts(panel, ours),
            finish: FinishFacts {
                finish: stack.shown_finish(),
                stored: stack.finish.is_some(),
                draft: panel.finish_draft.clone().filter(|_| ours),
            },
            names,
            stack,
            board: path,
        })
    }

    /// The project's fab profile as the New Project cards name it: a tier
    /// by its catalog name, a saved profile by its own. `None` when the
    /// project names none or one that is gone.
    fn fab_profile_name(&self, id: ProjectId) -> Option<String> {
        let project = self.document_state.project_by_id(id)?;
        let token = project.data.project_defaults.profile.as_deref()?;
        let name = match resolve_profile(token, &self.document_state.setup_profiles)? {
            ProfileRef::BuiltIn(tier) | ProfileRef::Edited { profile: tier, .. } => {
                t!(self.ui_state.i18n, profile_name_id(tier)).into_string()
            }
            ProfileRef::User(profile) => profile.name.clone(),
        };
        Some(name)
    }
}

/// Per row, what sits on a copper row's board layer; `None` on other rows.
fn copper_use(board: &PcbBoard, stack: &LayerStack) -> Vec<Option<CopperUse>> {
    stack
        .layers
        .iter()
        .map(|row| {
            if !row.kind.is_copper() {
                return None;
            }
            if row.layer.is_empty() {
                return Some(CopperUse::default());
            }
            let nets: BTreeSet<String> = board
                .zones
                .iter()
                .filter(|z| z.layer == row.layer && !z.net_name.is_empty())
                .map(|z| z.net_name.clone())
                .collect();
            Some(CopperUse {
                items: items_on_layer(board, &row.layer),
                nets: nets.into_iter().collect(),
            })
        })
        .collect()
}

/// The name each row shows: its own, else its copper layer's or kind's.
fn row_names(i18n: &I18n, stack: &LayerStack) -> Vec<String> {
    stack
        .layers
        .iter()
        .map(|layer| {
            if !layer.name.is_empty() {
                return layer.name.clone();
            }
            let copper = parse_pcb_layer(&layer.layer).and_then(|l| l.copper());
            match copper {
                Some(c) if layer.kind.is_copper() => {
                    layer_style::copper_name(i18n, c).into_string()
                }
                _ => kind_label(i18n, layer.kind).into_string(),
            }
        })
        .collect()
}

fn via_kind_word(i18n: &I18n, kind: ViaType) -> String {
    let id = match kind {
        ViaType::Through => ids::PCB_PRIM_VIA_KIND_THROUGH,
        ViaType::Blind => ids::PCB_PRIM_VIA_KIND_BLIND,
        ViaType::Micro => ids::PCB_PRIM_VIA_KIND_MICRO,
    };
    t!(i18n, id).into_string()
}

/// A drill pair's name, "Through 1–4".
fn pair_name(i18n: &I18n, kind: ViaType, (from, to): (usize, usize)) -> String {
    let word = via_kind_word(i18n, kind);
    t!(i18n, ids::PCB_PRIM_VIA_DRILL_PAIR, "kind" => word, "from" => from, "to" => to).into_string()
}

/// The Drill pairs table: `names` gives each copper end its row's name.
fn drill_pair_rows(
    i18n: &I18n,
    board: &PcbBoard,
    stack: &LayerStack,
    names: &[String],
) -> Vec<DrillPairRow> {
    let copper: Vec<&String> = stack
        .layers
        .iter()
        .zip(names)
        .filter(|(layer, _)| layer.kind.is_copper())
        .map(|(_, name)| name)
        .collect();
    let end = |place: usize| {
        place
            .checked_sub(1)
            .and_then(|index| copper.get(index))
            .map_or_else(String::new, |name| (*name).clone())
    };
    drill_pairs(board, stack)
        .into_iter()
        .map(|pair| DrillPairRow {
            name: pair_name(i18n, pair.kind, pair.span),
            from: end(pair.span.0),
            to: end(pair.span.1),
            kind: via_kind_word(i18n, pair.kind),
            span: pair.span,
            depth_mm: pair.depth_mm,
            min_drill_mm: pair.min_drill,
            aspect_ratio: pair.aspect_ratio(),
            vias: pair.vias,
        })
        .collect()
}

/// The Via types table, from the types the Via panel picks.
fn via_type_rows(i18n: &I18n, board: &PcbBoard) -> Vec<ViaTypeRow> {
    let types = via_types(board);
    let counts = via_type_counts(board, &types);
    types
        .iter()
        .zip(counts)
        .map(|(def, used)| {
            let protection = def.protection.class().code().map_or_else(
                || t!(i18n, ids::PCB_STACK_NONE).into_string(),
                str::to_string,
            );
            let (from, to) = def.span;
            let span = match def.kind {
                ViaType::Through => ids::PCB_PRIM_VIA_SPAN_THROUGH,
                ViaType::Blind => ids::PCB_PRIM_VIA_SPAN_BLIND,
                ViaType::Micro => ids::PCB_PRIM_VIA_SPAN_MICRO,
            };
            ViaTypeRow {
                name: t!(i18n, span, "from" => from, "to" => to).into_string(),
                pair: pair_name(i18n, def.kind, def.span),
                drill_mm: def.drill,
                finished_mm: ViaTypeDef::finished_hole(def.drill),
                pad_mm: def.diameter,
                protection,
                used,
            }
        })
        .collect()
}

/// The stack a board shows: its stored one, else the one its layers make.
pub(crate) fn shown_stack(board: &PcbBoard) -> LayerStack {
    board
        .stackup
        .clone()
        .unwrap_or_else(|| LayerStack::derived(&board.layers, board.thickness))
}

#[cfg(test)]
#[path = "pcb_layer_stack_tests.rs"]
mod tests;
