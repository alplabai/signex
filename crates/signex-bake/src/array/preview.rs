//! The instances of one array as the footprint Properties pattern drawing
//! shows them: the offset, number and dropped flag of each, from the same
//! walk the bake runs ([`super::walk`]).

use std::collections::BTreeMap;

use signex_sketch::SketchError;
use signex_sketch::array::{Array, ArrayKind};
use signex_sketch::expr::ast::ExprNode;
use signex_sketch::expr::eval::EvalContext;
use signex_sketch::parameter::resolve_quantities;
use signex_sketch::sketch::SketchData;

use super::walk::{Instance, Walk, Walked};

/// One instance of a previewed array.
#[derive(Debug, Clone, PartialEq)]
pub struct InstancePreview {
    /// Index along the array (`i`) and, for a grid, the row (`j`).
    pub i: usize,
    pub j: usize,
    /// Offset from the source in mm.
    pub offset_mm: [f64; 2],
    /// The pad number the bake gives it.
    pub number: String,
    /// Dropped by the depopulation mask or the suppressed list.
    pub is_skipped: bool,
}

/// What a preview can show of an array.
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayPreview {
    Instances(Vec<InstancePreview>),
    /// A count, spacing, sweep or mask does not evaluate, or the bake
    /// refuses the array.
    Unresolved,
    /// More instances than the preview's limit.
    TooMany(usize),
}

impl ArrayPreview {
    /// The instances the array keeps, when they are known.
    pub fn kept(&self) -> Option<usize> {
        match self {
            ArrayPreview::Instances(all) => Some(all.iter().filter(|c| !c.is_skipped).count()),
            ArrayPreview::Unresolved | ArrayPreview::TooMany(_) => None,
        }
    }
}

/// Preview `array` of `sketch`, visiting at most `limit` instances.
/// The parameters come from the sketch's parameter table; a polar array
/// needs the source and centre positions (`[x, y]` in mm), which the
/// caller reads from its last solve.
pub fn preview_array(
    array: &Array,
    sketch: &SketchData,
    source_mm: Option<[f64; 2]>,
    center_mm: Option<[f64; 2]>,
    limit: usize,
) -> ArrayPreview {
    let Ok(resolved) = resolve_quantities(&sketch.parameters) else {
        return ArrayPreview::Unresolved;
    };
    let params_ast: BTreeMap<String, ExprNode> = resolved
        .into_iter()
        .map(|(name, q)| (name, ExprNode::Literal(q)))
        .collect();
    let mut instances = Vec::new();
    let mut is_complete = true;
    let mut collect = |instance: Instance, _: &EvalContext, _: &mut Vec<String>| {
        match instance.offset_mm {
            Some(offset_mm) => instances.push(InstancePreview {
                i: instance.i,
                j: instance.j,
                offset_mm,
                number: instance.number,
                is_skipped: instance.is_skipped,
            }),
            None => is_complete = false,
        }
        Ok::<(), SketchError>(())
    };
    // The bake's warnings are not the preview's.
    let mut warnings = Vec::new();
    let walked = match &array.kind {
        ArrayKind::Linear {
            source,
            count_expr,
            dx_expr,
            dy_expr,
        } => walk(*source, array, &params_ast, limit).linear(
            count_expr,
            dx_expr,
            dy_expr,
            &mut warnings,
            &mut collect,
        ),
        ArrayKind::Grid {
            source,
            nx_expr,
            ny_expr,
            dx_expr,
            dy_expr,
            depopulation,
        } => walk(*source, array, &params_ast, limit).grid(
            nx_expr,
            ny_expr,
            dx_expr,
            dy_expr,
            depopulation.as_ref(),
            &mut warnings,
            &mut collect,
        ),
        ArrayKind::Polar {
            source,
            count_expr,
            sweep_angle_expr,
            depopulation,
            ..
        } => {
            let (Some([sx, sy]), Some([cx, cy])) = (source_mm, center_mm) else {
                return ArrayPreview::Unresolved;
            };
            walk(*source, array, &params_ast, limit).polar(
                (sx, sy),
                (cx, cy),
                count_expr,
                sweep_angle_expr,
                depopulation.as_ref(),
                &mut warnings,
                &mut collect,
            )
        }
    };
    match walked {
        Ok(Walked::Done) if is_complete => ArrayPreview::Instances(instances),
        Ok(Walked::TooMany(n)) => ArrayPreview::TooMany(n.unwrap_or(usize::MAX)),
        Ok(Walked::Done | Walked::Refused) | Err(_) => ArrayPreview::Unresolved,
    }
}

fn walk<'a>(
    source: signex_sketch::id::SketchEntityId,
    array: &'a Array,
    params_ast: &'a BTreeMap<String, ExprNode>,
    limit: usize,
) -> Walk<'a> {
    Walk {
        source,
        numbering: &array.numbering,
        params_ast,
        limit,
        visit_skipped: true,
    }
}
