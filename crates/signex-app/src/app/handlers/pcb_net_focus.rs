//! The nets a board keeps bright while the rest of its copper dims by the
//! Preferences ▸ PCB ▸ Display amount (#2514): a highlighted net, else the
//! nets of a route being drawn, of a drag, Move or paste under way, or of
//! what a placement puts down. The board scene is drawn again only when
//! that set changes: a tool or a carry starting or ending, Esc included.

use std::path::Path;

use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;

use super::pcb_tool::BoardToolSession;

impl Signex {
    /// The nets the board at `path` keeps bright, ascending; empty when
    /// nothing is in focus and no copper dims.
    pub(crate) fn board_focus_nets(&self, path: &Path) -> Vec<u32> {
        let query = &self.interaction_state.pcb_query;
        if let Some(highlight) = query.net_highlight.as_ref().filter(|h| h.path == path) {
            return vec![highlight.net];
        }
        let Some(board) = self.document_state.pcb_board(path) else {
            return Vec::new();
        };
        if let Some(carry) = (self.interaction_state.pcb_carry.as_ref()).filter(|c| c.path == path)
        {
            if let Some(paste) = &carry.paste {
                return pcb_geom::nets_of(&paste.copy.preview_board(board), |_| true);
            }
            if let Some(part) = &carry.part {
                return carry.anchor.map_or_else(Vec::new, |at| {
                    pcb_geom::nets_of(&part.preview_board(board, at), |_| true)
                });
            }
            let carried = carry.carried();
            return pcb_geom::nets_of(board, |id| carried.contains(&id));
        }
        (self.interaction_state.pcb_tool.as_ref())
            .filter(|session| session.path == path)
            .map(|session| self.tool_focus_nets(session))
            .unwrap_or_default()
    }

    /// The nets of a route being drawn, else the net a copper placement
    /// tool puts down; no net (0) is never in focus.
    fn tool_focus_nets(&self, session: &BoardToolSession) -> Vec<u32> {
        let mut nets: Vec<u32> = match session.router.as_ref().and_then(|r| r.bus.as_ref()) {
            Some(bus) if session.is_routing() => bus.members.iter().map(|m| m.net).collect(),
            _ if session.is_routing() => session.route.iter().map(|style| style.net).collect(),
            _ => self.placement_net(session.tool).into_iter().collect(),
        };
        nets.retain(|&net| net != 0);
        nets.sort_unstable();
        nets.dedup();
        nets
    }

    /// The net the placement form gives what `tool` places, if one is set.
    fn placement_net(&self, tool: BoardPlaceTool) -> Option<u32> {
        let defaults = &self.interaction_state.pcb_tool_defaults;
        match tool {
            BoardPlaceTool::Via => defaults.via_net,
            BoardPlaceTool::Pad => defaults.pad_net,
            BoardPlaceTool::Arc => defaults.arc_net,
            BoardPlaceTool::Track => defaults.track_net,
            BoardPlaceTool::Fill | BoardPlaceTool::SolidRegion | BoardPlaceTool::PolygonPour => {
                defaults.area_net
            }
            _ => None,
        }
    }

    /// Draw the board in view again when the nets it keeps bright, or the
    /// dim amount, are not the ones its scene was drawn with. Whether it
    /// was drawn again (that repaints the highlight too).
    pub(crate) fn sync_board_net_focus(&mut self) -> bool {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return false;
        };
        let nets = self.board_focus_nets(&path);
        let dim = self.ui_state.pcb_dim_others_percent;
        let wanted = signex_renderer::pcb::BoardPaint::default().with_net_focus(&nets, dim);
        let drawn = self.interaction_state.pcb_canvas.active_renderer_snapshot();
        if drawn.and_then(|s| s.paint.net_focus()) == wanted.net_focus() {
            return false;
        }
        self.sync_pcb_canvas_from_visible_board();
        true
    }
}
