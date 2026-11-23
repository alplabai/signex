//! The rubber band of the multi-corner board tools (Caner, 2026-11-21):
//! while the corners placed so far and the pointer enclose no area yet (one
//! corner placed, or all of them on one line), the preview draws the open
//! outline from the first corner to the pointer as thin lines, so the shape
//! shows from its first click. Solid Region, Polygon Pour, Keepout, Fill,
//! Pour Cutout and the Slice Pour line all draw it through here.

use signex_types::format::pcb_layer_token;
use signex_types::pcb::{Point, Segment};
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

/// The width, in mm, the open outline is previewed at.
pub(super) const OUTLINE_PREVIEW_WIDTH_MM: f64 = 0.1;

/// The lines joining `points` in order on `layer`, leaving out a line whose
/// ends meet (the pointer on the last corner).
pub(super) fn open_outline(points: &[Point], layer: PcbLayer) -> Vec<Segment> {
    let token = pcb_layer_token(layer);
    points
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .map(|pair| Segment {
            locked: false,
            attributes: Default::default(),
            uuid: Uuid::nil(),
            start: pair[0],
            end: pair[1],
            width: OUTLINE_PREVIEW_WIDTH_MM,
            layer: token.clone(),
            net: 0,
            unknown_columns: Default::default(),
        })
        .collect()
}
