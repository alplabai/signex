//! The board under a board sketch (#2234): what the sketch editor draws
//! beneath the sketch so the user draws against the real board, and what
//! its snaps reach (#2518). Read once when sketch mode starts; it is a
//! picture, never edited.

use pcb_geom::{GraphicShape, PointNm};
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard};

/// A track as drawn under the sketch, in board mm.
#[derive(Debug, Clone, PartialEq)]
pub struct BackdropTrack {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub width: f64,
}

/// A placed pad under the sketch, in board mm.
#[derive(Debug, Clone, PartialEq)]
pub struct BackdropPad {
    pub centre: (f64, f64),
    /// Width and height in the pad's own axes.
    pub size: (f64, f64),
    /// The pad's turn on the board, degrees counter-clockwise.
    pub rotation_deg: f64,
    /// Drawn as a disc rather than a turned rectangle.
    pub round: bool,
    /// Which copper it is, for its colour.
    pub copper: PadCopper,
    /// `J1-1` for a pad of J1, the pad number alone for a free pad.
    pub designator: String,
}

/// The copper a backdrop pad is drawn in: its side's layer, or the
/// Multi-Layer colour for a through-hole pad, as the PCB canvas does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadCopper {
    Top,
    Bottom,
    Through,
}

/// One courtyard drawing of a placed component, in board mm.
#[derive(Debug, Clone, PartialEq)]
pub struct BackdropCourtyard {
    /// The component's designator.
    pub reference: String,
    /// A line's two ends, or a rectangle's or polygon's corners.
    pub points: Vec<(f64, f64)>,
    /// The last point joins the first (a rectangle or polygon).
    pub closed: bool,
}

/// The parts of a board drawn under its sketch, in board mm.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardBackdrop {
    /// The board outline, closing vertex not repeated; empty when unset.
    pub outline: Vec<(f64, f64)>,
    pub tracks: Vec<BackdropTrack>,
    /// Vias as (centre, diameter).
    pub vias: Vec<((f64, f64), f64)>,
    /// Placed pads with copper: every pad but a non-plated one.
    pub pads: Vec<BackdropPad>,
    /// Drilled pads, plated and not, as (centre, drill diameter).
    pub holes: Vec<((f64, f64), f64)>,
    /// Component courtyards drawn as lines, rectangles or polygons; a
    /// courtyard arc or circle is not carried.
    pub courtyards: Vec<BackdropCourtyard>,
}

/// Where one pad sits, before its copper and drill are read.
struct PlacedPad {
    centre: (f64, f64),
    rotation_deg: f64,
    designator: String,
    /// The pad sits on the bottom side.
    bottom: bool,
}

/// Whether a native layer token names a bottom-side layer.
fn on_bottom(layer: &str) -> bool {
    signex_types::format::parse_pcb_layer(layer).and_then(|l| l.side())
        == Some(signex_types::pcb_layer::BoardSide::Bottom)
}

fn mm(p: PointNm) -> (f64, f64) {
    let p = p.to_mm();
    (p.x, p.y)
}

/// The same point for the badge's name lookup: the snap hands back the
/// feature's own coordinates, so only rounding separates them.
fn same_point(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

impl BoardBackdrop {
    pub fn from_board(board: &PcbBoard) -> Self {
        let mut backdrop = Self {
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
            ..Self::default()
        };
        for footprint in &board.footprints {
            backdrop.add_footprint(footprint);
        }
        for pad in &board.free.pads {
            let placed = PlacedPad {
                centre: (pad.position.x, pad.position.y),
                rotation_deg: pad.rotation,
                designator: pad.number.clone(),
                bottom: pad.layers.first().is_some_and(|l| on_bottom(l)),
            };
            backdrop.add_pad(pad, placed);
        }
        backdrop
    }

    /// A component's pads and courtyard. A pad or drawing whose placement
    /// does not compute (a non-finite position) is left out of the picture.
    fn add_footprint(&mut self, footprint: &Footprint) {
        for pad in &footprint.pads {
            let (Ok(centre), Ok(rotation_deg)) = (
                pcb_geom::pad_world_center(footprint, pad),
                pcb_geom::pad_world_rotation_deg(footprint, pad),
            ) else {
                continue;
            };
            let designator = match (footprint.reference.is_empty(), pad.number.is_empty()) {
                (false, false) => format!("{}-{}", footprint.reference, pad.number),
                (false, true) => footprint.reference.clone(),
                _ => pad.number.clone(),
            };
            let placed = PlacedPad {
                centre: mm(centre),
                rotation_deg,
                designator,
                bottom: on_bottom(&footprint.layer),
            };
            self.add_pad(pad, placed);
        }
        for graphic in footprint
            .graphics
            .iter()
            .filter(|g| pcb_geom::is_courtyard(g))
        {
            let Ok(Some(placed)) = pcb_geom::footprint_graphic_placement(footprint, graphic) else {
                continue;
            };
            let (points, closed) = match placed.shape {
                GraphicShape::Line { a, b } => (vec![mm(a), mm(b)], false),
                GraphicShape::Polygon { points } => (points.into_iter().map(mm).collect(), true),
                _ => continue,
            };
            self.courtyards.push(BackdropCourtyard {
                reference: footprint.reference.clone(),
                points,
                closed,
            });
        }
    }

    /// One pad: its drill, and its copper unless it is non-plated.
    fn add_pad(&mut self, pad: &Pad, placed: PlacedPad) {
        let drilled = matches!(pad.pad_type, PadType::Thru | PadType::NpThru);
        if drilled && let Some(drill) = pad.drill.as_ref().filter(|d| d.diameter > 0.0) {
            self.holes.push((placed.centre, drill.diameter));
        }
        if pad.pad_type == PadType::NpThru {
            return;
        }
        let copper = match (pad.pad_type, placed.bottom) {
            (PadType::Thru, _) => PadCopper::Through,
            (_, true) => PadCopper::Bottom,
            (_, false) => PadCopper::Top,
        };
        self.pads.push(BackdropPad {
            centre: placed.centre,
            size: (pad.size.x, pad.size.y),
            rotation_deg: placed.rotation_deg,
            round: pad.shape == PadShape::Circle,
            copper,
            designator: placed.designator,
        });
    }

    /// The pad whose centre is `pos`, for the snap badge's name.
    pub fn pad_at(&self, pos: (f64, f64)) -> Option<&BackdropPad> {
        self.pads.iter().find(|p| same_point(p.centre, pos))
    }

    /// The courtyard with a corner at `pos`, for the snap badge's name.
    pub fn courtyard_at(&self, pos: (f64, f64)) -> Option<&BackdropCourtyard> {
        self.courtyards
            .iter()
            .find(|c| c.points.iter().any(|&p| same_point(p, pos)))
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
            .chain(self.pads.iter().map(|p| p.centre))
            .chain(
                self.courtyards
                    .iter()
                    .flat_map(|c| c.points.iter().copied()),
            )
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
