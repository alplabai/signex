//! Panel docking system — wraps PaneGrid regions with tabbed panels.
//!
//! Signex has 3 dock regions (left, right, bottom) plus a center canvas.
//! Each region can hold multiple panels as tabs.

mod state;
mod types;
mod view;

pub use types::{DockArea, DockMessage, FloatingPanel, PanelPosition};
