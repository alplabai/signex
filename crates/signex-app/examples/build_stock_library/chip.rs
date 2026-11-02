//! Two-pad chip lands (resistor, capacitor, inductor, fuse, diode, LED)
//! in the EIA sizes 0201 to 2512, plain and with a cathode mark.
//!
//! One builder serves every size; [`CHIP_SIZES`] is the data. IPC-7351
//! gives fillet goals, not fixed per-size lands, so the lands are a
//! manufacturer's IPC-7351 table at density level B (nominal), in mm:
//!
//! ```text
//! size  pad (len x wid)  c-c   Z     G     body (l x w)  courtyard excess
//! 0201  0.46 x 0.42      0.66  1.12  0.20  0.6 x 0.3     0.15
//! 0402  0.62 x 0.62      0.90  1.52  0.28  1.0 x 0.5     0.15
//! 0603  0.95 x 1.00      1.60  2.55  0.65  1.6 x 0.8     0.25
//! 0805  1.15 x 1.45      1.80  2.95  0.65  2.0 x 1.25    0.25
//! 1206  1.15 x 1.80      3.00  4.15  1.85  3.2 x 1.6     0.25
//! 1210  1.15 x 2.70      3.00  4.15  1.85  3.2 x 2.5     0.25
//! 2512  1.45 x 3.50      6.20  7.65  4.75  6.4 x 3.2     0.25
//! ```
//!
//! Sources:
//! - 0201 to 1210: KEMET, "Surface Mount Multilayer Ceramic Chip
//!   Capacitors (SMD MLCCs) - C0G Dielectric, 10-250 VDC (Commercial
//!   Grade)", C1003_C0G, 3/30/2016, p.10, "Table 3 - Chip Capacitor Land
//!   Pattern Design Recommendations per IPC-7351", Density Level B.
//! - 2512: Vishay D/CRCW e3 thick-film chip resistors, Doc 20035,
//!   "Recommended Solder Pad Dimensions", reflow (no IPC-labelled nominal
//!   2512 land was found).
//! - Courtyard excess: IPC-7351 (Feb 2005) Tables 3-5 and 3-6, nominal;
//!   0.15 below 1608 metric, 0.25 from 0603 up.
//!
//! Pad 1 is the left pad. In the polarised variant it is the cathode,
//! marked by a 0.12 mm silkscreen bar `bar_gap` outside the copper, a fab
//! cathode line and a courtyard that holds the bar.

use signex_library::primitive::footprint::{Footprint, FpGraphic, FpGraphicKind, Polygon};
use signex_sketch::SketchData;
use signex_sketch::attr::{PadShape, PadSide, SilkAttr};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::plane::{Plane, PlaneKind};
use signex_types::layer::SignexLayer;

use super::ids::{StockIds, StockKind};
use super::{next_id, smd_pad, stock_timestamp};

/// One chip size: copper land, body and courtyard, all in mm.
pub struct ChipSize {
    /// EIA size code, the footprint is `Chip-<code>`.
    pub code: &'static str,
    /// Pad extent along the pad-to-pad axis.
    pub pad_len: f64,
    /// Pad extent across it.
    pub pad_wid: f64,
    /// Pad centre to pad centre.
    pub centre: f64,
    pub body_len: f64,
    pub body_wid: f64,
    /// Copper edge to cathode bar.
    pub bar_gap: f64,
    /// IPC courtyard excess beyond the copper.
    pub court_excess: f64,
}

pub const CHIP_SIZES: &[ChipSize] = &[
    size("0201", [0.46, 0.42, 0.66], [0.6, 0.3], 0.10, 0.15),
    size("0402", [0.62, 0.62, 0.90], [1.0, 0.5], 0.15, 0.15),
    size("0603", [0.95, 1.00, 1.60], [1.6, 0.8], 0.20, 0.25),
    size("0805", [1.15, 1.45, 1.80], [2.0, 1.25], 0.20, 0.25),
    size("1206", [1.15, 1.80, 3.00], [3.2, 1.6], 0.20, 0.25),
    size("1210", [1.15, 2.70, 3.00], [3.2, 2.5], 0.20, 0.25),
    size("2512", [1.45, 3.50, 6.20], [6.4, 3.2], 0.20, 0.25),
];

const fn size(
    code: &'static str,
    pad: [f64; 3],
    body: [f64; 2],
    bar_gap: f64,
    court_excess: f64,
) -> ChipSize {
    ChipSize {
        code,
        pad_len: pad[0],
        pad_wid: pad[1],
        centre: pad[2],
        body_len: body[0],
        body_wid: body[1],
        bar_gap,
        court_excess,
    }
}

/// Size row and polarity of a chip kind; `None` for the other footprints.
pub fn lookup(kind: StockKind) -> Option<(&'static ChipSize, bool)> {
    use StockKind::*;
    let (code, polarised) = match kind {
        Chip0201 => ("0201", false),
        Chip0402 => ("0402", false),
        Chip0603 => ("0603", false),
        Chip0805 => ("0805", false),
        Chip1206 => ("1206", false),
        Chip1210 => ("1210", false),
        Chip2512 => ("2512", false),
        Chip0201Polarised => ("0201", true),
        Chip0402Polarised => ("0402", true),
        Chip0603Polarised => ("0603", true),
        Chip0805Polarised => ("0805", true),
        Chip1206Polarised => ("1206", true),
        Chip1210Polarised => ("1210", true),
        Chip2512Polarised => ("2512", true),
        _ => return None,
    };
    CHIP_SIZES
        .iter()
        .find(|s| s.code == code)
        .map(|s| (s, polarised))
}

/// Round to a micrometre so derived positions equal their decimal literals.
fn um(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn mm(v: f64) -> String {
    format!("{v:?}mm")
}

pub fn build(ids: &StockIds, size: &ChipSize, polarised: bool) -> Footprint {
    let name = if polarised {
        format!("Chip-{}-Polarised", size.code)
    } else {
        format!("Chip-{}", size.code)
    };
    let mut fp = Footprint::with_identity(name, ids.footprint_uuid(), stock_timestamp());
    let plane = ids.plane_id();
    let mut eids = ids.entity_ids();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };

    sketch.parameters.insert("pad_spacing", mm(size.centre)); // centre-to-centre
    sketch.parameters.insert("pad_w", mm(size.pad_len));
    sketch.parameters.insert("pad_h", mm(size.pad_wid));
    sketch.parameters.insert("body_w", mm(size.body_len));
    sketch.parameters.insert("body_h", mm(size.body_wid));

    for (sign, label) in [(-0.5_f64, "1"), (0.5, "2")] {
        let e = smd_pad(
            next_id(&mut eids),
            plane,
            0.0,
            0.0,
            label,
            "pad_w",
            "pad_h",
            Some(&format!("{sign} * pad_spacing")),
            None,
            None,
            PadSide::Top,
            PadShape::Rect,
            Some("0.05mm"),
            Some("0mm"),
        );
        sketch.entities.push(e);
    }

    let outer = size.centre / 2.0 + size.pad_len / 2.0;
    let bar_x = um(outer + size.bar_gap);
    if polarised {
        // Cathode bar: the sketch is the source for the silkscreen line.
        let bar_hy = um(size.body_wid / 2.0 + 0.075);
        let mut point = |x: f64, y: f64| {
            let id = next_id(&mut eids);
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Point { x, y }));
            id
        };
        let (start, end) = (point(-bar_x, -bar_hy), point(-bar_x, bar_hy));
        let mut bar = Entity::new(next_id(&mut eids), plane, EntityKind::Line { start, end });
        bar.silk = Some(SilkAttr {
            layer: SignexLayer::TopSilk,
        });
        sketch.entities.push(bar);
    }

    {
        // Courtyard and fab outline are plain footprint fields: the
        // sketch courtyard tag (a unit struct) cannot be written as TOML.
        let plain_hx = um(outer + size.court_excess);
        let hx = if polarised {
            plain_hx.max(um(bar_x + 0.2))
        } else {
            plain_hx
        };
        let hy = um(size.pad_wid.max(size.body_wid) / 2.0 + size.court_excess);
        fp.courtyard = Polygon::new(vec![[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy]]);
        let (bx, by) = (size.body_len / 2.0, size.body_wid / 2.0);
        let fab = |kind| FpGraphic {
            kind,
            stroke_width: 0.1,
            filled: false,
            sketch_entity: None,
        };
        fp.fab_f = vec![fab(FpGraphicKind::Rectangle {
            from: [-bx, -by],
            to: [bx, by],
        })];
        if polarised {
            // Cathode line inside the pad-1 end of the body.
            let line_x = -um(bx - (0.3_f64).min(size.body_len / 4.0));
            fp.fab_f.push(fab(FpGraphicKind::Line {
                from: [line_x, -by],
                to: [line_x, by],
            }));
        }
    }

    fp.sketch = Some(sketch);
    fp
}
