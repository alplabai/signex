//! The answer of `update_pcb_from_schematic` run from operands (#2470): the
//! change list as JSON, each row with what it does and whether it applies.
//! The same list answers a dry run (what would be done) and an apply (what
//! was done). Shape: `docs/CLI.md`, "Update PCB from schematic".

use std::path::Path;

use serde::Serialize;
use signex_engine::pcb::eco::{
    ChangeReason, EcoChange, EcoChangeList, EcoRow, blocked_text, note_text,
};
use signex_types::pcb::PadNet;
use uuid::Uuid;

use super::table::{RowAction, Summary};

/// The whole answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct UpdatePcbAnswer {
    pub(crate) dry_run: bool,
    pub(crate) project: String,
    pub(crate) board: String,
    /// The board revision the list was read at.
    pub(crate) revision: u64,
    /// The board's revision now: past the step an apply made, else the same.
    pub(crate) board_revision: u64,
    /// How many rows apply (were applied, for an apply).
    pub(crate) applied: usize,
    pub(crate) counts: Counts,
    pub(crate) changes: Vec<Change>,
    pub(crate) notes: Vec<String>,
    /// The board's net classes the Net Blanket and Net Parameters directives
    /// add or change (exit criterion 3): put there by an apply, for a dry
    /// run what an apply would put.
    pub(crate) net_classes: Vec<String>,
    /// A dry run only: why an apply would be refused now (the board closed
    /// with unsaved edits, read-only, or moved past the list).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) execute_blocked: Option<String>,
}

/// The rows by action, as the dialog's tiles count them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct Counts {
    pub(crate) add: usize,
    pub(crate) change: usize,
    pub(crate) remove: usize,
    pub(crate) problems: usize,
}

/// One row. `from` and `to` are `null` for a pad with no net.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Change {
    pub(crate) kind: &'static str,
    pub(crate) action: &'static str,
    /// A designator, a net name, or a pad as `R1.2`.
    pub(crate) object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) footprint: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pad: Option<Uuid>,
    /// The board net number of a net row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) net: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) from: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) to: Option<Option<String>>,
    /// A footprint change: `footprint` or `other_version`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) locked: Option<bool>,
    pub(crate) ticked: bool,
    pub(crate) applies: bool,
    pub(crate) blocked: Option<String>,
}

/// Where the list came from and went.
pub(crate) struct AnswerFor<'a> {
    pub(crate) dry_run: bool,
    pub(crate) project: &'a Path,
    pub(crate) board: &'a Path,
    pub(crate) board_revision: u64,
    pub(crate) execute_blocked: Option<String>,
    pub(crate) net_classes: Vec<String>,
}

impl UpdatePcbAnswer {
    pub(crate) fn of(list: &EcoChangeList, at: AnswerFor<'_>) -> Self {
        let summary = Summary::of(list);
        Self {
            dry_run: at.dry_run,
            project: at.project.display().to_string(),
            board: at.board.display().to_string(),
            revision: list.revision,
            board_revision: at.board_revision,
            applied: list.rows.iter().filter(|r| list.applies(r)).count(),
            counts: Counts {
                add: summary.add,
                change: summary.change,
                remove: summary.remove,
                problems: summary.problems,
            },
            changes: list.rows.iter().map(|row| change(list, row)).collect(),
            notes: list.notes.iter().map(note_text).collect(),
            net_classes: at.net_classes,
            execute_blocked: at.execute_blocked,
        }
    }
}

fn action(row: &EcoRow) -> &'static str {
    match RowAction::of(&row.change) {
        RowAction::Add => "add",
        RowAction::Change => "change",
        RowAction::Remove => "remove",
    }
}

fn net_name(net: &Option<PadNet>) -> Option<String> {
    net.as_ref().map(|n| n.name.clone())
}

fn change(list: &EcoChangeList, row: &EcoRow) -> Change {
    let mut out = Change {
        kind: "",
        action: action(row),
        object: String::new(),
        footprint: None,
        pad: None,
        net: None,
        value: None,
        from: None,
        to: None,
        reason: None,
        locked: None,
        ticked: row.on,
        applies: list.applies(row),
        blocked: row.blocked.as_ref().map(blocked_text),
    };
    describe(&row.change, &mut out);
    out
}

/// The fields of one change.
fn describe(change: &EcoChange, out: &mut Change) {
    match change {
        EcoChange::AddFootprint { footprint } => {
            out.kind = "add_footprint";
            out.object = footprint.reference.clone();
            out.footprint = Some(footprint.uuid);
            out.value = Some(footprint.value.clone());
            let source = footprint
                .library_ref
                .as_ref()
                .map_or_else(|| footprint.footprint_id.clone(), ToString::to_string);
            out.to = Some(Some(source));
        }
        EcoChange::RemoveFootprint {
            footprint,
            reference,
        } => {
            out.kind = "remove_footprint";
            out.object = reference.clone();
            out.footprint = Some(*footprint);
        }
        EcoChange::ChangeFootprint {
            footprint,
            reference,
            from,
            to,
            reason,
            ..
        } => {
            out.kind = "change_footprint";
            out.object = reference.clone();
            out.footprint = Some(*footprint);
            out.from = Some(Some(from.clone()));
            out.to = Some(Some(to.to_string()));
            out.reason = Some(match reason {
                ChangeReason::Footprint => "footprint",
                ChangeReason::OtherVersion => "other_version",
            });
        }
        EcoChange::SetDesignator {
            footprint,
            from,
            to,
        } => {
            out.kind = "set_designator";
            out.object = from.clone();
            out.footprint = Some(*footprint);
            out.from = Some(Some(from.clone()));
            out.to = Some(Some(to.clone()));
        }
        EcoChange::SetValue {
            footprint,
            reference,
            from,
            to,
        } => {
            out.kind = "set_value";
            out.object = reference.clone();
            out.footprint = Some(*footprint);
            out.from = Some(Some(from.clone()));
            out.to = Some(Some(to.clone()));
        }
        EcoChange::Link {
            footprint,
            reference,
            ..
        } => {
            out.kind = "link";
            out.object = reference.clone();
            out.footprint = Some(*footprint);
        }
        EcoChange::AddNet { number, name } => {
            out.kind = "add_net";
            out.object = name.clone();
            out.net = Some(*number);
        }
        EcoChange::RemoveNet { number, name } => {
            out.kind = "remove_net";
            out.object = name.clone();
            out.net = Some(*number);
        }
        EcoChange::RenameNet { number, from, to } => {
            out.kind = "rename_net";
            out.object = from.clone();
            out.net = Some(*number);
            out.from = Some(Some(from.clone()));
            out.to = Some(Some(to.clone()));
        }
        EcoChange::PadNet {
            footprint,
            pad,
            reference,
            pad_number,
            from,
            to,
            locked,
        } => {
            out.kind = "pad_net";
            out.object = format!("{reference}.{pad_number}");
            out.footprint = Some(*footprint);
            out.pad = Some(*pad);
            out.from = Some(net_name(from));
            out.to = Some(net_name(to));
            out.locked = Some(*locked);
        }
    }
}
