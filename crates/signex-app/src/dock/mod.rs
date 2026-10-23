//! Panel docking system — wraps PaneGrid regions with tabbed panels.
//!
//! Signex has 3 dock regions (left, right, bottom) plus a center canvas.
//! Each region can hold multiple panels as tabs.

mod layout_sync;
mod placement;
mod state;
mod types;
mod view;

pub use layout_sync::{
    DOCK_LAYOUT_SAVE_DEBOUNCE, DockLayoutBarrier, DockLayoutMsg, DockLayoutSnapshot,
    DockLayoutStore, DockLayoutSync, DockLayoutWrite, RegionLayout,
};
pub use types::{DockArea, DockMessage, FloatingPanel, PanelPosition, PanelSite};
