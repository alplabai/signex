//! View-state messages of the footprint Properties panel: the hover link
//! between its fields and its illustration, and the page shown while
//! nothing is selected. None of them edits the footprint.

use signex_widgets::property_illustration::IllustrationMsg;

use crate::library::editor::footprint::state::FpPropertiesPage;

/// A view-state event of the footprint Properties panel.
#[derive(Debug, Clone)]
pub enum FpPropertiesMsg {
    /// The pointer moved onto or off a field row or an illustration part.
    Illustration(IllustrationMsg),
    /// A page of the nothing-selected view was picked.
    PageSelected(FpPropertiesPage),
}
