//! The board placement tools (#2352, #2360, #2373, #2389). One table, so the PCB
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
            Self::InteractiveRouting => Live("place_board_route"),
            Self::Track => Live("place_board_track"),
            Self::Fill => Live("place_board_fill"),
            Self::SolidRegion => Live("place_board_region"),
            Self::PolygonPour => Live("place_board_polygon_pour"),
            Self::Keepout => Live("place_board_keepout"),
            Self::Pad => Live("place_board_pad"),
            Self::Arc => Live("place_board_arc"),
            Self::TextString => Live("place_board_string"),
            Self::Dimension => Live("place_board_dimension"),
            Self::Component => Unavailable(ids::PCB_BAR_COMPONENT_UNAVAILABLE),
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
            Self::InteractiveRouting => (ids::PCB_BAR_ROUTE_TOOLTIP, ids::PCB_TOOL_HINT_ROUTE),
            Self::Track => (ids::PCB_BAR_TRACK_TOOLTIP, ids::PCB_TOOL_HINT_TRACK),
            Self::Fill => (ids::PCB_BAR_FILL_TOOLTIP, ids::PCB_TOOL_HINT_FILL),
            Self::SolidRegion => (ids::PCB_BAR_REGION_TOOLTIP, ids::PCB_TOOL_HINT_REGION),
            Self::PolygonPour => (ids::PCB_BAR_POUR_TOOLTIP, ids::PCB_TOOL_HINT_POUR),
            Self::Keepout => (ids::PCB_BAR_KEEPOUT_TOOLTIP, ids::PCB_TOOL_HINT_KEEPOUT),
            Self::Pad => (ids::PCB_BAR_PAD_TOOLTIP, ids::PCB_TOOL_HINT_PAD),
            // The Arc hint names the mode: `Signex::board_tool_hint`.
            Self::Arc => (ids::PCB_BAR_ARC_TOOLTIP, ids::PCB_TOOL_HINT_ARC),
            Self::TextString => (ids::PCB_BAR_STRING_TOOLTIP, ids::PCB_TOOL_HINT_STRING),
            Self::Dimension => (ids::PCB_BAR_DIMENSION_TOOLTIP, ids::PCB_TOOL_HINT_DIMENSION),
            Self::Component => (Self::Component.name(), Self::Component.name()),
        }
    }
}
