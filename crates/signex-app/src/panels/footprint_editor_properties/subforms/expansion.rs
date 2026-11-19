//! Reading a length expression the sub-forms show as a number.

use signex_sketch::expr::eval::{EvalContext, eval};
use signex_sketch::expr::parse::parse;

/// The value in mm of a length expression that needs no parameter
/// (`0.05mm`, `0.05`, `=-0.04mm`); a plain number reads as mm. `None` for an
/// expression that names a parameter or does not parse.
pub(super) fn literal_mm(expr: &str) -> Option<f64> {
    let body = expr.trim().trim_start_matches('=').trim();
    let quantity = parse(body)
        .ok()
        .and_then(|ast| eval(&ast, &EvalContext::default()).ok())?;
    quantity
        .as_mm()
        .ok()
        .or_else(|| quantity.as_count().ok())
        .filter(|mm| mm.is_finite())
}

#[cfg(test)]
mod tests {
    use super::literal_mm;

    #[test]
    fn plain_lengths_read_in_millimetres() {
        assert_eq!(literal_mm("0.05mm"), Some(0.05));
        assert_eq!(literal_mm("-0.04"), Some(-0.04));
        assert_eq!(literal_mm("=0.1mm"), Some(0.1));
    }

    #[test]
    fn a_parameter_or_broken_text_has_no_literal_value() {
        assert_eq!(literal_mm("mask_margin"), None);
        assert_eq!(literal_mm("0.1 +"), None);
    }
}
