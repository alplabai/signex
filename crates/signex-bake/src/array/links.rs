//! Pattern links (mockup row 191): every instance of a pattern as a placement of its
//! innermost source entity, so a pattern of a pattern repeats each instance
//! of its source, and a live mirror reflects each instance of its pattern.
//!
//! A plain array without links or overrides still bakes through
//! `bake_one_array`; this walk serves the arrays that have them, the
//! mirrors and the canvas's instance slots (`slots.rs`). Folding the plain
//! bake into one walk is a follow-up (#2689); see docs/FOOTPRINT_SKETCH.md,
//! "Pattern links".

use std::collections::BTreeMap;

use signex_expr::{ExprNode, parse};
use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::state::point_xy;

use crate::eval_util::strip_eq_prefix;

/// An instance's placement: where the source point goes (`m · p + t`) and
/// how a pad's rotation turns (`sign · deg + offset_deg`). Arrays move and
/// turn positions but keep pads unrotated, as the plain bake does; a mirror
/// reflects the rotation too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Place {
    m: [f64; 4],
    t: [f64; 2],
    sign: f64,
    offset_deg: f64,
}

impl Place {
    const IDENTITY: Self = Self {
        m: [1.0, 0.0, 0.0, 1.0],
        t: [0.0, 0.0],
        sign: 1.0,
        offset_deg: 0.0,
    };

    fn translation(dx: f64, dy: f64) -> Self {
        Self {
            t: [dx, dy],
            ..Self::IDENTITY
        }
    }

    fn rotation_about(centre: (f64, f64), theta: f64) -> Self {
        let (sin, cos) = theta.sin_cos();
        let m = [cos, -sin, sin, cos];
        Self {
            m,
            t: [
                centre.0 - (m[0] * centre.0 + m[1] * centre.1),
                centre.1 - (m[2] * centre.0 + m[3] * centre.1),
            ],
            ..Self::IDENTITY
        }
    }

    /// The reflection about the line through `a` and `b`; `None` when
    /// they coincide.
    fn reflection(a: (f64, f64), b: (f64, f64)) -> Option<Self> {
        let (vx, vy) = (b.0 - a.0, b.1 - a.1);
        let len2 = vx * vx + vy * vy;
        if len2 <= 1e-12 {
            return None;
        }
        let (ux, uy) = (vx / len2.sqrt(), vy / len2.sqrt());
        let m = [
            2.0 * ux * ux - 1.0,
            2.0 * ux * uy,
            2.0 * ux * uy,
            2.0 * uy * uy - 1.0,
        ];
        Some(Self {
            m,
            t: [
                a.0 - (m[0] * a.0 + m[1] * a.1),
                a.1 - (m[2] * a.0 + m[3] * a.1),
            ],
            sign: -1.0,
            offset_deg: 2.0 * vy.atan2(vx).to_degrees(),
        })
    }

    /// `self` after `inner`: place by `inner`, then by `self`.
    fn after(&self, inner: &Self) -> Self {
        let (a, b) = (self.m, inner.m);
        Self {
            m: [
                a[0] * b[0] + a[1] * b[2],
                a[0] * b[1] + a[1] * b[3],
                a[2] * b[0] + a[3] * b[2],
                a[2] * b[1] + a[3] * b[3],
            ],
            t: [
                a[0] * inner.t[0] + a[1] * inner.t[1] + self.t[0],
                a[2] * inner.t[0] + a[3] * inner.t[1] + self.t[1],
            ],
            sign: self.sign * inner.sign,
            offset_deg: self.sign * inner.offset_deg + self.offset_deg,
        }
    }

    pub(crate) fn apply(&self, p: (f64, f64)) -> (f64, f64) {
        (
            self.m[0] * p.0 + self.m[1] * p.1 + self.t[0],
            self.m[2] * p.0 + self.m[3] * p.1 + self.t[1],
        )
    }

    /// Whether this placement reflects what it places.
    pub(crate) fn mirrors(&self) -> bool {
        self.sign < 0.0
    }

    /// A pad rotation (degrees) as this placement leaves it.
    pub(crate) fn turn_deg(&self, deg: f64) -> f64 {
        if self.sign > 0.0 && self.offset_deg == 0.0 {
            return deg;
        }
        (self.sign * deg + self.offset_deg).rem_euclid(360.0)
    }
}

/// How a pattern numbers its instances: its scheme, and the row width when
/// it numbers row-major like a grid.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Basis<'a> {
    pub numbering: &'a NumberingScheme,
    pub nx: Option<usize>,
}

/// One instance of a pattern.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Instance {
    /// The key a per-instance override is stored under: `(array, i, j)` of
    /// a plain array, `(array, outer slot, inner instance)` of a linked one,
    /// `(mirror, mirrored instance, 0)` of a mirror (both indices numbered
    /// as [`Self::flat`]).
    pub key: (ArrayId, u32, u32),
    /// The instance's index in its pattern's numbering.
    pub flat: u64,
    pub place: Place,
    /// The `(i, j)` the pad expressions see: the innermost array's.
    pub index: (usize, usize),
    /// Whether this pattern bakes the instance itself. The outer slot 0 of
    /// a linked array is its source's instances, which the source bakes.
    pub own: bool,
}

/// Every instance of one pattern.
pub(crate) struct Expansion<'a> {
    /// The instance slots of the numbering, suppressed ones included.
    pub slots: u64,
    pub basis: Basis<'a>,
    pub instances: Vec<Instance>,
}

/// What an expansion reads, and where it reports.
pub(crate) struct Walk<'a, 'w> {
    pub sketch: &'a SketchData,
    pub solve: &'a FullSolveOutput,
    pub params_ast: &'a BTreeMap<String, ExprNode>,
    pub warnings: &'w mut Vec<String>,
    /// The patterns being expanded, outermost first: a pattern met again is
    /// a loop (only a damaged file has one).
    stack: Vec<ArrayId>,
    /// Keep the slots the suppression list holds back, marked `listed`
    /// ([`super::slots`]); the bake drops them.
    keep_listed: bool,
}

impl<'a, 'w> Walk<'a, 'w> {
    pub(crate) fn new(
        sketch: &'a SketchData,
        solve: &'a FullSolveOutput,
        params_ast: &'a BTreeMap<String, ExprNode>,
        warnings: &'w mut Vec<String>,
    ) -> Self {
        Self {
            sketch,
            solve,
            params_ast,
            warnings,
            stack: Vec::new(),
            keep_listed: false,
        }
    }

    /// This walk, keeping the slots the suppression list holds back.
    pub(crate) fn keeping_listed(mut self) -> Self {
        self.keep_listed = true;
        self
    }

    fn xy(&self, id: SketchEntityId) -> Option<(f64, f64)> {
        point_xy(
            id,
            &self.solve.result.state,
            &self.solve.result.index,
            self.sketch,
        )
    }

    /// Every instance of pattern `id` (an array or a mirror). `None`, with
    /// a warning, when it cannot expand: a loop, a missing line or
    /// pattern, a count that does not evaluate or is over the cap.
    pub(crate) fn expand(&mut self, id: ArrayId) -> Result<Option<Expansion<'a>>, SketchError> {
        if self.stack.contains(&id) {
            self.warnings.push(format!(
                "pattern {}: its source chain loops back to itself — pattern skipped",
                id.0
            ));
            return Ok(None);
        }
        self.stack.push(id);
        let expansion = if let Some(mirror) = self.sketch.pattern_mirror(id).copied() {
            self.expand_mirror(mirror.id, mirror.pattern, mirror.line)
        } else if let Some(array) = self.sketch.arrays.iter().find(|a| a.id == id) {
            self.expand_array(array)
        } else {
            self.warnings.push(format!(
                "pattern {}: not in the sketch — link skipped",
                id.0
            ));
            Ok(None)
        };
        self.stack.pop();
        expansion
    }

    fn expand_mirror(
        &mut self,
        id: ArrayId,
        pattern: ArrayId,
        line: SketchEntityId,
    ) -> Result<Option<Expansion<'a>>, SketchError> {
        let ends = self
            .sketch
            .entities
            .iter()
            .find(|e| e.id == line)
            .and_then(|e| match e.kind {
                signex_sketch::entity::EntityKind::Line { start, end } => {
                    Some((self.xy(start)?, self.xy(end)?))
                }
                _ => None,
            });
        let Some(reflect) = ends.and_then(|(a, b)| Place::reflection(a, b)) else {
            self.warnings.push(format!(
                "pattern mirror {}: its line is missing or has no length — mirror skipped",
                id.0
            ));
            return Ok(None);
        };
        let Some(inner) = self.expand(pattern)? else {
            return Ok(None);
        };
        let instances = inner
            .instances
            .iter()
            .filter_map(|x| {
                Some(Instance {
                    key: (id, u32::try_from(x.flat).ok()?, 0),
                    flat: inner.slots + x.flat,
                    place: reflect.after(&x.place),
                    index: x.index,
                    own: true,
                })
            })
            .collect();
        Ok(Some(Expansion {
            slots: inner.slots.saturating_mul(2),
            basis: inner.basis,
            instances,
        }))
    }

    fn expand_array(&mut self, array: &'a Array) -> Result<Option<Expansion<'a>>, SketchError> {
        let source = array.kind.source();
        let Some(mut walked) = self.walk_slots(array)? else {
            return Ok(None);
        };
        walked.slots.retain(|s| !s.listed);
        let linked = self
            .sketch
            .pattern_source_of(array.id)
            .filter(|s| self.sketch.is_pattern(*s));
        let Some(inner_id) = linked else {
            let instances = walked
                .slots
                .iter()
                .map(|s| Instance {
                    key: (array.id, s.i as u32, s.j as u32),
                    flat: s.flat,
                    place: s.place,
                    index: (s.i, s.j),
                    own: true,
                })
                .collect();
            return Ok(Some(Expansion {
                slots: walked.count,
                basis: Basis {
                    numbering: &array.numbering,
                    nx: walked.nx,
                },
                instances,
            }));
        };
        let Some(inner) = self.expand(inner_id)? else {
            return Ok(None);
        };
        let total = walked.slots.len().checked_mul(inner.instances.len());
        if !super::within_instance_cap("linked", source, total, self.warnings) {
            return Ok(None);
        }
        let mut instances = Vec::with_capacity(total.unwrap_or(0));
        for s in &walked.slots {
            for x in &inner.instances {
                let (Ok(i), Ok(j)) = (u32::try_from(s.flat), u32::try_from(x.flat)) else {
                    continue;
                };
                instances.push(Instance {
                    key: (array.id, i, j),
                    flat: s.flat * inner.slots + x.flat,
                    place: s.place.after(&x.place),
                    index: x.index,
                    own: s.flat != 0,
                });
            }
        }
        // A grid of a pattern numbers row by row, each row now `nx` times
        // as wide as the inner pattern has slots.
        let nx = walked
            .nx
            .and_then(|nx| usize::try_from(inner.slots).ok()?.checked_mul(nx));
        Ok(Some(Expansion {
            slots: walked.count.saturating_mul(inner.slots),
            basis: Basis {
                numbering: &array.numbering,
                nx,
            },
            instances,
        }))
    }

    /// The kept slots of one array's own layout, as its plain bake walks
    /// them (same counts, steps, suppression and mask).
    pub(crate) fn walk_slots(&mut self, array: &Array) -> Result<Option<Walked>, SketchError> {
        let source = array.kind.source();
        let mut ctx = EvalContext {
            params: self.params_ast.clone(),
            array_index: Some((0, 0)),
        };
        match &array.kind {
            ArrayKind::Linear {
                count_expr,
                dx_expr,
                dy_expr,
                ..
            } => {
                let count_ast = parse_expr(count_expr)?;
                let (dx_ast, dy_ast) = (parse_expr(dx_expr)?, parse_expr(dy_expr)?);
                let Some(count) = self.count("linear", source, &count_ast, &ctx)? else {
                    return Ok(None);
                };
                let mut slots = Vec::with_capacity(count);
                for i in 0..count {
                    ctx.array_index = Some((i, 0));
                    let dx = crate::params::eval_length_mm(&dx_ast, &ctx)?;
                    let dy = crate::params::eval_length_mm(&dy_ast, &ctx)?;
                    slots.push(Slot::new(
                        i,
                        0,
                        i as u64,
                        Place::translation(i as f64 * dx, i as f64 * dy),
                        false,
                    ));
                }
                Ok(Some(Walked::new(slots, count as u64, None)))
            }
            ArrayKind::Grid {
                nx_expr,
                ny_expr,
                dx_expr,
                dy_expr,
                depopulation,
                ..
            } => {
                let (nx_ast, ny_ast) = (parse_expr(nx_expr)?, parse_expr(ny_expr)?);
                let (dx_ast, dy_ast) = (parse_expr(dx_expr)?, parse_expr(dy_expr)?);
                let mask = mask_ast(depopulation.as_ref())?;
                let Some(nx) = self.count("grid", source, &nx_ast, &ctx)? else {
                    return Ok(None);
                };
                let Some(ny) = self.count("grid", source, &ny_ast, &ctx)? else {
                    return Ok(None);
                };
                if !super::within_instance_cap("grid", source, nx.checked_mul(ny), self.warnings) {
                    return Ok(None);
                }
                let mut slots = Vec::new();
                for j in 0..ny {
                    for i in 0..nx {
                        ctx.array_index = Some((i, j));
                        let Some(listed) =
                            self.visits(depopulation.as_ref(), mask.as_ref(), &ctx, source)
                        else {
                            continue;
                        };
                        let dx = crate::params::eval_length_mm(&dx_ast, &ctx)?;
                        let dy = crate::params::eval_length_mm(&dy_ast, &ctx)?;
                        let place = Place::translation(i as f64 * dx, j as f64 * dy);
                        slots.push(Slot::new(i, j, (j * nx + i) as u64, place, listed));
                    }
                }
                Ok(Some(Walked::new(slots, (nx * ny) as u64, Some(nx))))
            }
            ArrayKind::Polar {
                center,
                count_expr,
                sweep_angle_expr,
                depopulation,
                ..
            } => {
                let Some(centre) = self.xy(*center) else {
                    self.warnings.push(format!(
                        "polar array center {center}: position unknown — array skipped"
                    ));
                    return Ok(None);
                };
                let count_ast = parse_expr(count_expr)?;
                let sweep =
                    eval(&parse_expr(sweep_angle_expr)?, &ctx).map_err(SketchError::Expr)?;
                let mask = mask_ast(depopulation.as_ref())?;
                let Some(count) = self.count("polar", source, &count_ast, &ctx)? else {
                    return Ok(None);
                };
                let Ok(sweep_rad) = sweep.as_rad() else {
                    self.warnings.push(format!(
                        "polar array source {source}: sweep_angle_expr did not resolve to an angle — array skipped"
                    ));
                    return Ok(None);
                };
                let mut slots = Vec::new();
                for i in 0..count {
                    ctx.array_index = Some((i, 0));
                    let Some(listed) =
                        self.visits(depopulation.as_ref(), mask.as_ref(), &ctx, source)
                    else {
                        continue;
                    };
                    let theta = i as f64 * sweep_rad / count as f64;
                    slots.push(Slot::new(
                        i,
                        0,
                        i as u64,
                        Place::rotation_about(centre, theta),
                        listed,
                    ));
                }
                Ok(Some(Walked::new(slots, count as u64, None)))
            }
        }
    }

    /// A positive count within the instance cap, else `None` with a warning.
    fn count(
        &mut self,
        label: &str,
        source: SketchEntityId,
        ast: &ExprNode,
        ctx: &EvalContext,
    ) -> Result<Option<usize>, SketchError> {
        let Some(n) = super::eval_count(label, "count", source, ast, ctx, self.warnings)? else {
            return Ok(None);
        };
        if n <= 0 {
            self.warnings.push(format!(
                "{label} array source {source}: count resolved to {n} — array skipped"
            ));
            return Ok(None);
        }
        let n = n as usize;
        Ok(super::within_instance_cap(label, source, Some(n), self.warnings).then_some(n))
    }

    /// Whether the walk visits the slot at `ctx`'s index: `Some(false)`
    /// when it survives the suppression list and the mask (a mask that
    /// fails to evaluate keeps it, warned), `Some(true)` when the list
    /// holds it back and the walk keeps listed slots, else `None`.
    fn visits(
        &mut self,
        depopulation: Option<&GridDepopulation>,
        mask: Option<&ExprNode>,
        ctx: &EvalContext,
        source: SketchEntityId,
    ) -> Option<bool> {
        let Some((i, j)) = ctx.array_index else {
            return Some(false);
        };
        let listed = depopulation.is_some_and(|d| {
            d.suppressed_instances
                .iter()
                .any(|&(si, sj)| (si as usize, sj as usize) == (i, j))
        });
        if listed {
            return self.keep_listed.then_some(true);
        }
        match mask.map(|ast| eval(ast, ctx)) {
            Some(Ok(q)) => (q.value.abs() >= 1e-9).then_some(false),
            Some(Err(e)) => {
                self.warnings.push(format!(
                    "array source {source}: depopulation eval failed at ({i}, {j}): {e:?} — keeping instance"
                ));
                Some(false)
            }
            None => Some(false),
        }
    }
}

fn parse_expr(src: &str) -> Result<ExprNode, SketchError> {
    parse(strip_eq_prefix(src)).map_err(SketchError::Expr)
}

fn mask_ast(depopulation: Option<&GridDepopulation>) -> Result<Option<ExprNode>, SketchError> {
    depopulation
        .filter(|d| !d.mask_expr.trim().is_empty())
        .map(|d| parse(strip_eq_prefix(&d.mask_expr)))
        .transpose()
        .map_err(SketchError::Expr)
}

pub(crate) struct Slot {
    pub i: usize,
    pub j: usize,
    flat: u64,
    pub place: Place,
    /// Held back by the suppression list (only kept by a walk
    /// [`Walk::keeping_listed`]).
    pub listed: bool,
}

impl Slot {
    fn new(i: usize, j: usize, flat: u64, place: Place, listed: bool) -> Self {
        Self {
            i,
            j,
            flat,
            place,
            listed,
        }
    }
}

pub(crate) struct Walked {
    pub slots: Vec<Slot>,
    /// Every slot of the layout, suppressed ones included.
    pub count: u64,
    nx: Option<usize>,
}

impl Walked {
    fn new(slots: Vec<Slot>, count: u64, nx: Option<usize>) -> Self {
        Self { slots, count, nx }
    }
}

/// Whether `array` bakes through this walk rather than `bake_one_array`:
/// it repeats another pattern, or one of its instances has an override.
pub(crate) fn bakes_through_links(sketch: &SketchData, array: ArrayId) -> bool {
    sketch.pattern_source_of(array).is_some()
        || sketch.pad_overrides.iter().any(|o| o.array == array)
}

/// Bake the instances pattern `id` places itself into `out`.
pub(crate) fn bake_pattern(
    id: ArrayId,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_ast: &BTreeMap<String, ExprNode>,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let Some(source) = sketch.innermost_source(id) else {
        warnings.push(format!(
            "pattern {}: its source chain loops or ends at a missing pattern — pattern skipped",
            id.0
        ));
        return Ok(());
    };
    let expansion = Walk::new(sketch, solve, params_ast, warnings).expand(id)?;
    let Some(expansion) = expansion else {
        return Ok(());
    };
    super::overrides::bake_instances(source, &expansion, sketch, solve, params_ast, out, warnings)
}

/// Bake every live mirror's instances into `out`.
pub(crate) fn bake_mirrors(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_ast: &BTreeMap<String, ExprNode>,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    for mirror in &sketch.pattern_mirrors {
        bake_pattern(mirror.id, sketch, solve, params_ast, out, warnings)?;
    }
    Ok(())
}
