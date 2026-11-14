//! The board placement tools the PCB editor cannot run yet (#2352, #2360).
//! One table, so the PCB Active Bar and the Place menu on a board grey the
//! same set and give the same reason.

use signex_i18n::{MessageId, ids};

/// A board tool with no command yet: greyed on the PCB Active Bar and in
/// Place on a board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnavailableBoardTool {
    Component,
    Pad,
    Via,
    InteractiveRouting,
    Track,
    Arc,
    Fill,
    SolidRegion,
    PolygonPour,
    TextString,
    Dimension,
    Keepout,
}

impl UnavailableBoardTool {
    /// Place on a board, top to bottom.
    pub(crate) const PLACE_MENU: [Self; 12] = [
        Self::Component,
        Self::Pad,
        Self::Via,
        Self::InteractiveRouting,
        Self::Track,
        Self::Arc,
        Self::Fill,
        Self::SolidRegion,
        Self::PolygonPour,
        Self::TextString,
        Self::Dimension,
        Self::Keepout,
    ];

    /// The tool's name, as its Place row reads.
    pub(crate) fn name(self) -> MessageId {
        match self {
            Self::Component => ids::PCB_TOOL_COMPONENT,
            Self::Pad => ids::PCB_TOOL_PAD,
            Self::Via => ids::PCB_TOOL_VIA,
            Self::InteractiveRouting => ids::PCB_TOOL_ROUTE,
            Self::Track => ids::PCB_TOOL_TRACK,
            Self::Arc => ids::PCB_TOOL_ARC,
            Self::Fill => ids::PCB_TOOL_FILL,
            Self::SolidRegion => ids::PCB_TOOL_REGION,
            Self::PolygonPour => ids::PCB_TOOL_POLYGON_POUR,
            Self::TextString => ids::PCB_TOOL_STRING,
            Self::Dimension => ids::PCB_TOOL_DIMENSION,
            Self::Keepout => ids::PCB_TOOL_KEEPOUT,
        }
    }

    /// Why the tool is greyed: the Active Bar button's tooltip and the
    /// Place row's hint.
    pub(crate) fn reason(self) -> MessageId {
        match self {
            Self::Component => ids::PCB_BAR_COMPONENT_UNAVAILABLE,
            Self::Pad => ids::PCB_BAR_PAD_UNAVAILABLE,
            Self::Via => ids::PCB_BAR_VIA_UNAVAILABLE,
            Self::InteractiveRouting => ids::PCB_BAR_ROUTE_UNAVAILABLE,
            Self::Track => ids::PCB_BAR_TRACK_UNAVAILABLE,
            Self::Arc => ids::PCB_BAR_ARC_UNAVAILABLE,
            Self::Fill => ids::PCB_BAR_FILL_UNAVAILABLE,
            Self::SolidRegion => ids::PCB_BAR_REGION_UNAVAILABLE,
            Self::PolygonPour => ids::PCB_BAR_POLYGON_POUR_UNAVAILABLE,
            Self::TextString => ids::PCB_BAR_TEXT_UNAVAILABLE,
            Self::Dimension => ids::PCB_BAR_DIMENSION_UNAVAILABLE,
            Self::Keepout => ids::PCB_BAR_KEEPOUT_UNAVAILABLE,
        }
    }
}
