//! The drawings on layers no Gerber image carries (fab, assembly,
//! courtyard, documentation, mechanical, ...), for the whole-job exports
//! that keep them as document layers, and the board-layer view of every
//! mark those exports write.

use pcb_geom::{GeomError, Placement, PointNm, Side};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::PcbLayer;

use super::collect::Images;
use super::draw::{Drawing, Drawn, Pen, draw};
use super::frame::FabFrame;
use super::layers::FabLayer;
use super::marks::{Function, Mark};
use super::roles::image_layer;

/// The board layers that carry marks or that the layer table lists.
pub(crate) fn used_layers(
    board: &PcbBoard,
    images: &Images,
    documents: &[(PcbLayer, Vec<Mark>)],
) -> Vec<PcbLayer> {
    images
        .keys()
        .filter_map(|image| image_layer(*image))
        .chain(documents.iter().map(|(layer, _)| *layer))
        .chain(
            board
                .layers
                .iter()
                .filter_map(|def| parse_pcb_layer(&def.name)),
        )
        .collect()
}

/// The marks drawn on a board layer: its image's, or its drawings'.
pub(crate) fn board_marks<'a>(
    images: &'a Images,
    documents: &'a [(PcbLayer, Vec<Mark>)],
    layer: PcbLayer,
) -> &'a [Mark] {
    let image = images
        .iter()
        .find(|(image, _)| image_layer(**image) == Some(layer))
        .map(|(_, marks)| marks.as_slice());
    let drawing = || {
        documents
            .iter()
            .find(|(l, _)| *l == layer)
            .map(|(_, marks)| marks.as_slice())
    };
    image.or_else(drawing).unwrap_or(&[])
}

/// Each drawing layer's marks, in `PcbLayer::all` order. Texts wait for
/// the stroke font (#2387) here as on the images; they and the graphics the
/// geometry refuses are left out without a note, since none of these
/// layers is fabrication data.
pub(crate) fn collect_documents(board: &PcbBoard, frame: FabFrame) -> Vec<(PcbLayer, Vec<Mark>)> {
    let mut layers: Vec<(PcbLayer, Vec<Mark>)> = Vec::new();
    let board_to_fab = |p: Point| frame.of_mm(p, "graphic");
    for graphic in &board.graphics {
        let Some(layer) = drawing_layer(&graphic.layer) else {
            continue;
        };
        let pen = pen(&board_to_fab, false);
        place(&mut layers, layer, draw(&Drawing::from(graphic), &pen));
    }
    for footprint in &board.footprints {
        let Ok(placement) = Placement::of_footprint(footprint) else {
            continue;
        };
        let to_fab = |p: Point| -> Result<PointNm, GeomError> {
            Ok(frame.point(placement.to_world(PointNm::from_mm(p, "graphic")?)))
        };
        for graphic in &footprint.graphics {
            let Some(layer) = drawing_layer(&graphic.layer) else {
                continue;
            };
            let pen = pen(&to_fab, placement.side == Side::Bottom);
            place(&mut layers, layer, draw(&Drawing::from(graphic), &pen));
        }
    }
    layers.sort_by_key(|(layer, _)| PcbLayer::all().position(|l| l == *layer));
    layers
}

/// A known layer that has no Gerber image.
fn drawing_layer(token: &str) -> Option<PcbLayer> {
    parse_pcb_layer(token).filter(|layer| FabLayer::of(*layer).is_none())
}

fn pen<'a>(to_fab: &'a dyn Fn(Point) -> Result<PointNm, GeomError>, mirrored: bool) -> Pen<'a> {
    Pen {
        to_fab,
        arc_ccw_without_mid: mirrored,
        function: Function::Graphic,
        width_override: None,
    }
}

fn place(
    layers: &mut Vec<(PcbLayer, Vec<Mark>)>,
    layer: PcbLayer,
    drawn: Result<Drawn, GeomError>,
) {
    let Ok(Drawn::Marks(marks)) = drawn else {
        return;
    };
    match layers.iter_mut().find(|(l, _)| *l == layer) {
        Some((_, existing)) => existing.extend(marks),
        None => layers.push((layer, marks)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::TechnicalLayer;

    #[test]
    fn only_layers_without_an_image_are_collected() {
        let mut board = super::super::tests::fixture::fixture_board();
        let line = |layer: &str| -> signex_types::pcb::BoardGraphic {
            serde_json::from_value(serde_json::json!({
                "graphic_type": "line", "layer": layer, "width": 0.1,
                "start": {"x": 0.0, "y": 0.0}, "end": {"x": 1.0, "y": 0.0}
            }))
            .expect("a line")
        };
        board.graphics = vec![
            line("TopCourtyard"),
            line("TopSilkscreen"),
            line("Mechanical2"),
        ];
        let layers: Vec<PcbLayer> = collect_documents(&board, FabFrame::of(&board))
            .into_iter()
            .map(|(layer, _)| layer)
            .collect();
        assert!(layers.contains(&PcbLayer::Technical(TechnicalLayer::CourtyardTop)));
        assert!(!layers.contains(&PcbLayer::Technical(TechnicalLayer::SilkscreenTop)));
        assert_eq!(layers.len(), 2, "{layers:?}");
    }
}
