//! Place-from-library flow handler.
//!
//! Wires the picker modal's "Place" button into the active schematic
//! engine. Per LIBRARY_PLAN §5, a placed library symbol carries:
//!
//! * a `LibrarySourceRef { library_id, uuid, version, content_hash }`
//!   so the schematic can detect upstream library updates,
//! * an `embedded` slice (the symbol body) so the schematic still
//!   renders if the source library is missing, and
//! * a `shared_snapshot` of the BOM-relevant shared fields (mpn,
//!   manufacturer, description, value/tolerance/package/rating
//!   parameters) so BOM rollup doesn't have to crack the library
//!   open at export time.
//!
//! The schematic-engine slot for these fields lands in v0.9 Phase 3
//! (`signex_types::schematic::Symbol::source_ref / embedded /
//! shared_snapshot`). Until then this handler:
//!
//!   1. resolves the picked component out of `library.open_libraries`,
//!   2. computes the source-ref + embedded slice + shared snapshot
//!      so the trace records what _would_ be embedded, and
//!   3. closes the picker so the gesture is observably complete.
//!
//! When Phase 3 ships, replace the `tracing::warn!` with the engine
//! command that writes the embedded fields onto the placed
//! `Symbol`.

use iced::Task;
use std::path::PathBuf;

use signex_library::{ComponentId, Version};

use super::super::contracts::Message;
use super::super::state::Signex;
use crate::library::messages::LibraryMessage;

impl Signex {
    /// Run the place-from-library flow for a (library, component, version)
    /// tuple. Always returns [`Task::none()`] in Phase 1 — the message
    /// routing exists end-to-end, but the actual schematic engine
    /// embed lands in Phase 3 (see file-level docs).
    pub(crate) fn handle_place_library_component(
        &mut self,
        library_path: PathBuf,
        component_id: ComponentId,
        version: Version,
    ) -> Task<Message> {
        // Resolve the component head off the open library so the trace
        // records the same `(library_id, uuid, version, content_hash)`
        // tuple Phase 3 would write to the placed Symbol.
        let Some(lib) = self.library.library_at(&library_path) else {
            tracing::warn!(
                target: "signex::library",
                path = %library_path.display(),
                "place flow: library not open — picker likely outpaced close_library"
            );
            // Close the picker so the user sees the gesture completed.
            self.library.picker = None;
            return Task::none();
        };

        let library_id = lib.adapter.manifest().library.library_id;

        // Re-read the component to grab the head revision's content
        // hash + symbol body. Adapter calls are cheap for the local
        // git adapter; we don't bother caching across the session.
        let component = match lib.adapter.get_component(component_id) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    error = %e,
                    library = %library_path.display(),
                    component = %component_id,
                    "place flow: component lookup failed"
                );
                self.library.picker = None;
                return Task::none();
            }
        };

        let revision = match component.revisions.iter().find(|r| r.version == version) {
            Some(r) => r,
            None => component
                .head_revision()
                .or_else(|| component.revisions.last())
                .expect("non-empty component has at least one revision"),
        };

        // Phase 3 emits the engine command here. Until the schematic
        // Symbol type grows the embedded fields the only thing we can
        // do is leave a structured trace + close the picker so the
        // message routing is still exercised end-to-end.
        let embedded_pin_bytes = revision.schematic.symbol.sexpr.len();
        let shared_snapshot = revision.shared.slice_for_embed();
        tracing::warn!(
            target: "signex::library",
            library_id = %library_id,
            component_id = %component_id,
            version = %revision.version,
            content_hash = %hex_short(&revision.content_hash),
            embedded_bytes = embedded_pin_bytes,
            mpn = %shared_snapshot.mpn,
            "schematic engine wire-up shipped in Phase 3 — picker dismissed"
        );

        // Always close the picker — placement gesture is observably
        // complete from the user's POV regardless of Phase 3 wire-up.
        self.library.picker = None;
        Task::none()
    }
}

/// Render the first 8 bytes of a SHA-256 hash as hex for trace logs.
/// Keeps the log line short while still letting a developer correlate
/// across runs.
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
pub(crate) fn place_message_from_picker(
    library_path: PathBuf,
    component_id: ComponentId,
    version: Version,
) -> Message {
    Message::Library(LibraryMessage::PlaceLibraryComponent {
        library_path,
        component_id,
        version,
    })
}
