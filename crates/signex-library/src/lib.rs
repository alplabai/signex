//! Signex component library subsystem (v0.9-refactor-2 — DBLib model).
//!
//! Per `v0.9-refactor-2-plan.md`, components are **rows in TSV/JSONB
//! tables**, not files. Each row references reusable primitives (`Symbol`,
//! `Footprint`, `SimModel`) by `(library_id, uuid)` tuples ([`PrimitiveRef`])
//! instead of embedding their geometry. Symbols / footprints / sims stay as
//! standalone editable primitive files; the row binds them with metadata.

pub mod adapter;
pub mod adapters;
pub mod component;
pub mod diff;
pub mod distributor;
#[cfg(feature = "distributors-community")]
pub mod distributors;
pub mod hash;
pub mod identity;
pub mod lifecycle;
pub mod manifest;
pub mod manufacturer;
pub mod param;
pub mod primitive;
pub mod search;
#[cfg(feature = "search-tantivy")]
pub mod search_index;
pub mod tables;
pub mod templates;
pub mod where_used;

pub use adapter::{
    ComponentSummary, FieldSet, LibraryAdapter, LibraryError, LibraryQuery, PrimitiveSummary,
};
pub use adapters::library_set::LibrarySet;
#[cfg(feature = "ai-stub")]
pub use ai_stub::{PinGuess, PinoutGuess, extract_pinout};
pub use component::{ComponentRow, DatasheetRef, PinPadOverride, PlmReserved};
pub use diff::{
    BumpKind, LifecycleDiff, ListDiff, ParameterDiff, PinMapDiff, RowDiff, auto_bump_kind,
    diff_rows,
};
pub use distributor::{DistributorAdapter, DistributorError, DistributorPart, DistributorSource};
#[cfg(feature = "distributors-community")]
pub use distributors::{
    DigiKeyAdapter, DistributorCache, JlcpcbAdapter, KeyringStore, LcscAdapter, MouserAdapter,
};
pub use hash::hash_row_content;
pub use identity::{ComponentClass, InternalPn, Mpn, RowId};
pub use lifecycle::LifecycleState;
pub use manifest::{
    LibraryMeta, LibraryMode, Manifest, TableConfig, UserEntry, UsersConfig, WorkflowConfig,
};
pub use manufacturer::{AlternateStatus, DistributorListing, ManufacturerPart};
pub use param::{ParamMap, ParamValue};
pub use primitive::{
    Body3D, BodyShape, Drill, Footprint, FpGraphic, FpGraphicKind, LayerId, Pad, PadKind, PadShape,
    PinElectricalType, PinOrientation, Polygon, PrimitiveKind, PrimitiveRef, SimKind, SimModel,
    PinSymbolKind, StepAttachment, Symbol, SymbolFile, SymbolGraphic, SymbolGraphicKind,
    SymbolPin,
};
pub use search::{Facet, FacetOp, SearchIndex, SearchQuery};
#[cfg(feature = "search-tantivy")]
pub use search_index::{TantivyIndexError, TantivySearchIndex};
pub use tables::{
    TABLE_HEADER, TableSchema, append_row, delete_row, read_table, update_row, write_table,
};
pub use templates::{ParamKind, ParamSlot, ParameterTemplate, TemplateRegistry, TemplateViolation};
pub use where_used::{UseSite, WhereUsedIndex};

#[cfg(feature = "local-git")]
pub use adapters::local_git::LocalGitAdapter;

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_compiles() {
        // Smoke test: ensures the crate builds with all module declarations.
    }
}
