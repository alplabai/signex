//! PCB rendering — minimal v0.12 surface.
//!
//! The full PCB renderer is on the post-v0.12 roadmap (a Signex-only
//! PCB rendering spec lands in v0.13+). This file provides the
//! shape-only types `signex-app` needs to keep the PCB tab compiling
//! while the schematic cleanroom rewrite ships first.
//!
//! `render_pcb` is intentionally a no-op for v0.12; callers see an
//! empty PCB canvas (the bg-grid layer the canvas widget paints
//! directly is unaffected). This is a known limitation tracked for
//! follow-up; see `docs/internal/CLEANROOM_REWRITE_PLAN.md`.

use iced::widget::canvas::Frame;
use signex_types::pcb::PcbBoard;
use signex_types::theme::CanvasColors;

/// World ↔ screen transform for the PCB canvas — v0.12 pass-through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenTransform {
    pub offset_x: f32,
    pub offset_y: f32,
    pub scale: f32,
}

/// Frozen snapshot of a board for rendering. v0.12 stores the board
/// by clone; v0.13+ will replace this with a borrow + spec-driven
/// renderer.
#[derive(Debug, Clone)]
pub struct PcbRenderSnapshot {
    pub board: PcbBoard,
}

impl PcbRenderSnapshot {
    /// Build a snapshot from a board.
    pub fn from_board(board: &PcbBoard) -> Self {
        Self {
            board: board.clone(),
        }
    }
}

/// Render the PCB into a frame. v0.12: no-op pending the cleanroom
/// PCB renderer; the canvas widget's grid + chrome still draws.
pub fn render_pcb(
    _frame: &mut Frame,
    _snapshot: &PcbRenderSnapshot,
    _transform: ScreenTransform,
    _colors: &CanvasColors,
) {
    // Intentional no-op for v0.12 — see module doc.
}
