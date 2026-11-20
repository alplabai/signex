//! Name lookup: how an expression's references find their values.
//!
//! Each host supplies its own [`Resolver`]: a sketch's parameter table
//! (with the array index `i` / `j`), a part's fields and other parts'
//! fields (`R1.Value`), sheet and project variables. The evaluator owns
//! cycle detection, so no host has to repeat it. The markup lookups
//! (`${refdes:key}`, `@{name}`) are meant to read the same resolvers;
//! [`Binding::Text`] hands them the stored text unchanged.
//!
//! [`Layered`] stacks resolvers so an inner scope shadows an outer one
//! (sheet variables over project variables).

use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::ExprError;
use crate::ast::ExprNode;
use crate::unit::Quantity;

/// What a name is bound to.
#[derive(Clone, Debug, PartialEq)]
pub enum Binding<'a> {
    /// An evaluated value.
    Value(Quantity),
    /// A parsed expression, evaluated in the same evaluation (so its own
    /// references resolve and cycles are caught).
    Expr(Cow<'a, ExprNode>),
    /// A stored field or variable text under the `=` / `==` rule
    /// ([`crate::source::classify`]): an expression is parsed and
    /// evaluated, anything else must be a literal quantity.
    Text(Cow<'a, str>),
}

/// Looks names up for the evaluator.
///
/// A name is the identifier as written, dotted paths included
/// (`pad_pitch`, `R1.Value`). `Ok(None)` means the name is unknown here;
/// an `Err` is a host-specific refusal (a sketch's `i` outside an array).
pub trait Resolver {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError>;
}

/// A resolver that knows no names.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoBindings;

impl Resolver for NoBindings {
    fn resolve(&self, _name: &str) -> Result<Option<Binding<'_>>, ExprError> {
        Ok(None)
    }
}

/// Names bound to parsed expressions (a parameter table after parsing).
impl Resolver for BTreeMap<String, ExprNode> {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError> {
        Ok(self
            .get(name)
            .map(|node| Binding::Expr(Cow::Borrowed(node))))
    }
}

/// Names bound to values.
impl Resolver for BTreeMap<String, Quantity> {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError> {
        Ok(self.get(name).map(|q| Binding::Value(*q)))
    }
}

/// Names bound to stored texts (fields, variables).
impl Resolver for BTreeMap<String, String> {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError> {
        Ok(self
            .get(name)
            .map(|text| Binding::Text(Cow::Borrowed(text.as_str()))))
    }
}

/// Resolvers tried in order; the first that knows a name answers. Put the
/// innermost scope first: `Layered::new(vec![&sheet, &project])`.
pub struct Layered<'a> {
    layers: Vec<&'a dyn Resolver>,
}

impl<'a> Layered<'a> {
    pub fn new(layers: Vec<&'a dyn Resolver>) -> Self {
        Self { layers }
    }
}

impl Resolver for Layered<'_> {
    fn resolve(&self, name: &str) -> Result<Option<Binding<'_>>, ExprError> {
        for layer in &self.layers {
            if let Some(binding) = layer.resolve(name)? {
                return Ok(Some(binding));
            }
        }
        Ok(None)
    }
}
