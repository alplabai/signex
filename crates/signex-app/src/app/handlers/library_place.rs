//! Place-from-library flow handler.
//!
//! v0.9-refactor-2 (DBLib model): the place flow is keyed by
//! `(library_path, table, row_id)` instead of the legacy
//! `(library_path, component_id, version)`. The schematic-engine
//! wire-up is still pending — this stub exists so the dispatcher
//! routing compiles end to end.

use iced::Task;
use std::path::PathBuf;

use signex_library::RowId;

use super::super::contracts::Message;
use super::super::state::Signex;
use crate::library::messages::LibraryMessage;

impl Signex {
    /// Run the place-from-library flow for a `(library, table, row_id)`
    /// tuple. Returns [`Task::none()`] — the message routing exists
    /// end-to-end, but the schematic engine wire-up lands in a
    /// follow-up patch (TODO(v0.9): rebuild against row tier).
    #[allow(dead_code)]
    pub(crate) fn handle_place_library_component(
        &mut self,
        library_path: PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        tracing::info!(
            target: "signex::library",
            library = %library_path.display(),
            table = %table,
            row_id = %row_id,
            "place flow: row-tier engine wire-up ships in a follow-up patch"
        );
        self.library.picker = None;
        Task::none()
    }
}

/// Convert a picker selection into the `PlaceLibraryComponent`
/// dispatch message.
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
