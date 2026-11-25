//! The Materials and Finish sub-tabs of the Layer Stack (mockup rows 176
//! and 177). A material pick or a finish change is one board step through
//! the same stack edit as the table; the user material library is read and
//! written in tasks, one write at a time.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::pcb::stack_edit::StackEdit;
use signex_i18n::{ids, t};
use signex_types::material_library::{
    Material, USER_MATERIALS_FILE, add_user_material, builtin_materials, read_user_materials,
    write_user_materials,
};
use signex_types::pcb_stack::BoardFinish;

use super::{LayerStackPanel, shown_stack};
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::panels::layer_stack::{
    FinishMsg, MaterialDraft, MaterialPick, MaterialSource, MaterialsFacts, MaterialsMsg,
    StackSubTab, StackUnit, UserLibrary,
};
use crate::panels::pcb_primitive_properties::parse_number;
use crate::panels::{LayerStackMsg, PanelMsg};

impl Signex {
    pub(super) fn handle_stack_materials(&mut self, msg: &MaterialsMsg) -> Task<Message> {
        use MaterialsMsg as M;
        let panel = &mut self.interaction_state.pcb_layer_stack;
        match msg {
            M::Selected { board, pick } => {
                panel.follow(board);
                panel.material = (panel.material != Some(*pick)).then_some(*pick);
            }
            M::Used { board } => self.use_picked_material(board),
            M::DraftTyped { board, field, text } => {
                panel.follow(board);
                *panel.material_draft.text_mut(*field) = text.clone();
            }
            M::DraftKindPicked { board, kind } => {
                panel.follow(board);
                panel.material_draft.kind = *kind;
            }
            M::Added { board } => return self.add_drafted_material(board),
            M::Removed { board } => return self.remove_picked_material(board),
            M::UserLibraryRead(result) => {
                if panel.library == UserLibrary::Reading {
                    panel.library = match result {
                        Ok(entries) => UserLibrary::Read(entries.clone()),
                        Err(why) => UserLibrary::Failed(why.clone()),
                    };
                }
            }
            M::UserLibraryWritten(result) => {
                let write = &mut panel.library_write;
                write.running = false;
                write.error = result.clone().err();
                if std::mem::take(&mut write.owed) {
                    return self.save_user_library();
                }
            }
        }
        Task::none()
    }

    /// Read the user library the first time the Materials sub-tab shows.
    pub(super) fn read_user_library_when_shown(&mut self) -> Task<Message> {
        let panel = &mut self.interaction_state.pcb_layer_stack;
        if panel.sub_tab != StackSubTab::Materials || panel.library != UserLibrary::NotRead {
            return Task::none();
        }
        let Some(path) = user_materials_path() else {
            let why = t!(self.ui_state.i18n, ids::PCB_STACK_MATERIAL_NO_CONFIG);
            panel.library = UserLibrary::Failed(why.into_string());
            return Task::none();
        };
        panel.library = UserLibrary::Reading;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    read_user_materials(&path).map_err(|e| e.to_string())
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
            },
            |result| materials_message(MaterialsMsg::UserLibraryRead(result)),
        )
    }

    /// Write the user library as it is now, or owe the write while one runs.
    fn save_user_library(&mut self) -> Task<Message> {
        let panel = &mut self.interaction_state.pcb_layer_stack;
        let UserLibrary::Read(entries) = &panel.library else {
            return Task::none();
        };
        if panel.library_write.running {
            panel.library_write.owed = true;
            return Task::none();
        }
        let Some(path) = user_materials_path() else {
            let why = t!(self.ui_state.i18n, ids::PCB_STACK_MATERIAL_NO_CONFIG);
            panel.library_write.error = Some(why.into_string());
            return Task::none();
        };
        let entries = entries.clone();
        panel.library_write.running = true;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    write_user_materials(&path, &entries).map_err(|e| e.to_string())
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
            },
            |result| materials_message(MaterialsMsg::UserLibraryWritten(result)),
        )
    }

    /// "Use for selected layer": copy the picked entry into the selected row
    /// as one board step.
    fn use_picked_material(&mut self, board: &Path) {
        let panel = &self.interaction_state.pcb_layer_stack;
        if !panel.is_for(board) {
            return;
        }
        let facts = materials_facts(panel, true);
        let (Some(row), Some(material)) = (panel.selected, facts.picked().cloned()) else {
            return;
        };
        self.edit_layer_stack(board, &StackEdit::UseMaterial { row, material });
    }

    /// Add material: the draft, read in the tab's unit, joins the user
    /// library and is written; a bad draft is refused with its reason.
    fn add_drafted_material(&mut self, board: &Path) -> Task<Message> {
        let panel = &self.interaction_state.pcb_layer_stack;
        let UserLibrary::Read(entries) = &panel.library else {
            return Task::none();
        };
        if !panel.is_for(board) {
            return Task::none();
        }
        let added = draft_material(&panel.material_draft, panel.unit)
            .map_err(|text| {
                t!(self.ui_state.i18n, ids::PCB_STACK_NOT_A_NUMBER, "text" => text).into_string()
            })
            .and_then(|entry| add_user_material(entries, entry).map_err(|f| f.to_string()));
        match added {
            Ok(library) => {
                let panel = &mut self.interaction_state.pcb_layer_stack;
                panel.material = Some(MaterialPick {
                    source: MaterialSource::User,
                    index: library.len() - 1,
                });
                panel.library = UserLibrary::Read(library);
                panel.material_draft = MaterialDraft::default();
                self.save_user_library()
            }
            Err(why) => {
                let line = t!(self.ui_state.i18n, ids::PCB_STACK_MATERIAL_REFUSED, "why" => why);
                crate::diagnostics::log_warning(line.into_string());
                Task::none()
            }
        }
    }

    /// Remove the picked user entry and write the library. Boards keep the
    /// values they copied.
    fn remove_picked_material(&mut self, board: &Path) -> Task<Message> {
        let panel = &mut self.interaction_state.pcb_layer_stack;
        let Some(MaterialPick {
            source: MaterialSource::User,
            index,
        }) = panel.material.filter(|_| panel.is_for(board))
        else {
            return Task::none();
        };
        let UserLibrary::Read(entries) = &mut panel.library else {
            return Task::none();
        };
        if index >= entries.len() {
            return Task::none();
        }
        entries.remove(index);
        panel.material = None;
        self.save_user_library()
    }

    pub(super) fn handle_stack_finish(&mut self, msg: &FinishMsg) {
        let panel = &mut self.interaction_state.pcb_layer_stack;
        match msg {
            FinishMsg::Picked { board, finish } => self.set_board_finish(board, finish.clone()),
            FinishMsg::OverLaminateTyped { board, text } => {
                panel.follow(board);
                panel.finish_draft = Some(text.clone());
            }
            FinishMsg::OverLaminateSubmitted { board } => {
                let typed = panel
                    .is_for(board)
                    .then(|| panel.finish_draft.take())
                    .flatten();
                if let Some(text) = typed {
                    self.submit_over_laminate(board, &text);
                }
            }
        }
    }

    fn submit_over_laminate(&mut self, board: &Path, text: &str) {
        let Some(value) = parse_number(text) else {
            let why = t!(self.ui_state.i18n, ids::PCB_STACK_NOT_A_NUMBER, "text" => text);
            self.refuse_stack_edit(why.into_string());
            return;
        };
        let Some(mut finish) = self
            .document_state
            .pcb_board(board)
            .map(|b| shown_stack(b).shown_finish())
        else {
            return;
        };
        let unit = self.interaction_state.pcb_layer_stack.unit;
        finish.mask.over_laminate_mm = unit.to_mm(value);
        self.set_board_finish(board, finish);
    }

    /// Store `finish` on the board as one step; nothing when it is what the
    /// board shows already.
    fn set_board_finish(&mut self, board: &Path, finish: BoardFinish) {
        let shown = self
            .document_state
            .pcb_board(board)
            .map(|b| shown_stack(b).shown_finish());
        if shown.is_some_and(|shown| shown != finish) {
            self.edit_layer_stack(board, &StackEdit::SetFinish { finish });
        }
    }
}

/// The user library's file in the config folder.
fn user_materials_path() -> Option<PathBuf> {
    crate::config_root::config_root().map(|root| root.join(USER_MATERIALS_FILE))
}

fn materials_message(msg: MaterialsMsg) -> Message {
    Message::Dock(DockMessage::Panel(PanelMsg::LayerStack(
        LayerStackMsg::Materials(msg),
    )))
}

/// The Add material draft as an entry, its lengths read in `unit`; the text
/// that is not a number when one is not. Dk and Df left empty are 0.
fn draft_material(draft: &MaterialDraft, unit: StackUnit) -> Result<Material, String> {
    let number = |text: &str, empty_is_zero: bool| {
        if empty_is_zero && text.trim().is_empty() {
            return Ok(0.0);
        }
        parse_number(text).ok_or_else(|| text.to_string())
    };
    Ok(Material {
        name: draft.name.trim().to_string(),
        kind: draft.kind,
        dielectric_constant: number(&draft.dielectric_constant, true)?,
        loss_tangent: number(&draft.loss_tangent, true)?,
        thickness_mm: unit.to_mm(number(&draft.thickness, false)?),
    })
}

/// The Materials sub-tab's facts; the pick and the draft only for the board
/// they were made on.
pub(super) fn materials_facts(panel: &LayerStackPanel, ours: bool) -> MaterialsFacts {
    MaterialsFacts {
        built_in: builtin_materials(),
        library: panel.library.clone(),
        selected: panel.material.filter(|_| ours),
        draft: if ours {
            panel.material_draft.clone()
        } else {
            MaterialDraft::default()
        },
        write_error: panel.library_write.error.clone(),
    }
}
