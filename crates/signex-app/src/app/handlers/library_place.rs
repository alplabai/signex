//! Place-from-library flow handler.
//!
//! WS-E (refactor): the trace previously logged the embedded symbol
//! sexpr length and the BOM-relevant `SharedSide` slice. Both went
//! away with the v0.9 refactor — symbols and footprints are now
//! addressed by `PrimitiveRef` and BOM-rollup pulls fields off the
//! `Revision` directly. The handler is preserved as a thin shim so
//! the message routing still works; WS-F wires the engine command
//! that places the resolved primitives into the active schematic.

use iced::Task;
use std::path::PathBuf;

use signex_library::{ComponentId, Version};

use super::super::contracts::Message;
use super::super::state::Signex;
use crate::library::messages::LibraryMessage;

impl Signex {
    /// Run the place-from-library flow for a (library, component, version)
    /// tuple. Returns [`Task::none()`] in WS-E — the message routing
    /// exists end-to-end, but the actual schematic engine embed lands
    /// in WS-F (see file-level docs).
    #[allow(dead_code)]
    pub(crate) fn handle_place_library_component(
        &mut self,
        library_path: PathBuf,
        component_id: ComponentId,
        version: Version,
    ) -> Task<Message> {
        let library_id = self
            .library
            .library_at(&library_path)
            .map(|lib| lib.library_id);
        let Some(library_id) = library_id else {
            tracing::warn!(
                target: "signex::library",
                path = %library_path.display(),
                "place flow: library not open — picker likely outpaced close_library"
            );
            self.library.picker = None;
            return Task::none();
        };

        let component = match self.library.set.adapter(library_id) {
            Some(adapter) => match adapter.get_component(component_id) {
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
            },
            None => {
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
        // WS-F note: the pre-refactor place flow embedded the symbol
        // sexpr + a `SharedSlice` snapshot; both paths went away with
        // WS-B. Phase 3 will resolve the symbol primitive via
        // `LibrarySet::resolve_symbol(rev.symbol_ref)` and embed THAT
        // typed graph instead.
        // TODO(merge-with-WS-E): replace the trace below with the
        // actual engine command writing the bound `Symbol`.
        tracing::warn!(
            target: "signex::library",
            library_id = %library_id,
            component_id = %component_id,
            version = %revision.version,
            symbol_ref = %revision.symbol_ref,
            content_hash = %hex_short(&revision.content_hash),
            symbol_ref = %revision.symbol_ref,
            mpn = %revision.primary_mpn.mpn,
            "schematic engine wire-up shipped in Phase 3 — picker dismissed"
        );

        self.library.picker = None;
        Task::none()
    }
}

/// Render the first 8 bytes of a SHA-256 hash as hex for trace logs.
#[allow(dead_code)]
fn hex_short(hash: &[u8; 32]) -> String {
    let mut s = String::with_capacity(16);
    for b in &hash[..8] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Convert a picker selection into the `PlaceLibraryComponent`
/// dispatch message.
#[allow(dead_code)]
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
