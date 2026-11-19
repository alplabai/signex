//! The illustration builders of the schematic Properties panels (epic #2523,
//! #40-#51): one module per object kind, each turning the selected object's
//! rows into a [`signex_widgets::property_illustration::Illustration`]. They
//! are pure, so the tests read the parts they produce instead of pixels.

pub(super) mod footprint_thumb;
pub(super) mod part;
pub(super) mod pin;
