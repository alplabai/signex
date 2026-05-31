//! Build the 5 reference parametric footprints that ship in
//! `assets/stock-library/footprints/`.
//!
//! Run via:
//!
//! ```text
//! cargo run --example build_stock_library -p signex-app
//! ```
//!
//! Each footprint authors a single `BoardTopPlane`, parameterises the
//! interesting dimensions through `SketchData::parameters`, and lays
//! down one Point per pad with the per-pad delta carried on
//! `offset_x_expr` / `offset_y_expr`. The bake walker (signex-app's
//! `apply_sketch_edit`) is the consumer; here we only emit the
//! authored sketch — the bake fires the first time the user opens the
//! file in the footprint editor.
//!
//! Apache-clean: industry-standard footprint dimensions (JEDEC
//! SOIC-8 / QFN-16 / IPC-7351 R0805) only. No third-party EDA-tool
//! source / file-format docs / wikis consulted.

use std::error::Error;
use std::path::Path;

use signex_library::primitive::footprint::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::attr::{DrillSpec, PadAttr, PadKind, PadShape, PadSide, PasteAperturePattern};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

fn main() -> Result<(), Box<dyn Error>> {
    let dir = Path::new("assets/stock-library/footprints");
    std::fs::create_dir_all(dir)?;

    let mut summary: Vec<String> = Vec::new();

    let cases: [(&str, fn() -> Footprint); 5] = [
        ("SOIC-8.snxfpt", build_soic8),
        ("QFN-16.snxfpt", build_qfn16),
        ("R0805.snxfpt", build_r0805),
        ("MountingHole-3.2mm.snxfpt", build_mounting_hole_3p2),
        ("Fiducial-1mm.snxfpt", build_fiducial_1mm),
    ];

    for (filename, builder) in cases {
        let fp = builder();
        let path = dir.join(filename);
        let pad_count = count_sketch_pads(&fp);
        // v0.18.4 — emit TOML+TSV envelope (`snxfpt/1`). Pads land
        // in the per-footprint `pads_tsv = '''…'''` literal multi-
        // line block; the legacy JSON path was dropped.
        let file = FootprintFile::from_footprint(fp.clone());
        let text = file.to_toml_string()?;
        std::fs::write(&path, text.as_bytes())?;
        let line = format!(
            "wrote {:<32} name={:<22} pads={}",
            path.display(),
            fp.name,
            pad_count
        );
        println!("{line}");
        summary.push(line);
    }

    println!(
        "\n{} footprint(s) written to {}",
        summary.len(),
        dir.display()
    );
    Ok(())
}

/// Count Points in `sketch.entities` carrying a `pad` attribute.
fn count_sketch_pads(fp: &Footprint) -> usize {
    fp.sketch
        .as_ref()
        .map(|s| {
            s.entities
                .iter()
                .filter(|e| e.pad.is_some() && matches!(e.kind, EntityKind::Point { .. }))
                .count()
        })
        .unwrap_or(0)
}

/// Helper: author one SMD Point with a `PadAttr`. Anchored at
/// `(anchor_x, anchor_y)`, with optional per-pad relative offsets.
#[allow(clippy::too_many_arguments)]
fn smd_pad(
    plane: PlaneId,
    anchor_x: f64,
    anchor_y: f64,
    number: &str,
    size_x_expr: &str,
    size_y_expr: &str,
    offset_x_expr: Option<&str>,
    offset_y_expr: Option<&str>,
    rotation_expr: Option<&str>,
    side: PadSide,
    shape: PadShape,
    mask_margin_expr: Option<&str>,
    paste_margin_expr: Option<&str>,
) -> Entity {
    let mut e = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Point {
            x: anchor_x,
            y: anchor_y,
        },
    );
    e.pad = Some(PadAttr {
        number: number.into(),
        kind: PadKind::Smd,
        side,
        shape,
        size_x_expr: size_x_expr.into(),
        size_y_expr: size_y_expr.into(),
        rotation_expr: rotation_expr.map(str::to_string),
        offset_x_expr: offset_x_expr.map(str::to_string),
        offset_y_expr: offset_y_expr.map(str::to_string),
        drill: None,
        mask_margin_expr: mask_margin_expr.map(str::to_string),
        paste_margin_expr: paste_margin_expr.map(str::to_string),
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    });
    e
}

// ───────────────────── SOIC-8 ─────────────────────

/// JEDEC SOIC-8: 1.27 mm pitch, body ≈ 3.9 × 4.9 mm, two rows of 4
/// pads with 5.4 mm row-to-row centre spacing (IPC-7351 nominal).
/// Pad copper 0.6 × 1.55 mm, mask margin 0.05 mm.
fn build_soic8() -> Footprint {
    let mut fp = Footprint::empty("SOIC-8");
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };

    // Parameters: pitch, half-row spacing, pad size, body envelope.
    sketch.parameters.insert("pad_pitch", "1.27mm");
    sketch.parameters.insert("row_half", "2.7mm"); // 5.4 mm row-to-row → ±2.7 mm.
    sketch.parameters.insert("pad_w", "1.55mm");
    sketch.parameters.insert("pad_h", "0.6mm");
    sketch.parameters.insert("body_w", "3.9mm");
    sketch.parameters.insert("body_h", "4.9mm");

    // Pin numbering: 1 (top-left), 2..4 down the left side, 5 bottom-right,
    // 6..8 up the right side. Indices relative to row centre:
    //   left  pads (1..4): y = +1.5, +0.5, -0.5, -1.5 × pad_pitch
    //   right pads (5..8): y = -1.5, -0.5, +0.5, +1.5 × pad_pitch
    let left = [(1.5_f64, "1"), (0.5, "2"), (-0.5, "3"), (-1.5, "4")];
    let right = [(-1.5_f64, "5"), (-0.5, "6"), (0.5, "7"), (1.5, "8")];

    for (idx, label) in left {
        let e = smd_pad(
            plane,
            0.0,
            0.0,
            label,
            "pad_w",
            "pad_h",
            Some("-row_half"),
            Some(&format!("{idx} * pad_pitch")),
            None,
            PadSide::Top,
            PadShape::Rect,
            Some("0.05mm"),
            Some("0mm"),
        );
        sketch.entities.push(e);
    }
    for (idx, label) in right {
        let e = smd_pad(
            plane,
            0.0,
            0.0,
            label,
            "pad_w",
            "pad_h",
            Some("row_half"),
            Some(&format!("{idx} * pad_pitch")),
            None,
            PadSide::Top,
            PadShape::Rect,
            Some("0.05mm"),
            Some("0mm"),
        );
        sketch.entities.push(e);
    }

    fp.sketch = Some(sketch);
    fp
}

// ───────────────────── QFN-16 ─────────────────────

/// QFN-16: 0.5 mm pitch, 4 pads per side around a 3 × 3 mm body.
/// Side rows are at ±row_offset from origin; pad copper 0.3 × 0.6 mm
/// (long axis radial). Pin 1 is the top-left of the west side
/// (CCW numbering): W4..W1, S4..S1, E1..E4, N1..N4 → labelled 1..16.
fn build_qfn16() -> Footprint {
    let mut fp = Footprint::empty("QFN-16");
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };

    sketch.parameters.insert("pad_pitch", "0.5mm");
    sketch.parameters.insert("row_offset", "1.6mm"); // pad-centre offset from die centre
    sketch.parameters.insert("pad_long", "0.6mm"); // radial length
    sketch.parameters.insert("pad_short", "0.3mm"); // tangential width
    sketch.parameters.insert("body_w", "3.0mm");
    sketch.parameters.insert("body_h", "3.0mm");

    // Numbering (CCW from top of west side):
    //   1..4   = West side, y = +1.5, +0.5, -0.5, -1.5 × pitch (going down)
    //   5..8   = South side, x = -1.5, -0.5, +0.5, +1.5 × pitch (going right)
    //   9..12  = East side, y = -1.5, -0.5, +0.5, +1.5 × pitch (going up)
    //  13..16  = North side, x = +1.5, +0.5, -0.5, -1.5 × pitch (going left)
    // West row: long axis runs east-west → size_x = pad_long, size_y = pad_short.
    let west = [(1.5_f64, "1"), (0.5, "2"), (-0.5, "3"), (-1.5, "4")];
    for (idx, label) in west {
        let e = smd_pad(
            plane,
            0.0,
            0.0,
            label,
            "pad_long",
            "pad_short",
            Some("-row_offset"),
            Some(&format!("{idx} * pad_pitch")),
            None,
            PadSide::Top,
            PadShape::Rect,
            Some("0.05mm"),
            Some("0mm"),
        );
        sketch.entities.push(e);
    }
    let south = [(-1.5_f64, "5"), (-0.5, "6"), (0.5, "7"), (1.5, "8")];
    for (idx, label) in south {
        let e = smd_pad(
            plane,
            0.0,
            0.0,
            label,
            "pad_short",
            "pad_long",
            Some(&format!("{idx} * pad_pitch")),
            Some("-row_offset"),
            None,
            PadSide::Top,
            PadShape::Rect,
            Some("0.05mm"),
            Some("0mm"),
        );
        sketch.entities.push(e);
    }
    let east = [(-1.5_f64, "9"), (-0.5, "10"), (0.5, "11"), (1.5, "12")];
    for (idx, label) in east {
        let e = smd_pad(
            plane,
            0.0,
            0.0,
            label,
            "pad_long",
            "pad_short",
            Some("row_offset"),
            Some(&format!("{idx} * pad_pitch")),
            None,
            PadSide::Top,
            PadShape::Rect,
            Some("0.05mm"),
            Some("0mm"),
        );
        sketch.entities.push(e);
    }
    let north = [(1.5_f64, "13"), (0.5, "14"), (-0.5, "15"), (-1.5, "16")];
    for (idx, label) in north {
        let e = smd_pad(
            plane,
            0.0,
            0.0,
            label,
            "pad_short",
            "pad_long",
            Some(&format!("{idx} * pad_pitch")),
            Some("row_offset"),
            None,
            PadSide::Top,
            PadShape::Rect,
            Some("0.05mm"),
            Some("0mm"),
        );
        sketch.entities.push(e);
    }

    fp.sketch = Some(sketch);
    fp
}

// ───────────────────── R0805 ─────────────────────

/// IPC-7351 R0805 chip resistor: 2 SMD pads, centre-to-centre 2.0 mm,
/// pad copper 1.0 × 1.25 mm. Body sits between the pads (~2.0 × 1.25 mm).
fn build_r0805() -> Footprint {
    let mut fp = Footprint::empty("R0805");
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };

    sketch.parameters.insert("pad_spacing", "2.0mm"); // centre-to-centre
    sketch.parameters.insert("pad_w", "1.0mm");
    sketch.parameters.insert("pad_h", "1.25mm");
    sketch.parameters.insert("body_w", "2.0mm");
    sketch.parameters.insert("body_h", "1.25mm");

    for (sign, label) in [(-0.5_f64, "1"), (0.5, "2")] {
        let e = smd_pad(
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

    fp.sketch = Some(sketch);
    fp
}

// ───────────────────── Mounting Hole 3.2 mm ─────────────────────

/// Single non-plated through-hole at the origin. 3.2 mm drill
/// (clearance for an M3 fastener). The "pad" copper is suppressed by
/// using a copper-free annular zone equal to the pad size: we set the
/// pad outer to 6.0 mm round but the kind=NptHole means the bake
/// emits an unplated hole. Side=All (drill cuts both copper sides).
fn build_mounting_hole_3p2() -> Footprint {
    let mut fp = Footprint::empty("MountingHole-3.2mm");
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };

    sketch.parameters.insert("drill_dia", "3.2mm");
    sketch.parameters.insert("clearance_dia", "6.0mm");

    let mut e = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Point { x: 0.0, y: 0.0 },
    );
    e.pad = Some(PadAttr {
        number: "MH1".into(),
        kind: PadKind::NptHole,
        side: PadSide::All,
        shape: PadShape::Round,
        size_x_expr: "clearance_dia".into(),
        size_y_expr: "clearance_dia".into(),
        rotation_expr: None,
        offset_x_expr: None,
        offset_y_expr: None,
        drill: Some(DrillSpec {
            diameter_expr: "drill_dia".into(),
            slot_length_expr: None,
            plated: false,
        }),
        mask_margin_expr: Some("0.05mm".into()),
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    });
    sketch.entities.push(e);

    fp.sketch = Some(sketch);
    fp
}

// ───────────────────── Fiducial 1 mm ─────────────────────

/// Single fiducial vision-alignment marker at the origin.
/// 1 mm round copper, 2 mm mask opening (ring of bare substrate
/// around the copper dot — required for camera contrast). No paste,
/// no drill.
fn build_fiducial_1mm() -> Footprint {
    let mut fp = Footprint::empty("Fiducial-1mm");
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };

    sketch.parameters.insert("copper_dia", "1.0mm");
    sketch.parameters.insert("mask_dia", "2.0mm");

    // Mask margin = (mask_dia − copper_dia) / 2 = 0.5 mm.
    let mut e = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Point { x: 0.0, y: 0.0 },
    );
    e.pad = Some(PadAttr {
        number: "FID1".into(),
        kind: PadKind::Fiducial,
        side: PadSide::Top,
        shape: PadShape::Round,
        size_x_expr: "copper_dia".into(),
        size_y_expr: "copper_dia".into(),
        rotation_expr: None,
        offset_x_expr: None,
        offset_y_expr: None,
        drill: None,
        mask_margin_expr: Some("0.5mm".into()),
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    });
    sketch.entities.push(e);

    fp.sketch = Some(sketch);
    fp
}
