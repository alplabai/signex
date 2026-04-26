//! Signex component library subsystem (v0.9).
//!
//! See `docs/internal/docs/LIBRARY_PLAN.md` for design.

pub mod adapter;
pub mod component;
pub mod diff;
pub mod distributor;
pub mod embed;
pub mod hash;
pub mod identity;
pub mod lifecycle;
pub mod manifest;
pub mod search;
pub mod snxpart;

pub use adapter::{ComponentSummary, FieldSet, LibraryAdapter, LibraryError, LibraryQuery};
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
pub use snxpart::{SnxPartError, SnxPartFile, read_snxpart, snxpart_filename, write_snxpart};

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_compiles() {
        // Smoke test: ensures the crate builds with all module declarations.
    }
}
