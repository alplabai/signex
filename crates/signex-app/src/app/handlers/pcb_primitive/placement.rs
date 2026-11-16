//! The panel of the object a board placement tool places next (#2393): the
//! same facts as the placed object's, read off the object the next click
//! would add, so the form and the placed object cannot disagree.

use std::path::Path;

use signex_types::pcb::PcbBoard;

use super::facts::{track_facts, via_facts};
use super::zone::zone_facts;
use crate::app::Signex;
use crate::app::handlers::pcb_tool::has_form;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::panels::pcb_primitive_properties::{
    Editability, PcbPrimitiveFacts, PrimitiveTarget, ZoneKind,
};

impl Signex {
    /// The Properties panel's facts of what the tool armed on the board at
    /// `path` places next; `None` without such a tool, or for a tool with
    /// no form yet.
    pub(crate) fn pcb_placement_facts(
        &self,
        path: &Path,
        board: &PcbBoard,
    ) -> Option<PcbPrimitiveFacts> {
        let session = self
            .own_board_tool()
            .filter(|s| s.path.as_path() == path && has_form(s.tool))?;
        let paused = session.paused.is_some();
        let target = PrimitiveTarget::placement(path.to_path_buf());
        let i18n = &*self.ui_state.i18n;
        let facts = match session.tool {
            BoardPlaceTool::Via => {
                let via = self.next_board_via(board);
                let mut common =
                    self.primitive_common_on(board, target, Editability::Editable, Some(via.net));
                common.placement_paused = paused;
                PcbPrimitiveFacts::Via(via_facts(board, common, &[&via], i18n))
            }
            BoardPlaceTool::Track
            | BoardPlaceTool::InteractiveRouting
            | BoardPlaceTool::QuickRoute => {
                let track = self.next_board_track(board);
                let mut common =
                    self.primitive_common_on(board, target, Editability::Editable, Some(track.net));
                common.placement_paused = paused;
                PcbPrimitiveFacts::Track(track_facts(board, common, &[&track], i18n))
            }
            tool => {
                let kind = area_kind(tool)?;
                let zone = self.next_board_area(board)?;
                let net = kind.is_copper().then_some(zone.net);
                let mut common =
                    self.primitive_common_on(board, target, Editability::Editable, net);
                common.placement_paused = paused;
                let mut facts = zone_facts(board, common, &[&zone], i18n);
                // The stand-in outline is a rectangle: the tool names the kind.
                facts.kind = Some(kind);
                PcbPrimitiveFacts::Zone(facts)
            }
        };
        Some(facts)
    }
}

/// The kind of area `tool` draws; `None` for a tool that draws none.
fn area_kind(tool: BoardPlaceTool) -> Option<ZoneKind> {
    Some(match tool {
        BoardPlaceTool::Fill => ZoneKind::Fill,
        BoardPlaceTool::SolidRegion => ZoneKind::Region,
        BoardPlaceTool::PolygonPour => ZoneKind::Pour,
        BoardPlaceTool::Keepout => ZoneKind::Keepout,
        _ => return None,
    })
}
