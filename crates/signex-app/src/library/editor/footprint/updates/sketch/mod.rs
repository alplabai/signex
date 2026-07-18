//! Footprint sketch updates — the sketch concern folded into one
//! folder, split by sub-concern (ADR-0001 D1/D2). The router
//! `updates::apply_footprint_primitive_edit` sends each sketch message
//! group to one of these modules' `apply`:
//!
//! - [`ui`] — selection + tool/mode UI.
//! - [`placement`] — numeric placement-input buffer.
//! - [`entities`] — entity placement & drag geometry.
//! - [`pad_bridge`] — sketch↔pad bridge (roles / profile / corner radius).
//! - [`constraints`] — parameters & constraints.
//! - [`tools`] — the tool-click state machine (draw / edit / transform).
//!
//! Each concern's `apply` was `pub(super)` on the former flat sibling
//! (visible to `updates`); moved one level deeper it widens to
//! `pub(in …updates)`, and the modules are exposed to the parent here
//! so the router's `sketch::<concern>::apply` call sites resolve.

pub(super) mod constraints;
pub(super) mod entities;
pub(super) mod pad_bridge;
pub(super) mod placement;
pub(super) mod tools;
pub(super) mod ui;
