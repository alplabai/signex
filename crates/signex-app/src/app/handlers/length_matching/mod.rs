//! The Length Matching tab's and panel's events (#2797, mockup row 107).
//! View state goes to `PanelContext::length_matching_ui`; every edit is a
//! length-matching group edit (`signex_engine::pcb::length_match`) landed
//! as one board undo step, the same path `set_length_group` and
//! `remove_length_group` take, so both views and an agent see one result.

mod commands;
#[cfg(test)]
mod tests;

pub(crate) use commands::{length_matching, remove_length_group, set_length_group};

use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::length_match::{self, GroupEdit, LengthGroupRefusal};
use signex_engine::pcb::route::{diff_pair_of, pair_name};
use signex_i18n::{ids, t};
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_options::BoardOptions;

use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex, TabKind};
use crate::panels::PanelKind;
use crate::panels::length_matching::{
    LengthField, LengthGroupRow, LengthMatchingFacts, LengthMatchingMsg, LengthMemberRow,
};
use crate::panels::pcb_primitive_properties::parse_number;

/// The rows of `board`'s groups, each member with the pair it is half of.
fn facts_of(board: &PcbBoard, classes: &[NetClass], path: PathBuf) -> LengthMatchingFacts {
    let groups = length_match::report(board)
        .into_iter()
        .map(|report| LengthGroupRow {
            uuid: report.group.uuid,
            rule: report.group.rule_name(),
            name: report.group.name,
            target_mm: report.group.target_mm,
            tolerance_mm: report.group.tolerance_mm,
            topology: report.group.topology,
            members: report
                .members
                .into_iter()
                .map(|m| LengthMemberRow {
                    pair: pair_of_net(board, classes, &m.net),
                    net: m.net,
                    length_mm: m.length_mm,
                    delta_mm: m.delta_mm,
                    status: m.status,
                })
                .collect(),
        })
        .collect();
    let mut nets: Vec<String> = board
        .nets
        .iter()
        .filter(|n| n.number != 0 && !n.name.is_empty())
        .map(|n| n.name.clone())
        .collect();
    nets.sort();
    nets.dedup();
    LengthMatchingFacts {
        board: path,
        read_only: board.unknown.newer_minor_source.is_some(),
        groups,
        nets,
    }
}

/// The name of the differential pair the net called `net` is half of.
fn pair_of_net(board: &PcbBoard, classes: &[NetClass], net: &str) -> Option<String> {
    let number = board
        .nets
        .iter()
        .find(|n| n.name == net && n.number != 0)?
        .number;
    let pair = diff_pair_of(board, classes, number)?;
    let name = |wanted: u32| {
        board
            .nets
            .iter()
            .find(|n| n.number == wanted)
            .map(|n| n.name.as_str())
    };
    Some(pair_name(name(pair.positive)?, name(pair.negative)?))
}

impl Signex {
    pub(crate) fn handle_length_matching(&mut self, msg: LengthMatchingMsg) -> Task<Message> {
        use LengthMatchingMsg as M;
        let board = self
            .document_view
            .panel_ctx
            .length_matching
            .as_ref()
            .map(|facts| facts.board.clone());
        let ui = &mut self.document_view.panel_ctx.length_matching_ui;
        if let Some(board) = &board {
            ui.follow(board);
        }
        ui.apply(&msg);
        let task = match (msg, board) {
            (M::TabOpened, _) => {
                self.open_board_page_tab(TabKind::LengthMatching);
                Task::none()
            }
            (M::PanelShown, _) => self.show_panel(PanelKind::LengthMatching),
            (M::FieldSubmitted(field), Some(board)) => {
                self.submit_length_field(&board, field);
                Task::none()
            }
            (M::TopologyPicked { group, topology }, Some(board)) => {
                self.edit_length_group(&board, group, |edit, _| {
                    edit.topology = Some(topology);
                });
                Task::none()
            }
            (M::NetAdded { group, net }, Some(board)) => {
                self.edit_length_group(&board, group, |edit, members| {
                    let mut members = members.to_vec();
                    members.push(net);
                    edit.members = Some(members);
                });
                Task::none()
            }
            (M::MemberRemoved { group, net }, Some(board)) => {
                self.edit_length_group(&board, group, |edit, members| {
                    edit.members = Some(members.iter().filter(|m| **m != net).cloned().collect());
                });
                Task::none()
            }
            (M::GroupAdded, Some(board)) => {
                self.add_length_group(&board);
                Task::none()
            }
            (M::GroupDeleted(group), Some(board)) => {
                if let Some(name) = self.length_group_name(group)
                    && let Err(why) = self.remove_length_group_on(&board, &name)
                {
                    self.refuse_length_edit(why);
                }
                Task::none()
            }
            _ => Task::none(),
        };
        self.document_view.panel_ctx.length_matching = self.length_matching_facts();
        task
    }

    /// The groups of the board in view, on its PCB tab or its Length
    /// Matching tab; `None` without one.
    pub(crate) fn length_matching_facts(&self) -> Option<LengthMatchingFacts> {
        let docs = &self.document_state;
        let tab = docs.tabs.get(docs.active_tab)?;
        if !matches!(tab.kind, TabKind::Pcb | TabKind::LengthMatching) {
            return None;
        }
        let board = docs.pcb_board(&tab.path)?;
        Some(facts_of(
            board,
            self.board_net_classes(&tab.path),
            tab.path.clone(),
        ))
    }

    /// The name of the group `uuid` as the views show it.
    fn length_group_name(&self, uuid: uuid::Uuid) -> Option<String> {
        let facts = self.document_view.panel_ctx.length_matching.as_ref()?;
        facts.group(uuid).map(|g| g.name.clone())
    }

    /// Edit the group `uuid`: `fill` sets the fields of the edit from the
    /// group's members.
    fn edit_length_group(
        &mut self,
        board: &Path,
        uuid: uuid::Uuid,
        fill: impl FnOnce(&mut GroupEdit, &[String]),
    ) {
        let Some(facts) = self.document_view.panel_ctx.length_matching.as_ref() else {
            return;
        };
        let Some(group) = facts.group(uuid) else {
            return;
        };
        let members: Vec<String> = group.members.iter().map(|m| m.net.clone()).collect();
        let mut edit = GroupEdit {
            name: group.name.clone(),
            ..GroupEdit::default()
        };
        fill(&mut edit, &members);
        if let Err(why) = self.set_length_group_on(board, &edit) {
            self.refuse_length_edit(why);
        }
    }

    /// Enter in the picked group's Name, Target or Tolerance.
    fn submit_length_field(&mut self, board: &Path, field: LengthField) {
        if !field.is_group_field() {
            return;
        }
        let ui = &self.document_view.panel_ctx.length_matching_ui;
        let (Some(text), Some(group)) = (ui.drafts.get(&field).cloned(), ui.current) else {
            return;
        };
        let length = || {
            parse_number(&text).ok_or_else(|| {
                t!(self.ui_state.i18n, ids::PCB_PRIM_NOT_A_NUMBER, "text" => text.clone())
                    .into_string()
            })
        };
        let value = match field {
            LengthField::Name => Ok(None),
            _ => length().map(Some),
        };
        let value = match value {
            Ok(value) => value,
            Err(why) => {
                self.refuse_length_edit(why);
                return;
            }
        };
        let rename = text.clone();
        self.edit_length_group(board, group, |edit, _| match field {
            LengthField::Name => edit.rename = Some(rename),
            LengthField::Target => edit.target_mm = value,
            _ => edit.tolerance_mm = value,
        });
        self.document_view
            .panel_ctx
            .length_matching_ui
            .drop_drafts(&[field]);
    }

    /// Add Group: the New group row's name, target and tolerance.
    fn add_length_group(&mut self, board: &Path) {
        let ui = &self.document_view.panel_ctx.length_matching_ui;
        let typed = |field| ui.drafts.get(&field).cloned().unwrap_or_default();
        let (name, target, tolerance) = (
            typed(LengthField::NewName),
            typed(LengthField::NewTarget),
            typed(LengthField::NewTolerance),
        );
        let (Some(target_mm), Some(tolerance_mm)) =
            (parse_number(&target), parse_number(&tolerance))
        else {
            let bad = if parse_number(&target).is_none() {
                target
            } else {
                tolerance
            };
            let why = t!(self.ui_state.i18n, ids::PCB_PRIM_NOT_A_NUMBER, "text" => bad);
            self.refuse_length_edit(why.into_string());
            return;
        };
        let edit = GroupEdit {
            name: name.clone(),
            target_mm: Some(target_mm),
            tolerance_mm: Some(tolerance_mm),
            ..GroupEdit::default()
        };
        match self.set_length_group_on(board, &edit) {
            Ok(()) => {
                let made = self.document_state.pcb_board(board).and_then(|b| {
                    let name = name.trim();
                    b.options
                        .length_groups
                        .iter()
                        .find(|g| g.name == name)
                        .map(|g| g.uuid)
                });
                let ui = &mut self.document_view.panel_ctx.length_matching_ui;
                ui.drop_drafts(&[
                    LengthField::NewName,
                    LengthField::NewTarget,
                    LengthField::NewTolerance,
                ]);
                if let Some(made) = made {
                    ui.apply(&LengthMatchingMsg::GroupPicked(made));
                }
            }
            Err(why) => self.refuse_length_edit(why),
        }
    }

    fn refuse_length_edit(&self, why: String) {
        self.refuse_primitive_edit(ids::PANELS_PAIRED_LENGTH_MATCHING, why);
    }

    /// Make or change a group of the board at `path` as one undo step.
    pub(crate) fn set_length_group_on(
        &mut self,
        path: &Path,
        edit: &GroupEdit,
    ) -> Result<(), String> {
        self.land_length_groups(path, |board| {
            length_match::set_group(board, edit, uuid::Uuid::now_v7)
        })
    }

    /// Remove the group `name` of the board at `path` as one undo step.
    pub(crate) fn remove_length_group_on(&mut self, path: &Path, name: &str) -> Result<(), String> {
        self.land_length_groups(path, |board| length_match::remove_group(board, name))
    }

    /// The options `build` makes from the board at `path`, landed as one
    /// undo step labelled Length Matching; nothing when they are the same.
    fn land_length_groups(
        &mut self,
        path: &Path,
        build: impl FnOnce(&PcbBoard) -> Result<BoardOptions, LengthGroupRefusal>,
    ) -> Result<(), String> {
        let Some(engine) = self.document_state.board_engine(path) else {
            return Err(crate::app::BoardEditError::NotLoaded(path.to_path_buf()).to_string());
        };
        let board = engine.board();
        if board.unknown.newer_minor_source.is_some() {
            return Err(t!(self.ui_state.i18n, ids::PCB_PRIM_READ_ONLY).into_string());
        }
        let revision = engine.revision();
        let options = build(board).map_err(|refusal| refusal.to_string())?;
        if options == board.options {
            return Ok(());
        }
        let label = t!(self.ui_state.i18n, ids::PANELS_PAIRED_LENGTH_MATCHING).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: revision,
        };
        let command = BoardCommand::SetBoardOptions {
            options: Box::new(options),
        };
        self.execute_board_commands(path, vec![command], Some(step))
            .map_err(|error| error.to_string())?;
        self.document_view.panel_ctx.length_matching = self.length_matching_facts();
        Ok(())
    }
}
