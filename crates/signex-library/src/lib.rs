//! Signex component library subsystem (v0.9, refactored).
//!
//! Per `v0.9-library-refactor-plan.md`, the data model is split into three
//! reusable primitives (`Symbol`, `Footprint`, `SimModel`) addressed by
//! `(library_id, uuid)` tuples, and a thin binding [`Component`] record that
//! references them via [`PrimitiveRef`].
//!
//! See `docs/internal/docs/LIBRARY_PLAN.md` for the original (pre-refactor)
//! design rationale; refactor delta lives in
//! `.claude/PRPs/v0.9-library-refactor-plan.md`.

pub mod adapter;
pub mod adapters;
#[cfg(feature = "ai-stub")]
pub mod ai_stub;
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
pub mod snxpart;
pub mod templates;
pub mod where_used;

pub use adapter::{
    ComponentSummary, FieldSet, LibraryAdapter, LibraryError, LibraryQuery, PrimitiveSummary,
};
pub use adapters::library_set::LibrarySet;
#[cfg(feature = "ai-stub")]
pub use ai_stub::{PinGuess, PinoutGuess, extract_pinout};
pub use component::{Component, DatasheetRef, PinPadOverride, PlmReserved, Revision};
pub use diff::{
    BumpKind, LifecycleDiff, ListDiff, ParameterDiff, PinMapDiff, RevisionDiff, auto_bump_kind,
    diff_revisions,
};
pub use distributor::{DistributorAdapter, DistributorError, DistributorPart, DistributorSource};
#[cfg(feature = "distributors-community")]
pub use distributors::{
    DigiKeyAdapter, DistributorCache, JlcpcbAdapter, KeyringStore, LcscAdapter, MouserAdapter,
};
pub use hash::hash_revision_content;
pub use identity::{ComponentClass, ComponentId, InternalPn, Mpn, ParseVersionError, Version};
pub use lifecycle::LifecycleState;
pub use manifest::{LibraryMeta, LibraryMode, Manifest, UserEntry, UsersConfig, WorkflowConfig};
pub use manufacturer::{AlternateStatus, DistributorListing, ManufacturerPart};
pub use param::{ParamMap, ParamValue};
pub use primitive::{
    Body3D, BodyShape, Drill, Footprint, FpGraphic, FpGraphicKind, LayerId, Pad, PadKind, PadShape,
    PinElectricalType, PinOrientation, Polygon, PrimitiveKind, PrimitiveRef, SimKind, SimModel,
    StepAttachment, Symbol, SymbolGraphic, SymbolGraphicKind, SymbolPin,
};
pub use search::{Facet, FacetOp, SearchIndex, SearchQuery};
#[cfg(feature = "search-tantivy")]
pub use search_index::{TantivyIndexError, TantivySearchIndex};
pub use snxpart::{SnxPartError, SnxPartFile, read_snxpart, snxpart_filename, write_snxpart};
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
