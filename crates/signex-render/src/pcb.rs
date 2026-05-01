//! PCB rendering — minimal v0.12 surface.
//!
//! The full PCB renderer is on the post-v0.12 roadmap (a Signex-only
//! PCB rendering spec lands in v0.13+). This file provides a thin
//! shape-compatible surface so `signex-app`'s PCB tab keeps compiling
//! while the schematic cleanroom rewrite ships first.
//!
//! `render_pcb` is intentionally a no-op for v0.12; callers see the
//! grid + chrome the canvas widget paints directly. This is a known
//! limitation tracked for follow-up; see
//! `docs/internal/CLEANROOM_REWRITE_PLAN.md`.

use iced::widget::canvas::Frame;
use signex_types::pcb::PcbBoard;
use signex_types::schematic::Aabb;
use signex_types::theme::CanvasColors;

/// World ↔ screen transform for the PCB canvas — v0.12 pass-through.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenTransform {
    pub offset_x: f32,
    pub offset_y: f32,
    pub scale: f32,
}

/// Frozen snapshot of a board for rendering. v0.12: a thin newtype
/// wrapping [`PcbBoard`] with `Deref<Target = PcbBoard>` so consumers
/// reach `.footprints` / `.segments` / `.vias` / `.texts` / `.layers`
/// directly.
#[derive(Debug, Clone)]
pub struct PcbRenderSnapshot(pub PcbBoard);

impl PcbRenderSnapshot {
    /// Build a snapshot from a board.
    pub fn from_board(board: &PcbBoard) -> Self {
        Self(board.clone())
    }

    /// Coarse content bounding box — `None` when the board has no
    /// drawable items. Returned in board millimetres. Approximate;
    /// v0.13's full PCB renderer will produce a tighter value.
    pub fn content_bounds(&self) -> Option<Aabb> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut extend = |x: f64, y: f64| {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        };
        for fp in &self.0.footprints {
            extend(fp.position.x, fp.position.y);
        }
        for s in &self.0.segments {
            extend(s.start.x, s.start.y);
            extend(s.end.x, s.end.y);
        }
        for v in &self.0.vias {
            extend(v.position.x, v.position.y);
        }
        for t in &self.0.texts {
            extend(t.position.x, t.position.y);
        }
        if min_x.is_finite() {
            Some(Aabb::new(min_x, min_y, max_x, max_y))
        } else {
            None
        }
    }
}

impl std::ops::Deref for PcbRenderSnapshot {
    type Target = PcbBoard;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
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
