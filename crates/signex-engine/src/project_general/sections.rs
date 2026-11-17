//! The Project tab's Sheets, Variants, Libraries, ERC and annotation and
//! Version control edits (#1689-#1693). Each is one step on the project's
//! General history; the checks that refuse one live in `signex-types`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use signex_types::project::{AnnotateUndo, ListMove, ProjectData, VariantProblem};

/// One edit of a Project tab section. Plain data, so a script can emit it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "snake_case")]
pub enum ProjectSectionCommand {
    /// Move a sheet one page up or down (#1689).
    MoveSheet {
        filename: String,
        step: ListMove,
    },
    /// Add a variant after the others (#1690).
    AddVariant {
        name: String,
    },
    RenameVariant {
        from: String,
        to: String,
    },
    RemoveVariant {
        name: String,
    },
    /// `None` makes Base the active variant.
    SetActiveVariant {
        name: Option<String>,
    },
    /// Move a library one step up or down the list (#1691).
    MoveLibrary {
        path: PathBuf,
        step: ListMove,
    },
    /// The project's annotate undo choice; `None` follows Preferences
    /// (#1692). Clearing it over an unreadable section changes nothing.
    SetAnnotateUndo {
        choice: Option<AnnotateUndo>,
    },
    /// Commit every save to the project's repository (#1693).
    SetVersionControl {
        on: bool,
    },
}

impl ProjectSectionCommand {
    /// Apply to `data`. A refused edit changes nothing; a move at the end of
    /// its list is not refused, it just changes nothing.
    pub fn apply(&self, data: &mut ProjectData) -> Result<(), VariantProblem> {
        match self {
            Self::MoveSheet { filename, step } => {
                data.move_sheet(filename, *step);
            }
            Self::AddVariant { name } => data.add_variant(name)?,
            Self::RenameVariant { from, to } => data.rename_variant(from, to)?,
            Self::RemoveVariant { name } => data.remove_variant(name)?,
            Self::SetActiveVariant { name } => data.set_active_variant(name.as_deref())?,
            Self::MoveLibrary { path, step } => {
                data.move_library(path, *step);
            }
            Self::SetAnnotateUndo { choice } => {
                if !(choice.is_none() && data.has_unreadable_annotation()) {
                    data.set_annotate_undo(*choice);
                }
            }
            Self::SetVersionControl { on } => data.enable_git = *on,
        }
        Ok(())
    }
}

/// The section fields one undo step holds. Sheets and libraries are held as
/// an order only: their lists have other writers (see
/// `signex_types::project::ListMove`'s module).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SectionsSnapshot {
    sheet_order: Vec<String>,
    library_order: Vec<PathBuf>,
    variants: Vec<String>,
    active_variant: Option<String>,
    enable_git: bool,
}

impl SectionsSnapshot {
    pub(super) fn of(data: &ProjectData) -> Self {
        Self {
            sheet_order: data.sheet_order(),
            library_order: data.library_order(),
            variants: data.variant_definitions.clone(),
            active_variant: data.active_variant.clone(),
            enable_git: data.enable_git,
        }
    }

    pub(super) fn restore(self, data: &mut ProjectData) {
        data.restore_sheet_order(&self.sheet_order);
        data.restore_library_order(&self.library_order);
        data.variant_definitions = self.variants;
        data.active_variant = self.active_variant;
        data.enable_git = self.enable_git;
    }
}
