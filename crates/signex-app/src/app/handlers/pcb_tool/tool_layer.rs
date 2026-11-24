//! The layer the String and Graphics tools draw on (#2869). Neither draws on
//! copper, and a board opens on a copper layer, so arming one there makes
//! the layer its form shows active: the layer Properties names is the layer
//! the next click places on.

use signex_types::pcb_graphic::takes_board_graphics;
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{PcbViewMsg, Signex};

/// The layer the String or a Graphics tool's form shows, and arming the tool
/// makes active, while the active layer cannot take what it places.
pub(super) const DRAWING_LAYER: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);

/// Whether `tool` can place on `layer`; `None` for a tool that places on
/// any layer, or on copper.
fn takes(tool: BoardPlaceTool, layer: PcbLayer) -> Option<bool> {
    if tool == BoardPlaceTool::TextString {
        return Some(layer.takes_text());
    }
    tool.graphic_kind().map(|_| takes_board_graphics(layer))
}

impl Signex {
    /// `tool` was just armed: when it is the String or a Graphics tool and
    /// the active layer cannot take what it places, [`DRAWING_LAYER`]
    /// becomes the active layer of the board it is armed on.
    pub(super) fn activate_tool_layer(&mut self, tool: BoardPlaceTool) {
        let Some(session) = self.own_board_tool() else {
            return;
        };
        let (path, window) = (session.path.clone(), session.window);
        let active = self.pcb_layer_view(&path).active();
        if takes(tool, active) == Some(false) {
            self.apply_pcb_view_event(window, PcbViewMsg::ActiveLayerPicked(DRAWING_LAYER));
        }
    }
}
