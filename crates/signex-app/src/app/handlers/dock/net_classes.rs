//! Net Classes section of the Properties panel (#1042).
//!
//! `step` turns one [`NetClassMsg`] into a `signex_engine::NetClassCommand`
//! (or an undo/redo) on the active project's table. It is pure: no disk, no
//! iced. The `Signex` method around it marks the `.snxprj` dirty; the file is
//! written only by an explicit save.

use signex_engine::{NetClassCommand, NetClassHistory};
use signex_types::net_class::{NetClass, NetClassStyle, first_matching_class, parse_hex_color};

use crate::panels::{NetClassDraft, NetClassField, NetClassMsg, NetClassPanel};

fn split_list(text: &str) -> Vec<String> {
    text.split(',').map(str::to_string).collect()
}

fn unique_name(classes: &[NetClass]) -> String {
    (1..)
        .map(|n| format!("Class {n}"))
        .find(|name| classes.iter().all(|c| &c.name != name))
        .unwrap_or_default()
}

/// The command a committed field edit stands for.
fn commit_command(
    class: &NetClass,
    field: NetClassField,
    draft: &NetClassDraft,
) -> Result<NetClassCommand, String> {
    let name = class.name.clone();
    match field {
        NetClassField::Name => Ok(NetClassCommand::Rename {
            from: name,
            to: draft.name.clone(),
        }),
        NetClassField::Color => {
            let text = draft.color.trim();
            let color = if text.is_empty() {
                None
            } else {
                Some(parse_hex_color(text).ok_or("Colour must be #RRGGBB or #RRGGBBAA.")?)
            };
            Ok(NetClassCommand::SetStyle {
                name,
                style: NetClassStyle {
                    color,
                    ..class.style
                },
            })
        }
        NetClassField::Width => {
            let text = draft.width.trim();
            let wire_width_mm = if text.is_empty() {
                None
            } else {
                Some(
                    text.parse::<f64>()
                        .map_err(|_| "Width must be a number of millimetres.".to_string())?,
                )
            };
            Ok(NetClassCommand::SetStyle {
                name,
                style: NetClassStyle {
                    wire_width_mm,
                    ..class.style
                },
            })
        }
        NetClassField::Nets => Ok(NetClassCommand::SetMembers {
            name,
            nets: split_list(&draft.nets),
            patterns: class.patterns.clone(),
        }),
        NetClassField::Patterns => Ok(NetClassCommand::SetMembers {
            name,
            nets: class.nets.clone(),
            patterns: split_list(&draft.patterns),
        }),
    }
}

/// Apply `msg` to the table, its history and the panel state. Returns `true`
/// when the table changed.
pub(crate) fn step(
    classes: &mut Vec<NetClass>,
    history: &mut NetClassHistory,
    panel: &mut NetClassPanel,
    msg: &NetClassMsg,
) -> bool {
    panel.notice = None;
    let selected = panel.selected.min(classes.len().saturating_sub(1));
    let selected_class = classes.get(selected).cloned();

    let command = match msg {
        NetClassMsg::Select(idx) => {
            panel.selected = (*idx).min(classes.len().saturating_sub(1));
            sync(panel, classes);
            return false;
        }
        NetClassMsg::Draft(field, text) => {
            *panel.draft.field_mut(*field) = text.clone();
            return false;
        }
        NetClassMsg::Undo | NetClassMsg::Redo => {
            let moved = if matches!(msg, NetClassMsg::Undo) {
                history.undo(classes)
            } else {
                history.redo(classes)
            };
            sync(panel, classes);
            return moved;
        }
        NetClassMsg::AssignNet { net, class, .. } => NetClassCommand::AssignNet {
            net: net.clone(),
            class: class.clone(),
        },
        NetClassMsg::Add => NetClassCommand::Add {
            class: NetClass::named(unique_name(classes)),
        },
        NetClassMsg::RemoveSelected => match selected_class {
            Some(c) => NetClassCommand::Remove { name: c.name },
            None => return false,
        },
        NetClassMsg::SetLine(line) => match selected_class {
            Some(c) => NetClassCommand::SetStyle {
                name: c.name,
                style: NetClassStyle {
                    line: *line,
                    ..c.style
                },
            },
            None => return false,
        },
        NetClassMsg::MoveUp | NetClassMsg::MoveDown => match selected_class {
            Some(c) => {
                let to_index = if matches!(msg, NetClassMsg::MoveUp) {
                    selected.saturating_sub(1)
                } else {
                    selected + 1
                };
                NetClassCommand::Move {
                    name: c.name,
                    to_index,
                }
            }
            None => return false,
        },
        NetClassMsg::Commit(field) => match &selected_class {
            Some(c) => match commit_command(c, *field, &panel.draft) {
                Ok(command) => command,
                Err(notice) => {
                    panel.notice = Some(notice);
                    return false;
                }
            },
            None => return false,
        },
    };

    let before = classes.clone();
    if let Err(e) = history.apply(classes, command) {
        panel.notice = Some(e.to_string());
        return false;
    }
    let changed = *classes != before;
    if let NetClassMsg::AssignNet { net, class, .. } = msg
        && let Some(winner) = first_matching_class(classes, net)
        && class.as_deref() != Some(winner.name.as_str())
    {
        panel.notice = Some(match class {
            Some(_) => format!(
                "{net} is still matched by \"{}\", which comes first in the table.",
                winner.name
            ),
            None => format!(
                "{net} is removed from its listed class but is still matched by \"{}\" \
                 through a pattern.",
                winner.name
            ),
        });
    }
    panel.selected = match msg {
        NetClassMsg::Add => classes.len() - 1,
        NetClassMsg::MoveUp => selected.saturating_sub(1),
        NetClassMsg::MoveDown => (selected + 1).min(classes.len().saturating_sub(1)),
        _ => selected,
    };
    sync(panel, classes);
    changed
}

/// Clamp the selection and reload the drafts from the selected class.
fn sync(panel: &mut NetClassPanel, classes: &[NetClass]) {
    panel.selected = panel.selected.min(classes.len().saturating_sub(1));
    panel.draft = classes
        .get(panel.selected)
        .map(NetClassDraft::from_class)
        .unwrap_or_default();
}

impl super::super::super::Signex {
    /// Handle a Net Classes message: against the active project, except a net
    /// assignment, which goes to the project its label's sheet belongs to.
    pub(super) fn handle_net_class_message(&mut self, msg: &NetClassMsg) {
        let document = &mut self.document_state;
        let target = match msg {
            NetClassMsg::AssignNet { project, .. } => Some(*project),
            _ => document.active_project,
        };
        let Some(id) = target else {
            return;
        };
        let Some(project) = document.projects.iter_mut().find(|p| p.id == id) else {
            return;
        };
        let history = document.net_class_history.entry(id).or_default();
        let mut panel = std::mem::take(&mut document.panel_ctx.net_classes);
        let borrowed_panel = panel.project != Some(id);
        let mut scratch = NetClassPanel::default();
        // The panel's selection and drafts belong to the project that was
        // active when they were made; never apply them to another one. An
        // assignment to a project the panel is not showing runs on a scratch
        // panel instead, so it leaves the shown project's selection alone.
        let working = if borrowed_panel && matches!(msg, NetClassMsg::AssignNet { .. }) {
            sync(&mut scratch, &project.data.net_classes);
            &mut scratch
        } else {
            if borrowed_panel {
                panel.project = Some(id);
                panel.selected = 0;
                panel.notice = None;
                sync(&mut panel, &project.data.net_classes);
            }
            &mut panel
        };
        let top_before = history.top_undo_id();
        let changed = step(&mut project.data.net_classes, history, working, msg);
        // The pick's own step is the new top of the project's history.
        let pick_step = history.top_undo_id().filter(|top| {
            changed && matches!(msg, NetClassMsg::AssignNet { .. }) && Some(*top) != top_before
        });
        if borrowed_panel && matches!(msg, NetClassMsg::AssignNet { .. }) {
            panel.notice = scratch.notice;
        }
        document.panel_ctx.net_classes = panel;
        if changed {
            document.dirty_paths.insert(project.path.clone());
        }
        if let Some(step) = pick_step {
            self.record_net_class_pick(id, step);
        }
        self.refresh_panel_ctx();
    }
}

mod pick_undo;
#[cfg(test)]
mod pick_undo_tests;
#[cfg(test)]
mod tests;
