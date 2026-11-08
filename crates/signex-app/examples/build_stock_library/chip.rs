//! Two-pad chip lands (resistor, capacitor, inductor, fuse, diode, LED)
//! in the EIA sizes 0201 to 2512, plain and with a cathode mark.
//!
//! One builder serves every size; [`CHIP_SIZES`] is the data. IPC-7351
//! gives fillet goals, not fixed per-size lands, so the lands are a
//! manufacturer's table at "IPC-7351 Density Level B (manufacturer
//! tables)", in mm:
//!
//! ```text
//! size  pad (len x wid)  c-c   Z     G     body (l x w)  courtyard (l x w)  height
//! 0201  0.46 x 0.42      0.66  1.12  0.20  0.6 x 0.3     1.5 x 0.8          0.33
//! 0402  0.62 x 0.62      0.90  1.52  0.28  1.0 x 0.5     1.9 x 1.0          0.60
//! 0603  0.95 x 1.00      1.60  2.55  0.65  1.6 x 0.8     3.1 x 1.5          0.92
//! 0805  1.15 x 1.45      1.80  2.95  0.65  2.0 x 1.25    3.5 x 2.0          1.40
//! 1206  1.15 x 1.80      3.00  4.15  1.85  3.2 x 1.6     4.7 x 2.3          1.80
//! 1210  1.15 x 2.70      3.00  4.15  1.85  3.2 x 2.5     4.7 x 3.2          2.30
//! 2512  1.45 x 3.50      6.20  7.65  4.75  6.4 x 3.2     8.2 x 4.0          0.70
//! ```
//!
//! Sources:
//! - 0201 to 1210: KEMET, "Surface Mount Multilayer Ceramic Chip
//!   Capacitors (SMD MLCCs) - C0G Dielectric, 10-250 VDC (Commercial
//!   Grade)", C1003_C0G, 3/30/2016, p.10, "Table 3 - Chip Capacitor Land
//!   Pattern Design Recommendations per IPC-7351", Density Level B: pad
//!   (Y x X), C, and the courtyard V1 x V2.
//! - 2512: Vishay D/CRCW e3 thick-film chip resistors, Doc 20035,
//!   "Recommended Solder Pad Dimensions", reflow (no IPC-labelled nominal
//!   2512 land was found). Its courtyard follows the rule the KEMET
//!   values obey: pad extent plus 0.25 per side, rounded up to 0.1.
//! - Height: the thickest code of the size in KEMET Table 2A (nominal plus
//!   tolerance, 0201 to 1210) and Vishay D/CRCW H for 2512 (0.6 + 0.1).
//!
//! Pad 1 is the left pad. In the polarised variant it is the cathode,
//! marked by a 0.12 mm silkscreen bar `BAR_GAP` outside the copper (0.1 mm
//! clear of the mask opening), a fab cathode line and a courtyard that
//! holds the bar plus 0.2, rounded up to 0.1.

use signex_library::primitive::footprint::{Footprint, FpGraphic, FpGraphicKind, Polygon};
use signex_sketch::SketchData;
use signex_sketch::attr::{PadShape, PadSide, SilkAttr};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::plane::{Plane, PlaneKind};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use super::ids::{StockIds, StockKind};
use super::{next_id, smd_pad, stock_timestamp};

/// Copper edge to the centre of the cathode bar. The bar is 0.12 wide and
/// the pads carry a 0.05 mask margin, so this leaves 0.11 to the opening.
pub const BAR_GAP: f64 = 0.22;

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
    /// Courtyard extent along the pad-to-pad axis (KEMET V1).
    pub court_len: f64,
    /// Courtyard extent across it (KEMET V2).
    pub court_wid: f64,
    /// Maximum body thickness, for the 3D body.
    pub height: f64,
}

pub const CHIP_SIZES: &[ChipSize] = &[
    size("0201", [0.46, 0.42, 0.66], [0.6, 0.3], [1.5, 0.8], 0.33),
    size("0402", [0.62, 0.62, 0.90], [1.0, 0.5], [1.9, 1.0], 0.60),
    size("0603", [0.95, 1.00, 1.60], [1.6, 0.8], [3.1, 1.5], 0.92),
    size("0805", [1.15, 1.45, 1.80], [2.0, 1.25], [3.5, 2.0], 1.40),
    size("1206", [1.15, 1.80, 3.00], [3.2, 1.6], [4.7, 2.3], 1.80),
    size("1210", [1.15, 2.70, 3.00], [3.2, 2.5], [4.7, 3.2], 2.30),
    size("2512", [1.45, 3.50, 6.20], [6.4, 3.2], [8.2, 4.0], 0.70),
];

const fn size(
    code: &'static str,
    pad: [f64; 3],
    body: [f64; 2],
    court: [f64; 2],
    height: f64,
) -> ChipSize {
    ChipSize {
        code,
        pad_len: pad[0],
        pad_wid: pad[1],
        centre: pad[2],
        body_len: body[0],
        body_wid: body[1],
        court_len: court[0],
        court_wid: court[1],
        height,
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
        Soic8 | Qfn16 | MountingHole3p2 | Fiducial1mm => return None,
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
    fp.body_3d.height_mm = size.height as f32;
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
    let bar_x = um(outer + BAR_GAP);
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
            layer: PcbLayer::Technical(TechnicalLayer::SilkscreenTop),
        });
        sketch.entities.push(bar);
    }

    {
        // Courtyard and fab outline are plain footprint fields: the
        // sketch courtyard tag (a unit struct) cannot be written as TOML.
        let (hx, hy) = if polarised {
            // Hold the bar plus 0.2, rounded up to 0.1 on the full length.
            let full = (2.0 * (bar_x + 0.2) * 10.0 - 1e-6).ceil() / 10.0;
            ((full / 2.0).max(size.court_len / 2.0), size.court_wid / 2.0)
        } else {
            (size.court_len / 2.0, size.court_wid / 2.0)
        };
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
