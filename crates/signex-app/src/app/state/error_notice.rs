//! The user-visible error card: a short heading, the readable sentence and,
//! apart from it, the system part (path and OS error).

use signex_i18n::{I18n, MessageId, Translatable, UserText, ids};

/// A user-visible error card: a short heading plus the detail line.
///
/// One card serves every operation that has to stop and tell the user
/// something failed. The heading is data rather than a literal in the
/// view because the card was already shared by operations that are not
/// exports — see [`super::DocumentState::error_notice`]. Both texts are kept
/// as catalog messages and rendered at view time (#2058), so a language
/// switch re-renders an open card.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorNotice {
    /// Heading, e.g. "Export Failed". Names the operation, not the cause.
    pub title: Translatable,
    /// What went wrong, in the user's terms. May span several lines.
    pub detail: Translatable,
    /// The system's own words (a path, an OS or library error chain), shown
    /// as they are in a mono block under the sentence; `None` when the
    /// sentence says it all.
    pub system: Option<UserText>,
    /// A choice the card offers beside dismissing it; `None` for a plain
    /// notice, whose only button is OK.
    pub action: Option<NoticeAction>,
    /// A sentence of the card's own after the detail, e.g. that the edits
    /// are still open and unsaved (mockup row 11); `None` for most cards.
    /// Kept out of [`Self::detail_line_en_us`].
    pub note: Option<MessageId>,
}

/// How bad what the card reports is; picks its heading glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeSeverity {
    /// The operation failed or was refused (red).
    Error,
    /// The operation stopped to ask, or finished with something to review
    /// (amber).
    Warn,
}

/// The headings whose card is a warning; every other heading is an error.
const WARNING_TITLES: [MessageId; 5] = [
    ids::NOTICE_DISK_CHANGED_TITLE,
    ids::NOTICE_DISK_MISSING_TITLE,
    ids::NOTICE_DISK_FOLDER_MISSING_TITLE,
    ids::NOTICE_IMPORTED_WITH_APPROXIMATIONS,
    ids::NOTICE_BOM_EXPORTED_WITH_ISSUES,
];

/// The choices of a File Changed on Disk card with what each does, as
/// `(button label, effect)`, in the order the card lists them. Save As is
/// left out where it cannot run (a board has no Save As).
pub fn disk_changed_choices(save_as: bool) -> Vec<(MessageId, MessageId)> {
    let mut choices = Vec::with_capacity(3);
    if save_as {
        choices.push((
            ids::DIALOG_DISK_CONFLICT_SAVE_AS,
            ids::NOTICE_DISK_CHANGED_SAVE_AS,
        ));
    }
    choices.push((ids::DIALOG_OVERWRITE, ids::NOTICE_DISK_CHANGED_OVERWRITE));
    choices.push((
        ids::DIALOG_DISK_CONFLICT_RELOAD,
        ids::NOTICE_DISK_CHANGED_RELOAD,
    ));
    choices
}

/// The choice a notice card offers. Dismissing the card is always "Cancel".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeAction {
    /// A save found its file changed or gone since the document was loaded or
    /// last saved (#1826): overwrite it, reload it, or cancel.
    DiskChanged {
        path: std::path::PathBuf,
        drift: crate::persist::DiskDrift,
    },
    /// A save of a project read from a newer minor was refused (#2904): the
    /// card offers Save Project As… for it (#2908).
    SaveProjectAs { project: super::ProjectId },
}

impl ErrorNotice {
    pub fn new(title: impl Into<Translatable>, detail: impl Into<Translatable>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            system: None,
            action: None,
            note: None,
        }
    }

    /// `self` with `text` as its system part: the path and the OS or library
    /// error, kept apart from the readable sentence.
    #[must_use]
    pub fn with_system(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        self.system = (!text.trim().is_empty()).then(|| UserText::new(text));
        self
    }

    /// `self` with `error`'s chain as its system part, one cause per line:
    /// the path line over the OS error line (mockup row 11). A middle
    /// context (a rename step) keeps its own line: it says where the save
    /// broke.
    #[must_use]
    pub fn with_error_chain(self, error: &anyhow::Error) -> Self {
        let lines: Vec<String> = error.chain().map(ToString::to_string).collect();
        self.with_system(lines.join("\n"))
    }

    /// The heading in `i18n`'s language.
    pub fn title_text(&self, i18n: &I18n) -> String {
        self.title.render(i18n)
    }

    /// The detail in `i18n`'s language.
    pub fn detail_text(&self, i18n: &I18n) -> String {
        self.detail.render(i18n)
    }

    /// The detail and the card's note after it, one space apart: the
    /// readable sentence as the card shows it.
    pub fn sentence_text(&self, i18n: &I18n) -> String {
        let detail = self.detail_text(i18n);
        match self.note {
            Some(note) => format!("{detail} {}", i18n.message_plain(note, &[])),
            None => detail,
        }
    }

    /// The sentence and, after a colon, the system part, as one en-US line:
    /// the text that leaves the screen (a CLI reason), as the card read
    /// before the system part had its own block.
    pub fn detail_line_en_us(&self) -> String {
        let detail = self.detail.render(&I18n::en_us());
        match &self.system {
            Some(system) => {
                let system: Vec<&str> = system.as_str().lines().collect();
                format!("{detail}: {}", system.join(": "))
            }
            None => detail,
        }
    }

    /// Red for a failure or refusal, amber for a card that asks or warns,
    /// keyed on the heading.
    pub fn severity(&self) -> NoticeSeverity {
        match self.title.id() {
            Some(id) if WARNING_TITLES.contains(&id) => NoticeSeverity::Warn,
            _ => NoticeSeverity::Error,
        }
    }

    /// What the card's Copy button puts on the clipboard for a bug report:
    /// the heading, the sentence (with its note) and the system part, a
    /// blank line apart.
    pub fn copy_text(&self, i18n: &I18n) -> String {
        let mut parts = vec![self.title_text(i18n), self.sentence_text(i18n)];
        if let Some(system) = &self.system {
            parts.push(system.as_str().to_owned());
        }
        parts.retain(|part| !part.trim().is_empty());
        parts.join("\n\n")
    }

    /// Whether the card offers Save As…: a File Changed on Disk card of a
    /// document that has a Save As (a sheet, symbol or footprint). Named
    /// kinds only: a board, or any kind added later, offers none until its
    /// Save As exists.
    pub fn offers_save_as(&self) -> bool {
        use crate::app::file_extension::has_extension;
        matches!(
            &self.action,
            Some(NoticeAction::DiskChanged {
                path,
                drift: crate::persist::DiskDrift::Changed,
            }) if ["snxsch", "snxsym", "snxfpt"]
                .iter()
                .any(|ext| has_extension(path, ext))
        )
    }

    /// A save would overwrite a change made outside Signex (or re-create a
    /// deleted file); the user chooses (#1826).
    pub fn disk_changed(path: std::path::PathBuf, drift: crate::persist::DiskDrift) -> Self {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let (title, detail) = match drift {
            crate::persist::DiskDrift::Changed => (
                ids::NOTICE_DISK_CHANGED_TITLE,
                Translatable::new(ids::NOTICE_DISK_CHANGED),
            ),
            crate::persist::DiskDrift::Missing => (
                ids::NOTICE_DISK_MISSING_TITLE,
                Translatable::new(ids::NOTICE_DISK_MISSING).arg("path", path.display().to_string()),
            ),
            crate::persist::DiskDrift::FolderMissing => (
                ids::NOTICE_DISK_FOLDER_MISSING_TITLE,
                Translatable::new(ids::NOTICE_DISK_FOLDER_MISSING).arg(
                    "folder",
                    path.parent().unwrap_or(&path).display().to_string(),
                ),
            ),
        };
        Self {
            title: title.into(),
            detail: detail.arg("name", name),
            system: None,
            action: Some(NoticeAction::DiskChanged { path, drift }),
            note: None,
        }
    }

    /// An export deliverable (PDF, netlist, BOM) could not be produced.
    pub fn export(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_EXPORT_FAILED, detail)
    }

    /// A write of open edits failed, and the operation that needed it did
    /// not proceed. The card says the edits are still open and unsaved
    /// (mockup row 11).
    pub fn save(detail: impl Into<Translatable>) -> Self {
        Self {
            note: Some(ids::NOTICE_SAVE_FAILED_EDITS_KEPT),
            ..Self::new(ids::NOTICE_SAVE_FAILED, detail)
        }
    }

    /// A write failed that leaves no open edits behind, such as the global
    /// library list, whose change is dropped. Same heading as
    /// [`Self::save`], without its note.
    pub fn save_with_no_open_edits(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_SAVE_FAILED, detail)
    }

    /// Enable Version Control created the repository but could not finish
    /// setting it up, so the project is not auto-committing.
    pub fn version_control(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_VERSION_CONTROL_FAILED, detail)
    }

    /// Auto-commit refused to start a repository in the project's folder.
    pub fn version_control_paused(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_VERSION_CONTROL_PAUSED, detail)
    }

    /// An external page could not be opened in the default browser.
    pub fn link(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_LINK_FAILED, detail)
    }

    /// Annotate or Reset Duplicate Designators was refused up front.
    pub fn annotate(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_ANNOTATE_REFUSED, detail)
    }

    /// A delete was refused because it would have removed a file the
    /// project does not own (#139).
    pub fn delete(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_DELETE_REFUSED, detail)
    }

    /// Excluding a document from the project was refused because it would
    /// not do what the user asked, or would lose edits.
    pub fn exclude_refused(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_EXCLUDE_REFUSED, detail)
    }

    /// A quit was held because a rename or delete is still running.
    pub fn quit_held(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_QUIT_HELD, detail)
    }

    /// A project Replace was refused before anything changed.
    pub fn replace(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_REPLACE_REFUSED, detail)
    }

    /// A library edit was refused while that library is being moved.
    pub fn library_change(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_LIBRARY_CHANGE_REFUSED, detail)
    }

    /// A recovery copy could not be restored; the copy is kept.
    pub fn recovery(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_RECOVERY_FAILED, detail)
    }

    /// The BOM was written, but validation found issues the user should
    /// still see. Advisory: the file exists.
    pub fn export_advisory(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_BOM_EXPORTED_WITH_ISSUES, detail)
    }

    /// A rename moved the file but could not finish updating what refers to it.
    pub fn rename(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_RENAME_INCOMPLETE, detail)
    }

    /// A Properties edit of a placed sheet symbol (name / file) was refused
    /// and the sheet was left as it was.
    pub fn sheet_edit(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_SHEET_SYMBOL_NOT_CHANGED, detail)
    }

    /// A rename did not happen, or was undone, and nothing refers to the new name.
    pub fn rename_failed(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_RENAME_FAILED, detail)
    }

    /// File ▸ Import ▸ Altium Project could not convert or open the design.
    pub fn import(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_IMPORT_FAILED, detail)
    }

    /// The Altium import succeeded but approximated or skipped something
    /// the user must review. Advisory: the converted files exist.
    pub fn import_advisory(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_IMPORTED_WITH_APPROXIMATIONS, detail)
    }

    /// A file could not be opened — unreadable, unparseable, or empty
    /// where it must not be (#532).
    pub fn open(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_OPEN_FAILED, detail)
    }

    /// An edit was refused because the library ships with Signex and is
    /// mounted read-only. Names the library; says why, in plain terms.
    pub fn read_only(library: &str) -> Self {
        Self::new(
            ids::NOTICE_LIBRARY_READ_ONLY,
            Translatable::new(ids::NOTICE_LIBRARY_READ_ONLY_DETAIL).arg("library", library),
        )
    }

    /// An edit was refused because the library is open read-only: its Git
    /// history is missing (Skip for now, mockup row 138).
    pub fn history_missing(library: &str) -> Self {
        Self::new(
            ids::NOTICE_LIBRARY_READ_ONLY,
            Translatable::new(ids::NOTICE_LIBRARY_HISTORY_MISSING_DETAIL).arg("library", library),
        )
    }

    /// A History restore did not happen.
    pub fn restore(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_RESTORE_FAILED, detail)
    }

    /// A new project or document could not be created.
    pub fn create(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_CREATE_FAILED, detail)
    }

    /// The requested action has no backing flow yet.
    pub fn unavailable(detail: impl Into<Translatable>) -> Self {
        Self::new(ids::NOTICE_NOT_AVAILABLE, detail)
    }
}

/// Text that is not ours (an OS or library error, an `anyhow` chain, a path,
/// user content), as a whole notice detail shown as it is. The epic keeps
/// `anyhow` chains as English detail under a translated heading (#2053 row 5).
pub fn verbatim(text: impl Into<String>) -> Translatable {
    Translatable::verbatim(UserText::new(text))
}

/// A detail sentence that IS ours but is built in English by a producer that
/// also feeds the Messages panel or an error chain, which the catalog has not
/// reached yet (#2061). Shown as it is; grep `untranslated(` for what is left.
pub fn untranslated(text: impl Into<String>) -> Translatable {
    Translatable::verbatim(UserText::new(text))
}
