//! A `Document` operand: the document a command names, by id or by path.

use std::path::Path;

use signex_types::document_id::DocumentId;

use super::entries::DocumentEntry;
use crate::app::Signex;
use crate::app::state::scope::path_key_with;

/// How a command names its document. The text form is the id (a hyphenated
/// uuid) or an absolute path; the two never overlap, a path is never a uuid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentTarget {
    Id(DocumentId),
    Path(String),
}

impl DocumentTarget {
    /// `text` as a target, or what a target is when it is neither form.
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        if text.chars().any(char::is_control) {
            return Err("a document id or an absolute path, without control characters");
        }
        if let Ok(id) = text.parse::<DocumentId>() {
            return Ok(Self::Id(id));
        }
        if Path::new(text).is_absolute() {
            return Ok(Self::Path(text.to_string()));
        }
        Err("a document id or an absolute path")
    }
}

impl std::fmt::Display for DocumentTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Id(id) => id.fmt(f),
            Self::Path(path) => f.write_str(path),
        }
    }
}

impl Signex {
    /// The document `target` names, among every document listed (loaded or
    /// only referenced by an open project). Never the active document: a
    /// target that names nothing is refused with the reason.
    pub(crate) fn resolve_document_target(
        &self,
        target: &DocumentTarget,
    ) -> Result<DocumentEntry, String> {
        let entries = self.document_entries();
        match target {
            DocumentTarget::Id(id) => entries
                .into_iter()
                .find(|entry| entry.id == Some(*id))
                .ok_or_else(|| format!("no document has the id {id}")),
            DocumentTarget::Path(text) => {
                let given = Path::new(text);
                let key = path_key_with(given, self.identity_fold_for(given));
                entries
                    .into_iter()
                    .find(|entry| {
                        entry.path.as_deref().is_some_and(|path| {
                            path_key_with(path, self.identity_fold_for(path)) == key
                        })
                    })
                    .ok_or_else(|| format!("not an open or referenced document: {text}"))
            }
        }
    }
}
