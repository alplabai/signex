//! Flipping a footprint to the other board face (ADR-0005 §7).

use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::pcb::Footprint;

/// Flip `footprint` to the other face about its own y axis.
///
/// The footprint, its pads and its graphics swap their sided layers
/// (top <-> bottom); a layer this build does not know is left alone. Local
/// coordinates are untouched: the world transform mirrors them. Rotation
/// becomes `-rotation` (mod 360), which keeps every pad where a physical
/// turn about the vertical axis puts it, since `R(-t) M = M R(t)` for the
/// x mirror `M`.
pub fn flip_footprint(footprint: &mut Footprint) {
    flip_layer(&mut footprint.layer);
    for pad in &mut footprint.pads {
        pad.layers.iter_mut().for_each(flip_layer);
    }
    for graphic in &mut footprint.graphics {
        flip_layer(&mut graphic.layer);
    }
    // 0.0 - 0.0 keeps +0; rem_euclid maps -x into [0, 360).
    footprint.rotation = (-footprint.rotation).rem_euclid(360.0);
}

fn flip_layer(layer: &mut String) {
    if let Some(parsed) = parse_pcb_layer(layer) {
        *layer = pcb_layer_token(parsed.flipped());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::pad_world_center;
    use crate::units::PointNm;
    use signex_types::pcb::{Pad, PadShape, PadType, Point};
    use uuid::Uuid;

    fn footprint(rotation: f64, layer: &str) -> Footprint {
        let pad = Pad {
            uuid: Uuid::nil(),
            number: "1".into(),
            pad_type: PadType::Smd,
            shape: PadShape::Rect,
            position: Point::new(1.5, 0.75),
            size: Point::new(1.0, 1.0),
            drill: None,
            layers: vec![layer.into(), "Unknown.Layer".into()],
            net: None,
            roundrect_ratio: 0.0,
            unknown: Default::default(),
            unknown_columns: Default::default(),
        };
        Footprint {
            uuid: Uuid::nil(),
            reference: "R1".into(),
            value: String::new(),
            footprint_id: String::new(),
            position: Point::new(10.0, 20.0),
            rotation,
            layer: layer.into(),
            locked: false,
            pads: vec![pad],
            graphics: Vec::new(),
            properties: Vec::new(),
            unknown: Default::default(),
            unknown_columns: Default::default(),
        }
    }

    #[test]
    fn flip_swaps_layers_and_mirrors_the_rotation() {
        let mut fp = footprint(30.0, "TopCopper");
        flip_footprint(&mut fp);
        assert_eq!(fp.layer, "BottomCopper");
        assert_eq!(fp.pads[0].layers, ["BottomCopper", "Unknown.Layer"]);
        assert_eq!(fp.rotation, 330.0);
        assert_eq!(fp.pads[0].position, Point::new(1.5, 0.75));
        flip_footprint(&mut fp);
        assert_eq!(fp.layer, "TopCopper");
        assert_eq!(fp.rotation, 30.0);
    }

    #[test]
    fn flip_mirrors_the_pad_about_the_footprint_y_axis() {
        // A turn about the vertical axis through the origin: x -> 2*ox - x.
        for rotation in [0.0, 90.0, 30.0, 270.0] {
            let mut fp = footprint(rotation, "TopCopper");
            let before = pad_world_center(&fp, &fp.pads[0]).unwrap();
            flip_footprint(&mut fp);
            let after = pad_world_center(&fp, &fp.pads[0]).unwrap();
            let ox = PointNm::from_mm(fp.position, "t").unwrap().x;
            assert!((after.x - (2 * ox - before.x)).abs() <= 1, "{rotation}");
            assert!((after.y - before.y).abs() <= 1, "{rotation}");
        }
    }
}
