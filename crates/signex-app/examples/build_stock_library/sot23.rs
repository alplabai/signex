//! SOT-23 (JEDEC TO-236): three leads, pins 1 and 2 on one side at a
//! 0.95 mm pitch, pin 3 centred on the other side.
//!
//! Land, in mm, from the onsemi SOT-23 (Case 318-08) package drawing,
//! "Recommended Mounting Footprint": pad 0.8 x 0.9 (across x along the
//! lead), 0.95 pitch, 2.0 row to row centre. Body 2.9 x 1.3 and 1.11 high
//! (the drawing's maximum). The courtyard holds the larger of body and
//! land plus 0.25 per side, the rule the chip lands follow.
//!
//! Pin 1 is bottom left, pin 2 bottom right, pin 3 top centre (y up), the
//! pinout of the stock bipolar (B, E, C) and MOSFET (G, S, D) symbols.

use signex_library::primitive::footprint::{Footprint, FpGraphic, FpGraphicKind, Polygon};
use signex_sketch::SketchData;
use signex_sketch::attr::{PadShape, PadSide};
use signex_sketch::plane::{Plane, PlaneKind};

use super::ids::StockIds;
use super::{next_id, smd_pad, stock_timestamp};

const PITCH: f64 = 0.95;
const ROW_HALF: f64 = 1.0;
const PAD_W: f64 = 0.8;
const PAD_H: f64 = 0.9;
const BODY_W: f64 = 2.9;
const BODY_H: f64 = 1.3;
const HEIGHT: f64 = 1.11;
const COURTYARD_EXCESS: f64 = 0.25;

pub fn build(ids: &StockIds) -> Footprint {
    let mut fp = Footprint::with_identity("SOT-23", ids.footprint_uuid(), stock_timestamp());
    fp.body_3d.height_mm = HEIGHT as f32;
    let plane = ids.plane_id();
    let mut eids = ids.entity_ids();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };

    sketch.parameters.insert("pad_pitch", "0.95mm");
    sketch.parameters.insert("row_half", "1.0mm");
    sketch.parameters.insert("pad_w", "0.8mm");
    sketch.parameters.insert("pad_h", "0.9mm");
    sketch.parameters.insert("body_w", "2.9mm");
    sketch.parameters.insert("body_h", "1.3mm");

    let pads = [
        ("1", "-pad_pitch", "-row_half"),
        ("2", "pad_pitch", "-row_half"),
        ("3", "0mm", "row_half"),
    ];
    for (label, x, y) in pads {
        let e = smd_pad(
            next_id(&mut eids),
            plane,
            0.0,
            0.0,
            label,
            "pad_w",
            "pad_h",
            Some(x),
            Some(y),
            None,
            PadSide::Top,
            PadShape::Rect,
            Some("0.05mm"),
            Some("0mm"),
        );
        sketch.entities.push(e);
    }

    let (bx, by) = (BODY_W / 2.0, BODY_H / 2.0);
    let hx = round_up_tenth(bx.max(PITCH + PAD_W / 2.0) + COURTYARD_EXCESS);
    let hy = round_up_tenth(by.max(ROW_HALF + PAD_H / 2.0) + COURTYARD_EXCESS);
    fp.courtyard = Polygon::new(vec![[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy]]);
    fp.fab_f = vec![FpGraphic {
        kind: FpGraphicKind::Rectangle {
            from: [-bx, -by],
            to: [bx, by],
        },
        stroke_width: 0.1,
        filled: false,
        sketch_entity: None,
    }];

    fp.sketch = Some(sketch);
    fp
}

/// Round a half extent up so the full extent is a whole 0.1 mm.
fn round_up_tenth(half: f64) -> f64 {
    ((2.0 * half * 10.0 - 1e-6).ceil() / 10.0) / 2.0
}
