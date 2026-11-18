//! Pour fills for the PCB scene (#2216).
//!
//! `pcb-geom` computes a pour's copper (`pcb_geom::zone_fills`); this hands
//! each pour of the snapshot its pieces. [`emit`](super::emit) draws the
//! pieces as copper and only strokes the pour's outline, on the GPU and CPU
//! paths alike (both read the same `Scene`).

use pcb_geom::{Polygon, ZoneFills};

use super::PcbSnapshot;

/// One area of a pour's copper in board mm: an outer ring and its holes.
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneFillPiece {
    pub vertices: Vec<[f32; 2]>,
    pub holes: Vec<Vec<[f32; 2]>>,
}

impl ZoneFillPiece {
    fn of(polygon: &Polygon) -> Self {
        let ring = |points: &[pcb_geom::PointNm]| {
            points
                .iter()
                .map(|p| {
                    let mm = p.to_mm();
                    [mm.x as f32, mm.y as f32]
                })
                .collect()
        };
        ZoneFillPiece {
            vertices: ring(&polygon.outer),
            holes: polygon.holes.iter().map(|h| ring(h)).collect(),
        }
    }
}

impl PcbSnapshot {
    /// This snapshot with each pour's copper from `fills`, which must be
    /// the fills of the board the snapshot was built from. A pour `fills`
    /// does not hold is drawn as its outline alone.
    pub fn with_zone_fills(mut self, fills: &ZoneFills) -> Self {
        for zone in &mut self.zones {
            if let Some(pieces) = zone.fill.as_mut() {
                *pieces = fills
                    .of(zone.source_order)
                    .map(|f| f.pieces.iter().map(ZoneFillPiece::of).collect())
                    .unwrap_or_default();
            }
        }
        self
    }
}

#[cfg(test)]
#[path = "zone_fill_tests.rs"]
mod tests;
