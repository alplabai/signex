//! Signex component library subsystem (v0.9).
//!
//! See `docs/internal/docs/LIBRARY_PLAN.md` for design.

pub mod adapter;
pub mod adapters;
pub mod component;
pub mod diff;
pub mod distributor;
#[cfg(feature = "distributors-community")]
pub mod distributors;
pub mod embed;
pub mod hash;
pub mod identity;
pub mod lifecycle;
pub mod manifest;
pub mod search;
#[cfg(feature = "search-tantivy")]
pub mod search_index;
pub mod snxpart;
pub mod where_used;

pub use adapter::{ComponentSummary, FieldSet, LibraryAdapter, LibraryError, LibraryQuery};
#[cfg(feature = "ai-stub")]
pub use ai_stub::{PinGuess, PinoutGuess, extract_pinout};
pub use component::{Component, Revision};
pub use diff::{BumpKind, RevisionDiff, auto_bump_kind, diff_revisions};
pub use distributor::{DistributorAdapter, DistributorError, DistributorPart, DistributorSource};
pub use embed::{
    AvlEntry, ComplianceTags, DatasheetRef, FootprintBody, ModelRef, ParamMap, ParamValue, PcbSide,
    PlmLink, PriceBreak, PricingSnapshot, SchematicSide, SharedSide, SharedSlice, SpiceModel,
    SupplierLink, SymbolBody, TemplateId, VariantOverride,
};
pub use hash::hash_revision_content;
pub use identity::{ComponentId, InternalPn, Mpn, Version};
pub use lifecycle::LifecycleState;
pub use manifest::{LibraryMeta, LibraryMode, Manifest, UserEntry, UsersConfig, WorkflowConfig};
pub use search::{Facet, FacetOp, SearchIndex, SearchQuery};
#[cfg(feature = "search-tantivy")]
pub use search_index::{TantivyIndexError, TantivySearchIndex};
pub use snxpart::{SnxPartError, SnxPartFile, read_snxpart, snxpart_filename, write_snxpart};
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
