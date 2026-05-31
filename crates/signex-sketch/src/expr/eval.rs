//! Expression evaluator for the parametric sketch parameter table.
//!
//! Walks the [`ExprNode`] tree and produces a [`Quantity`] with
//! unit-family type-checking on every binary op. Cleanroom; no
//! third-party expression-evaluator source consulted.
//!
//! # Semantics summary
//!
//! - `Add` / `Sub` — operands must share a unit family. Length+Length
//!   converts RHS to LHS unit before adding; the result keeps LHS's
//!   unit. Same for Angle+Angle and Dimensionless+Dimensionless.
//! - `Mul` — `Length × Dimensionless → Length`,
//!   `Angle × Dimensionless → Angle`,
//!   `Dimensionless × Dimensionless → Dimensionless`. `Length × Length`
//!   would produce an Area, which we don't model in v0.13, so it errors.
//! - `Div` / `Mod` — `Length / Length → Dimensionless`,
//!   `Length / Dimensionless → Length`, `Angle / Dimensionless → Angle`,
//!   `Dimensionless / Dimensionless → Dimensionless`.
//! - `Pow` — `Dimensionless ^ Dimensionless → Dimensionless` is the
//!   only supported case; `Length ^ n` for `n != 1` would produce an
//!   Area or higher, which we don't model.
//! - Comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`) — operands must
//!   share a family; the result is a `Dimensionless` `0.0` or `1.0`.
//!   `==` and `!=` use a `1e-12` tolerance against the canonical value
//!   to avoid exact-equality footguns; the strict orderings use exact
//!   `f64` comparison.
//! - Logical (`&&`, `||`) — operands must be `Dimensionless`; any
//!   non-zero value is true.
//! - `Unary(Neg)` — negate the value, keep the unit.
//! - `Unary(Not)` — operand must be `Dimensionless`; result is the
//!   logical inverse as `Dimensionless` `0.0` or `1.0`.
//! - `Ternary` — condition must be `Dimensionless`.
//! - `Lookup` — match key against each entry of `keys` (same family +
//!   value within tolerance) and return `eval(values[i])`. Errors if
//!   no match or shapes differ.

use std::collections::BTreeMap;

use crate::expr::ExprError;
use crate::expr::ast::{ArrayIndex, BinOp, ExprNode, UnaryOp};
use crate::unit::{Quantity, Unit, UnitFamily};

/// Tolerance for `Eq`/`Ne` comparisons and `Lookup` key matching, in
/// canonical units (mm for Length, rad for Angle, raw for Count).
const EQ_TOL: f64 = 1e-12;

/// Per-evaluation context — the parameter table (raw AST values, so
/// references resolve recursively) plus the optional `(i, j)` array
/// index for inside an array expansion.
#[derive(Clone, Debug, Default)]
pub struct EvalContext {
    /// Parameter table as `name → ExprNode` so [`ExprNode::Ref`] can
    /// resolve recursively. Cycle detection is the responsibility of
    /// [`crate::parameter::resolve`] (Task 4.5).
    pub params: BTreeMap<String, ExprNode>,
    /// `Some((i, j))` inside an array expansion; `None` otherwise.
    /// Used by [`ExprNode::ArrayIndex`].
    pub array_index: Option<(usize, usize)>,
}

/// Walk `node` and produce a [`Quantity`].
///
/// Recursive interpreter; each binary op is type-checked by family
/// before reduction. Errors are bubbled up as [`ExprError`].
pub fn eval(node: &ExprNode, ctx: &EvalContext) -> Result<Quantity, ExprError> {
    match node {
        ExprNode::Literal(q) => Ok(*q),
        ExprNode::Ref(name) => {
            let inner = ctx
                .params
                .get(name)
                .ok_or_else(|| ExprError::Unknown(name.clone()))?;
            eval(inner, ctx)
        }
        ExprNode::ArrayIndex(which) => match ctx.array_index {
            Some((i, j)) => match which {
                ArrayIndex::I => Ok(Quantity::count(i as f64)),
                ArrayIndex::J => Ok(Quantity::count(j as f64)),
            },
            None => Err(ExprError::ArrayIndexOutsideArray),
        },
        ExprNode::Binary(op, lhs, rhs) => {
            let lv = eval(lhs, ctx)?;
            let rv = eval(rhs, ctx)?;
            eval_binop(*op, lv, rv)
        }
        ExprNode::Unary(op, inner) => {
            let v = eval(inner, ctx)?;
            eval_unaryop(*op, v)
        }
        ExprNode::Ternary(cond, then_branch, else_branch) => {
            let cv = eval(cond, ctx)?;
            let raw = cv.as_count()?;
            if raw != 0.0 {
                eval(then_branch, ctx)
            } else {
                eval(else_branch, ctx)
            }
        }
        ExprNode::Lookup { key, keys, values } => eval_lookup(key, keys, values, ctx),
    }
}

/// Apply a binary operator to two already-evaluated quantities.
fn eval_binop(op: BinOp, lhs: Quantity, rhs: Quantity) -> Result<Quantity, ExprError> {
    match op {
        BinOp::Add => eval_add_sub(lhs, rhs, |a, b| a + b),
        BinOp::Sub => eval_add_sub(lhs, rhs, |a, b| a - b),
        BinOp::Mul => eval_mul(lhs, rhs),
        BinOp::Div => eval_div_mod(lhs, rhs, |a, b| a / b),
        BinOp::Mod => eval_div_mod(lhs, rhs, f64::rem_euclid),
        BinOp::Pow => eval_pow(lhs, rhs),
        BinOp::Eq => eval_compare(lhs, rhs, CompareOp::Eq),
        BinOp::Ne => eval_compare(lhs, rhs, CompareOp::Ne),
        BinOp::Lt => eval_compare(lhs, rhs, CompareOp::Lt),
        BinOp::Le => eval_compare(lhs, rhs, CompareOp::Le),
        BinOp::Gt => eval_compare(lhs, rhs, CompareOp::Gt),
        BinOp::Ge => eval_compare(lhs, rhs, CompareOp::Ge),
        BinOp::And => eval_logical(lhs, rhs, |a, b| a && b),
        BinOp::Or => eval_logical(lhs, rhs, |a, b| a || b),
    }
}

/// Apply `Neg` or `Not` to a single quantity.
fn eval_unaryop(op: UnaryOp, v: Quantity) -> Result<Quantity, ExprError> {
    match op {
        UnaryOp::Neg => Ok(Quantity {
            value: -v.value,
            unit: v.unit,
        }),
        UnaryOp::Not => {
            let raw = v.as_count()?;
            Ok(Quantity::count(if raw == 0.0 { 1.0 } else { 0.0 }))
        }
    }
}

/// Add / subtract — operands must share a family. RHS is converted to
/// LHS's unit before applying `f`; the result keeps LHS's unit.
fn eval_add_sub(
    lhs: Quantity,
    rhs: Quantity,
    f: fn(f64, f64) -> f64,
) -> Result<Quantity, ExprError> {
    let lf = lhs.unit.family();
    let rf = rhs.unit.family();
    if lf != rf {
        return Err(ExprError::UnitMismatch {
            lhs: lhs.unit,
            rhs: rhs.unit,
        });
    }
    // Both sides share a family; convert RHS into LHS's unit so the
    // result keeps LHS's unit.
    let rhs_in_lhs_unit = convert_to(rhs, lhs.unit)?;
    Ok(Quantity {
        value: f(lhs.value, rhs_in_lhs_unit),
        unit: lhs.unit,
    })
}

/// Multiplication — only the family combinations modelled in v0.13.
fn eval_mul(lhs: Quantity, rhs: Quantity) -> Result<Quantity, ExprError> {
    let lf = lhs.unit.family();
    let rf = rhs.unit.family();
    match (lf, rf) {
        (UnitFamily::Length, UnitFamily::Count) => Ok(Quantity {
            value: lhs.value * rhs.value,
            unit: lhs.unit,
        }),
        (UnitFamily::Count, UnitFamily::Length) => Ok(Quantity {
            value: lhs.value * rhs.value,
            unit: rhs.unit,
        }),
        (UnitFamily::Angle, UnitFamily::Count) => Ok(Quantity {
            value: lhs.value * rhs.value,
            unit: lhs.unit,
        }),
        (UnitFamily::Count, UnitFamily::Angle) => Ok(Quantity {
            value: lhs.value * rhs.value,
            unit: rhs.unit,
        }),
        (UnitFamily::Count, UnitFamily::Count) => Ok(Quantity::count(lhs.value * rhs.value)),
        // Length × Length would produce an Area; we don't model that.
        // Length × Angle, Angle × Angle, Angle × Length are also not
        // physical for sketch parameters.
        _ => Err(ExprError::UnitMismatch {
            lhs: lhs.unit,
            rhs: rhs.unit,
        }),
    }
}

/// Division and modulus — same family combinations on both sides.
/// Length / Length collapses to Dimensionless; the others mirror
/// [`eval_mul`]. A zero divisor on any branch surfaces
/// [`ExprError::Domain`] rather than producing inf / NaN — `1mm / 0`
/// and `0 % 0` would otherwise flow into the LM solver as a poisoned
/// state and silently break convergence.
fn eval_div_mod(
    lhs: Quantity,
    rhs: Quantity,
    f: fn(f64, f64) -> f64,
) -> Result<Quantity, ExprError> {
    let lf = lhs.unit.family();
    let rf = rhs.unit.family();
    match (lf, rf) {
        (UnitFamily::Length, UnitFamily::Length) => {
            // Convert both to mm so the ratio is well-defined.
            let l_mm = lhs.as_mm()?;
            let r_mm = rhs.as_mm()?;
            check_nonzero(r_mm)?;
            Ok(Quantity::count(f(l_mm, r_mm)))
        }
        (UnitFamily::Length, UnitFamily::Count) => {
            check_nonzero(rhs.value)?;
            Ok(Quantity {
                value: f(lhs.value, rhs.value),
                unit: lhs.unit,
            })
        }
        (UnitFamily::Angle, UnitFamily::Angle) => {
            let l_rad = lhs.as_rad()?;
            let r_rad = rhs.as_rad()?;
            check_nonzero(r_rad)?;
            Ok(Quantity::count(f(l_rad, r_rad)))
        }
        (UnitFamily::Angle, UnitFamily::Count) => {
            check_nonzero(rhs.value)?;
            Ok(Quantity {
                value: f(lhs.value, rhs.value),
                unit: lhs.unit,
            })
        }
        (UnitFamily::Count, UnitFamily::Count) => {
            check_nonzero(rhs.value)?;
            Ok(Quantity::count(f(lhs.value, rhs.value)))
        }
        _ => Err(ExprError::UnitMismatch {
            lhs: lhs.unit,
            rhs: rhs.unit,
        }),
    }
}

/// Reject a zero divisor before we feed it to `/` or `%`. Pure-zero
/// equality is intentional; sub-ULP non-zero values still divide
/// cleanly under f64 (returning a large finite ratio rather than inf).
fn check_nonzero(divisor: f64) -> Result<(), ExprError> {
    if divisor == 0.0 {
        Err(ExprError::Domain("division by zero".into()))
    } else {
        Ok(())
    }
}

/// Power — only `Dimensionless ^ Dimensionless` is supported in v0.13.
/// `Length ^ 2` would need an Area unit; non-integer powers of Length
/// are similarly meaningless without a richer unit model.
fn eval_pow(base: Quantity, exp: Quantity) -> Result<Quantity, ExprError> {
    // Exponent must be dimensionless.
    if exp.unit.family() != UnitFamily::Count {
        return Err(ExprError::UnitMismatch {
            lhs: base.unit,
            rhs: exp.unit,
        });
    }
    let e = exp.value;
    match base.unit.family() {
        UnitFamily::Count => Ok(Quantity::count(base.value.powf(e))),
        // Length^1 reduces to Length; everything else needs Area.
        UnitFamily::Length | UnitFamily::Angle => {
            if (e - 1.0).abs() < EQ_TOL {
                Ok(base)
            } else {
                Err(ExprError::Domain(format!(
                    "{:?}^{} requires Area / higher-order unit (not modelled in v0.13)",
                    base.unit, e
                )))
            }
        }
    }
}

/// Comparison operators classified for [`eval_compare`].
#[derive(Clone, Copy)]
enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Comparison — operands must share a family; canonicalise both sides
/// (mm, rad, or raw) and compare. Returns a `Dimensionless` `0.0` or
/// `1.0`.
fn eval_compare(lhs: Quantity, rhs: Quantity, op: CompareOp) -> Result<Quantity, ExprError> {
    let lf = lhs.unit.family();
    let rf = rhs.unit.family();
    if lf != rf {
        return Err(ExprError::UnitMismatch {
            lhs: lhs.unit,
            rhs: rhs.unit,
        });
    }
    let (l, r) = canonical_pair(lhs, rhs)?;
    let result = match op {
        CompareOp::Eq => (l - r).abs() <= EQ_TOL,
        CompareOp::Ne => (l - r).abs() > EQ_TOL,
        CompareOp::Lt => l < r,
        CompareOp::Le => l <= r,
        CompareOp::Gt => l > r,
        CompareOp::Ge => l >= r,
    };
    Ok(Quantity::count(if result { 1.0 } else { 0.0 }))
}

/// Logical `&&` / `||` — both operands must be `Dimensionless`; non-zero
/// is true.
fn eval_logical(
    lhs: Quantity,
    rhs: Quantity,
    f: fn(bool, bool) -> bool,
) -> Result<Quantity, ExprError> {
    let l = lhs.as_count()?;
    let r = rhs.as_count()?;
    let result = f(l != 0.0, r != 0.0);
    Ok(Quantity::count(if result { 1.0 } else { 0.0 }))
}

/// `Lookup` — find the index `i` such that `key ≈ keys[i]` and return
/// `eval(values[i])`.
fn eval_lookup(
    key: &ExprNode,
    keys: &[ExprNode],
    values: &[ExprNode],
    ctx: &EvalContext,
) -> Result<Quantity, ExprError> {
    if keys.len() != values.len() {
        return Err(ExprError::LookupShapeMismatch);
    }
    let key_q = eval(key, ctx)?;
    for (k_node, v_node) in keys.iter().zip(values.iter()) {
        let k_q = eval(k_node, ctx)?;
        if same_canonical(key_q, k_q)? {
            return eval(v_node, ctx);
        }
    }
    Err(ExprError::LookupNoMatch)
}

/// Convert `q` into the unit `target_unit`. Caller has already
/// verified that the families match.
fn convert_to(q: Quantity, target_unit: Unit) -> Result<f64, ExprError> {
    if q.unit == target_unit {
        return Ok(q.value);
    }
    match target_unit.family() {
        UnitFamily::Length => {
            let mm = q.as_mm()?;
            Ok(match target_unit {
                Unit::Mm => mm,
                Unit::Mil => mm / 0.0254,
                Unit::In => mm / 25.4,
                Unit::Um => mm / 0.001,
                _ => unreachable!("target_unit family was Length"),
            })
        }
        UnitFamily::Angle => {
            let rad = q.as_rad()?;
            Ok(match target_unit {
                Unit::Rad => rad,
                Unit::Deg => rad * 180.0 / std::f64::consts::PI,
                _ => unreachable!("target_unit family was Angle"),
            })
        }
        UnitFamily::Count => Ok(q.as_count()?),
    }
}

/// Convert two same-family quantities to canonical units (mm / rad /
/// raw) and return the pair.
fn canonical_pair(lhs: Quantity, rhs: Quantity) -> Result<(f64, f64), ExprError> {
    debug_assert_eq!(lhs.unit.family(), rhs.unit.family());
    Ok(match lhs.unit.family() {
        UnitFamily::Length => (lhs.as_mm()?, rhs.as_mm()?),
        UnitFamily::Angle => (lhs.as_rad()?, rhs.as_rad()?),
        UnitFamily::Count => (lhs.as_count()?, rhs.as_count()?),
    })
}

/// Two quantities count as equal for `Lookup` matching iff they share
/// a family and their canonical values are within `EQ_TOL`.
fn same_canonical(a: Quantity, b: Quantity) -> Result<bool, ExprError> {
    if a.unit.family() != b.unit.family() {
        return Ok(false);
    }
    let (av, bv) = canonical_pair(a, b)?;
    Ok((av - bv).abs() <= EQ_TOL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::ast::{BinOp, ExprNode};

    fn ctx() -> EvalContext {
        EvalContext::default()
    }

    #[test]
    fn literal_passes_through() {
        let q = eval(&ExprNode::literal_mm(0.5), &ctx()).unwrap();
        assert_eq!(q, Quantity::length(0.5));
    }

    #[test]
    fn add_same_unit() {
        let e = ExprNode::Binary(
            BinOp::Add,
            Box::new(ExprNode::literal_mm(1.0)),
            Box::new(ExprNode::literal_mm(2.0)),
        );
        let q = eval(&e, &ctx()).unwrap();
        assert_eq!(q, Quantity::length(3.0));
    }

    #[test]
    fn add_mixed_lengths_uses_lhs_unit() {
        // 1mm + 100mil = 1mm + 2.54mm = 3.54mm (result keeps LHS unit)
        let e = ExprNode::Binary(
            BinOp::Add,
            Box::new(ExprNode::Literal(Quantity {
                value: 1.0,
                unit: Unit::Mm,
            })),
            Box::new(ExprNode::Literal(Quantity {
                value: 100.0,
                unit: Unit::Mil,
            })),
        );
        let q = eval(&e, &ctx()).unwrap();
        assert_eq!(q.unit, Unit::Mm);
        assert!((q.value - 3.54).abs() < 1e-10);
    }

    #[test]
    fn unit_mismatch_errors() {
        let e = ExprNode::Binary(
            BinOp::Add,
            Box::new(ExprNode::literal_mm(1.0)),
            Box::new(ExprNode::Literal(Quantity {
                value: 90.0,
                unit: Unit::Deg,
            })),
        );
        assert!(matches!(
            eval(&e, &ctx()),
            Err(ExprError::UnitMismatch { .. })
        ));
    }

    #[test]
    fn div_by_zero_length_returns_domain_error() {
        // 1mm / 0mm — Length / Length branch.
        let e = ExprNode::Binary(
            BinOp::Div,
            Box::new(ExprNode::literal_mm(1.0)),
            Box::new(ExprNode::literal_mm(0.0)),
        );
        assert!(matches!(eval(&e, &ctx()), Err(ExprError::Domain(_))));
    }

    #[test]
    fn div_by_zero_count_returns_domain_error() {
        // 5 / 0 — Count / Count branch.
        let e = ExprNode::Binary(
            BinOp::Div,
            Box::new(ExprNode::Literal(Quantity::count(5.0))),
            Box::new(ExprNode::Literal(Quantity::count(0.0))),
        );
        assert!(matches!(eval(&e, &ctx()), Err(ExprError::Domain(_))));
    }

    #[test]
    fn mod_by_zero_returns_domain_error() {
        // 0 % 0 — the prompt-named edge case.
        let e = ExprNode::Binary(
            BinOp::Mod,
            Box::new(ExprNode::Literal(Quantity::count(0.0))),
            Box::new(ExprNode::Literal(Quantity::count(0.0))),
        );
        assert!(matches!(eval(&e, &ctx()), Err(ExprError::Domain(_))));
    }

    #[test]
    fn mod_by_zero_length_returns_domain_error() {
        // 5mm % 0mm — Length / Length branch via the Mod operator.
        let e = ExprNode::Binary(
            BinOp::Mod,
            Box::new(ExprNode::literal_mm(5.0)),
            Box::new(ExprNode::literal_mm(0.0)),
        );
        assert!(matches!(eval(&e, &ctx()), Err(ExprError::Domain(_))));
    }

    #[test]
    fn div_by_zero_angle_returns_domain_error() {
        // 90deg / 0deg — Angle / Angle branch.
        let e = ExprNode::Binary(
            BinOp::Div,
            Box::new(ExprNode::Literal(Quantity {
                value: 90.0,
                unit: Unit::Deg,
            })),
            Box::new(ExprNode::Literal(Quantity {
                value: 0.0,
                unit: Unit::Deg,
            })),
        );
        assert!(matches!(eval(&e, &ctx()), Err(ExprError::Domain(_))));
    }

    #[test]
    fn nonzero_divisor_still_works() {
        // Sanity: the check doesn't accidentally reject finite divisors.
        let e = ExprNode::Binary(
            BinOp::Div,
            Box::new(ExprNode::literal_mm(10.0)),
            Box::new(ExprNode::literal_mm(2.0)),
        );
        let q = eval(&e, &ctx()).unwrap();
        assert_eq!(q, Quantity::count(5.0));
    }
}
