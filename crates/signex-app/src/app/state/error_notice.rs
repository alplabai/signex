//! The user-visible error card: a short heading plus the detail line.

/// A user-visible error card: a short heading plus the detail line.
///
/// One card serves every operation that has to stop and tell the user
/// something failed. The heading is data rather than a literal in the
/// view because the card was already shared by operations that are not
/// exports — see [`super::DocumentState::error_notice`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorNotice {
    /// Heading, e.g. "Export Failed". Names the operation, not the cause.
    pub title: String,
    /// What went wrong, in the user's terms. May span several lines.
    pub detail: String,
}

impl ErrorNotice {
    /// An export deliverable (PDF, netlist, BOM) could not be produced.
    pub fn export(detail: impl Into<String>) -> Self {
        Self {
            title: "Export Failed".to_owned(),
            detail: detail.into(),
        }
    }

    /// A write failed, and the operation that needed it did not proceed.
    pub fn save(detail: impl Into<String>) -> Self {
        Self {
            title: "Save Failed".to_owned(),
            detail: detail.into(),
        }
    }

    /// Enable Version Control created the repository but could not finish
    /// setting it up, so the project is not auto-committing.
    pub fn version_control(detail: impl Into<String>) -> Self {
        Self {
            title: "Version Control Setup Failed".to_owned(),
            detail: detail.into(),
        }
    }

    /// Auto-commit refused to start a repository in the project's folder.
    pub fn version_control_paused(detail: impl Into<String>) -> Self {
        Self {
            title: "Version Control Paused".to_owned(),
            detail: detail.into(),
        }
    }

    /// An external page could not be opened in the default browser.
    pub fn link(detail: impl Into<String>) -> Self {
        Self {
            title: "Could Not Open Link".to_owned(),
            detail: detail.into(),
        }
    }

    /// Annotate or Reset Duplicate Designators was refused up front.
    pub fn annotate(detail: impl Into<String>) -> Self {
        Self {
            title: "Annotate Refused".to_owned(),
            detail: detail.into(),
        }
    }

    /// A delete was refused because it would have removed a file the
    /// project does not own (#139).
    pub fn delete(detail: impl Into<String>) -> Self {
        Self {
            title: "Delete Refused".to_owned(),
            detail: detail.into(),
        }
    }

    /// Excluding a document from the project was refused because it would
    /// not do what the user asked, or would lose edits.
    pub fn exclude_refused(detail: impl Into<String>) -> Self {
        Self {
            title: "Exclude Refused".to_owned(),
            detail: detail.into(),
        }
    }

    /// A quit was held because a rename or delete is still running.
    pub fn quit_held(detail: impl Into<String>) -> Self {
        Self {
            title: "App Not Closed".to_owned(),
            detail: detail.into(),
        }
    }

    /// A recovery copy could not be restored; the copy is kept.
    pub fn recovery(detail: impl Into<String>) -> Self {
        Self {
            title: "Recovery Failed".to_owned(),
            detail: detail.into(),
        }
    }

    /// The BOM was written, but validation found issues the user should
    /// still see. Advisory: the file exists.
    pub fn export_advisory(detail: impl Into<String>) -> Self {
        Self {
            title: "BOM Exported with Issues".to_owned(),
            detail: detail.into(),
        }
    }

    /// A rename moved the file but could not finish updating what refers to it.
    pub fn rename(detail: impl Into<String>) -> Self {
        Self {
            title: "Rename Incomplete".to_owned(),
            detail: detail.into(),
        }
    }

    /// A Properties edit of a placed sheet symbol (name / file) was refused
    /// and the sheet was left as it was.
    pub fn sheet_edit(detail: impl Into<String>) -> Self {
        Self {
            title: "Sheet Symbol Not Changed".to_owned(),
            detail: detail.into(),
        }
    }

    /// A rename did not happen, or was undone, and nothing refers to the new name.
    pub fn rename_failed(detail: impl Into<String>) -> Self {
        Self {
            title: "Rename Failed".to_owned(),
            detail: detail.into(),
        }
    }

    /// File ▸ Import ▸ Altium Project could not convert or open the design.
    pub fn import(detail: impl Into<String>) -> Self {
        Self {
            title: "Altium Import Failed".to_owned(),
            detail: detail.into(),
        }
    }

    /// The Altium import succeeded but approximated or skipped something
    /// the user must review. Advisory: the converted files exist.
    pub fn import_advisory(detail: impl Into<String>) -> Self {
        Self {
            title: "Imported with Approximations".to_owned(),
            detail: detail.into(),
        }
    }

    /// A file could not be opened — unreadable, unparseable, or empty
    /// where it must not be (#532).
    pub fn open(detail: impl Into<String>) -> Self {
        Self {
            title: "Open Failed".to_owned(),
            detail: detail.into(),
        }
    }

    /// An edit was refused because the library ships with Signex and is
    /// mounted read-only. Names the library; says why, in plain terms.
    pub fn read_only(library: &str) -> Self {
        Self {
            title: "Library Is Read-Only".to_owned(),
            detail: format!("The library \"{library}\" ships with Signex and is not editable."),
        }
    }

    /// A History restore did not happen.
    pub fn restore(detail: impl Into<String>) -> Self {
        Self {
            title: "Restore Failed".to_owned(),
            detail: detail.into(),
        }
    }

    /// A new project or document could not be created.
    pub fn create(detail: impl Into<String>) -> Self {
        Self {
            title: "Create Failed".to_owned(),
            detail: detail.into(),
        }
    }

    /// The requested action has no backing flow yet.
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            title: "Not Available".to_owned(),
            detail: detail.into(),
        }
    }
}
