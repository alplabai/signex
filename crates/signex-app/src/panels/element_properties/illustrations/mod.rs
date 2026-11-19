//! Line-art property illustrations of the schematic Properties panel: data
//! builders that turn a selected object's rows into an
//! [`signex_widgets::property_illustration::Illustration`]. They are pure, so
//! the tests read the parts they produce instead of pixels.

pub(super) mod footprint_thumb;
pub(super) mod part;
pub(super) mod pin;
