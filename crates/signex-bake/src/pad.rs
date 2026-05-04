//! Pad bake — turns SketchData + solved state into Vec<Pad>.
//!
//! Phase 7 Task 7.1 of the v0.13 sketch-mode plan. Walks every entity
//! tagged with [`PadAttr`], evaluates its expression strings against
//! the resolved parameter table, and emits one
//! [`signex_library::primitive::footprint::Pad`] per entity.
//!
//! Layer-name strings are produced by
//! [`signex_types::layer::SignexLayer::altium_label`] so the baked
//! footprint matches the Altium-style label set ("Top Layer", "Top
//! Solder", "Top Paste", …) used by the rest of the Signex PCB
//! taxonomy.
//!
//! Cleanroom: no third-party constraint-solver, footprint-generator,
//! or numerical-library source consulted.
//!
//! # v0.13 scope
//!
//! - `PadKind::{Smd, Tht, NptHole, ConnectorPad}` bake natively.
//! - `PadKind::Fiducial` bakes as `LibPadKind::Smd` with default
//!   1 mm mask margin, no paste, no drill, Round shape.
//! - `PadKind::Castellated` bakes as `LibPadKind::Tht` with a warning.
//! - `PadShape::Chamfered` falls back to `LibPadShape::RoundRect`
//!   with a warning.
//! - `PadShape::Custom(SketchProfile)` falls back to
//!   `LibPadShape::Rect` with a warning.
//! - `PasteAperturePattern::{Grid, Custom}` warn + fall back to
//!   `Single` (one aperture).
//! - Closed-profile attrs other than `pad` (silk / courtyard /
//!   mask_opening / mask_exclude / paste_aperture / pour / keepout /
//!   board_cutout / v_score) emit a "deferred to v0.14+" warning.
//! - `construction = true` entities are skipped silently.

use std::collections::{BTreeMap, HashMap};

use signex_library::primitive::footprint::{
    Drill as LibDrill, LayerId, Pad as LibPad, PadKind as LibPadKind, PadShape as LibPadShape,
    Polygon as LibPolygon,
};
use signex_sketch::attr::{
    CustomPadShape, PadAttr, PadKind, PadShape, PadSide, PasteAperturePattern,
};
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::{eval, EvalContext};
use signex_sketch::expr::parse::parse;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::state::point_xy;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::unit::{Quantity, UnitFamily};
use signex_sketch::SketchError;
use signex_types::layer::SignexLayer;

/// Bake every entity tagged with [`PadAttr`] into a [`LibPad`]. Adds
/// human-readable warnings to `warnings` for v0.14+ features.
pub fn bake_pads(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_canonical: &HashMap<String, f64>,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    // Build the params AST once; per-entity calls share it. Each entity
    // re-wraps it into an EvalContext to slot in the optional
    // array_index for that bake (None for direct, non-array bakes).
    let mut params_ast: BTreeMap<String, ExprNode> = BTreeMap::new();
    for (name, value) in params_canonical {
        params_ast.insert(name.clone(), ExprNode::Literal(Quantity::length(*value)));
    }

    for entity in &sketch.entities {
        if entity.construction {
            continue;
        }
        let pad_attr = match entity.pad.as_ref() {
            Some(p) => p,
            None => continue,
        };

        let pad = bake_one_pad(
            entity.id, pad_attr, &params_ast, None, 0.0, 0.0, None, sketch, solve, warnings,
        )?;
        out.push(pad);
    }

    // Emit one warning per closed-profile attr the entity carries —
    // these all bake in v0.14+. Construction entities are silently
    // skipped (matching the first loop above): they exist as solver
    // scaffolding only and never produce baked geometry, so warning
    // about their bake-attrs would be misleading noise.
    for entity in &sketch.entities {
        if entity.construction {
            continue;
        }
        if entity.silk.is_some() {
            warnings.push(format!(
                "entity {}: SilkAttr ignored (v0.14 feature)",
                entity.id
            ));
        }
        if entity.courtyard.is_some() {
            warnings.push(format!(
                "entity {}: CourtyardAttr ignored (v0.14 feature)",
                entity.id
            ));
        }
        if entity.mask_opening.is_some() {
            warnings.push(format!(
                "entity {}: MaskOpeningAttr ignored (v0.14 feature)",
                entity.id
            ));
        }
        if entity.mask_exclude.is_some() {
            warnings.push(format!(
                "entity {}: MaskExcludeAttr ignored (v0.14 feature)",
                entity.id
            ));
        }
        if entity.paste_aperture.is_some() {
            warnings.push(format!(
                "entity {}: PasteApertureAttr ignored (v0.14 feature)",
                entity.id
            ));
        }
        if entity.pour.is_some() {
            warnings.push(format!(
                "entity {}: PourAttr ignored — Footprint::pours field lands in v0.14, fill generation in v0.15",
                entity.id
            ));
        }
        if entity.keepout.is_some() {
            warnings.push(format!(
                "entity {}: KeepoutAttr ignored — Footprint::keepouts field lands in v0.14, DRC enforcement in v0.15",
                entity.id
            ));
        }
        if entity.board_cutout.is_some() {
            warnings.push(format!(
                "entity {}: BoardCutoutAttr ignored — Footprint::cutouts field lands in v0.14, PCB outline subtraction in v0.14",
                entity.id
            ));
        }
        if entity.v_score.is_some() {
            warnings.push(format!(
                "entity {}: VScoreHintAttr ignored — Footprint::v_scores field lands in v0.14; panelisation tool consumes them in v0.16+",
                entity.id
            ));
        }
    }

    Ok(())
}

/// Per-pad bake — exposed `pub(crate)` so `array::bake_arrays` can
/// reuse the body without code duplication.
///
/// `extra_dx` / `extra_dy` are the array offset (mm) added on top of
/// the pad's own `offset_x_expr` / `offset_y_expr`.
/// `extra_pad_number`, when `Some`, overrides `pad_attr.number`
/// (LinearArray numbering scheme).
#[allow(clippy::too_many_arguments)]
pub(crate) fn bake_one_pad(
    sketch_point_id: SketchEntityId,
    pad_attr: &PadAttr,
    params_ast: &BTreeMap<String, ExprNode>,
    array_index: Option<(usize, usize)>,
    extra_dx: f64,
    extra_dy: f64,
    extra_pad_number: Option<String>,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    warnings: &mut Vec<String>,
) -> Result<LibPad, SketchError> {
    let ctx = EvalContext {
        params: params_ast.clone(),
        array_index,
    };

    // Look up the sketch point.
    let (px, py) = point_xy(sketch_point_id, &solve.result.state, &solve.result.index, sketch)
        .ok_or(SketchError::EntityNotFound(sketch_point_id))?;

    // Position = sketch point + array offset + per-pad authored offset.
    let ox = opt_eval_mm(&pad_attr.offset_x_expr, &ctx)?.unwrap_or(0.0);
    let oy = opt_eval_mm(&pad_attr.offset_y_expr, &ctx)?.unwrap_or(0.0);
    let position = [px + extra_dx + ox, py + extra_dy + oy];

    let size_x = eval_mm(&pad_attr.size_x_expr, &ctx)?;
    let size_y = eval_mm(&pad_attr.size_y_expr, &ctx)?;

    let rotation = rotation_deg(&pad_attr.rotation_expr, &ctx)?;

    let pad_number = extra_pad_number.unwrap_or_else(|| pad_attr.number.clone());
    let is_fiducial = pad_attr.kind == PadKind::Fiducial;

    let mask_margin = match opt_eval_mm(&pad_attr.mask_margin_expr, &ctx)? {
        Some(v) => Some(v),
        None if is_fiducial => Some(1.0),
        None => None,
    };

    // Fiducials never get paste; ignore any authored paste expression.
    let paste_margin = if is_fiducial {
        if pad_attr.paste_margin_expr.is_some() {
            warnings.push(format!(
                "pad {}: paste_margin_expr ignored on Fiducial (no paste applied)",
                pad_number
            ));
        }
        None
    } else {
        opt_eval_mm(&pad_attr.paste_margin_expr, &ctx)?
    };

    let drill = match &pad_attr.drill {
        Some(d) => Some(LibDrill {
            diameter: eval_mm(&d.diameter_expr, &ctx)?,
            slot_length: opt_eval_mm(&d.slot_length_expr, &ctx)?,
        }),
        None => None,
    };
    if is_fiducial && drill.is_some() {
        warnings.push(format!(
            "pad {}: drill ignored on Fiducial (vision marker has no hole)",
            pad_number
        ));
    }

    let shape = if is_fiducial {
        if !matches!(pad_attr.shape, PadShape::Round) {
            warnings.push(format!(
                "pad {}: Fiducial shape forced to Round",
                pad_number
            ));
        }
        LibPadShape::Round
    } else {
        bake_shape(&pad_attr.shape, &ctx, warnings, &pad_number)?
    };

    // Warn on paste-aperture patterns we don't bake yet.
    match &pad_attr.paste_apertures {
        PasteAperturePattern::Single => {}
        PasteAperturePattern::Grid { .. } => warnings.push(format!(
            "pad {}: PasteAperturePattern::Grid ignored (v0.14 feature) — falling back to Single aperture",
            pad_number
        )),
        PasteAperturePattern::Custom { .. } => warnings.push(format!(
            "pad {}: PasteAperturePattern::Custom ignored (v0.14 feature) — falling back to Single aperture",
            pad_number
        )),
    }

    let layers = if is_fiducial {
        fiducial_layers(pad_attr.side)
    } else {
        derive_layers(pad_attr.kind, pad_attr.side)
    };

    Ok(LibPad {
        number: pad_number,
        kind: lib_kind(pad_attr.kind, warnings, &pad_attr.number),
        shape,
        size: [size_x, size_y],
        position,
        rotation,
        layers,
        drill: if is_fiducial { None } else { drill },
        solder_mask_margin: mask_margin,
        paste_margin,
    })
}

// ─────────────────────────────────────────────────────────────────────
// Helpers — expression eval
// ─────────────────────────────────────────────────────────────────────

/// Strip the optional Altium-style leading `=` and surrounding
/// whitespace so authored expressions like `= pad_w` parse cleanly.
/// Matches the convention used by [`signex_sketch::parameter`] and
/// [`signex_sketch::solver::residual::resolve_dim`].
fn strip_eq_prefix(src: &str) -> &str {
    let s = src.trim();
    s.strip_prefix('=').map(|s| s.trim_start()).unwrap_or(s)
}

fn eval_mm(expr: &str, ctx: &EvalContext) -> Result<f64, SketchError> {
    let body = strip_eq_prefix(expr);
    let ast = parse(body).map_err(SketchError::Expr)?;
    let q = eval(&ast, ctx).map_err(SketchError::Expr)?;
    q.as_mm().map_err(SketchError::Unit)
}

fn opt_eval_mm(expr: &Option<String>, ctx: &EvalContext) -> Result<Option<f64>, SketchError> {
    match expr.as_deref() {
        Some(e) => Ok(Some(eval_mm(e, ctx)?)),
        None => Ok(None),
    }
}

fn rotation_deg(
    expr: &Option<String>,
    ctx: &EvalContext,
) -> Result<f64, SketchError> {
    match expr.as_deref() {
        Some(e) => {
            let body = strip_eq_prefix(e);
            let ast = parse(body).map_err(SketchError::Expr)?;
            let q = eval(&ast, ctx).map_err(SketchError::Expr)?;
            match q.unit.family() {
                UnitFamily::Angle => {
                    // Convert canonical-rad to degrees.
                    Ok(q.as_rad().map_err(SketchError::Unit)?.to_degrees())
                }
                _ => Ok(q.value), // dimensionless treated as degrees
            }
        }
        None => Ok(0.0),
    }
}

// ─────────────────────────────────────────────────────────────────────
// Helpers — kind / shape / layer mapping
// ─────────────────────────────────────────────────────────────────────

fn lib_kind(k: PadKind, warnings: &mut Vec<String>, pad_number: &str) -> LibPadKind {
    match k {
        PadKind::Smd | PadKind::Fiducial => LibPadKind::Smd,
        PadKind::Tht => LibPadKind::Tht,
        PadKind::NptHole => LibPadKind::NptHole,
        PadKind::ConnectorPad => LibPadKind::ConnectorPad,
        PadKind::Castellated => {
            warnings.push(format!(
                "pad {}: Castellated baked as Tht in v0.13 — lib gains a Castellated variant in v0.14 (needed for fab gerber outline export to identify halved holes)",
                pad_number
            ));
            LibPadKind::Tht
        }
    }
}

/// Build a `LayerId` from a `SignexLayer`, using its Altium-style
/// display label as the string-typed wrapper's content.
fn signex_layer_id(l: SignexLayer) -> LayerId {
    LayerId::new(l.altium_label())
}

/// Layer set for a Fiducial pad — copper + mask only, no paste.
fn fiducial_layers(side: PadSide) -> Vec<LayerId> {
    match side {
        PadSide::Top => vec![
            signex_layer_id(SignexLayer::TopCopper),
            signex_layer_id(SignexLayer::TopSolderMask),
        ],
        PadSide::Bottom => vec![
            signex_layer_id(SignexLayer::BottomCopper),
            signex_layer_id(SignexLayer::BottomSolderMask),
        ],
        PadSide::All => vec![
            signex_layer_id(SignexLayer::TopCopper),
            signex_layer_id(SignexLayer::BottomCopper),
            signex_layer_id(SignexLayer::TopSolderMask),
            signex_layer_id(SignexLayer::BottomSolderMask),
        ],
    }
}

/// Layer set for a normal pad based on mounting style + copper side.
///
/// Names produced by [`SignexLayer::altium_label`].
fn derive_layers(kind: PadKind, side: PadSide) -> Vec<LayerId> {
    match (kind, side) {
        (PadKind::Smd | PadKind::ConnectorPad, PadSide::Top) => vec![
            signex_layer_id(SignexLayer::TopCopper),
            signex_layer_id(SignexLayer::TopSolderMask),
            signex_layer_id(SignexLayer::TopPaste),
        ],
        (PadKind::Smd | PadKind::ConnectorPad, PadSide::Bottom) => vec![
            signex_layer_id(SignexLayer::BottomCopper),
            signex_layer_id(SignexLayer::BottomSolderMask),
            signex_layer_id(SignexLayer::BottomPaste),
        ],
        (PadKind::Smd | PadKind::ConnectorPad, PadSide::All) => vec![
            signex_layer_id(SignexLayer::TopCopper),
            signex_layer_id(SignexLayer::BottomCopper),
            signex_layer_id(SignexLayer::TopSolderMask),
            signex_layer_id(SignexLayer::BottomSolderMask),
            signex_layer_id(SignexLayer::TopPaste),
            signex_layer_id(SignexLayer::BottomPaste),
        ],
        (PadKind::Tht | PadKind::Castellated, _) => vec![
            signex_layer_id(SignexLayer::TopCopper),
            signex_layer_id(SignexLayer::BottomCopper),
            signex_layer_id(SignexLayer::TopSolderMask),
            signex_layer_id(SignexLayer::BottomSolderMask),
        ],
        (PadKind::NptHole, _) => vec![
            signex_layer_id(SignexLayer::TopSolderMask),
            signex_layer_id(SignexLayer::BottomSolderMask),
        ],
        // Fiducial is handled by `fiducial_layers` before this fn is
        // called; defending against future callers that might forget
        // that branch.
        (PadKind::Fiducial, side) => fiducial_layers(side),
    }
}

/// Map sketch `PadShape` to the library's baked `LibPadShape`.
fn bake_shape(
    s: &PadShape,
    ctx: &EvalContext,
    warnings: &mut Vec<String>,
    pad_number: &str,
) -> Result<LibPadShape, SketchError> {
    Ok(match s {
        PadShape::Round => LibPadShape::Round,
        PadShape::Rect => LibPadShape::Rect,
        PadShape::Oval => LibPadShape::Oval,
        PadShape::RoundRect { radius_ratio_expr } => {
            let ast = parse(strip_eq_prefix(radius_ratio_expr)).map_err(SketchError::Expr)?;
            let q = eval(&ast, ctx).map_err(SketchError::Expr)?;
            let ratio = q.value.clamp(0.0, 0.5);
            LibPadShape::RoundRect {
                radius_ratio: ratio,
            }
        }
        PadShape::Chamfered {
            chamfer_ratio_expr,
            corners: _,
        } => {
            let ast = parse(strip_eq_prefix(chamfer_ratio_expr)).map_err(SketchError::Expr)?;
            let q = eval(&ast, ctx).map_err(SketchError::Expr)?;
            warnings.push(format!(
                "pad {}: Chamfered shape approximated as RoundRect in v0.13 (v0.14 adds chamfer)",
                pad_number
            ));
            LibPadShape::RoundRect {
                radius_ratio: q.value.clamp(0.0, 0.5),
            }
        }
        PadShape::Custom(CustomPadShape::StaticPoints { points }) => {
            LibPadShape::Custom(LibPolygon {
                points: points.clone(),
            })
        }
        PadShape::Custom(CustomPadShape::SketchProfile { source: _ }) => {
            warnings.push(format!(
                "pad {}: Custom::SketchProfile falls back to bbox Rect in v0.13 (v0.14 bakes the profile)",
                pad_number
            ));
            LibPadShape::Rect
        }
    })
}
