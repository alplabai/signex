mod caps;
mod expr_parser;
mod functions;
mod resolver;
mod si_units;
mod unit_algebra;
mod unit_parser;

use signex_expr::{NoBindings, Quantity, eval, parse};

/// Parse and evaluate `src` with no names bound.
pub(crate) fn value(src: &str) -> Result<Quantity, signex_expr::ExprError> {
    eval(&parse(src)?, &NoBindings)
}
