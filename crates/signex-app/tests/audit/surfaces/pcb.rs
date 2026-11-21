//! Mockup group `pcb` (rows m93-m110).
//!
//! Still to add: m94 Properties: Component (PCB), m95 Properties: Track, m96
//! Properties: Via, m97 Properties: Pad (PCB), m98 Properties: Arc, m99
//! Properties: Zone / pour, m100 Properties: Board text, m101 Properties:
//! Dimension, m102 Properties: Graphic / mask, m103 Layers panel (+ view
//! options), m104 Layer tab context menu / layer strip, m105 Layer Stack,
//! m106 PCB (Nets, Net Classes, Differential Pairs, xSignals, Polygons),
//! m107 Length Matching, m108 PCB active bar, m109 Board 3D view, m110 Board
//! sketch view + sketch attr panel.

use super::checks;
use crate::fixture;
use crate::surface::{Check, Surface, WindowSel};

pub fn rows() -> Vec<Surface> {
    vec![Surface {
        id: "m93",
        title: "Properties: Board Options",
        // The board open with nothing selected: Properties shows the board.
        open: |app, r, fx| {
            fixture::open_board(app, r, fx);
            Ok(())
        },
        window: WindowSel::Auto,
        checks: BOARD,
    }]
}

const BOARD: &[Check] = &[("a board tab is active", checks::board_tab_active)];
