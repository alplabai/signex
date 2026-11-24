//! The Polygon Manager (mockup row 178): every polygon pour of a board in
//! one table, drawn as a document tab (`app/polygon_manager_tab`), and the
//! compact list of the PCB panel's Polygons mode (mockup row 106). One
//! model, two pure views (DECISIONS 2026-11-18, "Tab + compact panel
//! pair"): both read the rows the app copies from the board
//! (`app/handlers/polygon_manager`) and the view state in
//! [`PolygonsUi`], and both send [`PolygonMsg`]. Every edit is the engine
//! command the pour's Properties runs for the same field, one undo step.

mod model;
mod page;
mod panel;
mod table;
#[cfg(test)]
mod tests;
mod toolbar;
mod widgets;

pub use model::{
    PolygonFacts, PolygonMsg, PolygonSubTab, PolygonsUi, PourField, PourRow, PourState,
    shared_dead_copper, shared_value,
};
pub use page::view;
pub use panel::view as panel_view;

use super::{PanelMsg, PcbPanelMsg};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn msg(event: PolygonMsg) -> PanelMsg {
    PanelMsg::PcbPanel(PcbPanelMsg::Polygons(event))
}
