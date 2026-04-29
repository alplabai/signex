//! Signex native library container — `.snxlib`.
//!
//! A `.snxlib` is a TOML+TSV envelope (same wire pattern as `.snxsch`
//! / `.snxpcb`) that lists the components belonging to a library
//! package. Components are tabular rows — they reference symbol /
//! footprint / sim primitives by UUID, but the primitives themselves
//! live in standalone `.snxsym` / `.snxfpt` / `.snxsim` files inside
//! the package directory. Components are *never* exposed as files in
//! the project tree (a library can have thousands of them).
//!
//! v0.10.0 ships only the read-only browser surface — the in-memory
//! types in this module + a `LibraryBrowser` tab kind in `signex-app`.
//! Future sub-releases (v0.10.7+) layer Component Editor flows on
//! top of these same types without reshaping the schema.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// In-memory representation of a `.snxlib` package.
///
/// Mirrors the [`SnxLibrary`](crate::format::SnxLibrary) wire envelope
/// — every TSV row in `[components]` becomes one
/// [`LibraryComponent`]; package-level metadata lives at the top of
/// the file as TOML manifest fields.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Library {
    /// Stable per-library UUID. Matches the manifest `library_id` field.
    pub uuid: Uuid,
    /// Human-readable name (`"Resistors"`, `"Op-Amps Standard"`).
    pub name: String,
    /// Optional one-line description shown in the Library Browser.
    pub description: String,
    /// Components in declaration order. The Library Browser table is
    /// sorted by name client-side; round-trips preserve disk order so
    /// hand-edits stay diff-friendly.
    pub components: Vec<LibraryComponent>,
}

/// One row in the `[components]` TSV block.
///
/// `symbol_uuid` / `footprint_uuid` are sentinel-`nil` for an
/// unbound component — v0.10.0 ships every component as unbound (no
/// picker yet). v0.10.8 wires the Pick Symbol/Footprint flow that
/// fills these in. Bound rows still carry free-form `value` /
/// `footprint_name` strings so the Browser's table stays useful even
/// before the picker lands.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LibraryComponent {
    pub uuid: Uuid,
    pub name: String,
    pub value: String,
    /// Free-form footprint label shown in the table. The bound
    /// footprint primitive (when present) is referenced separately by
    /// `footprint_uuid` so the visible label can diverge from the
    /// primitive's filename — matches Altium's "Footprint name vs.
    /// linked PCB Lib" split.
    pub footprint_name: String,
    pub description: String,
    pub symbol_uuid: Uuid,
    pub footprint_uuid: Uuid,
}

impl LibraryComponent {
    /// Convenience constructor for an unbound row (sentinel-nil
    /// symbol / footprint UUIDs). Used by the browser-side fixtures
    /// and by v0.10.8 row creation before the picker fills in real
    /// UUIDs.
    pub fn unbound(uuid: Uuid, name: impl Into<String>) -> Self {
        Self {
            uuid,
            name: name.into(),
            ..Self::default()
        }
    }

    /// `true` once the Pick Symbol picker has bound this row to an
    /// `.snxsym` primitive. v0.10.0 has no picker, so this is always
    /// false in shipped libraries; reserved for v0.10.8.
    pub fn has_symbol_binding(&self) -> bool {
        !self.symbol_uuid.is_nil()
    }

    /// Same predicate for footprints. v0.10.8 wires the binding side.
    pub fn has_footprint_binding(&self) -> bool {
        !self.footprint_uuid.is_nil()
    }
}
