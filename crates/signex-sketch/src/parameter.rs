//! Parameter table with topological resolution.
//!
//! A sketch's `parameters` field is a `name → source-string` table.
//! Values can reference each other (`body_w = "= pad_pitch *
//! (pin_count - 1) + 2mm"`), so resolution must:
//!
//! 1. Parse each source string into an [`ExprNode`].
//! 2. Walk the AST to discover [`ExprNode::Ref`] dependencies.
//! 3. Topologically sort the parameter graph; reject cycles
//!    ([`signex_expr::dependency_order`]).
//! 4. Evaluate in topo order, accumulating already-resolved
//!    parameters as `Literal(Quantity)` ASTs in the eval context so
//!    later expressions resolve cleanly.

use std::collections::{BTreeMap, HashMap};

use signex_expr::{LiteralContext, dependency_order, expression_body, is_reserved_constant};

use crate::expr::ExprError;
use crate::expr::ast::ExprNode;
use crate::expr::eval::{EvalContext, eval};
use crate::expr::parse::parse_with;
use crate::unit::{Quantity, UnitFamily};

pub use signex_types::sketch::parameter::ParameterTable;

/// Resolve every parameter to a canonical-unit [`Quantity`]: millimetres
/// for `Length`, radians for `Angle`, a plain number for `Count`. Unlike
/// [`resolve`] this keeps each parameter's unit family.
///
/// Returns `ExprError::Cycle(name)` if the dependency graph contains
/// a cycle through `name`. Other parse / eval errors propagate as
/// the corresponding [`ExprError`] variant.
pub fn resolve_quantities(table: &ParameterTable) -> Result<HashMap<String, Quantity>, ExprError> {
    // 1. Parse every parameter source string into an AST. A name the parser
    //    reads as a constant (`pi`) could never be referenced, so it is
    //    refused; a prefix-only literal (`10m`, `4k7`) is refused by the
    //    `Geometry` context instead of baking as a count of millimetres.
    let mut asts: BTreeMap<String, ExprNode> = BTreeMap::new();
    for (name, src) in table.iter() {
        if is_reserved_constant(name) {
            return Err(ExprError::ReservedName(name.clone()));
        }
        let body = expression_body(src);
        let ast = parse_with(body, LiteralContext::Geometry)?;
        asts.insert(name.clone(), ast);
    }

    // 2. Topologically sort by dependency; a cycle is an error.
    let order = dependency_order(&asts)?;

    // 3. Evaluate in topo order. Each freshly-resolved value is
    //    re-injected into the eval context as a Literal(Quantity)
    //    so subsequent expressions reference its concrete value.
    let mut ctx = EvalContext::default();
    let mut resolved_quantities: HashMap<String, Quantity> = HashMap::new();
    for name in &order {
        let ast = asts.get(name).expect("topo order names a param we have");
        let q = eval(ast, &ctx)?;
        ctx.params.insert(name.clone(), ExprNode::Literal(q));
        resolved_quantities.insert(name.clone(), q);
    }

    // 4. Convert each Quantity to its canonical unit.
    let mut out = HashMap::with_capacity(resolved_quantities.len());
    for (name, q) in resolved_quantities {
        let canonical = match q.unit.family() {
            UnitFamily::Length => Quantity::length(q.as_mm()?),
            UnitFamily::Angle => Quantity::angle(q.as_rad()?),
            UnitFamily::Count => Quantity::count(q.as_count()?),
            // `eval` already refuses these at the sketch boundary.
            UnitFamily::Derived => {
                return Err(ExprError::WrongFamily {
                    expected: UnitFamily::Length,
                    got: UnitFamily::Derived,
                });
            }
        };
        out.insert(name, canonical);
    }
    Ok(out)
}

/// Resolve every parameter to a canonical-unit `f64` (mm for
/// `Length`, rad for `Angle`, raw for `Count`).
///
/// Returns `ExprError::Cycle(name)` if the dependency graph contains
/// a cycle through `name`. Other parse / eval errors propagate as
/// the corresponding [`ExprError`] variant.
pub fn resolve(table: &ParameterTable) -> Result<HashMap<String, f64>, ExprError> {
    Ok(resolve_quantities(table)?
        .into_iter()
        .map(|(name, q)| (name, q.value))
        .collect())
}

/// Evaluate one expression `src` (an optional leading `=`) against the
/// resolved parameter `table`, keeping its unit family.
pub fn eval_with_parameters(src: &str, table: &ParameterTable) -> Result<Quantity, ExprError> {
    let ast = parse_with(expression_body(src), LiteralContext::General)?;
    let params = resolve_quantities(table)?
        .into_iter()
        .map(|(name, q)| (name, ExprNode::Literal(q)))
        .collect();
    let ctx = EvalContext {
        params,
        array_index: None,
    };
    eval(&ast, &ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn resolve_single_literal() {
        let mut t = ParameterTable::new();
        t.insert("pad_pitch", "0.5mm");
        let r = resolve(&t).unwrap();
        assert!(approx_eq(*r.get("pad_pitch").unwrap(), 0.5));
    }

    #[test]
    fn resolve_chained_params() {
        let mut t = ParameterTable::new();
        t.insert("pad_pitch", "0.5mm");
        t.insert("pin_count", "16");
        t.insert("body_w", "= pad_pitch * (pin_count - 1) + 2.0mm");
        let r = resolve(&t).unwrap();
        // pad_pitch = 0.5 mm; (16 − 1) × 0.5 = 7.5; + 2 = 9.5 mm.
        assert!(approx_eq(*r.get("body_w").unwrap(), 9.5));
    }

    #[test]
    fn resolve_handles_altium_style_eq_prefix() {
        let mut t = ParameterTable::new();
        t.insert("a", "= 1mm");
        t.insert("b", "= a + 1mm");
        let r = resolve(&t).unwrap();
        assert!(approx_eq(*r.get("b").unwrap(), 2.0));
    }

    #[test]
    fn resolve_two_cycle_errors() {
        let mut t = ParameterTable::new();
        t.insert("a", "= b");
        t.insert("b", "= a");
        let err = resolve(&t).unwrap_err();
        match err {
            ExprError::Cycle(_) => {}
            other => panic!("expected Cycle, got {other:?}"),
        }
    }

    #[test]
    fn resolve_three_cycle_errors() {
        let mut t = ParameterTable::new();
        t.insert("a", "= b");
        t.insert("b", "= c");
        t.insert("c", "= a");
        assert!(matches!(resolve(&t).unwrap_err(), ExprError::Cycle(_)));
    }

    #[test]
    fn resolve_self_reference_errors() {
        let mut t = ParameterTable::new();
        t.insert("a", "= a + 1mm");
        assert!(matches!(resolve(&t).unwrap_err(), ExprError::Cycle(_)));
    }

    #[test]
    fn resolve_canonical_units() {
        let mut t = ParameterTable::new();
        t.insert("len_in_mil", "100mil"); // → 2.54 mm canonical
        t.insert("angle_deg", "90deg"); // → π/2 rad canonical
        t.insert("count", "16"); // dimensionless 16.0
        let r = resolve(&t).unwrap();
        assert!(approx_eq(*r.get("len_in_mil").unwrap(), 2.54));
        assert!(approx_eq(
            *r.get("angle_deg").unwrap(),
            std::f64::consts::FRAC_PI_2
        ));
        assert!(approx_eq(*r.get("count").unwrap(), 16.0));
    }

    #[test]
    fn resolve_unknown_ref_errors() {
        let mut t = ParameterTable::new();
        t.insert("a", "= some_unbound_param * 2");
        assert!(matches!(resolve(&t).unwrap_err(), ExprError::Unknown(_)));
    }

    #[test]
    fn resolve_diamond_dependency() {
        // d depends on b and c, both of which depend on a.
        let mut t = ParameterTable::new();
        t.insert("a", "1mm");
        t.insert("b", "= a + 1mm");
        t.insert("c", "= a + 2mm");
        t.insert("d", "= b + c");
        let r = resolve(&t).unwrap();
        // b = 2mm, c = 3mm, d = 5mm
        assert!(approx_eq(*r.get("d").unwrap(), 5.0));
    }

    #[test]
    fn parameter_table_default_is_empty() {
        let t = ParameterTable::default();
        let r = resolve(&t).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn a_parameter_named_after_a_reserved_constant_is_refused() {
        // `= pi * 2` would read the constant, never this value.
        let mut t = ParameterTable::new();
        t.insert("pi", "3mm");
        t.insert("w", "= pi * 2");
        let err = resolve(&t).unwrap_err();
        assert_eq!(err, ExprError::ReservedName("pi".to_string()));
        assert!(err.to_string().contains("reserved constant"));
    }

    #[test]
    fn pi_still_reads_as_the_constant_in_a_parameter() {
        let mut t = ParameterTable::new();
        t.insert("turn", "= 2 * pi");
        let r = resolve(&t).unwrap();
        assert!(approx_eq(*r.get("turn").unwrap(), std::f64::consts::TAU));
    }

    #[test]
    fn prefix_only_literals_are_refused_in_a_parameter() {
        for literal in ["10m", "1u", "4k7", "2n", "= 10m + 1mm"] {
            let mut t = ParameterTable::new();
            t.insert("a", literal);
            assert!(
                matches!(resolve(&t), Err(ExprError::BadQuantity(_))),
                "{literal} must be refused"
            );
        }
    }

    #[test]
    fn literals_with_a_unit_still_resolve_in_a_parameter() {
        let mut t = ParameterTable::new();
        t.insert("a", "10mm");
        t.insert("b", "250um");
        t.insert("c", "= a + 0.5mm");
        let r = resolve(&t).unwrap();
        assert!(approx_eq(*r.get("a").unwrap(), 10.0));
        assert!(approx_eq(*r.get("b").unwrap(), 0.25));
        assert!(approx_eq(*r.get("c").unwrap(), 10.5));
    }
}
