//! The board placement tools (#2352, #2360, #2373, #2389, #2216, #2424). One
//! table, so the PCB Active Bar and the Place menu on a board run the same
//! command for a live tool, and grey the same set with the same reason for a
//! tool the board editor cannot run yet.

use signex_i18n::{MessageId, ids};
use signex_types::pcb_graphic::BoardGraphicKind;

/// A board placement tool, in the Place menu's order, then the Graphics
/// tools in Place ▸ Graphics order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPlaceTool {
    Component,
    Pad,
    Via,
    InteractiveRouting,
    /// A differential pair routed side by side (#2389).
    DiffPairRouting,
    /// Straight to the target, other nets not avoided (#2389).
    QuickRoute,
    /// Several selected nets routed side by side as a bus (#2389).
    MultiRoute,
    Track,
    Arc,
    Fill,
    SolidRegion,
    PolygonPour,
    PourCutout,
    SlicePour,
    TextString,
    Dimension,
    Keepout,
    /// A line on a non-copper layer (#2424).
    Line,
    /// An arc on a non-copper layer by its centre, start and end; `Arc` is
    /// the copper one.
    GraphicArc,
    FullCircle,
    Rectangle,
    TextFrame,
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
    pub(crate) const PLACE_MENU: [Self; 17] = [
        Self::Component,
        Self::Pad,
        Self::Via,
        Self::InteractiveRouting,
        Self::DiffPairRouting,
        Self::QuickRoute,
        Self::MultiRoute,
        Self::Track,
        Self::Arc,
        Self::Fill,
        Self::SolidRegion,
        Self::PolygonPour,
        Self::PourCutout,
        Self::SlicePour,
        Self::TextString,
        Self::Dimension,
        Self::Keepout,
    ];

    /// Place ▸ Graphics on a board, top to bottom (#2424).
    pub(crate) const GRAPHICS_MENU: [Self; 5] = [
        Self::Line,
        Self::GraphicArc,
        Self::FullCircle,
        Self::Rectangle,
        Self::TextFrame,
    ];

    pub(crate) fn state(self) -> ToolState {
        use ToolState::{Live, Unavailable};
        match self {
            Self::Via => Live("place_board_via"),
            Self::InteractiveRouting => Live("place_board_route"),
            Self::DiffPairRouting => Live("place_board_diff_pair"),
            Self::QuickRoute => Live("place_board_quick_route"),
            Self::MultiRoute => Live("place_board_multi_route"),
            Self::Track => Live("place_board_track"),
            Self::Fill => Live("place_board_fill"),
            Self::SolidRegion => Live("place_board_region"),
            Self::PolygonPour => Live("place_board_polygon_pour"),
            Self::PourCutout => Live("place_board_pour_cutout"),
            Self::SlicePour => Live("place_board_slice_pour"),
            Self::Keepout => Live("place_board_keepout"),
            Self::Pad => Live("place_board_pad"),
            Self::Arc => Live("place_board_arc"),
            Self::TextString => Live("place_board_string"),
            Self::Dimension => Live("place_board_dimension"),
            Self::Line => Live("place_board_line"),
            Self::GraphicArc => Live("place_board_graphic_arc"),
            Self::FullCircle => Live("place_board_circle"),
            Self::Rectangle => Live("place_board_rectangle"),
            Self::TextFrame => Live("place_board_text_frame"),
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
            Self::DiffPairRouting => ids::PCB_TOOL_DIFF_PAIR,
            Self::QuickRoute => ids::PCB_TOOL_QUICK_ROUTE,
            Self::MultiRoute => ids::PCB_TOOL_MULTI_ROUTE,
            Self::Track => ids::PCB_TOOL_TRACK,
            Self::Arc => ids::PCB_TOOL_ARC,
            Self::Fill => ids::PCB_TOOL_FILL,
            Self::SolidRegion => ids::PCB_TOOL_REGION,
            Self::PolygonPour => ids::PCB_TOOL_POLYGON_POUR,
            Self::PourCutout => ids::PCB_TOOL_POUR_CUTOUT,
            Self::SlicePour => ids::PCB_TOOL_SLICE_POUR,
            Self::TextString => ids::PCB_TOOL_STRING,
            Self::Dimension => ids::PCB_TOOL_DIMENSION,
            Self::Keepout => ids::PCB_TOOL_KEEPOUT,
            Self::Line => ids::PCB_TOOL_LINE,
            Self::GraphicArc => ids::PCB_TOOL_GRAPHIC_ARC,
            Self::FullCircle => ids::PCB_TOOL_FULL_CIRCLE,
            Self::Rectangle => ids::PCB_TOOL_RECTANGLE,
            Self::TextFrame => ids::PCB_TOOL_TEXT_FRAME,
        }
    }

    /// The live tool's Active Bar tooltip and the hint the Messages panel
    /// shows when it is armed.
    pub(crate) fn texts(self) -> (MessageId, MessageId) {
        match self {
            Self::Via => (ids::PCB_BAR_VIA_TOOLTIP, ids::PCB_TOOL_HINT_VIA),
            Self::InteractiveRouting => (ids::PCB_BAR_ROUTE_TOOLTIP, ids::PCB_TOOL_HINT_ROUTE),
            Self::DiffPairRouting => (ids::PCB_BAR_DIFF_PAIR_TOOLTIP, ids::PCB_TOOL_HINT_DIFF_PAIR),
            Self::QuickRoute => (
                ids::PCB_BAR_QUICK_ROUTE_TOOLTIP,
                ids::PCB_TOOL_HINT_QUICK_ROUTE,
            ),
            Self::MultiRoute => (
                ids::PCB_BAR_MULTI_ROUTE_TOOLTIP,
                ids::PCB_TOOL_HINT_MULTI_ROUTE,
            ),
            Self::Track => (ids::PCB_BAR_TRACK_TOOLTIP, ids::PCB_TOOL_HINT_TRACK),
            Self::Fill => (ids::PCB_BAR_FILL_TOOLTIP, ids::PCB_TOOL_HINT_FILL),
            Self::SolidRegion => (ids::PCB_BAR_REGION_TOOLTIP, ids::PCB_TOOL_HINT_REGION),
            Self::PolygonPour => (ids::PCB_BAR_POUR_TOOLTIP, ids::PCB_TOOL_HINT_POUR),
            Self::PourCutout => (
                ids::PCB_BAR_POUR_CUTOUT_TOOLTIP,
                ids::PCB_TOOL_HINT_POUR_CUTOUT,
            ),
            Self::SlicePour => (
                ids::PCB_BAR_SLICE_POUR_TOOLTIP,
                ids::PCB_TOOL_HINT_SLICE_POUR,
            ),
            Self::Keepout => (ids::PCB_BAR_KEEPOUT_TOOLTIP, ids::PCB_TOOL_HINT_KEEPOUT),
            Self::Pad => (ids::PCB_BAR_PAD_TOOLTIP, ids::PCB_TOOL_HINT_PAD),
            // The Arc hint names the mode: `Signex::board_tool_hint`.
            Self::Arc => (ids::PCB_BAR_ARC_TOOLTIP, ids::PCB_TOOL_HINT_ARC),
            Self::TextString => (ids::PCB_BAR_STRING_TOOLTIP, ids::PCB_TOOL_HINT_STRING),
            Self::Dimension => (ids::PCB_BAR_DIMENSION_TOOLTIP, ids::PCB_TOOL_HINT_DIMENSION),
            Self::Component => (Self::Component.name(), Self::Component.name()),
            Self::Line => (ids::PCB_BAR_LINE_TOOLTIP, ids::PCB_TOOL_HINT_LINE),
            Self::GraphicArc => (
                ids::PCB_BAR_GRAPHIC_ARC_TOOLTIP,
                ids::PCB_TOOL_HINT_GRAPHIC_ARC,
            ),
            Self::FullCircle => (ids::PCB_BAR_CIRCLE_TOOLTIP, ids::PCB_TOOL_HINT_CIRCLE),
            Self::Rectangle => (ids::PCB_BAR_RECTANGLE_TOOLTIP, ids::PCB_TOOL_HINT_RECTANGLE),
            Self::TextFrame => (
                ids::PCB_BAR_TEXT_FRAME_TOOLTIP,
                ids::PCB_TOOL_HINT_TEXT_FRAME,
            ),
        }
    }

    /// The Graphics tool that places a drawing of `kind`: its name names the
    /// kind in Properties and undo labels.
    pub(crate) fn for_graphic(kind: BoardGraphicKind) -> Self {
        match kind {
            BoardGraphicKind::Line => Self::Line,
            BoardGraphicKind::Arc => Self::GraphicArc,
            BoardGraphicKind::Circle => Self::FullCircle,
            BoardGraphicKind::Rectangle => Self::Rectangle,
            BoardGraphicKind::TextFrame => Self::TextFrame,
        }
    }

    /// The board drawing a Graphics tool places; `None` for another tool.
    pub(crate) fn graphic_kind(self) -> Option<BoardGraphicKind> {
        Some(match self {
            Self::Line => BoardGraphicKind::Line,
            Self::GraphicArc => BoardGraphicKind::Arc,
            Self::FullCircle => BoardGraphicKind::Circle,
            Self::Rectangle => BoardGraphicKind::Rectangle,
            Self::TextFrame => BoardGraphicKind::TextFrame,
            _ => return None,
        })
    }
}
