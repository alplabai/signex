//! View-state messages of the footprint Properties panel: the hover link
//! between its fields and its illustration. None of them edits the
//! footprint.

use signex_widgets::property_illustration::IllustrationMsg;

/// A view-state event of the footprint Properties panel.
#[derive(Debug, Clone)]
pub enum FpPropertiesMsg {
    /// The pointer moved onto or off a field row or an illustration part.
    Illustration(IllustrationMsg),
}
