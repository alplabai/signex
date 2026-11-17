//! Panel geometry (#2410): where the copies of a board, the rails, the
//! fiducials, the tooling holes and the mouse bites or V-score lines of a
//! [`BoardPanel`](signex_types::pcb_panel::BoardPanel) lie, and the whole
//! panel as one board for the fabrication outputs. Integer nm inside, as the
//! rest of the crate. Rules and pictures: `docs/PANELISATION.md`.

mod board;
mod layout;
mod tabs;

#[cfg(test)]
mod tests;

pub use board::{PANEL_ROUTE_LAYER, panel_board};
pub use layout::{PanelLayout, PanelSegment, layout};
