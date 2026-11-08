//! The persistent identity of a document (#2172).
//!
//! A document is one file a person or an agent can name: a sheet, a board, a
//! symbol or footprint container, a project, a component row. Its id is made
//! with the document, stored in the document's own file and read back on every
//! open, so it outlives tabs, project closes and restarts. Which stored field
//! carries it per kind (the sheet's own uuid, the container's `file_uuid`,
//! the project's `document_id`, the row id) is in `docs/DOCUMENT_IDS.md`.
//!
//! Text form is the hyphenated uuid, as in every file.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The id of one document. A newtype over [`Uuid`], so a document id cannot be
/// passed where an element id is meant; the uuid is a v7 like every new id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocumentId(Uuid);

impl DocumentId {
    /// A fresh id for a document being created.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// The id stored as `uuid`. The nil uuid names no document: a file that
    /// carries it (a test fixture, a hand-made file) has none yet.
    pub fn from_uuid(uuid: Uuid) -> Option<Self> {
        (!uuid.is_nil()).then_some(Self(uuid))
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(f)
    }
}

impl FromStr for DocumentId {
    type Err = DocumentIdError;

    /// The hyphenated form only: the form a file stores and a command prints.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || DocumentIdError(text.to_string());
        if text.len() != 36 {
            return Err(bad());
        }
        let uuid = Uuid::parse_str(text).map_err(|_| bad())?;
        Self::from_uuid(uuid).ok_or_else(bad)
    }
}

/// The text is not a document id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentIdError(String);

impl fmt::Display for DocumentIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` is not a document id (a hyphenated uuid)", self.0)
    }
}

impl std::error::Error for DocumentIdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_id_is_a_v7_uuid_and_prints_hyphenated() {
        let id = DocumentId::new();
        assert_eq!(id.as_uuid().get_version_num(), 7);
        let text = id.to_string();
        assert_eq!(text.len(), 36);
        assert_eq!(text.parse::<DocumentId>(), Ok(id));
    }

    #[test]
    fn two_new_ids_differ() {
        assert_ne!(DocumentId::new(), DocumentId::new());
    }

    #[test]
    fn the_nil_uuid_is_no_document() {
        assert_eq!(DocumentId::from_uuid(Uuid::nil()), None);
        assert!(
            "00000000-0000-0000-0000-000000000000"
                .parse::<DocumentId>()
                .is_err()
        );
    }

    #[test]
    fn only_the_hyphenated_form_parses() {
        let id = DocumentId::new();
        assert!(
            id.as_uuid()
                .simple()
                .to_string()
                .parse::<DocumentId>()
                .is_err()
        );
        assert!(format!("{{{id}}}").parse::<DocumentId>().is_err());
        assert!("not-an-id".parse::<DocumentId>().is_err());
        assert!("".parse::<DocumentId>().is_err());
    }

    #[test]
    fn it_serializes_as_the_plain_uuid_string() {
        let id = DocumentId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<DocumentId>(&json).unwrap(), id);
    }
}
