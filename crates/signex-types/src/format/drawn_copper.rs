//! Whether a board carries drawn copper (#2883): a board drawing or string
//! on a copper layer. The token rule reads this; the copper itself is
//! built in `pcb-geom` (`drawn_copper`), from the same layer test.

use super::pcb_rows::parse_pcb_layer;
use crate::pcb::PcbBoard;
use crate::pcb_layer::PcbLayer;

/// True when `layer` names a copper layer this build knows.
fn is_copper_layer(layer: &str) -> bool {
    parse_pcb_layer(layer).is_some_and(PcbLayer::is_copper)
}

/// True when any board drawing (line, arc, circle, rectangle, polygon or
/// text frame) or board string sits on a copper layer.
pub fn board_has_drawn_copper(board: &PcbBoard) -> bool {
    board.graphics.iter().any(|g| is_copper_layer(&g.layer))
        || board.texts.iter().any(|t| is_copper_layer(&t.layer))
}
