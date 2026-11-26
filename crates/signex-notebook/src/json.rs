//! How a date reads in a JSON result: as the token the file writes
//! (`"2026-11-24"`). The block kind and status serialise next to their
//! types in `annotation.rs`.

use crate::date::IsoDate;

impl serde::Serialize for IsoDate {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
