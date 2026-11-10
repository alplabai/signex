//! The board under a board sketch (#2234): what the sketch editor draws
//! beneath the sketch so the user draws against the real board. Read once
//! when sketch mode starts; it is a picture, never edited.

use signex_types::pcb::PcbBoard;

/// A track as drawn under the sketch, in board mm.
#[derive(Debug, Clone, PartialEq)]
pub struct BackdropTrack {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub width: f64,
}

/// The parts of a board drawn under its sketch, in board mm.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardBackdrop {
    /// The board outline, closing vertex not repeated; empty when unset.
    pub outline: Vec<(f64, f64)>,
    pub tracks: Vec<BackdropTrack>,
    /// Vias as (centre, diameter).
    pub vias: Vec<((f64, f64), f64)>,
}

impl BoardBackdrop {
    pub fn from_board(board: &PcbBoard) -> Self {
        Self {
            outline: board.outline.iter().map(|p| (p.x, p.y)).collect(),
            tracks: board
                .segments
                .iter()
                .map(|s| BackdropTrack {
                    from: (s.start.x, s.start.y),
                    to: (s.end.x, s.end.y),
                    width: s.width,
                })
                .collect(),
            vias: board
                .vias
                .iter()
                .map(|v| ((v.position.x, v.position.y), v.diameter))
                .collect(),
        }
    }

    /// The box around everything drawn, or `None` for an empty board.
    pub fn bbox_mm(&self) -> Option<(f64, f64, f64, f64)> {
        let points = self
            .outline
            .iter()
            .copied()
            .chain(self.tracks.iter().flat_map(|t| [t.from, t.to]))
            .chain(self.vias.iter().map(|v| v.0))
            .filter(|p| p.0.is_finite() && p.1.is_finite());
        points.fold(None, |bbox, (x, y)| {
            Some(match bbox {
                Some((a, b, c, d)) => (
                    f64::min(a, x),
                    f64::min(b, y),
                    f64::max(c, x),
                    f64::max(d, y),
                ),
                None => (x, y, x, y),
            })
        })
    }
}
