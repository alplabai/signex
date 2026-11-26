//! The drawing of a footprint thumbnail row (mockup row 119): the
//! footprint's own pads on the board ground, which is the same in both
//! themes. Taken from the open footprint file, so no read is needed;
//! view-only, it publishes nothing.

use std::sync::Arc;

use iced::widget::canvas::{self, Geometry};
use iced::{Element, Length, Point, Rectangle, Renderer, Size, Theme, Widget, mouse};
use signex_library::primitive::{Footprint, Pad, PadKind, PadShape};

use crate::styles::Radii;
use crate::styles::board_palette::{BoardPalette, BoardSwatch};

/// Share of the tile side kept free around the pads.
const MARGIN_RATIO: f32 = 0.14;

/// One pad as the tile draws it, axis-aligned, in footprint mm (y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TilePad {
    pub center: (f64, f64),
    /// Width and height after the pad's quarter turns.
    pub size: (f64, f64),
    /// Corner radius as a share of the shorter side; 0.5 is fully round.
    pub roundness: f64,
    /// Copper on every layer (a plated through-hole pad).
    pub through: bool,
    /// A drilled hole with no copper.
    pub hole_only: bool,
    /// Drill diameter, mm.
    pub drill: Option<f64>,
}

impl TilePad {
    fn of(pad: &Pad) -> Self {
        let quarter = (pad.rotation.rem_euclid(180.0) - 90.0).abs() < 45.0;
        let (w, h) = (pad.size[0].abs(), pad.size[1].abs());
        let roundness = match pad.shape {
            PadShape::Round | PadShape::Oval => 0.5,
            PadShape::RoundRect { radius_ratio } => radius_ratio.clamp(0.0, 0.5),
            _ => 0.0,
        };
        let hole_only = pad.kind == PadKind::NptHole;
        Self {
            center: (pad.position[0], pad.position[1]),
            size: if quarter { (h, w) } else { (w, h) },
            roundness,
            through: !hole_only && pad.drill.is_some(),
            hole_only,
            drill: pad
                .drill
                .as_ref()
                .map(|d| d.diameter)
                .filter(|d| d.is_finite() && *d > 0.0),
        }
    }
}

/// What a footprint thumbnail draws: its pads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FootprintTile {
    pads: Vec<TilePad>,
}

impl FootprintTile {
    /// The tile of `footprint`; pads with a non-finite position or size are
    /// left out.
    pub fn of(footprint: &Footprint) -> Self {
        Self {
            pads: footprint
                .pads
                .iter()
                .filter(|p| p.position.iter().chain(&p.size).all(|v| v.is_finite()))
                .map(TilePad::of)
                .collect(),
        }
    }

    pub fn pads(&self) -> &[TilePad] {
        &self.pads
    }

    /// The box around every pad, mm.
    fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        self.pads.iter().fold(None, |acc, pad| {
            let (hw, hh) = (pad.size.0 / 2.0, pad.size.1 / 2.0);
            let (x0, y0, x1, y1) = (
                pad.center.0 - hw,
                pad.center.1 - hh,
                pad.center.0 + hw,
                pad.center.1 + hh,
            );
            Some(match acc {
                None => (x0, y0, x1, y1),
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            })
        })
    }

    /// The scale (px per mm) and offset that centre the pads in `size`
    /// with a margin; `None` with no pads.
    pub fn fit(&self, size: Size) -> Option<(f32, Point)> {
        let (x0, y0, x1, y1) = self.bounds()?;
        let w = ((x1 - x0) as f32).max(1e-3);
        let h = ((y1 - y0) as f32).max(1e-3);
        let free_w = (size.width * (1.0 - 2.0 * MARGIN_RATIO)).max(1.0);
        let free_h = (size.height * (1.0 - 2.0 * MARGIN_RATIO)).max(1.0);
        let scale = (free_w / w).min(free_h / h);
        let offset = Point::new(
            (size.width - w * scale) / 2.0 - x0 as f32 * scale,
            (size.height - h * scale) / 2.0 - y0 as f32 * scale,
        );
        Some((scale, offset))
    }
}

struct TileCanvas {
    tile: Arc<FootprintTile>,
}

impl<M> canvas::Program<M> for TileCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let palette = BoardPalette::new();
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let ground = canvas::Path::rounded_rectangle(
            Point::ORIGIN,
            bounds.size(),
            Radii::GRAPHITE.small.into(),
        );
        frame.fill(&ground, palette.canvas);
        let Some((scale, offset)) = self.tile.fit(bounds.size()) else {
            return vec![frame.into_geometry()];
        };
        let map =
            |x: f64, y: f64| Point::new(offset.x + x as f32 * scale, offset.y + y as f32 * scale);
        let copper = palette.swatch(BoardSwatch::TopCopper);
        for pad in self.tile.pads() {
            let (w, h) = (pad.size.0 as f32 * scale, pad.size.1 as f32 * scale);
            if !pad.hole_only {
                let top_left = map(
                    pad.center.0 - pad.size.0 / 2.0,
                    pad.center.1 - pad.size.1 / 2.0,
                );
                let radius = w.min(h) * pad.roundness as f32;
                let body =
                    canvas::Path::rounded_rectangle(top_left, Size::new(w, h), radius.into());
                let ink = if pad.through {
                    palette.multi_layer
                } else {
                    copper
                };
                frame.fill(&body, ink);
            }
            let drill = pad
                .drill
                .map(|d| d as f32 * scale)
                .or_else(|| pad.hole_only.then_some(w.min(h)));
            if let Some(diameter) = drill {
                let hole = canvas::Path::circle(map(pad.center.0, pad.center.1), diameter / 2.0);
                frame.fill(&hole, palette.canvas);
            }
        }
        vec![frame.into_geometry()]
    }
}

/// A `width` × `height` tile of `tile` on the board ground.
pub(crate) fn footprint_tile<'a, M: 'a>(
    tile: Arc<FootprintTile>,
    width: f32,
    height: f32,
) -> Element<'a, M> {
    let canvas = canvas::Canvas::new(TileCanvas { tile })
        .width(width)
        .height(height);
    iced::widget::container::<_, Theme>(canvas)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .clip(true)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(number: &str, x: f64, kind: PadKind, rotation: f64) -> Pad {
        Pad {
            number: number.into(),
            kind,
            shape: PadShape::Rect,
            position: [x, 0.0],
            size: [0.6, 0.5],
            rotation,
            ..Pad::default()
        }
    }

    /// Mockup row 119: a footprint tile draws that footprint's own pads, so
    /// two footprints get two pictures.
    #[test]
    fn the_tile_holds_the_footprints_own_pads() {
        let mut fp = Footprint::empty("R_0402");
        fp.pads = vec![
            pad("1", -0.5, PadKind::Smd, 0.0),
            pad("2", 0.5, PadKind::Smd, 90.0),
        ];
        let tile = FootprintTile::of(&fp);
        assert_eq!(tile.pads().len(), 2);
        assert_eq!(tile.pads()[0].center, (-0.5, 0.0));
        assert_eq!(
            tile.pads()[1].size,
            (0.5, 0.6),
            "a quarter turn swaps the sides"
        );
        assert!(!tile.pads()[0].through);

        let mut other = fp.clone();
        other.pads.push(pad("3", 1.5, PadKind::Smd, 0.0));
        assert_ne!(FootprintTile::of(&other), tile);
    }

    #[test]
    fn a_non_plated_hole_is_only_its_hole_and_bad_pads_are_left_out() {
        let mut fp = Footprint::empty("MH");
        fp.pads = vec![
            pad("", 0.0, PadKind::NptHole, 0.0),
            pad("2", f64::NAN, PadKind::Smd, 0.0),
        ];
        let tile = FootprintTile::of(&fp);
        assert_eq!(tile.pads().len(), 1);
        assert!(tile.pads()[0].hole_only);
        assert!(!tile.pads()[0].through);
    }

    /// The pads are centred in the tile with a margin on every side.
    #[test]
    fn the_pads_fit_inside_the_tile() {
        let mut fp = Footprint::empty("R");
        fp.pads = vec![
            pad("1", -0.5, PadKind::Smd, 0.0),
            pad("2", 0.5, PadKind::Smd, 0.0),
        ];
        let tile = FootprintTile::of(&fp);
        let size = Size::new(40.0, 28.0);
        let (scale, offset) = tile.fit(size).expect("pads");
        let left = offset.x + (-0.8_f32) * scale;
        let right = offset.x + 0.8 * scale;
        assert!(left > 0.0 && right < size.width, "{left} {right}");
        assert!(((left + right) / 2.0 - size.width / 2.0).abs() < 1e-3);
        assert!(FootprintTile::default().fit(size).is_none());
    }
}
