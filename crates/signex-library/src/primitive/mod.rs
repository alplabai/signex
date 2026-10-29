//! Reusable geometry primitives — `Symbol`, `Footprint`, `SimModel`.
//!
//! Per `v0.9-refactor-2-plan.md` §2, primitives are addressed by
//! `(library_id, uuid)` tuples (a [`PrimitiveRef`]) and bound onto `Component`
//! revisions. Two MPNs sharing a SOIC-8 footprint reference the same
//! `Footprint` primitive — they don't carry their own copy.
//!
//! Module layout matches the plan:
//! - [`Symbol`] / [`SymbolPin`] / [`PinDirection`] / [`PinOrientation`]
//!   in [`symbol`]
//! - [`Footprint`] / [`Pad`] / [`PadKind`] / [`PadShape`] / [`Body3D`] /
//!   [`BodyShape`] / [`StepAttachment`] / [`Drill`] / [`Polygon`] /
//!   [`FpGraphic`] in [`footprint`]
//! - [`SimModel`] / [`SimKind`] in [`sim`]
//! - [`PrimitiveRef`] in [`ref_`]

pub mod container_merge;
mod copy_for_library;
pub mod footprint;
pub mod ref_;
pub mod sim;
pub mod symbol;
mod tsv_cell;

pub use container_merge::{ContainerMerge, DuplicatedUuid};
pub use copy_for_library::unique_copy_name;
pub use footprint::{
    Body3D, BodyShape, Drill, Footprint, FootprintFile, FootprintFileError, FpGraphic,
    FpGraphicKind, LayerId, LoadRepair, MountStyle, Pad, PadKind, PadShape, Polygon,
    StepAttachment,
};
pub use ref_::PrimitiveRef;
pub use sim::{SimFile, SimFileError, SimKind, SimModel};
pub use symbol::{
    BODY_ALTERNATE, BODY_COMMON, BODY_NORMAL, CHAIN_ARC_SAMPLES, CHAIN_ENDPOINT_EPSILON_MM,
    ChainError, ChainSegment, ComponentType, PinDirection, PinOrientation, PinSymbolKind, Symbol,
    SymbolFile, SymbolFileError, SymbolGraphic, SymbolGraphicKind, SymbolPin,
    chain_into_closed_contour, normalize_arc_endpoints_deg,
};

/// Discriminator surfaced on `PrimitiveSummary` so a single `list_*` API can
/// describe heterogeneous primitive collections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum PrimitiveKind {
    Symbol,
    Footprint,
    Sim,
}
