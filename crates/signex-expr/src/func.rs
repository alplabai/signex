//! The pure functions an expression may call (see [`Func`]).

use crate::ExprError;
use crate::ast::Func;
use crate::eval::check_nonzero;
use crate::unit::{Quantity, UnitFamily};

/// Apply `func` to already-evaluated arguments.
pub(crate) fn apply(func: Func, args: &[Quantity]) -> Result<Quantity, ExprError> {
    if !func.accepts(args.len()) {
        return Err(ExprError::Domain(format!(
            "{}() does not take {} arguments",
            func.name(),
            args.len()
        )));
    }
    match func {
        Func::Min => extreme(args, |candidate, best| candidate < best),
        Func::Max => extreme(args, |candidate, best| candidate > best),
        Func::Abs => {
            let x = args[0];
            Ok(Quantity {
                value: x.value.abs(),
                unit: x.unit,
            })
        }
        Func::Sqrt => sqrt(args[0]),
        Func::Round => stepped(args, f64::round),
        Func::Floor => stepped(args, f64::floor),
        Func::Ceil => stepped(args, f64::ceil),
    }
}

/// The argument that wins `beats` against every other, compared in base
/// units. All arguments need one dimension; the winner keeps its unit.
fn extreme(args: &[Quantity], beats: fn(f64, f64) -> bool) -> Result<Quantity, ExprError> {
    let Some((&first, rest)) = args.split_first() else {
        return Err(ExprError::Domain("min() / max() need a value".into()));
    };
    let mut best = first;
    for &candidate in rest {
        let (cv, cd) = candidate.to_base();
        let (bv, bd) = best.to_base();
        if cd != bd {
            return Err(ExprError::UnitMismatch {
                lhs: best.unit,
                rhs: candidate.unit,
            });
        }
        if beats(cv, bv) {
            best = candidate;
        }
    }
    Ok(best)
}

/// Square root. A plain number must not be negative; a value with a unit
/// needs every dimension exponent even (`sqrt(area)` is a length,
/// `sqrt(1mm)` is an error).
fn sqrt(x: Quantity) -> Result<Quantity, ExprError> {
    let (value, dim) = x.to_base();
    if value < 0.0 {
        return Err(ExprError::Domain(format!(
            "sqrt of a negative value ({value})"
        )));
    }
    let root = dim.root().ok_or_else(|| {
        ExprError::Domain(format!(
            "sqrt of {} needs even unit exponents",
            dim.compound_symbol()
        ))
    })?;
    Ok(Quantity::from_base(value.sqrt(), root))
}

/// `round` / `floor` / `ceil`. One argument: a plain number. Two
/// arguments: `x` to a multiple of `step`, both of one dimension, so the
/// answer does not depend on the unit `x` was written in.
fn stepped(args: &[Quantity], f: fn(f64) -> f64) -> Result<Quantity, ExprError> {
    let x = args[0];
    let Some(&step) = args.get(1) else {
        if x.unit.family() != UnitFamily::Count {
            return Err(ExprError::WrongFamily {
                expected: UnitFamily::Count,
                got: x.unit.family(),
            });
        }
        return Ok(Quantity::count(f(x.value)));
    };
    let (xv, xd) = x.to_base();
    let (sv, sd) = step.to_base();
    if xd != sd {
        return Err(ExprError::UnitMismatch {
            lhs: x.unit,
            rhs: step.unit,
        });
    }
    check_nonzero(sv)?;
    Ok(Quantity::from_base(f(xv / sv) * sv, xd))
}
