//! Where a sketch's parameters are read, and naming, renaming, deleting or
//! minting one without leaving a dangling reference (mockup row 189).
//!
//! One walker, [`visit_expressions`] / [`visit_expressions_mut`], reaches
//! every expression string a [`SketchData`] holds: the parameter table, a
//! body plane's height, dimension targets, pattern counts, spacings and
//! numbering, and every bake attribute expression. Its patterns name every
//! field of every type they open, so a new field anywhere in the sketch model
//! does not compile until it is placed here, as an expression or not.
//!
//! References are found token by token ([`signex_expr::rename`]), so a source
//! that does not parse yet still counts as a use, and `pitch` never matches
//! inside `pitch2`.

use std::collections::BTreeMap;

use signex_expr::{
    LiteralContext, expression_body, inline_reference, is_identifier, is_reserved_constant,
    reads_reference, referenced_names, rename_reference,
};

use crate::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
use crate::attr::{
    BoardCutoutAttr, CourtyardAttr, DrillSpec, KeepoutAttr, MaskExcludeAttr, MaskOpeningAttr,
    MountingHoleAttr, PadAttr, PadShape, PadStackOverrides, PasteApertureAttr,
    PasteAperturePattern, PourAttr, SilkAttr, ThermalRelief, VScoreHintAttr,
};
use crate::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::entity::{Entity, EntityKind};
use crate::expr::ExprError;
use crate::expr::ast::{ArrayIndex, ExprNode};
use crate::expr::eval::{EvalContext, eval};
use crate::expr::parse::parse_with;
use crate::id::{ConstraintId, SketchEntityId};
use crate::parameter::ParameterTable;
use crate::plane::{Plane, PlaneId, PlaneKind};
use crate::sketch::SketchData;
use crate::unit::{Quantity, UnitFamily};

/// A place in a sketch that holds an expression, and so may read a
/// parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterUse {
    /// Another parameter's expression.
    Parameter { name: String },
    /// A body plane's height.
    PlaneOffset { plane: PlaneId },
    /// A dimension's value; `angle` for an angle dimension.
    Dimension {
        constraint: ConstraintId,
        angle: bool,
    },
    /// A pattern's count, spacing, sweep, mask or numbering.
    Pattern {
        array: ArrayId,
        kind: PatternKind,
        field: PatternField,
    },
    /// A pad's size, placement, drill, margins or shape.
    Pad {
        entity: SketchEntityId,
        number: String,
        field: PadField,
    },
    /// A pour, cutout, mounting hole or V-score value.
    Attribute {
        entity: SketchEntityId,
        field: AttributeField,
    },
}

/// The kind of a pattern ([`ArrayKind`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternKind {
    Linear,
    Grid,
    Circular,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternField {
    Count,
    Dx,
    Dy,
    Nx,
    Ny,
    SweepAngle,
    DepopulationMask,
    NumberingStart,
    NumberingStep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadField {
    SizeX,
    SizeY,
    Rotation,
    OffsetX,
    OffsetY,
    DrillDiameter,
    SlotLength,
    MaskMargin,
    PasteMargin,
    PasteGridNx,
    PasteGridNy,
    PasteGridCoverage,
    PasteTop,
    PasteBottom,
    MaskTop,
    MaskBottom,
    CornerRadiusRatio,
    ChamferRatio,
    TrapezoidDeltaX,
    TrapezoidDeltaY,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeField {
    PourClearance,
    PourMinThickness,
    ThermalGap,
    ThermalSpokeWidth,
    CutoutEdgeRadius,
    MountingHoleDrill,
    VScoreDepth,
    VScoreMinWeb,
}

/// Visit an optional expression.
macro_rules! visit_some {
    ($visit:expr, $site:expr, $slot:expr) => {
        if let Some(source) = $slot {
            $visit($site, source);
        }
    };
}

/// Visit a pattern's numbering expressions.
macro_rules! numbering_walk {
    ($visit:expr, $site:expr, $numbering:expr) => {
        match $numbering {
            NumberingScheme::LinearIncrement {
                start_expr,
                step_expr,
                sweep: _,
            } => {
                $visit($site(PatternField::NumberingStart), start_expr);
                $visit($site(PatternField::NumberingStep), step_expr);
            }
            NumberingScheme::BgaRowCol {
                skip_letters: _,
                start_row: _,
                start_col: _,
            }
            | NumberingScheme::Explicit { names: _ } => {}
        }
    };
}

/// The walker, written once and expanded over `&SketchData` and
/// `&mut SketchData`: default binding modes make every binding a shared or
/// a mutable reference to match, so the two bodies cannot drift.
macro_rules! expression_walker {
    ($(#[$doc:meta])* $name:ident $(, $m:tt)?) => {
        $(#[$doc])*
        pub fn $name(
            sketch: &$($m)? SketchData,
            visit: &mut dyn FnMut(ParameterUse, &$($m)? String),
        ) {
            let SketchData {
                planes,
                entities,
                constraints,
                arrays,
                parameters,
                baked_kinds: _,
                // Links name ids only. Overrides hold what a Pads-mode edit
                // wrote (literals); a parameter rename does not reach them.
                driven_dimensions: _,
                pattern_sources: _,
                pattern_mirrors: _,
                pad_overrides: _,
            } = sketch;
            let ParameterTable(table) = parameters;
            for (name, source) in table {
                visit(ParameterUse::Parameter { name: name.clone() }, source);
            }
            for Plane { id, kind } in planes {
                match kind {
                    PlaneKind::BoardTop => {}
                    PlaneKind::BodyTop { offset_z_expr } => {
                        visit(ParameterUse::PlaneOffset { plane: *id }, offset_z_expr)
                    }
                }
            }
            for Constraint { id, kind } in constraints {
                let (target, angle) = match kind {
                    ConstraintKind::DistancePtPt { p1: _, p2: _, target }
                    | ConstraintKind::DistancePtLine { point: _, line: _, target }
                    | ConstraintKind::DistancePtCircle { point: _, circle: _, target } => {
                        (target, false)
                    }
                    ConstraintKind::Angle { l1: _, l2: _, target } => (target, true),
                    ConstraintKind::Coincident { p1: _, p2: _ }
                    | ConstraintKind::PointOnLine { point: _, line: _ }
                    | ConstraintKind::PointOnArc { point: _, arc: _ }
                    | ConstraintKind::Horizontal { line: _ }
                    | ConstraintKind::Vertical { line: _ }
                    | ConstraintKind::Parallel { l1: _, l2: _ }
                    | ConstraintKind::Perpendicular { l1: _, l2: _ }
                    | ConstraintKind::EqualLength { l1: _, l2: _ }
                    | ConstraintKind::EqualRadius { e1: _, e2: _ }
                    | ConstraintKind::TangentLineArc { line: _, arc: _ }
                    | ConstraintKind::TangentArcArc { a1: _, a2: _, internal: _ }
                    | ConstraintKind::SymmetricAboutLine { p1: _, p2: _, line: _ }
                    | ConstraintKind::SymmetricAboutPoint { p1: _, p2: _, center: _ }
                    | ConstraintKind::Midpoint { point: _, line: _ }
                    | ConstraintKind::Fixed { point: _ } => continue,
                };
                match target {
                    DimTarget::Literal(_) => {}
                    DimTarget::Expr(source) => visit(
                        ParameterUse::Dimension { constraint: *id, angle },
                        source,
                    ),
                }
            }
            for Array { id, kind, numbering } in arrays {
                let array = *id;
                match kind {
                    ArrayKind::Linear { source: _, count_expr, dx_expr, dy_expr, depopulation } => {
                        let site = |field| ParameterUse::Pattern {
                            array,
                            kind: PatternKind::Linear,
                            field,
                        };
                        visit(site(PatternField::Count), count_expr);
                        visit(site(PatternField::Dx), dx_expr);
                        visit(site(PatternField::Dy), dy_expr);
                        if let Some(GridDepopulation { mask_expr, suppressed_instances: _ }) = depopulation {
                            visit(site(PatternField::DepopulationMask), mask_expr);
                        }
                        numbering_walk!(visit, site, numbering);
                    }
                    ArrayKind::Grid { source: _, nx_expr, ny_expr, dx_expr, dy_expr, depopulation } => {
                        let site = |field| ParameterUse::Pattern {
                            array,
                            kind: PatternKind::Grid,
                            field,
                        };
                        visit(site(PatternField::Nx), nx_expr);
                        visit(site(PatternField::Ny), ny_expr);
                        visit(site(PatternField::Dx), dx_expr);
                        visit(site(PatternField::Dy), dy_expr);
                        if let Some(GridDepopulation { mask_expr, suppressed_instances: _ }) = depopulation {
                            visit(site(PatternField::DepopulationMask), mask_expr);
                        }
                        numbering_walk!(visit, site, numbering);
                    }
                    ArrayKind::Polar {
                        source: _,
                        center: _,
                        count_expr,
                        sweep_angle_expr,
                        depopulation,
                    } => {
                        let site = |field| ParameterUse::Pattern {
                            array,
                            kind: PatternKind::Circular,
                            field,
                        };
                        visit(site(PatternField::Count), count_expr);
                        visit(site(PatternField::SweepAngle), sweep_angle_expr);
                        if let Some(GridDepopulation { mask_expr, suppressed_instances: _ }) = depopulation {
                            visit(site(PatternField::DepopulationMask), mask_expr);
                        }
                        numbering_walk!(visit, site, numbering);
                    }
                }
            }
            for Entity {
                id,
                plane: _,
                construction: _,
                centerline: _,
                kind,
                pad,
                silk,
                courtyard,
                mask_opening,
                mask_exclude,
                paste_aperture,
                pour,
                keepout,
                board_cutout,
                v_score,
                board_outline: _,
                mounting_hole,
                // The board targets carry no expressions.
                board_slot: _,
                mechanical: _,
                component_anchor: _,
            } in entities
            {
                let entity = *id;
                // Geometry is numbers, never expressions.
                match kind {
                    EntityKind::Point { x: _, y: _ }
                    | EntityKind::Line { start: _, end: _ }
                    | EntityKind::Arc { center: _, start: _, end: _, sweep_ccw: _ }
                    | EntityKind::Circle { center: _, radius: _ } => {}
                }
                // Roles that carry a layer and no expression.
                if let Some(attr) = silk {
                    let SilkAttr { layer: _ } = attr;
                }
                if let Some(attr) = courtyard {
                    let CourtyardAttr {} = attr;
                }
                if let Some(attr) = mask_opening {
                    let MaskOpeningAttr { layer: _ } = attr;
                }
                if let Some(attr) = mask_exclude {
                    let MaskExcludeAttr { layer: _ } = attr;
                }
                if let Some(attr) = paste_aperture {
                    let PasteApertureAttr { layer: _ } = attr;
                }
                if let Some(attr) = keepout {
                    let KeepoutAttr { layer: _, kinds: _ } = attr;
                }
                if let Some(PadAttr {
                    number,
                    kind: _,
                    side: _,
                    shape,
                    size_x_expr,
                    size_y_expr,
                    rotation_expr,
                    offset_x_expr,
                    offset_y_expr,
                    drill,
                    mask_margin_expr,
                    paste_margin_expr,
                    paste_apertures,
                    template: _,
                    library: _,
                    stack,
                    feature_top: _,
                    feature_bottom: _,
                    testpoint: _,
                    electrical_type: _,
                    net: _,
                    locked: _,
                    hole_tolerance_plus_mm: _,
                    hole_tolerance_minus_mm: _,
                    hole_rotation_deg: _,
                    copper_offset_x_mm: _,
                    copper_offset_y_mm: _,
                    owned: _,
                }) = pad
                {
                    let number = number.clone();
                    let site = |field| ParameterUse::Pad {
                        entity,
                        number: number.clone(),
                        field,
                    };
                    visit(site(PadField::SizeX), size_x_expr);
                    visit(site(PadField::SizeY), size_y_expr);
                    visit_some!(visit, site(PadField::Rotation), rotation_expr);
                    visit_some!(visit, site(PadField::OffsetX), offset_x_expr);
                    visit_some!(visit, site(PadField::OffsetY), offset_y_expr);
                    if let Some(DrillSpec { diameter_expr, slot_length_expr, plated: _ }) = drill {
                        visit(site(PadField::DrillDiameter), diameter_expr);
                        visit_some!(visit, site(PadField::SlotLength), slot_length_expr);
                    }
                    visit_some!(visit, site(PadField::MaskMargin), mask_margin_expr);
                    visit_some!(visit, site(PadField::PasteMargin), paste_margin_expr);
                    match paste_apertures {
                        PasteAperturePattern::Single | PasteAperturePattern::Custom { source: _ } => {}
                        PasteAperturePattern::Grid { nx_expr, ny_expr, coverage_expr } => {
                            visit(site(PadField::PasteGridNx), nx_expr);
                            visit(site(PadField::PasteGridNy), ny_expr);
                            visit(site(PadField::PasteGridCoverage), coverage_expr);
                        }
                    }
                    let PadStackOverrides {
                        paste_top_expr,
                        paste_top_pct: _,
                        paste_bottom_expr,
                        paste_bottom_pct: _,
                        paste_top_enabled: _,
                        paste_bottom_enabled: _,
                        mask_top_expr,
                        mask_bottom_expr,
                        mask_top_tented: _,
                        mask_bottom_tented: _,
                        thermal_relief: _,
                        corner_radius_pct: _,
                    } = stack;
                    visit_some!(visit, site(PadField::PasteTop), paste_top_expr);
                    visit_some!(visit, site(PadField::PasteBottom), paste_bottom_expr);
                    visit_some!(visit, site(PadField::MaskTop), mask_top_expr);
                    visit_some!(visit, site(PadField::MaskBottom), mask_bottom_expr);
                    match shape {
                        PadShape::Round | PadShape::Rect | PadShape::Oval | PadShape::Custom(_) => {}
                        PadShape::RoundRect { radius_ratio_expr } => {
                            visit(site(PadField::CornerRadiusRatio), radius_ratio_expr)
                        }
                        PadShape::Chamfered { chamfer_ratio_expr, corners: _ } => {
                            visit(site(PadField::ChamferRatio), chamfer_ratio_expr)
                        }
                        PadShape::Trapezoid { delta_x_expr, delta_y_expr } => {
                            visit(site(PadField::TrapezoidDeltaX), delta_x_expr);
                            visit(site(PadField::TrapezoidDeltaY), delta_y_expr);
                        }
                    }
                }
                let site = |field| ParameterUse::Attribute { entity, field };
                if let Some(PourAttr {
                    layer: _,
                    net: _,
                    fill_type: _,
                    thermal_relief,
                    clearance_expr,
                    min_thickness_expr,
                    priority: _,
                }) = pour
                {
                    let ThermalRelief {
                        enabled: _,
                        gap_expr,
                        spoke_width_expr,
                        spoke_count: _,
                    } = thermal_relief;
                    visit(site(AttributeField::ThermalGap), gap_expr);
                    visit(site(AttributeField::ThermalSpokeWidth), spoke_width_expr);
                    visit_some!(visit, site(AttributeField::PourClearance), clearance_expr);
                    visit_some!(visit, site(AttributeField::PourMinThickness), min_thickness_expr);
                }
                if let Some(BoardCutoutAttr { edge_radius_expr: Some(source), through: _ }) = board_cutout {
                    visit(site(AttributeField::CutoutEdgeRadius), source);
                }
                if let Some(MountingHoleAttr { drill_expr: Some(source), plated: _, net: _ }) = mounting_hole {
                    visit(site(AttributeField::MountingHoleDrill), source);
                }
                if let Some(VScoreHintAttr { depth_fraction_expr, min_web_expr, side: _ }) = v_score {
                    visit(site(AttributeField::VScoreDepth), depth_fraction_expr);
                    visit_some!(visit, site(AttributeField::VScoreMinWeb), min_web_expr);
                }
            }
        }
    };
}

expression_walker!(
    /// Call `visit` with every expression `sketch` holds and the place
    /// that holds it.
    visit_expressions
);
expression_walker!(
    /// [`visit_expressions`] with each expression open for writing.
    visit_expressions_mut,
    mut
);

/// Every place that reads the parameter `name`, the parameter's own
/// expression excluded, in walk order.
pub fn used_by(sketch: &SketchData, name: &str) -> Vec<ParameterUse> {
    let mut uses = Vec::new();
    visit_expressions(sketch, &mut |site, source| {
        if !is_own_row(&site, name) && reads_reference(source, name) {
            uses.push(site);
        }
    });
    uses
}

/// [`used_by`] for every parameter at once, in one walk. A parameter no
/// expression reads maps to an empty list.
pub fn uses_by_parameter(sketch: &SketchData) -> BTreeMap<String, Vec<ParameterUse>> {
    let mut uses: BTreeMap<String, Vec<ParameterUse>> = sketch
        .parameters
        .iter()
        .map(|(name, _)| (name.clone(), Vec::new()))
        .collect();
    visit_expressions(sketch, &mut |site, source| {
        for name in referenced_names(source) {
            if is_own_row(&site, name) {
                continue;
            }
            if let Some(list) = uses.get_mut(name) {
                list.push(site.clone());
            }
        }
    });
    uses
}

fn is_own_row(site: &ParameterUse, name: &str) -> bool {
    matches!(site, ParameterUse::Parameter { name: own } if own == name)
}

/// Why a name cannot be a parameter's, or a rename or delete found no
/// parameter to act on.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParameterNameError {
    #[error("a parameter needs a name")]
    Empty,
    #[error("'{0}' is not a name: use letters, digits and '_', not starting with a digit")]
    NotAName(String),
    #[error("'{0}' is reserved and cannot name a parameter")]
    Reserved(String),
    #[error("a parameter named '{0}' already exists")]
    Taken(String),
    #[error("there is no parameter named '{0}'")]
    Missing(String),
}

/// Whether `name` may name a new parameter of `table`: a plain identifier,
/// not a constant the parser reads (`pi`), not an array index (`i`, `j`),
/// not already taken.
pub fn check_new_name(table: &ParameterTable, name: &str) -> Result<(), ParameterNameError> {
    if name.is_empty() {
        return Err(ParameterNameError::Empty);
    }
    if !is_identifier(name) {
        return Err(ParameterNameError::NotAName(name.to_string()));
    }
    if is_reserved_constant(name) || ArrayIndex::from_name(name).is_some() {
        return Err(ParameterNameError::Reserved(name.to_string()));
    }
    if table.get_raw(name).is_some() {
        return Err(ParameterNameError::Taken(name.to_string()));
    }
    Ok(())
}

/// The first free name `<stem>_1`, `<stem>_2`, ... in `table`.
pub fn unique_name(table: &ParameterTable, stem: &str) -> String {
    (1..)
        .map(|n| format!("{stem}_{n}"))
        .find(|candidate| table.get_raw(candidate).is_none())
        .expect("an unbounded range always finds a free name")
}

/// Rename the parameter `old` to `new` and every reference to it, in one
/// change. Renaming to the same name changes nothing.
pub fn rename_parameter(
    sketch: &mut SketchData,
    old: &str,
    new: &str,
) -> Result<(), ParameterNameError> {
    if sketch.parameters.get_raw(old).is_none() {
        return Err(ParameterNameError::Missing(old.to_string()));
    }
    if old == new {
        return Ok(());
    }
    check_new_name(&sketch.parameters, new)?;
    visit_expressions_mut(sketch, &mut |_, source| {
        if reads_reference(source, old) {
            *source = rename_reference(source, old, new);
        }
    });
    if let Some(source) = sketch.parameters.0.remove(old) {
        sketch.parameters.insert(new, source);
    }
    Ok(())
}

/// What [`delete_parameter`] does when other expressions read the
/// parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeletePolicy {
    /// Put the parameter's value in place of each reference, then delete.
    Inline,
    /// Refuse, naming the uses.
    Refuse,
}

/// Why a delete did not happen.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum DeleteParameterError {
    #[error("there is no parameter named '{0}'")]
    Missing(String),
    #[error("the parameter is used in {} place(s)", .0.len())]
    InUse(Vec<ParameterUse>),
}

/// Delete the parameter `name`. With [`DeletePolicy::Inline`] every
/// reference is first replaced by the parameter's value as a literal
/// (`0.65mm`), or by its expression in parentheses when it has no value;
/// returns how many places changed.
pub fn delete_parameter(
    sketch: &mut SketchData,
    name: &str,
    policy: DeletePolicy,
) -> Result<usize, DeleteParameterError> {
    if sketch.parameters.get_raw(name).is_none() {
        return Err(DeleteParameterError::Missing(name.to_string()));
    }
    let uses = used_by(sketch, name);
    if !uses.is_empty() && policy == DeletePolicy::Refuse {
        return Err(DeleteParameterError::InUse(uses));
    }
    if !uses.is_empty() {
        let value = value_source(&sketch.parameters, name);
        visit_expressions_mut(sketch, &mut |site, source| {
            if !is_own_row(&site, name) && reads_reference(source, name) {
                *source = inline_reference(source, name, &value);
            }
        });
    }
    sketch.parameters.0.remove(name);
    Ok(uses.len())
}

/// The text that stands in for `name` when it is inlined: its value as a
/// literal, or its own expression when it has no value.
fn value_source(table: &ParameterTable, name: &str) -> String {
    match resolve_rows(table).remove(name) {
        Some(RowValue::Value(q)) => literal_of(q),
        _ => expression_body(table.get_raw(name).unwrap_or_default()).to_string(),
    }
}

/// A canonical sketch value as source text the parser reads back to the
/// same value: `0.65mm`, `90deg`, `4`. Rounded to 1e-9 of its unit, below
/// the nanometre the file stores.
pub fn literal_of(q: Quantity) -> String {
    match q.unit.family() {
        UnitFamily::Length => format!("{}mm", tidy(q.value)),
        UnitFamily::Angle => format!("{}deg", tidy(q.value.to_degrees())),
        UnitFamily::Count | UnitFamily::Derived => tidy(q.value).to_string(),
    }
}

/// `v` rounded to 1e-9, with no negative zero.
fn tidy(v: f64) -> f64 {
    let rounded = (v * 1e9).round() / 1e9;
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// The state of one parameter row, read on its own so one bad row does not
/// hide the others' values.
#[derive(Clone, Debug, PartialEq)]
pub enum RowValue {
    /// The value in its canonical unit (mm, rad or a count).
    Value(Quantity),
    /// This row's own expression is wrong.
    Error(ExprError),
    /// The expression is fine but reads `by`, which has no value.
    Blocked { by: String },
}

/// Every row of `table`, each evaluated on its own.
pub fn resolve_rows(table: &ParameterTable) -> BTreeMap<String, RowValue> {
    let mut parsed: BTreeMap<String, Result<ExprNode, ExprError>> = BTreeMap::new();
    for (name, source) in table.iter() {
        let ast = if is_reserved_constant(name) {
            Err(ExprError::ReservedName(name.clone()))
        } else {
            parse_with(expression_body(source), LiteralContext::Geometry)
        };
        parsed.insert(name.clone(), ast);
    }
    let ctx = EvalContext {
        params: parsed
            .iter()
            .filter_map(|(name, ast)| Some((name.clone(), ast.as_ref().ok()?.clone())))
            .collect(),
        array_index: None,
    };
    let raw: BTreeMap<String, Result<Quantity, ExprError>> = parsed
        .iter()
        .map(|(name, ast)| {
            let value = match ast {
                Ok(ast) => eval(ast, &ctx).and_then(canonical),
                Err(e) => Err(e.clone()),
            };
            (name.clone(), value)
        })
        .collect();
    raw.iter()
        .map(|(name, value)| {
            let row = match value {
                Ok(q) => RowValue::Value(*q),
                Err(e @ ExprError::Cycle(_)) => RowValue::Error(e.clone()),
                Err(e) => match failing_dependency(table, name, &raw) {
                    Some(by) => RowValue::Blocked { by },
                    None => RowValue::Error(e.clone()),
                },
            };
            (name.clone(), row)
        })
        .collect()
}

/// The first parameter `name`'s own expression reads that has no value.
fn failing_dependency(
    table: &ParameterTable,
    name: &str,
    raw: &BTreeMap<String, Result<Quantity, ExprError>>,
) -> Option<String> {
    let source = table.get_raw(name)?;
    referenced_names(source)
        .into_iter()
        .find(|dep| *dep != name && raw.get(*dep).is_some_and(Result::is_err))
        .map(str::to_string)
}

/// `q` in its family's canonical unit, as the resolver stores it.
fn canonical(q: Quantity) -> Result<Quantity, ExprError> {
    Ok(match q.unit.family() {
        UnitFamily::Length => Quantity::length(q.as_mm()?),
        UnitFamily::Angle => Quantity::angle(q.as_rad()?),
        UnitFamily::Count => Quantity::count(q.as_count()?),
        UnitFamily::Derived => {
            return Err(ExprError::WrongFamily {
                expected: UnitFamily::Length,
                got: UnitFamily::Derived,
            });
        }
    })
}

/// Why a dimension did not become a parameter.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MakeParameterError {
    #[error("the constraint is gone")]
    NoConstraint,
    #[error("only a distance or angle dimension has a value to name")]
    NotADimension,
}

/// Name the value of the dimension `constraint`: a new parameter holds it
/// (`dim_1 = 0.65mm`, `angle_1 = 90deg`, or the expression it already had)
/// and the dimension reads the parameter. A dimension that already reads
/// exactly one parameter keeps it. Returns the parameter's name.
pub fn parameter_from_dimension(
    sketch: &mut SketchData,
    constraint: ConstraintId,
) -> Result<String, MakeParameterError> {
    let Some(index) = sketch.constraints.iter().position(|c| c.id == constraint) else {
        return Err(MakeParameterError::NoConstraint);
    };
    let angle = matches!(sketch.constraints[index].kind, ConstraintKind::Angle { .. });
    let Some(target) = sketch.constraints[index].kind.dim_target() else {
        return Err(MakeParameterError::NotADimension);
    };
    let source = match target {
        DimTarget::Literal(v) if angle => literal_of(Quantity::angle(*v)),
        DimTarget::Literal(v) => literal_of(Quantity::length(*v)),
        DimTarget::Expr(source) => {
            let body = expression_body(source);
            if is_identifier(body) && sketch.parameters.get_raw(body).is_some() {
                return Ok(body.to_string());
            }
            body.to_string()
        }
    };
    let name = unique_name(&sketch.parameters, if angle { "angle" } else { "dim" });
    sketch.parameters.insert(name.clone(), source);
    if let Some(target) = dim_target_mut(&mut sketch.constraints[index].kind) {
        *target = DimTarget::Expr(name.clone());
    }
    Ok(name)
}

fn dim_target_mut(kind: &mut ConstraintKind) -> Option<&mut DimTarget> {
    match kind {
        ConstraintKind::DistancePtPt { target, .. }
        | ConstraintKind::DistancePtLine { target, .. }
        | ConstraintKind::DistancePtCircle { target, .. }
        | ConstraintKind::Angle { target, .. } => Some(target),
        _ => None,
    }
}
