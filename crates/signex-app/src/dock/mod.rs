//! Panel docking system — wraps PaneGrid regions with tabbed panels.
//!
//! Signex has 3 dock regions (left, right, bottom) plus a center canvas.
//! Each region can hold multiple panels as tabs.

mod first_run;
mod graphite;
mod layout_sync;
mod measure;
mod placement;
mod state;
mod summary;
mod types;
mod view;

pub use first_run::{GRAPHITE_BOTTOM_HEIGHT, graphite_first_run};
pub use layout_sync::{
    DOCK_LAYOUT_SAVE_DEBOUNCE, DockLayoutBarrier, DockLayoutMsg, DockLayoutSnapshot,
    DockLayoutStore, DockLayoutSync, DockLayoutWrite, RegionLayout,
};
pub use placement::{DropGeometry, RegionExtent, drop_target};
pub use types::{
    DockArea, DockMessage, DockTabAction, DockTabMenu, FloatingPanel, PanelPosition, PanelSite,
};
