//! Evaluating a sketch expression: the shared evaluator from
//! `signex-expr` over the sketch's parameter table and array index, with
//! the sketch's unit boundary on the result.

use std::borrow::Cow;
use std::collections::BTreeMap;

use signex_expr::{Binding, Resolver};

use crate::expr::ExprError;
use crate::expr::ast::{ArrayIndex, ExprNode};
use crate::unit::{Quantity, Unit, UnitFamily};

/// Per-evaluation context — the parameter table (raw AST values, so
/// references resolve recursively) plus the optional `(i, j)` array
/// index for inside an array expansion.
#[derive(Clone, Debug, Default)]
pub struct EvalContext {
    /// Parameter table as `name → ExprNode` so [`ExprNode::Ref`] can
    /// resolve recursively. The evaluator reports a reference cycle as
    /// [`ExprError::Cycle`].
    pub params: BTreeMap<String, ExprNode>,
    /// `Some((i, j))` inside an array expansion; `None` otherwise.
    /// Binds the names of [`ArrayIndex`].
    pub array_index: Option<(usize, usize)>,
}

impl EvalContext {
    /// This context with every unitless (count) parameter read as
    /// millimetres: the lenient convention for an expression that must be a
    /// length, shared by the solver's dimension targets and signex-bake.
    #[must_use]
    pub fn with_unitless_as_mm(&self) -> Self {
        let params = self
            .params
            .iter()
            .map(|(name, node)| {
                let node = match node {
                    ExprNode::Literal(q) if q.unit == Unit::Dimensionless => {
                        ExprNode::Literal(Quantity::length(q.value))
                    }
                    other => other.clone(),
                };
                (name.clone(), node)
            })
            .collect();
        Self {
            params,
            array_index: self.array_index,
        }
    }
}

/// `i` / `j` are the array index (an error outside an array); every other
/// name is a parameter.
impl Resolver for EvalContext {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError> {
        if let Some(which) = ArrayIndex::from_name(name) {
            let (i, j) = self.array_index.ok_or(ExprError::ArrayIndexOutsideArray)?;
            let index = match which {
                ArrayIndex::I => i,
                ArrayIndex::J => j,
            };
            return Ok(Some(Binding::Value(Quantity::count(index as f64))));
        }
        Ok(self
            .params
            .get(name)
            .map(|node| Binding::Expr(Cow::Borrowed(node))))
    }
}

/// Walk `node` and produce a sketch [`Quantity`]: a length, an angle or a
/// count (see [`sketch_value`]).
pub fn eval(node: &ExprNode, ctx: &EvalContext) -> Result<Quantity, ExprError> {
    sketch_value(signex_expr::eval(node, ctx)?)
}

/// The sketch boundary: a sketch value is a length, an angle or a plain
/// number. An intermediate area is fine (`a * b / c`), but a result such
/// as `2mm * 3mm` or `5mm ^ 2` is refused here.
pub fn sketch_value(q: Quantity) -> Result<Quantity, ExprError> {
    match q.unit.family() {
        UnitFamily::Length | UnitFamily::Angle | UnitFamily::Count => Ok(q),
        UnitFamily::Derived => Err(ExprError::WrongFamily {
            expected: UnitFamily::Length,
            got: UnitFamily::Derived,
        }),
    }
}
