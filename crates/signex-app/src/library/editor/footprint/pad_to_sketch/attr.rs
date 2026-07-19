//! Pure mappers between the editor's `EditorPad` and sketch attribute
//! types, plus the small string / plane helpers shared by the mint
//! and solve modules.

use signex_library::primitive::footprint::{
    Footprint, PadKind as LibPadKind, PadShape as LibPadShape,
};
use signex_sketch::attr::{
    ChamferedCorners as SkChamferedCorners, CustomPadShape, PadAttr, PadKind as SkPadKind,
    PadShape as SkPadShape, PadSide, PasteAperturePattern,
};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;

use super::super::state::EditorPad;

/// v0.24 Track A — UUID slug for parameter-name namespacing. Strips
/// dashes so the resulting parameter name is a valid identifier in
/// the expression language.
pub(super) fn id_slug(id: SketchEntityId) -> String {
    id.0.simple().to_string()
}

/// Look up (or create) the footprint's `BoardTop` plane and return
/// its ID. The pad-mirror code assumes every minted entity lives on
/// this single plane.
pub(super) fn ensure_board_top_plane(footprint: &mut Footprint) -> PlaneId {
    let sketch = footprint.sketch.get_or_insert_with(SketchData::default);
    if let Some(p) = sketch
        .planes
        .iter()
        .find(|p| matches!(p.kind, PlaneKind::BoardTop))
    {
        return p.id;
    }
    let p = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let id = p.id;
    sketch.planes.push(p);
    id
}

/// Build a sketch-side `PadAttr` from an `EditorPad`. Carries number /
/// kind / side / shape + size expressions + drill spec. Other PadAttr
/// fields default; the v0.22 mirror path overwrites them.
pub(super) fn pad_attr_from_editor_pad(pad: &EditorPad) -> PadAttr {
    // v0.18.12.1 — carry `drill_diameter_mm` into the sketch PadAttr.
    // Without this, NPT-hole pads lose their drill on the first
    // sketch round-trip. Plated/NPT semantics follow the pad kind.
    let drill = pad
        .drill_diameter_mm
        .map(|d| signex_sketch::attr::DrillSpec {
            diameter_expr: format!("{}mm", format_f64(d)),
            slot_length_expr: None,
            plated: !matches!(pad.kind, LibPadKind::NptHole),
        });
    PadAttr {
        number: pad.number.clone(),
        kind: map_kind(pad.kind),
        side: map_side(pad),
        shape: map_shape(&pad.shape),
        size_x_expr: format!("{}mm", format_f64(pad.size_mm.0)),
        size_y_expr: format!("{}mm", format_f64(pad.size_mm.1)),
        // Hardcoding `None` here baked every sketch-mirrored pad at 0°
        // while `EditorPad::to_pad` wrote the true angle to the literal
        // `Pad` — two persistence paths, two answers for one pad.
        rotation_expr: Some(super::rotation_expr(pad.rotation_deg)),
        offset_x_expr: None,
        offset_y_expr: None,
        drill,
        mask_margin_expr: None,
        paste_margin_expr: None,
        paste_apertures: PasteAperturePattern::Single,
        ..PadAttr::default()
    }
}

pub(super) fn map_kind(k: LibPadKind) -> SkPadKind {
    match k {
        LibPadKind::Smd => SkPadKind::Smd,
        LibPadKind::Tht => SkPadKind::Tht,
        LibPadKind::NptHole => SkPadKind::NptHole,
        LibPadKind::ConnectorPad => SkPadKind::ConnectorPad,
        LibPadKind::Castellated => SkPadKind::Castellated,
        LibPadKind::Fiducial => SkPadKind::Fiducial,
        // Future-proof the non_exhaustive lib enum.
        _ => SkPadKind::Smd,
    }
}

pub(super) fn map_side(pad: &EditorPad) -> PadSide {
    use crate::library::editor::footprint::layers::FpLayer;
    let primary = pad.primary_layer();
    match primary {
        FpLayer::FCu | FpLayer::FFab | FpLayer::FSilks => PadSide::Top,
        FpLayer::BCu | FpLayer::BFab | FpLayer::BSilks => PadSide::Bottom,
        _ => PadSide::All,
    }
}

pub(super) fn map_shape(s: &LibPadShape) -> SkPadShape {
    match s {
        LibPadShape::Round => SkPadShape::Round,
        LibPadShape::Rect => SkPadShape::Rect,
        LibPadShape::Oval => SkPadShape::Oval,
        LibPadShape::RoundRect { radius_ratio } => SkPadShape::RoundRect {
            radius_ratio_expr: format_f64(*radius_ratio),
        },
        LibPadShape::Chamfered {
            chamfer_ratio,
            corners,
        } => SkPadShape::Chamfered {
            chamfer_ratio_expr: format_f64(*chamfer_ratio),
            corners: SkChamferedCorners {
                top_left: corners.top_left,
                top_right: corners.top_right,
                bottom_left: corners.bottom_left,
                bottom_right: corners.bottom_right,
            },
        },
        LibPadShape::Custom(poly) => {
            // Convert lib's free-form polygon into a sketch
            // CustomPadShape::StaticPoints — sketch-profile bake
            // (closed-loop walker) is not used here since literal
            // pads don't have a sketch profile to walk.
            SkPadShape::Custom(CustomPadShape::StaticPoints {
                points: poly.points.clone(),
            })
        }
    }
}

/// Format a float with up to 4 fractional digits, trimming trailing
/// zeros. Keeps the generated expression strings readable
/// (e.g. `1.5` rather than `1.5000000000000`).
pub(super) fn format_f64(v: f64) -> String {
    let s = format!("{v:.4}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}
