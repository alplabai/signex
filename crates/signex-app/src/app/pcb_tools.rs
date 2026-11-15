//! The board placement tools (#2352, #2360, #2373). One table, so the PCB
//! Active Bar and the Place menu on a board run the same command for a live
//! tool, and grey the same set with the same reason for a tool the board
//! editor cannot run yet.

use signex_i18n::{MessageId, ids};

/// A board placement tool, in the Place menu's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPlaceTool {
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

/// Whether a tool runs on a board yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolState {
    /// The catalog command that arms it.
    Live(&'static str),
    /// Greyed on the PCB Active Bar and in Place on a board, with why: the
    /// button's tooltip and the row's hint.
    Unavailable(MessageId),
}

impl BoardPlaceTool {
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

    pub(crate) fn state(self) -> ToolState {
        use ToolState::{Live, Unavailable};
        match self {
            Self::Via => Live("place_board_via"),
            Self::Track => Live("place_board_track"),
            Self::Fill => Live("place_board_fill"),
            Self::SolidRegion => Live("place_board_region"),
            Self::Keepout => Live("place_board_keepout"),
            Self::Component => Unavailable(ids::PCB_BAR_COMPONENT_UNAVAILABLE),
            Self::Pad => Unavailable(ids::PCB_BAR_PAD_UNAVAILABLE),
            Self::InteractiveRouting => Unavailable(ids::PCB_BAR_ROUTE_UNAVAILABLE),
            Self::Arc => Unavailable(ids::PCB_BAR_ARC_UNAVAILABLE),
            Self::PolygonPour => Unavailable(ids::PCB_BAR_POLYGON_POUR_UNAVAILABLE),
            Self::TextString => Unavailable(ids::PCB_BAR_TEXT_UNAVAILABLE),
            Self::Dimension => Unavailable(ids::PCB_BAR_DIMENSION_UNAVAILABLE),
        }
    }

    /// The catalog command that arms the tool; `None` while the board
    /// editor cannot run it.
    pub(crate) fn command(self) -> Option<&'static str> {
        match self.state() {
            ToolState::Live(command) => Some(command),
            ToolState::Unavailable(_) => None,
        }
    }

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

    /// The live tool's Active Bar tooltip and the hint the Messages panel
    /// shows when it is armed.
    pub(crate) fn texts(self) -> (MessageId, MessageId) {
        match self {
            Self::Via => (ids::PCB_BAR_VIA_TOOLTIP, ids::PCB_TOOL_HINT_VIA),
            Self::Track => (ids::PCB_BAR_TRACK_TOOLTIP, ids::PCB_TOOL_HINT_TRACK),
            Self::Fill => (ids::PCB_BAR_FILL_TOOLTIP, ids::PCB_TOOL_HINT_FILL),
            Self::SolidRegion => (ids::PCB_BAR_REGION_TOOLTIP, ids::PCB_TOOL_HINT_REGION),
            Self::Keepout => (ids::PCB_BAR_KEEPOUT_TOOLTIP, ids::PCB_TOOL_HINT_KEEPOUT),
            other => (other.name(), other.name()),
        }
    }
}
