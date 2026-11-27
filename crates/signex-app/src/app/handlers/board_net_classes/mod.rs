//! Net classes of a board (#2400). A board has no table of its own: it
//! reads the net-class table of its project (`ProjectData::net_classes`),
//! the one the schematic's net classes and DRC read, so Update PCB has
//! nothing to copy. Every edit is a `signex_engine::NetClassCommand`, one
//! step of the project's net-class history; the `.snxprj` is marked dirty
//! and written only by an explicit save. One path for the panel, the
//! palette, the CLI and the agent socket.

use std::path::Path;

use serde_json::{Value, json};
use signex_engine::NetClassCommand;
use signex_i18n::{ids, t};
use signex_types::net_class::{NetClass, NetClassBoardRules, NetClassStyle, parse_hex_color};

use crate::app::{ProjectId, Signex};
use crate::panels::{
    BoardNetClassDraft, BoardNetClassField, BoardNetClassMsg, BoardNetClassView, NetClassPanel,
};

/// The first `Class <n>` no class of `classes` has.
pub(crate) fn unused_class_name(classes: &[NetClass]) -> String {
    (1..)
        .map(|n| format!("Class {n}"))
        .find(|name| classes.iter().all(|c| &c.name != name))
        .unwrap_or_default()
}

/// A typed length in mm: empty is unset (`0.0`); `None` for text that is
/// not a number.
pub(crate) fn typed_mm(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return Some(0.0);
    }
    text.parse::<f64>().ok().filter(|mm| mm.is_finite())
}

/// `rules` with `field` set to `mm`; the name and colour cells are not
/// rules and leave them as they are.
pub(crate) fn with_rule(
    mut rules: NetClassBoardRules,
    field: BoardNetClassField,
    mm: f64,
) -> NetClassBoardRules {
    match field {
        BoardNetClassField::Width => rules.trace_width = mm,
        BoardNetClassField::Clearance => rules.clearance = mm,
        BoardNetClassField::ViaDiameter => rules.via_diameter = mm,
        BoardNetClassField::ViaDrill => rules.via_drill = mm,
        BoardNetClassField::DiffPairGap => rules.diff_pair_gap = mm,
        BoardNetClassField::DiffPairWidth => rules.diff_pair_width = mm,
        BoardNetClassField::Name | BoardNetClassField::Color => {}
    }
    rules
}

/// The table as an answer: each class with its board rules and members.
pub(crate) fn classes_json(classes: &[NetClass]) -> Value {
    json!({ "classes": classes })
}

impl Signex {
    /// The net-class table of `board`'s project; empty for a loose board.
    pub(crate) fn board_net_classes(&self, board: &Path) -> &[NetClass] {
        self.document_state
            .board_project(board, None)
            .map_or(&[], |p| p.data.net_classes.as_slice())
    }

    /// The project whose table `board` uses, or why it has none.
    fn board_net_class_project(&self, board: &Path) -> Result<ProjectId, String> {
        self.document_state
            .board_project(board, None)
            .map(|p| p.id)
            .ok_or_else(|| {
                format!(
                    "{} is in no open project, so it has no net classes",
                    board.display()
                )
            })
    }

    /// Apply `command` to the table of `board`'s project as one step of its
    /// history. The answer is the table after it.
    pub(crate) fn edit_board_net_classes(
        &mut self,
        board: &Path,
        command: NetClassCommand,
    ) -> Result<Value, String> {
        let id = self.board_net_class_project(board)?;
        let document = &mut self.document_state;
        // The cursor to undo back to is the state before the first edit.
        document.note_project_clean_state(id);
        let project = document
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("the board's project is not open")?;
        let history = document.net_class_history.entry(id).or_default();
        let before = project.data.net_classes.clone();
        history
            .apply(&mut project.data.net_classes, command)
            .map_err(|e| e.to_string())?;
        let changed = project.data.net_classes != before;
        let answer = classes_json(&project.data.net_classes);
        if changed {
            document.sync_project_history_dirty(id, false);
            self.resync_net_class_panel(id, &before);
        }
        self.refresh_panel_ctx();
        Ok(answer)
    }

    /// Step the net-class history of `board`'s project back (or forward,
    /// `redo`). Whether it moved.
    pub(crate) fn step_board_net_classes(&mut self, board: &Path, redo: bool) -> bool {
        let Ok(id) = self.board_net_class_project(board) else {
            return false;
        };
        let document = &mut self.document_state;
        let Some(project) = document.projects.iter_mut().find(|p| p.id == id) else {
            return false;
        };
        let Some(history) = document.net_class_history.get_mut(&id) else {
            return false;
        };
        let before = project.data.net_classes.clone();
        let moved = if redo {
            history.redo(&mut project.data.net_classes)
        } else {
            history.undo(&mut project.data.net_classes)
        };
        if moved {
            document.sync_project_history_dirty(id, false);
            self.resync_net_class_panel(id, &before);
        }
        self.refresh_panel_ctx();
        moved
    }

    /// A new class in `board`'s project, `name` or the next `Class <n>`.
    pub(crate) fn add_board_net_class(
        &mut self,
        board: &Path,
        name: Option<String>,
    ) -> Result<Value, String> {
        let name = name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| unused_class_name(self.board_net_classes(board)));
        self.edit_board_net_classes(
            board,
            NetClassCommand::Add {
                class: NetClass::named(name),
            },
        )
    }

    /// `net` leaves `class`'s list of member nets.
    pub(crate) fn remove_board_net_from_class(
        &mut self,
        board: &Path,
        class: &str,
        net: &str,
    ) -> Result<Value, String> {
        let found = self
            .board_net_classes(board)
            .iter()
            .find(|c| c.name == class)
            .cloned()
            .ok_or_else(|| format!("no net class named \"{class}\""))?;
        self.edit_board_net_classes(
            board,
            NetClassCommand::SetMembers {
                name: found.name,
                nets: found.nets.into_iter().filter(|n| n != net).collect(),
                patterns: found.patterns,
            },
        )
    }

    /// The command a typed cell stands for, or why its text is refused.
    fn cell_command(
        &self,
        board: &Path,
        draft: &BoardNetClassDraft,
    ) -> Result<NetClassCommand, String> {
        let i18n = &self.ui_state.i18n;
        let class = self
            .board_net_classes(board)
            .iter()
            .find(|c| c.name == draft.class)
            .ok_or_else(|| format!("no net class named \"{}\"", draft.class))?;
        Ok(match draft.field {
            BoardNetClassField::Name => NetClassCommand::Rename {
                from: class.name.clone(),
                to: draft.text.clone(),
            },
            BoardNetClassField::Color => {
                let text = draft.text.trim();
                let color = if text.is_empty() {
                    None
                } else {
                    Some(parse_hex_color(text).ok_or_else(|| {
                        t!(i18n, ids::BOARD_NET_CLASSES_INVALID_COLOR).into_string()
                    })?)
                };
                NetClassCommand::SetStyle {
                    name: class.name.clone(),
                    style: NetClassStyle {
                        color,
                        ..class.style
                    },
                }
            }
            rule => {
                let mm = typed_mm(&draft.text).ok_or_else(|| {
                    t!(
                        i18n,
                        ids::BOARD_NET_CLASSES_INVALID_NUMBER,
                        "field" => rule.operand()
                    )
                    .into_string()
                })?;
                NetClassCommand::SetBoardRules {
                    name: class.name.clone(),
                    rules: with_rule(class.board_rules(), rule, mm),
                }
            }
        })
    }

    /// A board Net Classes panel event, on the board in view.
    pub(crate) fn handle_board_net_class(&mut self, msg: BoardNetClassMsg) {
        let Some(board) = self.active_tab_path() else {
            return;
        };
        let result: Result<(), String> = match msg {
            BoardNetClassMsg::AddRequested => self.add_board_net_class(&board, None).map(drop),
            BoardNetClassMsg::DeleteRequested(name) => self
                .edit_board_net_classes(&board, NetClassCommand::Remove { name })
                .map(drop),
            BoardNetClassMsg::CellEdited { class, field, text } => {
                let state = self.interaction_state.board_net_classes.for_board(&board);
                state.draft = Some(BoardNetClassDraft { class, field, text });
                // A keystroke updates the shown view alone, no full refresh.
                self.mirror_board_net_class_state(&board);
                return;
            }
            BoardNetClassMsg::CellSubmitted => {
                let state = self.interaction_state.board_net_classes.for_board(&board);
                match state.draft.take() {
                    Some(draft) => self
                        .cell_command(&board, &draft)
                        .and_then(|command| self.edit_board_net_classes(&board, command))
                        .map(drop),
                    None => Ok(()),
                }
            }
            BoardNetClassMsg::NetPicked { class, net } => self
                .edit_board_net_classes(
                    &board,
                    NetClassCommand::AssignNet {
                        net,
                        class: Some(class),
                    },
                )
                .map(drop),
            BoardNetClassMsg::NetRemoved { class, net } => self
                .remove_board_net_from_class(&board, &class, &net)
                .map(drop),
            BoardNetClassMsg::UndoRequested => {
                self.step_board_net_classes(&board, false);
                Ok(())
            }
            BoardNetClassMsg::RedoRequested => {
                self.step_board_net_classes(&board, true);
                Ok(())
            }
        };
        self.interaction_state
            .board_net_classes
            .for_board(&board)
            .notice = result.err();
        self.mirror_board_net_class_state(&board);
    }

    /// Show the panel state of `board` in the view of the board in view.
    fn mirror_board_net_class_state(&mut self, board: &Path) {
        let (draft, notice) = self.interaction_state.board_net_classes.of(board);
        if let Some(view) = self.document_view.panel_ctx.net_classes.board.as_mut() {
            view.draft = draft;
            view.notice = notice;
        }
    }

    /// `panel` with the board Net Classes panel's view of the board in
    /// view: the table, the board's nets and the history flags read now,
    /// the draft and notice from the Model.
    pub(crate) fn with_board_net_classes(&self, mut panel: NetClassPanel) -> NetClassPanel {
        panel.board = self.active_tab_path().and_then(|path| {
            let (draft, notice) = self.interaction_state.board_net_classes.of(&path);
            let board = self.active_pcb()?;
            let project = self.document_state.board_project(&path, None);
            let history = project.and_then(|p| self.document_state.net_class_history.get(&p.id));
            let mut nets: Vec<String> = board
                .nets
                .iter()
                .map(|n| n.name.clone())
                .filter(|n| !n.is_empty())
                .collect();
            nets.sort();
            nets.dedup();
            Some(BoardNetClassView {
                in_project: project.is_some(),
                classes: project
                    .map(|p| p.data.net_classes.clone())
                    .unwrap_or_default(),
                nets,
                can_undo: history.is_some_and(|h| h.can_undo()),
                can_redo: history.is_some_and(|h| h.can_redo()),
                draft,
                notice,
            })
        });
        panel
    }
}

#[cfg(test)]
mod tests;
