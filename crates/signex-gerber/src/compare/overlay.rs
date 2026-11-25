//! The board drawn over the package: its outline, cut-outs and copper
//! keep-outs, placed by the exporter's frame rule (`FabFrame`).

use pcb_geom::PointNm;
use signex_output::fab::FabFrame;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use signex_types::pcb_sketch::PcbKeepout;

use crate::FileFunction;

/// One keep-out of the board in the file frame, with the copper layers it
/// keeps clear.
pub(super) struct Keepout {
    pub layers: Vec<CopperLayer>,
    pub ring: Vec<PointNm>,
}

/// The board outline then each cut-out, in the file frame. A ring with a
/// point the geometry cannot hold is left out, as the exporter leaves it.
pub(super) fn outline(board: &PcbBoard) -> Vec<Vec<PointNm>> {
    let frame = FabFrame::of(board);
    let cutouts = board
        .sketch
        .iter()
        .flat_map(|sketch| sketch.cutouts.iter())
        .map(|cutout| cutout.boundary.as_slice());
    std::iter::once(board.outline.as_slice())
        .chain(cutouts)
        .filter_map(|ring| ring_in_frame(ring, frame))
        .collect()
}

/// Every keep-out of the board that keeps copper out: the keep-outs drawn
/// on the board (#2373) and those the board sketch baked, as the design
/// rule check reads them.
pub(super) fn keepouts(board: &PcbBoard) -> Vec<Keepout> {
    let frame = FabFrame::of(board);
    let drawn: Vec<PcbKeepout> = board.zones.iter().filter_map(|z| z.as_keepout()).collect();
    let sketch = board
        .sketch
        .iter()
        .flat_map(|stored| stored.keepouts.iter());
    sketch
        .chain(&drawn)
        .filter_map(|keepout| {
            let layers: Vec<CopperLayer> = keepout
                .layers
                .iter()
                .filter_map(|layer| layer.copper())
                .collect();
            if layers.is_empty() {
                return None;
            }
            let ring = ring_in_frame(&keepout.boundary, frame)?;
            Some(Keepout { layers, ring })
        })
        .collect()
}

/// The keep-out rings that apply to `board_layer` on a board of `copper`
/// copper layers: Top is L1, Bottom is Ln, an inner keep-out applies to
/// every inner layer (the file numbers inner layers by position, the board
/// by name).
pub(super) fn keepouts_on(
    keepouts: &[Keepout],
    board_layer: &FileFunction,
    copper: u32,
) -> Vec<Vec<PointNm>> {
    let FileFunction::Copper { number, .. } = *board_layer else {
        return Vec::new();
    };
    keepouts
        .iter()
        .filter(|keepout| {
            keepout.layers.iter().any(|layer| match layer {
                CopperLayer::Top => number == 1,
                CopperLayer::Bottom => number == copper,
                CopperLayer::Inner(_) => number > 1 && number < copper,
            })
        })
        .map(|keepout| keepout.ring.clone())
        .collect()
}

/// `ring` in the file frame; `None` for fewer than three points or a
/// point the geometry cannot hold.
fn ring_in_frame(ring: &[Point], frame: FabFrame) -> Option<Vec<PointNm>> {
    if ring.len() < 3 {
        return None;
    }
    ring.iter()
        .map(|point| frame.of_mm(*point, "board overlay").ok())
        .collect()
}
