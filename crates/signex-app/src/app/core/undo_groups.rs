//! What one project-wide undo group touched and how its messages name it;
//! `app/undo_groups.rs` moves the sheets and explains the rules.

use signex_i18n_ids::{MessageId, Translatable, ids};

/// The sheets one undo group touched.
#[derive(Debug, Clone, Default)]
pub struct UndoGroupSheets {
    /// Sheets whose open engine recorded the group's entry.
    pub live: Vec<std::path::PathBuf>,
    /// Sheets the last undo of the group actually reverted. A redo requires
    /// exactly these, so one edited after the undo blocks it.
    pub undone: Vec<std::path::PathBuf>,
    /// What the group is, named in its undo and redo messages.
    pub kind: UndoGroupKind,
    /// Sheets the run loaded into tabless parked engines (re-prefix or reset
    /// of a sheet that was not open). Back at their loaded state after an undo,
    /// they are clean, not unsaved.
    pub parked: Vec<std::path::PathBuf>,
    /// Sheets with no unsaved edits when the run began. One whose tab closes
    /// before the undo joins `parked`: it holds nothing the file lacks.
    pub clean_before: Vec<std::path::PathBuf>,
    /// Parked sheets an undo took back to disk state and dropped. Their
    /// history is gone, so the group can no longer be redone as a whole.
    pub dropped: Vec<std::path::PathBuf>,
    /// Boards whose step carries the group, moved with its sheets (#2366).
    pub boards: Vec<std::path::PathBuf>,
}

/// Which run stamped an undo group.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UndoGroupKind {
    #[default]
    Annotate,
    Reprefix,
    Replace,
    /// Tools ▸ Assign Footprints (#2248).
    Footprints,
    /// Tools ▸ Annotation ▸ Number Schematic Sheets (#2247).
    SheetNumbers,
    /// Tools ▸ Annotation ▸ Back Annotate Schematics (#1312).
    BackAnnotate,
    /// An agent's `apply_edits` call that edited several sheets (#2178).
    AgentEdit,
    /// A board footprint swap and the linked parts' footprints (#2366).
    FootprintSwap,
    /// A board pin or gate swap and its schematic rewiring (#2410).
    PartSwap,
    /// Synchronize Sheet Entries and Ports writing a sheet's ports (#1276).
    SheetEntrySync,
    /// A power port's rename across its rail (mockup row 45).
    RailRename,
    /// Relink: every image naming one file points at another (#1116).
    ImageRelink,
    /// A Parameter Manager edit across several sheets (#1784).
    Parameters,
}

impl UndoGroupSheets {
    pub(crate) fn noun(&self) -> &'static str {
        match self.kind {
            UndoGroupKind::Annotate => "annotate",
            UndoGroupKind::Reprefix => "re-prefix",
            UndoGroupKind::Replace => "replace",
            UndoGroupKind::Footprints => "footprint assignment",
            UndoGroupKind::SheetNumbers => "sheet numbering",
            UndoGroupKind::BackAnnotate => "back annotate",
            UndoGroupKind::AgentEdit => "agent edit",
            UndoGroupKind::FootprintSwap => "footprint change",
            UndoGroupKind::PartSwap => "pin or gate swap",
            UndoGroupKind::SheetEntrySync => "sheet entry sync",
            UndoGroupKind::RailRename => "power rail rename",
            UndoGroupKind::ImageRelink => "image relink",
            UndoGroupKind::Parameters => "parameter edit",
        }
    }

    /// The step as a message names it: "the project-wide annotate".
    pub(crate) fn wide(&self) -> String {
        let scope = match self.kind {
            UndoGroupKind::AgentEdit => "cross-sheet",
            _ => "project-wide",
        };
        format!("{scope} {}", self.noun())
    }

    pub(crate) fn title(&self) -> &'static str {
        match self.kind {
            UndoGroupKind::Annotate => "Annotate",
            UndoGroupKind::Reprefix => "Re-prefix",
            UndoGroupKind::Replace => "Replace",
            UndoGroupKind::Footprints => "Assign Footprints",
            UndoGroupKind::SheetNumbers => "Sheet numbering",
            UndoGroupKind::BackAnnotate => "Back annotate",
            UndoGroupKind::AgentEdit => "Agent edit",
            UndoGroupKind::FootprintSwap => "Change Footprint",
            UndoGroupKind::PartSwap => "Pin or gate swap",
            UndoGroupKind::SheetEntrySync => "Sheet entry sync",
            UndoGroupKind::RailRename => "Rename power rail",
            UndoGroupKind::ImageRelink => "Relink image",
            UndoGroupKind::Parameters => "Parameter Manager",
        }
    }

    /// The one line of a step refused because `sheets` cannot move with the
    /// others (#2760).
    pub(crate) fn out_of_reach(&self, undo: bool, sheets: &[std::path::PathBuf]) -> Translatable {
        let id = if undo {
            ids::NOTICE_EDIT_GROUP_UNDO_OUT_OF_REACH
        } else {
            ids::NOTICE_EDIT_GROUP_REDO_OUT_OF_REACH
        };
        self.refusal(id, sheets)
    }

    /// The line of a redo refused because `dropped` went back to its saved
    /// state.
    pub(crate) fn redo_dropped(&self) -> Translatable {
        self.refusal(ids::NOTICE_EDIT_GROUP_REDO_DROPPED, &self.dropped)
            .arg("command", self.title())
    }

    /// The line of a step `blocked` sheets' later steps refuse.
    pub(crate) fn blocked(&self, undo: bool, blocked: &[std::path::PathBuf]) -> Translatable {
        if undo {
            self.refusal(ids::NOTICE_EDIT_GROUP_UNDO_BLOCKED, blocked)
        } else {
            self.refusal(ids::NOTICE_EDIT_GROUP_REDO_BLOCKED, blocked)
                .arg("noun", self.noun())
        }
    }

    /// `id` naming this step and `sheets`.
    fn refusal(&self, id: MessageId, sheets: &[std::path::PathBuf]) -> Translatable {
        Translatable::new(id)
            .arg("run", self.wide())
            .arg("files", file_names(sheets))
    }
}

pub(crate) fn file_names(paths: &[std::path::PathBuf]) -> String {
    paths
        .iter()
        .map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        })
        .collect::<Vec<_>>()
        .join(", ")
}
