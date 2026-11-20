//! The words of a finished Generate: what was written, why nothing was,
//! and what the files leave out.

use signex_i18n::{I18n, Localized, ids, t};
use signex_output::fab::{FabError, FabNote};

use super::{FabProblem, FabWritten};

/// What the assembly drawings or the STEP model leave out (#2489).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardNote {
    /// Holes left out of the STEP board body, each meeting a hole cut
    /// before it, as in the 3D view.
    HolesLeftOut(usize),
    /// A part's model could not be read; the STEP model holds its plain
    /// body. The model's source as text, and why.
    ModelUnread { source: String, why: String },
    /// Characters of the assembly PDF that no bundled font covers.
    CharsNotCovered(String),
}

/// The line that says what Generate wrote and where.
pub(crate) fn written_text(written: &FabWritten, i18n: &I18n) -> Localized {
    let folder = written.folder.display().to_string();
    t!(
        i18n,
        ids::FAB_OUTPUTS_WRITTEN,
        "count" => written.files.len(),
        "folder" => folder.as_str()
    )
}

/// Why Generate wrote nothing.
pub(crate) fn problem_text(problem: &FabProblem, i18n: &I18n) -> Localized {
    match problem {
        FabProblem::Refused(FabError::NothingSelected) => {
            t!(i18n, ids::FAB_OUTPUTS_NOTHING_SELECTED)
        }
        FabProblem::Refused(FabError::BadName) => t!(i18n, ids::FAB_OUTPUTS_BAD_NAME),
        FabProblem::Refused(FabError::NoOutline) => t!(i18n, ids::FAB_OUTPUTS_NO_OUTLINE),
        FabProblem::BoardGone => t!(i18n, ids::FAB_OUTPUTS_BOARD_GONE),
        FabProblem::WriteFailed(why) => {
            t!(i18n, ids::FAB_OUTPUTS_WRITE_FAILED, "why" => why.as_str())
        }
        FabProblem::PanelRefused(why) => {
            t!(i18n, ids::FAB_OUTPUTS_PANEL_REFUSED, "why" => why.as_str())
        }
        FabProblem::AssemblyRefused(why) => {
            t!(i18n, ids::FAB_OUTPUTS_ASSEMBLY_REFUSED, "why" => why.as_str())
        }
        FabProblem::StepRefused(why) => {
            t!(i18n, ids::FAB_OUTPUTS_STEP_REFUSED, "why" => why.as_str())
        }
    }
}

/// What the written fabrication files leave out.
pub(crate) fn note_text(note: FabNote, i18n: &I18n) -> Localized {
    let count = note.count();
    match note {
        FabNote::ZonesNotFilled { .. } => t!(i18n, ids::FAB_OUTPUTS_NOTE_ZONES, "count" => count),
        FabNote::PartsWithoutDesignator { .. } => {
            t!(i18n, ids::FAB_OUTPUTS_NOTE_UNNAMED_PARTS, "count" => count)
        }
        FabNote::ItemsSkipped { .. } => t!(i18n, ids::FAB_OUTPUTS_NOTE_SKIPPED, "count" => count),
    }
}

/// What the written drawings or model leave out.
pub(crate) fn board_note_text(note: &BoardNote, i18n: &I18n) -> Localized {
    match note {
        BoardNote::HolesLeftOut(count) => {
            t!(i18n, ids::FAB_OUTPUTS_NOTE_HOLES, "count" => *count)
        }
        BoardNote::ModelUnread { source, why } => t!(
            i18n,
            ids::FAB_OUTPUTS_NOTE_MODEL_UNREAD,
            "source" => source.as_str(),
            "why" => why.as_str()
        ),
        BoardNote::CharsNotCovered(chars) => {
            t!(i18n, ids::FAB_OUTPUTS_NOTE_CHARS, "chars" => chars.as_str())
        }
    }
}

/// The older files of the same stem left in the folder; `None` when there
/// are none.
pub(crate) fn stale_text(written: &FabWritten, i18n: &I18n) -> Option<Localized> {
    if written.stale.is_empty() {
        return None;
    }
    let files = written.stale.join(", ");
    Some(t!(
        i18n,
        ids::FAB_OUTPUTS_STALE,
        "count" => written.stale.len(),
        "files" => files.as_str()
    ))
}

/// How much a note of a finished Generate matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteSeverity {
    /// The set leaves something out: a zone not filled, a part with no
    /// designator, a model unread.
    Warning,
    /// Older files beside the set: sent with it they can break a fab order.
    Error,
}

/// Every note of `written`, in the order the dialog lists them, each with
/// its severity: what the files and the documents leave out, then the stale
/// files.
pub(crate) fn outcome_notes(written: &FabWritten, i18n: &I18n) -> Vec<(NoteSeverity, Localized)> {
    let mut notes: Vec<(NoteSeverity, Localized)> = written
        .notes
        .iter()
        .map(|n| (NoteSeverity::Warning, note_text(*n, i18n)))
        .collect();
    notes.extend(
        written
            .board_notes
            .iter()
            .map(|n| (NoteSeverity::Warning, board_note_text(n, i18n))),
    );
    notes.extend(stale_text(written, i18n).map(|line| (NoteSeverity::Error, line)));
    notes
}

/// The Messages line of a finished Generate: what was written, then every
/// note and the stale files. It is a warning when the set is incomplete
/// or the folder holds older files, so it is not read as a clean set.
pub(crate) fn written_message(written: &FabWritten, i18n: &I18n) -> (String, bool) {
    let mut parts = vec![written_text(written, i18n).into_string()];
    parts.extend(
        written
            .notes
            .iter()
            .map(|n| note_text(*n, i18n).into_string()),
    );
    parts.extend(
        written
            .board_notes
            .iter()
            .map(|n| board_note_text(n, i18n).into_string()),
    );
    parts.extend(stale_text(written, i18n).map(Localized::into_string));
    let is_warning = parts.len() > 1;
    (parts.join(" "), is_warning)
}
