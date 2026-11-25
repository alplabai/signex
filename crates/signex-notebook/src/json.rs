//! How the block kind, status and date read in a JSON result: as the
//! tokens the file writes (`"measurement"`, `"open"`, `"2026-11-24"`).

use crate::annotation::{AnnotationKind, Status};
use crate::date::IsoDate;

impl serde::Serialize for AnnotationKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.token())
    }
}

impl serde::Serialize for Status {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.token())
    }
}

impl serde::Serialize for IsoDate {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
