//! Command-driven editing of the project's net-class table.
//!
//! The table lives in the `.snxprj` (`ProjectData::net_classes`), not in a
//! schematic sheet, so it has its own small command vocabulary and its own
//! undo history instead of riding the per-sheet [`Engine`](crate::Engine).
//! The shape matches the sheet engine's: a UI never mutates the table
//! directly, it applies a [`NetClassCommand`]; every command is plain data
//! (`serde`-serialisable, so a script or an AI agent can emit it) and every
//! accepted one is one undo step.
//!
//! The table is tiny, so history stores whole-table snapshots rather than
//! inverse commands.

use serde::{Deserialize, Serialize};
use signex_types::net_class::{NetClass, NetClassStyle};

/// How many undo steps are kept.
pub const HISTORY_LIMIT: usize = 100;

/// One edit to the net-class table. Classes are addressed by name, which is
/// unique within the table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum NetClassCommand {
    /// Append a class. Last in table order means lowest precedence.
    Add {
        class: NetClass,
    },
    Remove {
        name: String,
    },
    Rename {
        from: String,
        to: String,
    },
    SetStyle {
        name: String,
        style: NetClassStyle,
    },
    /// Replace the exact member nets and glob patterns. Entries are trimmed;
    /// blanks and repeats are dropped.
    SetMembers {
        name: String,
        nets: Vec<String>,
        patterns: Vec<String>,
    },
    /// Make `net` a member of exactly one class: drop it from every class's
    /// exact member list, then add it to `class` (or to none, for `None`).
    /// Glob patterns are left alone, so an earlier class whose pattern still
    /// matches `net` keeps winning; the caller can tell with
    /// `signex_types::net_class::first_matching_class`.
    AssignNet {
        net: String,
        class: Option<String>,
    },
    /// Move a class to `to_index` (clamped to the table). The first matching
    /// class wins, so this is the precedence control.
    Move {
        name: String,
        to_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetClassError {
    #[error("a net class needs a name")]
    EmptyName,
    #[error("a net class named \"{0}\" already exists")]
    DuplicateName(String),
    #[error("\"{0}\" is reserved for \"no class\" in the net label picker; pick another name")]
    ReservedName(String),
    #[error("no net class named \"{0}\"")]
    Unknown(String),
    #[error("wire width must be a positive, finite number of millimetres")]
    InvalidWidth,
}

/// Apply `command` to `classes`. On error `classes` is unchanged.
pub fn apply_net_class_command(
    classes: &mut Vec<NetClass>,
    command: NetClassCommand,
) -> Result<(), NetClassError> {
    match command {
        NetClassCommand::Add { mut class } => {
            class.name = class.name.trim().to_string();
            check_new_name(classes, &class.name)?;
            check_style(&class.style)?;
            class.nets = clean(class.nets);
            class.patterns = clean(class.patterns);
            classes.push(class);
        }
        NetClassCommand::Remove { name } => {
            let at = index_of(classes, &name)?;
            classes.remove(at);
        }
        NetClassCommand::Rename { from, to } => {
            let at = index_of(classes, &from)?;
            let to = to.trim().to_string();
            if to != from {
                check_new_name(classes, &to)?;
                classes[at].name = to;
            }
        }
        NetClassCommand::SetStyle { name, style } => {
            check_style(&style)?;
            let at = index_of(classes, &name)?;
            classes[at].style = style;
        }
        NetClassCommand::SetMembers {
            name,
            nets,
            patterns,
        } => {
            let at = index_of(classes, &name)?;
            classes[at].nets = clean(nets);
            classes[at].patterns = clean(patterns);
        }
        NetClassCommand::AssignNet { net, class } => {
            let net = net.trim().to_string();
            if net.is_empty() {
                return Ok(());
            }
            let target = match &class {
                Some(name) => Some(index_of(classes, name)?),
                None => None,
            };
            let already_member = target.is_some_and(|at| classes[at].nets.contains(&net));
            for (at, member) in classes.iter_mut().enumerate() {
                if !(already_member && Some(at) == target) {
                    member.nets.retain(|n| *n != net);
                }
            }
            if let (Some(at), false) = (target, already_member) {
                classes[at].nets.push(net);
            }
        }
        NetClassCommand::Move { name, to_index } => {
            let at = index_of(classes, &name)?;
            let class = classes.remove(at);
            classes.insert(to_index.min(classes.len()), class);
        }
    }
    Ok(())
}

fn index_of(classes: &[NetClass], name: &str) -> Result<usize, NetClassError> {
    classes
        .iter()
        .position(|c| c.name == name)
        .ok_or_else(|| NetClassError::Unknown(name.to_string()))
}

fn check_new_name(classes: &[NetClass], name: &str) -> Result<(), NetClassError> {
    if name.is_empty() {
        return Err(NetClassError::EmptyName);
    }
    if name == signex_types::net_class::NO_CLASS_ENTRY {
        return Err(NetClassError::ReservedName(name.to_string()));
    }
    if classes.iter().any(|c| c.name == name) {
        return Err(NetClassError::DuplicateName(name.to_string()));
    }
    Ok(())
}

fn check_style(style: &NetClassStyle) -> Result<(), NetClassError> {
    match style.wire_width_mm {
        Some(w) if !(w.is_finite() && w > 0.0) => Err(NetClassError::InvalidWidth),
        _ => Ok(()),
    }
}

fn clean(entries: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(entries.len());
    for entry in entries {
        let entry = entry.trim();
        if !entry.is_empty() && !out.iter().any(|e| e == entry) {
            out.push(entry.to_string());
        }
    }
    out
}

/// Undo/redo for one project's net-class table.
#[derive(Debug, Clone, Default)]
pub struct NetClassHistory {
    undo: Vec<Vec<NetClass>>,
    redo: Vec<Vec<NetClass>>,
}

impl NetClassHistory {
    /// Apply `command` as one undo step. A refused command, or one that
    /// changes nothing, leaves the history untouched.
    pub fn apply(
        &mut self,
        classes: &mut Vec<NetClass>,
        command: NetClassCommand,
    ) -> Result<(), NetClassError> {
        let mut next = classes.clone();
        apply_net_class_command(&mut next, command)?;
        if next == *classes {
            return Ok(());
        }
        let before = std::mem::replace(classes, next);
        self.undo.push(before);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(())
    }

    /// Step back. `false` when there is nothing to undo.
    pub fn undo(&mut self, classes: &mut Vec<NetClass>) -> bool {
        match self.undo.pop() {
            Some(previous) => {
                self.redo.push(std::mem::replace(classes, previous));
                true
            }
            None => false,
        }
    }

    /// Step forward. `false` when there is nothing to redo.
    pub fn redo(&mut self, classes: &mut Vec<NetClass>) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(classes, next));
                true
            }
            None => false,
        }
    }

    /// How many steps can be undone. A caller that stamps a step it just
    /// applied with this can later tell whether it is still the newest one.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests;
