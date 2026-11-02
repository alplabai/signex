//! Two-pad chip lands (resistor, capacitor, inductor, fuse, diode, LED)
//! in the EIA sizes 0201 to 2512, plain and with a cathode mark.
//!
//! One builder serves every size; [`CHIP_SIZES`] is the data. Land numbers
//! (mm) for 0201, 0402, 0603, 1206, 1210 and 2512 come from the IPC-7351B
//! chip-component formulas at the nominal density level (B), with the
//! EIA body size and terminal tolerances, rounded to 0.05 mm:
//!
//! ```text
//! size  pad (len x wid)  centre  body (l x w)  courtyard excess
//! 0201  0.35 x 0.30      0.70    0.6 x 0.3      0.10
//! 0402  0.55 x 0.55      1.05    1.0 x 0.5      0.15
//! 0603  0.80 x 0.90      1.65    1.6 x 0.8      0.25
//! 0805  1.00 x 1.25      2.00    2.0 x 1.25     shipped land, kept as is
//! 1206  1.00 x 1.75      3.05    3.2 x 1.6      0.25
//! 1210  1.00 x 2.65      3.05    3.2 x 2.5      0.25
//! 2512  1.10 x 3.40      6.20    6.4 x 3.2      0.25
//! ```
//!
//! The 0805 row is the land that shipped first (#1657) and stays byte
//! for byte; IPC-7351B nominal for 0805 is about 1.0 x 1.4 at 1.9 centre.
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
    /// Courtyard half-extent of the plain footprint (IPC excess included).
    pub court_hx: f64,
    pub court_hy: f64,
    /// Plain variant carries courtyard and fab outline (the shipped
    /// 0805 does not).
    pub plain_outline: bool,
}

pub const CHIP_SIZES: &[ChipSize] = &[
    size("0201", [0.35, 0.30, 0.70], [0.6, 0.3], 0.10, [0.625, 0.25]),
    size("0402", [0.55, 0.55, 1.05], [1.0, 0.5], 0.15, [0.95, 0.425]),
    size("0603", [0.80, 0.90, 1.65], [1.6, 0.8], 0.20, [1.475, 0.70]),
    ChipSize {
        code: "0805",
        pad_len: 1.0,
        pad_wid: 1.25,
        centre: 2.0,
        body_len: 2.0,
        body_wid: 1.25,
        bar_gap: 0.2,
        court_hx: 1.9,
        court_hy: 0.9,
        plain_outline: false,
    },
    size("1206", [1.00, 1.75, 3.05], [3.2, 1.6], 0.20, [2.275, 1.125]),
    size("1210", [1.00, 2.65, 3.05], [3.2, 2.5], 0.20, [2.275, 1.575]),
    size("2512", [1.10, 3.40, 6.20], [6.4, 3.2], 0.20, [3.90, 1.95]),
];

const fn size(
    code: &'static str,
    pad: [f64; 3],
    body: [f64; 2],
    bar_gap: f64,
    court: [f64; 2],
) -> ChipSize {
    ChipSize {
        code,
        pad_len: pad[0],
        pad_wid: pad[1],
        centre: pad[2],
        body_len: body[0],
        body_wid: body[1],
        bar_gap,
        court_hx: court[0],
        court_hy: court[1],
        plain_outline: true,
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

    if polarised || size.plain_outline {
        // Courtyard and fab outline are plain footprint fields: the
        // sketch courtyard tag (a unit struct) cannot be written as TOML.
        let hx = if polarised {
            size.court_hx.max(um(bar_x + 0.2))
        } else {
            size.court_hx
        };
        let hy = size.court_hy;
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
