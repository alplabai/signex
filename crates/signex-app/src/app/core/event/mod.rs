//! What the core reports back after it changed a document (ADR-0006, slice
//! 2): the GUI adapter projects these into its own state.

mod document_changed;
mod notice;

pub(crate) use document_changed::{ChangeCause, DocumentChanged};
pub(crate) use notice::{
    Notice, NoticeBody, NoticeLevel, engine_notices, legacy_warning, locked_skips_line,
    read_only_refusal_message,
};

/// One thing the core reports, in the order it happened.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CoreEvent {
    DocumentChanged(DocumentChanged),
    Notice(Notice),
}

impl From<DocumentChanged> for CoreEvent {
    fn from(changed: DocumentChanged) -> Self {
        Self::DocumentChanged(changed)
    }
}

impl From<Notice> for CoreEvent {
    fn from(notice: Notice) -> Self {
        Self::Notice(notice)
    }
}
