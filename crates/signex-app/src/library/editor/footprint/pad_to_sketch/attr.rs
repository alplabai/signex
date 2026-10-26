//! Pure mappers between the editor's `EditorPad` and sketch attribute
//! types, plus the small string / plane helpers shared by the mint
//! and solve modules.

use signex_library::primitive::footprint::{
    Footprint, PadKind as LibPadKind, PadShape as LibPadShape,
};
use signex_sketch::attr::{
    ChamferedCorners as SkChamferedCorners, CustomPadShape, DrillSpec, PadAttr,
    PadKind as SkPadKind, PadShape as SkPadShape, PadSide, PadStackOverrides, PasteAperturePattern,
};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;

use super::super::state::EditorPad;

/// v0.22 Phase D1 — Pads-mode → Sketch attribute mirror. Push every
/// editor-owned pad field onto the `PadAttr` carried by the pad's
/// centre `Point`, for each pad that has one.
///
/// Lives here rather than in `FootprintEditorState` because it is
/// pad↔sketch mapping policy, which is what this module is; the
/// editor-state module only calls it.
pub fn mirror_pad_attrs_into_sketch(pads: &[EditorPad], sketch: &mut SketchData) {
    for pad in pads {
        let Some(id) = pad.sketch_entity_id else {
            continue;
        };
        let Some(entity) = sketch.entities.iter_mut().find(|e| e.id == id) else {
            continue;
        };
        let Some(attr) = entity.pad.as_mut() else {
            continue;
        };
        attr.number = pad.number.clone();
        attr.net = pad.net.clone();
        attr.locked = pad.locked;
        attr.electrical_type = pad.electrical_type;
        attr.template = pad.template.clone();
        attr.library = pad.template_library.clone();
        attr.feature_top = pad.feature_top;
        attr.feature_bottom = pad.feature_bottom;
        attr.testpoint = pad.testpoint;
        attr.hole_tolerance_plus_mm = pad.hole_tolerance_plus_mm;
        attr.hole_tolerance_minus_mm = pad.hole_tolerance_minus_mm;
        attr.hole_rotation_deg = pad.hole_rotation_deg;
        attr.copper_offset_x_mm = pad.copper_offset_x_mm;
        attr.copper_offset_y_mm = pad.copper_offset_y_mm;
        mirror_rotation_expr(&mut attr.rotation_expr, pad.rotation_deg);
        mirror_drill(&mut attr.drill, pad);
        mirror_len_expr(
            &mut attr.mask_margin_expr,
            pad.passthrough.solder_mask_margin_mm,
        );
        mirror_len_expr(&mut attr.paste_margin_expr, pad.passthrough.paste_margin_mm);
        mirror_shape(&mut attr.shape, &pad.shape);
        mirror_stack(&mut attr.stack, pad);
    }
}

/// Open-time reconcile, the inverse of [`mirror_pad_attrs_into_sketch`]:
/// seed every linked `EditorPad` field the mirror writes (and the solve
/// rebakes) from the sketch `PadAttr`.
///
/// The sketch `PadAttr` is authoritative. A Sketch-mode edit writes only
/// `PadAttr`, and a file saved before #960 carries no extended pad columns,
/// so the `Pad` list alone would hand the first Pads-mode edit default
/// values to mirror over the sketch's real ones. Where a post-#960 file
/// holds both copies they agree (the mirror keeps them equal), and if they
/// ever differ the sketch wins, because that is what the next solve bakes.
///
/// A margin bound to a parameter (not a plain `mm` literal) cannot be read
/// without a solve; the editor keeps the baked value the `Pad` carries.
pub fn seed_pads_from_sketch_attrs(pads: &mut [EditorPad], sketch: &SketchData) {
    for pad in pads {
        let Some(id) = pad.sketch_entity_id else {
            continue;
        };
        let Some(attr) = sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.pad.as_ref())
        else {
            continue;
        };
        pad.net = attr.net.clone();
        pad.locked = attr.locked;
        pad.electrical_type = attr.electrical_type;
        pad.template = attr.template.clone();
        pad.template_library = attr.library.clone();
        pad.feature_top = attr.feature_top;
        pad.feature_bottom = attr.feature_bottom;
        pad.testpoint = attr.testpoint;
        pad.hole_tolerance_plus_mm = attr.hole_tolerance_plus_mm;
        pad.hole_tolerance_minus_mm = attr.hole_tolerance_minus_mm;
        pad.hole_rotation_deg = attr.hole_rotation_deg;
        pad.copper_offset_x_mm = attr.copper_offset_x_mm;
        pad.copper_offset_y_mm = attr.copper_offset_y_mm;
        let st = &attr.stack;
        seed_margin(&mut pad.stack.paste_margin_top, &st.paste_top_expr);
        seed_margin(&mut pad.stack.paste_margin_bottom, &st.paste_bottom_expr);
        seed_margin(&mut pad.stack.mask_margin_top, &st.mask_top_expr);
        seed_margin(&mut pad.stack.mask_margin_bottom, &st.mask_bottom_expr);
        pad.stack.paste_enabled_top = st.paste_top_enabled;
        pad.stack.paste_enabled_bottom = st.paste_bottom_enabled;
        pad.stack.mask_tented_top = st.mask_top_tented;
        pad.stack.mask_tented_bottom = st.mask_bottom_tented;
        pad.stack.thermal_relief = st.thermal_relief;
        pad.stack.corner_radius_pct = st.corner_radius_pct;
    }
}

/// `None` expr -> `None`; a bare numeric literal, with or without a unit
/// suffix (exactly what [`is_numeric_literal`] accepts, so exactly what
/// [`mirror_len_expr`] may have written) -> its value in mm; anything else
/// (a parameter reference) leaves the editor value as it is. A unitless
/// number reads as mm.
fn seed_margin(slot: &mut Option<f64>, expr: &Option<String>) {
    let Some(e) = expr.as_deref() else {
        *slot = None;
        return;
    };
    if let Some(mm) = literal_mm(e) {
        *slot = Some(mm);
    }
}

fn literal_mm(expr: &str) -> Option<f64> {
    use signex_sketch::expr::eval::{EvalContext, eval};
    use signex_sketch::expr::parse::parse;
    if !is_numeric_literal(expr) {
        return None;
    }
    let body = expr.trim().trim_start_matches('=').trim();
    let mm = parse(body)
        .ok()
        .and_then(|ast| eval(&ast, &EvalContext::default()).ok())
        .and_then(|q| q.as_mm().ok())
        // Unitless: the evaluator calls it dimensionless; read it as mm.
        .or_else(|| body.parse::<f64>().ok());
    mm.filter(|v| v.is_finite())
}

/// Pad-stack overrides onto `PadAttr.stack`. `signex_bake::pad` rebuilds
/// `footprint.pads` from the sketch on every solve, so a stack field left
/// out of this mirror is reverted by the next sketch edit. Margins follow
/// the same rule as [`mirror_len_expr`]: a margin bound to a named
/// parameter is left alone.
fn mirror_stack(stack: &mut PadStackOverrides, pad: &EditorPad) {
    let ui = &pad.stack;
    mirror_len_expr(&mut stack.paste_top_expr, ui.paste_margin_top);
    mirror_len_expr(&mut stack.paste_bottom_expr, ui.paste_margin_bottom);
    // The `*_pct` overrides are documented as mutually exclusive with the
    // `*_expr` ones; an editor margin that set an expression supersedes them.
    if stack.paste_top_expr.is_some() {
        stack.paste_top_pct = None;
    }
    if stack.paste_bottom_expr.is_some() {
        stack.paste_bottom_pct = None;
    }
    mirror_len_expr(&mut stack.mask_top_expr, ui.mask_margin_top);
    mirror_len_expr(&mut stack.mask_bottom_expr, ui.mask_margin_bottom);
    stack.paste_top_enabled = ui.paste_enabled_top;
    stack.paste_bottom_enabled = ui.paste_enabled_bottom;
    stack.mask_top_tented = ui.mask_tented_top;
    stack.mask_bottom_tented = ui.mask_tented_bottom;
    stack.thermal_relief = ui.thermal_relief;
    stack.corner_radius_pct = ui.corner_radius_pct;
}

/// `signex_bake::pad` reads `PadAttr::shape`, so the sketch copy of
/// the shape IS what gets baked. Leaving it out of the mirror let a
/// flip swap the editor pad's chamfer corners while the sketch — and
/// therefore the bake — kept the pre-flip ones: two representations,
/// two answers, and the part does not seat.
///
/// A `SketchProfile` custom shape is never overwritten. That variant
/// is not derived from `pad.shape` at all — it names the traced loop
/// that IS the pad's copper, and `map_shape` would replace it with a
/// static point list, silently severing the profile link.
fn mirror_shape(current: &mut SkPadShape, shape: &LibPadShape) {
    if matches!(
        current,
        SkPadShape::Custom(CustomPadShape::SketchProfile { .. })
    ) {
        return;
    }
    *current = map_shape(shape);
}

/// Rotation was the one geometry field the mirror never wrote, so a
/// sketch-baked pad came back at 0° while the literal `Pad` carried
/// the true angle. It is written now, but only over an expression this
/// module could itself have written.
///
/// The discriminator is "is this a bare numeric literal", NOT "does it
/// start with `=`". The `=` prefix is OPTIONAL throughout this
/// codebase — `signex_sketch::solver::residual::resolve_dim` strips it
/// before parsing and `signex_bake::pad::rotation_deg` does the same —
/// so a bare `leg_angle` or `apex_angle * 2` is a fully valid authored
/// parameter binding. Keying on `=` would destroy exactly those, with
/// no warning and no undo entry for the sketch attribute.
pub fn mirror_rotation_expr(current: &mut Option<String>, deg: f64) {
    if current.as_deref().is_none_or(is_numeric_literal) {
        *current = Some(super::rotation_expr(deg));
    }
}

/// Optional millimetre length (slot length, mask / paste margin) onto
/// its sketch expression. Same rule as rotation: only an absent or
/// bare-literal expression is overwritten, so a margin bound to a named
/// parameter survives the Pads-mode edit.
fn mirror_len_expr(current: &mut Option<String>, mm: Option<f64>) {
    if current.as_deref().is_none_or(is_numeric_literal) {
        *current = mm.map(mm_expr);
    }
}

/// Drill diameter and slot length onto the sketch `DrillSpec`. Without
/// this a Pads-mode drill or slot edit is reverted by the next
/// solve-and-bake, which replaces `footprint.pads` from the sketch. A
/// diameter bound to a parameter is left alone, and so is its drill.
fn mirror_drill(current: &mut Option<DrillSpec>, pad: &EditorPad) {
    let Some(diameter) = pad.drill_diameter_mm else {
        if current
            .as_ref()
            .is_some_and(|d| is_numeric_literal(&d.diameter_expr))
        {
            *current = None;
        }
        return;
    };
    let spec = current.get_or_insert_with(|| DrillSpec {
        diameter_expr: mm_expr(diameter),
        slot_length_expr: None,
        plated: true,
    });
    if is_numeric_literal(&spec.diameter_expr) {
        spec.diameter_expr = mm_expr(diameter);
    }
    spec.plated = !matches!(pad.kind, LibPadKind::NptHole);
    mirror_len_expr(
        &mut spec.slot_length_expr,
        pad.passthrough.drill_slot_length_mm,
    );
}

/// True when `expr` is a plain number with an optional unit suffix
/// (`90`, `-45.5deg`, `1rad`, and the `=`-prefixed forms of each) —
/// i.e. carries no parameter reference or arithmetic, so overwriting
/// it loses nothing the user authored by name.
fn is_numeric_literal(expr: &str) -> bool {
    let body = expr.trim().trim_start_matches('=').trim();
    let number = body
        .trim_end_matches(|c: char| c.is_ascii_alphabetic())
        .trim();
    !number.is_empty() && number.parse::<f64>().is_ok()
}

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
    let drill = pad.drill_diameter_mm.map(|d| DrillSpec {
        diameter_expr: format!("{}mm", format_f64(d)),
        slot_length_expr: pad.passthrough.drill_slot_length_mm.map(mm_expr),
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
        // `signex_bake::pad` reads exactly these once the sketch owns
        // the pads, so a `None` here erased the legacy margins on save.
        mask_margin_expr: pad.passthrough.solder_mask_margin_mm.map(mm_expr),
        paste_margin_expr: pad.passthrough.paste_margin_mm.map(mm_expr),
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

/// A millimetre length as a sketch expression (`"1.8mm"`).
fn mm_expr(v: f64) -> String {
    format!("{}mm", format_f64(v))
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

#[cfg(test)]
mod seed_margin_tests {
    use super::*;

    #[test]
    fn seeding_accepts_what_the_mirror_accepts() {
        for (expr, mm) in [
            ("0.1mm", 0.1),
            ("=0.1mm", 0.1),
            ("0.1", 0.1),
            ("-0.04", -0.04),
            ("4mil", 0.1016),
            ("2um", 0.002),
            ("0.01in", 0.254),
        ] {
            assert!(is_numeric_literal(expr), "{expr:?} is a literal");
            let mut slot = Some(9.0);
            seed_margin(&mut slot, &Some(expr.into()));
            let got = slot.unwrap_or_else(|| panic!("{expr:?} not seeded"));
            assert!((got - mm).abs() < 1e-9, "{expr:?} -> {got}, want {mm}");
        }
        let mut slot = Some(9.0);
        seed_margin(&mut slot, &Some("pad_margin * 2".into()));
        assert_eq!(slot, Some(9.0), "parametric expr keeps the editor value");
        seed_margin(&mut slot, &None);
        assert_eq!(slot, None);
    }
}
