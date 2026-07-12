//! Footprint-editor dock message handlers, grouped by concern.
//!
//! Each submodule holds the `impl Signex` handlers for one slice of the
//! footprint-editor Properties/dock surface; the parent `sch_library`
//! router (`handle_dock_sch_library_message`) delegates each `PanelMsg`
//! arm to one of these. Split out of the former flat `footprint_*`
//! siblings so the shared `footprint` namespace lives in the folder, not
//! in every filename (ADR-0001 §5 — group by folder, no redundant prefix).

mod grid;
mod library;
mod pad;
mod props;
mod shape;
mod silk;
mod sketch;

pub(in crate::app::handlers::dock::sch_library) use silk::{SilkLineEndpoint, SilkTextField};
