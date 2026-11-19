//! Expression evaluator.
//!
//! Walks the [`ExprNode`] tree and produces a [`Quantity`] with unit
//! checking on every operator. Names are looked up through a
//! [`Resolver`]. Cleanroom; no third-party expression-evaluator source
//! consulted.
//!
//! # Semantics summary
//!
//! - `Add` / `Sub` — operands must share a unit family (and, for derived
//!   units, a dimension). Length+Length converts RHS to LHS unit before
//!   adding; the result keeps LHS's unit. Same for Angle+Angle and
//!   Dimensionless+Dimensionless.
//! - `Mul` — a length or angle times a plain number keeps its unit, as
//!   does a plain number times a plain number. Every other product
//!   multiplies in base units and adds the dimension exponents, so
//!   `mm * mm` is an area and `V * A` is a power.
//! - `Div` / `Mod` — `Length / Length → Dimensionless`,
//!   `Length / Dimensionless → Length`, `Angle / Dimensionless → Angle`,
//!   `Dimensionless / Dimensionless → Dimensionless`. Every other quotient
//!   subtracts the dimension exponents (`3.3V / 10mA` is `330Ω`); `%`
//!   needs equal dimensions or a plain divisor.
//! - `Pow` — the exponent must be dimensionless. A plain base takes any
//!   exponent; a base with a unit needs a whole exponent up to
//!   [`crate::MAX_DIM_EXPONENT`], and `x ^ 1` is `x` unchanged.
//! - Comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`) — operands must
//!   share a family and dimension; the result is a `Dimensionless` `0.0`
//!   or `1.0`. `==` and `!=` use a `1e-12` tolerance against the canonical
//!   value to avoid exact-equality footguns; the strict orderings use
//!   exact `f64` comparison.
//! - Logical (`&&`, `||`) — operands must be `Dimensionless`; any
//!   non-zero value is true.
//! - `Unary(Neg)` — negate the value, keep the unit.
//! - `Unary(Not)` — operand must be `Dimensionless`; result is the
//!   logical inverse as `Dimensionless` `0.0` or `1.0`.
//! - `Ternary` — condition must be `Dimensionless`.
//! - `Lookup` — match key against each entry of `keys` (same family +
//!   value within tolerance) and return `eval(values[i])`. Errors if
//!   no match or shapes differ.
//! - `Call` — see [`crate::Func`].
//! - `Ref` — resolved through the [`Resolver`]. A name met again while
//!   it is still being evaluated is [`ExprError::Cycle`]; a chain of more
//!   than [`MAX_EXPR_DEPTH`] references is [`ExprError::TooDeep`], and so is
//!   a walk deeper than [`MAX_EVAL_DEPTH`] tree levels summed over every
//!   body the references lead into.

use crate::ast::{BinOp, ExprNode, UnaryOp};
use crate::resolver::{Binding, Resolver};
use crate::source::{FieldText, classify};
use crate::unit::{Quantity, Unit, UnitFamily, parse_quantity};
use crate::{ExprError, MAX_EVAL_DEPTH, MAX_EXPR_DEPTH};

/// Tolerance for `Eq`/`Ne` comparisons and `Lookup` key matching, in
/// canonical units (mm for Length, rad for Angle, raw for Count, base
/// units for a derived dimension).
pub(crate) const EQ_TOL: f64 = 1e-12;

/// Walk `node` and produce a [`Quantity`], looking names up in `resolver`.
///
/// Recursive interpreter; each operator is unit-checked before
/// reduction. Errors are bubbled up as [`ExprError`].
pub fn eval(node: &ExprNode, resolver: &dyn Resolver) -> Result<Quantity, ExprError> {
    Evaluator {
        resolver,
        active: Vec::new(),
        depth: 0,
    }
    .eval(node)
}

/// One evaluation: the resolver, the names being evaluated right now
/// (innermost last, for cycle detection) and the recursion budget spent.
struct Evaluator<'r> {
    resolver: &'r dyn Resolver,
    active: Vec<String>,
    /// Tree levels currently on the stack, across every reference followed.
    /// The one budget that bounds the evaluator's recursion.
    depth: usize,
}

impl Evaluator<'_> {
    fn eval(&mut self, node: &ExprNode) -> Result<Quantity, ExprError> {
        if self.depth >= MAX_EVAL_DEPTH {
            return Err(ExprError::TooDeep {
                max: MAX_EVAL_DEPTH,
            });
        }
        self.depth += 1;
        let result = self.eval_node(node);
        self.depth -= 1;
        let q = result?;
        // `powf`, overflowing products and the like can yield NaN / inf with
        // no error of their own; refuse to hand those to the solver or bake.
        if q.value.is_finite() {
            Ok(q)
        } else {
            Err(ExprError::NonFinite(format!("{}", q.value)))
        }
    }

    fn eval_node(&mut self, node: &ExprNode) -> Result<Quantity, ExprError> {
        match node {
            ExprNode::Literal(q) => Ok(*q),
            ExprNode::Ref(name) => self.eval_ref(name),
            ExprNode::Binary(op, lhs, rhs) => {
                let lv = self.eval(lhs)?;
                let rv = self.eval(rhs)?;
                eval_binop(*op, lv, rv)
            }
            ExprNode::Unary(op, inner) => {
                let v = self.eval(inner)?;
                eval_unaryop(*op, v)
            }
            ExprNode::Ternary(cond, then_branch, else_branch) => {
                let cv = self.eval(cond)?;
                let raw = cv.as_count()?;
                if raw != 0.0 {
                    self.eval(then_branch)
                } else {
                    self.eval(else_branch)
                }
            }
            ExprNode::Lookup { key, keys, values } => self.eval_lookup(key, keys, values),
            ExprNode::Call(func, args) => {
                let values = args
                    .iter()
                    .map(|arg| self.eval(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                crate::func::apply(*func, &values)
            }
        }
    }

    /// Resolve `name` and evaluate what it is bound to. Kept out of line
    /// so its locals do not enlarge every `eval_node` frame of a deep tree.
    #[inline(never)]
    fn eval_ref(&mut self, name: &str) -> Result<Quantity, ExprError> {
        if self.active.iter().any(|n| n == name) {
            return Err(ExprError::Cycle(name.to_string()));
        }
        if self.active.len() >= MAX_EXPR_DEPTH {
            return Err(ExprError::TooDeep {
                max: MAX_EXPR_DEPTH,
            });
        }
        let resolver = self.resolver;
        let binding = resolver
            .resolve(name)?
            .ok_or_else(|| ExprError::Unknown(name.to_string()))?;
        self.active.push(name.to_string());
        let result = match binding {
            Binding::Value(q) => self.eval(&ExprNode::Literal(q)),
            Binding::Expr(node) => self.eval(&node),
            Binding::Text(text) => self.eval_text(&text),
        };
        self.active.pop();
        result
    }

    /// Evaluate a stored field text under the `=` / `==` rule: an
    /// expression is parsed and evaluated, anything else must be a literal
    /// quantity.
    #[inline(never)]
    fn eval_text(&mut self, text: &str) -> Result<Quantity, ExprError> {
        match classify(text) {
            FieldText::Expression(body) => {
                let node = crate::parse::parse(body)?;
                self.eval(&node)
            }
            FieldText::Literal(literal) => Ok(parse_quantity(literal)?),
        }
    }

    /// `Lookup` — find the index `i` such that `key ≈ keys[i]` and return
    /// `eval(values[i])`.
    fn eval_lookup(
        &mut self,
        key: &ExprNode,
        keys: &[ExprNode],
        values: &[ExprNode],
    ) -> Result<Quantity, ExprError> {
        if keys.len() != values.len() {
            return Err(ExprError::LookupShapeMismatch);
        }
        let key_q = self.eval(key)?;
        for (k_node, v_node) in keys.iter().zip(values.iter()) {
            let k_q = self.eval(k_node)?;
            if same_canonical(key_q, k_q)? {
                return self.eval(v_node);
            }
        }
        Err(ExprError::LookupNoMatch)
    }
}

/// Apply a binary operator to two already-evaluated quantities.
fn eval_binop(op: BinOp, lhs: Quantity, rhs: Quantity) -> Result<Quantity, ExprError> {
    match op {
        BinOp::Add => eval_add_sub(lhs, rhs, |a, b| a + b),
        BinOp::Sub => eval_add_sub(lhs, rhs, |a, b| a - b),
        BinOp::Mul => eval_mul(lhs, rhs),
        BinOp::Div => eval_div_mod(lhs, rhs, |a, b| a / b, derived_quotient),
        BinOp::Mod => eval_div_mod(lhs, rhs, f64::rem_euclid, derived_remainder),
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
    if lf == UnitFamily::Derived {
        // Derived values are already in base units; only the dimensions
        // have to agree.
        if lhs.unit != rhs.unit {
            return Err(ExprError::UnitMismatch {
                lhs: lhs.unit,
                rhs: rhs.unit,
            });
        }
        return Ok(Quantity {
            value: f(lhs.value, rhs.value),
            unit: lhs.unit,
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

/// Multiplication. The scaling cases keep the operand's own unit, exactly
/// as before unit algebra; every other product is formed in base units.
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
        // Length × Length is an area, V × A a power: multiply in base
        // units and add the exponents. A host that only accepts lengths,
        // angles and counts (the sketch) rejects the result at its own
        // boundary.
        _ => {
            let (lv, ld) = lhs.to_base();
            let (rv, rd) = rhs.to_base();
            Ok(Quantity::from_base(lv * rv, ld.times(rd)?))
        }
    }
}

/// Quotient of two quantities outside the same-family cases of
/// [`eval_div_mod`]: base units, exponents subtracted.
fn derived_quotient(lhs: Quantity, rhs: Quantity) -> Result<Quantity, ExprError> {
    let (lv, ld) = lhs.to_base();
    let (rv, rd) = rhs.to_base();
    check_nonzero(rv)?;
    Ok(Quantity::from_base(lv / rv, ld.per(rd)?))
}

/// Remainder outside the same-family cases of [`eval_div_mod`]: a plain
/// divisor keeps the dividend's dimension, equal dimensions give a plain
/// number, anything else is a mismatch.
fn derived_remainder(lhs: Quantity, rhs: Quantity) -> Result<Quantity, ExprError> {
    let (lv, ld) = lhs.to_base();
    let (rv, rd) = rhs.to_base();
    if rd.is_none() {
        check_nonzero(rv)?;
        Ok(Quantity::from_base(lv.rem_euclid(rv), ld))
    } else if ld == rd {
        check_nonzero(rv)?;
        Ok(Quantity::count(lv.rem_euclid(rv)))
    } else {
        Err(ExprError::UnitMismatch {
            lhs: lhs.unit,
            rhs: rhs.unit,
        })
    }
}

/// Division and modulus. Length / Length collapses to Dimensionless; the
/// scaling cases mirror [`eval_mul`] and every other pair goes to
/// `derived` ([`derived_quotient`] or [`derived_remainder`]). A zero
/// divisor on any branch surfaces [`ExprError::Domain`] rather than
/// producing inf / NaN — `1mm / 0` and `0 % 0` would otherwise flow into
/// the LM solver as a poisoned state and silently break convergence.
fn eval_div_mod(
    lhs: Quantity,
    rhs: Quantity,
    f: fn(f64, f64) -> f64,
    derived: fn(Quantity, Quantity) -> Result<Quantity, ExprError>,
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
        _ => derived(lhs, rhs),
    }
}

/// Reject a zero divisor before we feed it to `/` or `%`. Pure-zero
/// equality is intentional; sub-ULP non-zero values still divide
/// cleanly under f64 (returning a large finite ratio rather than inf).
pub(crate) fn check_nonzero(divisor: f64) -> Result<(), ExprError> {
    if divisor == 0.0 {
        Err(ExprError::Domain("division by zero".into()))
    } else {
        Ok(())
    }
}

/// Power. A plain base takes any plain exponent. A base with a unit needs
/// a whole exponent (its dimension exponents are multiplied by it), except
/// that `x ^ 1` returns `x` with its own unit.
fn eval_pow(base: Quantity, exp: Quantity) -> Result<Quantity, ExprError> {
    // Exponent must be dimensionless.
    if exp.unit.family() != UnitFamily::Count {
        return Err(ExprError::UnitMismatch {
            lhs: base.unit,
            rhs: exp.unit,
        });
    }
    let e = exp.value;
    if base.unit.family() == UnitFamily::Count {
        return Ok(Quantity::count(base.value.powf(e)));
    }
    if (e - 1.0).abs() < EQ_TOL {
        return Ok(base);
    }
    let limit = f64::from(crate::MAX_DIM_EXPONENT);
    if e.fract() != 0.0 || e.abs() > limit {
        return Err(ExprError::Domain(format!(
            "{:?}^{e}: a value with a unit needs a whole exponent up to {limit}",
            base.unit
        )));
    }
    let n = e as i32;
    let (value, dim) = base.to_base();
    Ok(Quantity::from_base(value.powi(n), dim.raised_to(n)?))
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
        // Derived values are stored in base units already.
        UnitFamily::Derived => Ok(q.value),
    }
}

/// Convert two same-family quantities to canonical units (mm / rad /
/// raw / base) and return the pair. Two derived quantities must share a
/// dimension.
fn canonical_pair(lhs: Quantity, rhs: Quantity) -> Result<(f64, f64), ExprError> {
    debug_assert_eq!(lhs.unit.family(), rhs.unit.family());
    Ok(match lhs.unit.family() {
        UnitFamily::Length => (lhs.as_mm()?, rhs.as_mm()?),
        UnitFamily::Angle => (lhs.as_rad()?, rhs.as_rad()?),
        UnitFamily::Count => (lhs.as_count()?, rhs.as_count()?),
        UnitFamily::Derived => {
            if lhs.unit != rhs.unit {
                return Err(ExprError::UnitMismatch {
                    lhs: lhs.unit,
                    rhs: rhs.unit,
                });
            }
            (lhs.value, rhs.value)
        }
    })
}

/// Two quantities count as equal for `Lookup` matching iff they share
/// a family and dimension and their canonical values are within
/// `EQ_TOL`.
fn same_canonical(a: Quantity, b: Quantity) -> Result<bool, ExprError> {
    if a.unit.family() != b.unit.family() || a.dimension() != b.dimension() {
        return Ok(false);
    }
    let (av, bv) = canonical_pair(a, b)?;
    Ok((av - bv).abs() <= EQ_TOL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::NoBindings;

    fn ctx() -> NoBindings {
        NoBindings
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
