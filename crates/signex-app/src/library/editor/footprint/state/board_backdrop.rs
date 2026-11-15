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

    /// Width and height of the outline's extent in mm, or `None` without
    /// an outline.
    pub fn outline_size_mm(&self) -> Option<(f64, f64)> {
        let (&(x, y), rest) = self.outline.split_first()?;
        let (lo, hi) = rest.iter().fold(((x, y), (x, y)), |(lo, hi), &(x, y)| {
            ((lo.0.min(x), lo.1.min(y)), (hi.0.max(x), hi.1.max(y)))
        });
        Some((hi.0 - lo.0, hi.1 - lo.1))
    }

    /// The area the outline encloses in mm², or `None` without an outline.
    pub fn outline_area_mm2(&self) -> Option<f64> {
        if self.outline.len() < 3 {
            return None;
        }
        let ring: Vec<signex_sketch::geom::Point2> =
            self.outline.iter().map(|&p| p.into()).collect();
        Some(signex_sketch::geom::predicates::signed_area(&ring).abs())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn with_outline(outline: &[(f64, f64)]) -> BoardBackdrop {
        BoardBackdrop {
            outline: outline.to_vec(),
            ..BoardBackdrop::default()
        }
    }

    /// #2356: the Properties panel's outline facts, either winding.
    #[test]
    fn an_outline_has_its_size_and_area() {
        let cw = with_outline(&[(0.0, 0.0), (0.0, 20.0), (30.0, 20.0), (30.0, 0.0)]);
        assert_eq!(cw.outline_size_mm(), Some((30.0, 20.0)));
        assert_eq!(cw.outline_area_mm2(), Some(600.0));
        let ccw = with_outline(&[(5.0, 5.0), (15.0, 5.0), (15.0, 10.0), (5.0, 10.0)]);
        assert_eq!(ccw.outline_size_mm(), Some((10.0, 5.0)));
        assert_eq!(ccw.outline_area_mm2(), Some(50.0));
    }

    #[test]
    fn no_outline_has_no_size_or_area() {
        let none = BoardBackdrop::default();
        assert_eq!(none.outline_size_mm(), None);
        assert_eq!(none.outline_area_mm2(), None);
    }
}
