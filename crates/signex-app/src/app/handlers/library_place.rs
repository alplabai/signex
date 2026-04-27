//! Place-from-library flow handler.
//!
//! v0.9-refactor-2 (DBLib model): the place flow is keyed by
//! `(library_path, table, row_id)`. Resolution path:
//!
//!   1. Look up the open library by path → grab its `library_id`.
//!   2. Read the picked row through the mounted adapter
//!      (`LibrarySet::get(library_id)`).
//!   3. Resolve the row's `symbol_ref` via `LibrarySet::resolve_symbol`
//!      so the trace records the embedded-symbol presence and pin
//!      count alongside the row's content hash.
//!   4. Trace the structured place-flow fields and close the picker.
//!
//! The actual schematic-engine embed (writing the
//! `LibrarySourceRef { library_id, uuid, version, content_hash }` +
//! embedded slice + shared snapshot onto the placed
//! `signex_types::schematic::Symbol`) lands in v0.9 Phase 3 once the
//! schematic-side schema slots exist. Until then this handler
//! exercises the dispatch routing end-to-end and gives operators a
//! correlatable trace per place gesture.

use iced::Task;
use std::path::PathBuf;

use signex_library::RowId;

use super::super::contracts::Message;
use super::super::state::Signex;
use crate::library::messages::LibraryMessage;

impl Signex {
    /// Resolve a `(library, table, row_id)` selection through the
    /// mounted adapter + `LibrarySet`, then close the picker. Always
    /// returns [`Task::none()`] — Phase 3 will swap the structured
    /// trace for an engine command that writes the embedded fields
    /// onto the placed `Symbol`.
    pub(crate) fn handle_place_library_component(
        &mut self,
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        let Some(lib) = self.library.library_at(&library_path) else {
            tracing::warn!(
                target: "signex::library",
                path = %library_path.display(),
                "place flow: library not open — picker likely outpaced close_library"
            );
            self.library.picker = None;
            return Task::none();
        };
        let library_id = lib.library_id;

        let Some(adapter) = self.library.set.get(library_id) else {
            tracing::warn!(
                target: "signex::library",
                library_id = %library_id,
                path = %library_path.display(),
                "place flow: adapter not mounted on LibrarySet"
            );
            self.library.picker = None;
            return Task::none();
        };

        let row = match adapter.read_row(&table, row_id) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    error = %e,
                    library = %library_path.display(),
                    table = %table,
                    row = %row_id,
                    "place flow: row lookup failed"
                );
                self.library.picker = None;
                return Task::none();
            }
        };

        let symbol = self.library.set.resolve_symbol(&row.symbol_ref);
        let pin_count = symbol.as_ref().map(|s| s.pins.len()).unwrap_or(0);

        tracing::warn!(
            target: "signex::library",
            library_id = %library_id,
            table = %table,
            row_id = %row.row_id,
            internal_pn = %row.internal_pn,
            mpn = %row.primary_mpn.mpn,
            manufacturer = %row.primary_mpn.manufacturer,
            symbol_resolved = symbol.is_some(),
            pin_count,
            content_hash = %hex_short(&row.content_hash),
            "schematic engine wire-up ships in v0.9 Phase 3 — picker dismissed"
        );

        self.library.picker = None;
        Task::none()
    }
}

/// Render the first 8 bytes of a SHA-256 hash as hex for trace logs.
fn hex_short(hash: &[u8; 32]) -> String {
    let mut s = String::with_capacity(16);
    for b in &hash[..8] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Convert a picker selection into the `PlaceLibraryComponent`
/// dispatch message. Pulled out so the picker handler stays small
/// and the dispatcher's "what message do I emit?" logic lives next
/// to the rest of the place-flow code.
#[allow(dead_code)]
pub(crate) fn place_message_from_picker(
    library_path: PathBuf,
    table: String,
    row_id: RowId,
) -> Message {
    Message::Library(LibraryMessage::PlaceLibraryComponent {
        library_path,
        table,
        row_id,
    })
}
