//! The one mirror between a board sketch's stored angles and what the user
//! reads. A board shows Y up while the model's y runs down, so every angle
//! dimension is shown as `mirror(stored)` and stored as `mirror(typed)`.
//! Stored data changes only when the user commits an edit.

use signex_expr::{expression_body, is_identifier};

use crate::constraint::{ConstraintKind, DimTarget};
use crate::id::ConstraintId;
use crate::parameter_refs::{literal_of, unique_name};
use crate::sketch::SketchData;
use crate::solver::residual::{ResolvedParams, resolve_dim};
use crate::unit::Quantity;

/// Rounding between two routes to the same angle (`30` and `30deg`), far
/// below any value a user can tell apart.
const SAME_VALUE_ULPS: f64 = 4.0;

/// The angle read in the opposite sense: a literal negates, `name` and
/// `-name` swap, `-(x)` and `x` swap, and any other `x` becomes `-(x)`.
pub fn mirror(target: &DimTarget) -> DimTarget {
    match target {
        DimTarget::Literal(rad) => DimTarget::Literal(0.0 - rad),
        DimTarget::Expr(source) => DimTarget::Expr(mirror_source(source)),
    }
}

fn mirror_source(source: &str) -> String {
    let body = expression_body(source);
    if is_identifier(body) {
        return format!("-{body}");
    }
    if let Some(name) = negated_name(body) {
        return name.to_string();
    }
    match negated_group(body) {
        Some(inner) => inner.to_string(),
        None => format!("-({body})"),
    }
}

/// The name in `-name`.
fn negated_name(body: &str) -> Option<&str> {
    body.strip_prefix('-').filter(|name| is_identifier(name))
}

/// The `x` of `-(x)`, when its brackets are one group around all of it.
fn negated_group(body: &str) -> Option<&str> {
    let inner = body.strip_prefix("-(")?.strip_suffix(')')?;
    is_balanced(inner).then(|| inner.trim())
}

/// Whether every bracket in `text` closes inside it.
fn is_balanced(text: &str) -> bool {
    let mut depth = 0_usize;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => return false,
            ')' => depth -= 1,
            _ => {}
        }
    }
    depth == 0
}

/// The parameter a shown angle reads, plainly or negated (`slope`, `-slope`).
pub fn shown_parameter(shown: &str) -> Option<&str> {
    let body = expression_body(shown);
    if is_identifier(body) {
        Some(body)
    } else {
        negated_name(body)
    }
}

/// Whether `typed` is what is `shown`: the same expression once normalised,
/// or the same value with no parameter in it. A rounded display does not
/// count, so typing 30 over a stored 30.00004 is an edit.
pub fn retypes_shown(shown: &DimTarget, typed: &DimTarget, params: &ResolvedParams) -> bool {
    if let (DimTarget::Expr(a), DimTarget::Expr(b)) = (shown, typed)
        && normal_form(a) == normal_form(b)
    {
        return true;
    }
    matches!(
        (resolve_dim(typed, &ResolvedParams::new()), resolve_dim(shown, params)),
        (Ok(a), Ok(b)) if same_value(a, b)
    )
}

fn normal_form(source: &str) -> String {
    mirror_source(&mirror_source(source))
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn same_value(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= SAME_VALUE_ULPS * f64::EPSILON * a.abs().max(b.abs())
}

/// Make parameter on a board angle: a new parameter holds what is shown and
/// the dimension stores `-name`, so the number and the geometry stay. One
/// already bound keeps its parameter. `None` if `constraint` is no angle.
pub fn parameter_from_mirrored_angle(
    sketch: &mut SketchData,
    constraint: ConstraintId,
) -> Option<String> {
    let index = sketch.constraints.iter().position(|c| c.id == constraint)?;
    let ConstraintKind::Angle { target, .. } = &sketch.constraints[index].kind else {
        return None;
    };
    let source = match mirror(target) {
        DimTarget::Literal(rad) => literal_of(Quantity::angle(rad)),
        DimTarget::Expr(shown) => {
            if let Some(name) = shown_parameter(&shown)
                && sketch.parameters.get_raw(name).is_some()
            {
                return Some(name.to_string());
            }
            shown
        }
    };
    let name = unique_name(&sketch.parameters, "angle");
    sketch.parameters.insert(name.clone(), source);
    if let ConstraintKind::Angle { target, .. } = &mut sketch.constraints[index].kind {
        *target = DimTarget::Expr(mirror_source(&name));
    }
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expr(source: &str) -> DimTarget {
        DimTarget::Expr(source.into())
    }

    #[test]
    fn mirror_is_its_own_inverse() {
        for form in [
            "30deg", "-30deg", "slope", "-slope", "(a+b)", "-(a+b)", "-a+b", "a+b", "-(a)+(b)",
        ] {
            assert_eq!(mirror(&mirror(&expr(form))), expr(form), "{form}");
        }
        for rad in [0.0, 0.5, -0.5] {
            let literal = DimTarget::Literal(rad);
            assert_eq!(mirror(&mirror(&literal)), literal);
        }
    }

    #[test]
    fn mirror_takes_each_form_to_its_negation() {
        assert_eq!(mirror(&expr("slope")), expr("-slope"));
        assert_eq!(mirror(&expr("-slope")), expr("slope"));
        assert_eq!(mirror(&expr("-(a+b)")), expr("a+b"));
        assert_eq!(mirror(&expr("= a + b")), expr("-(a + b)"));
        assert_eq!(mirror(&expr("-a+b")), expr("-(-a+b)"));
        assert_eq!(mirror(&expr("-(a)+(b)")), expr("-(-(a)+(b))"));
        // A double negation is its name, the same value in its plain form.
        assert_eq!(mirror(&expr("-(-slope)")), expr("-slope"));
        assert_eq!(mirror(&mirror(&expr("-(-slope)"))), expr("slope"));
    }

    #[test]
    fn a_rounded_display_is_not_a_retype() {
        let params = ResolvedParams::new();
        let thirty = DimTarget::Literal(30.0_f64.to_radians());
        let near = DimTarget::Literal(30.00004_f64.to_radians());
        assert!(retypes_shown(&thirty, &expr("30deg"), &params));
        assert!(!retypes_shown(&near, &thirty, &params));
        assert!(retypes_shown(&expr("a + b"), &expr("a+b"), &params));
        assert!(retypes_shown(&expr("slope"), &expr("-(-slope)"), &params));
    }
}
