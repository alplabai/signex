//! The `.snxlib` token follows the content (`docs/FORMAT_VERSIONS.md`): a
//! library stays `snxlib/2`, byte for byte, until a manufacturer part
//! carries a hand-entered stock or lifecycle (#3046), which writes
//! `snxlib/2.1`.

use super::{FORMAT_TOKEN, FORMAT_TOKEN_MANUAL_SUPPLY, LibraryFile, SnxlibManifest};
use crate::manufacturer::MANUAL_SUPPLY_KEYS;

/// The part-table cells that hold manufacturer parts.
const PART_COLUMNS: [&str; 2] = ["primary_mpn", "alternates"];

impl LibraryFile {
    /// The lowest token that holds this file's content. From content, so a
    /// library whose last hand-entered value goes drops back to `snxlib/2`.
    ///
    /// Looks for the keys as `serde_json` writes them: inside a JSON string
    /// a quote is escaped, so a note cannot fake one. A false match would
    /// only raise the token, never lose a value.
    pub fn format_token(&self) -> &'static str {
        let manual = self.tables.values().any(|table| {
            table.rows.iter().any(|row| {
                PART_COLUMNS.iter().any(|column| {
                    row.cells
                        .get(*column)
                        .is_some_and(|cell| MANUAL_SUPPLY_KEYS.iter().any(|k| cell.contains(k)))
                })
            })
        });
        if manual {
            FORMAT_TOKEN_MANUAL_SUPPLY
        } else {
            FORMAT_TOKEN
        }
    }

    /// The manifest as a write emits it: with [`Self::format_token`].
    pub(super) fn written_manifest(&self) -> SnxlibManifest {
        SnxlibManifest {
            format: self.format_token().to_string(),
            ..self.manifest.clone()
        }
    }
}
