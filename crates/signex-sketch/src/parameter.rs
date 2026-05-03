//! Parameter table with topological resolution.
//!
//! A sketch's `parameters` field is a `name → source-string` table.
//! Values can reference each other (`body_w = "= pad_pitch *
//! (pin_count - 1) + 2mm"`), so resolution must:
//!
//! 1. Parse each source string into an [`ExprNode`].
//! 2. Walk the AST to discover [`ExprNode::Ref`] dependencies.
//! 3. Topologically sort the parameter graph; reject cycles.
//! 4. Evaluate in topo order, accumulating already-resolved
//!    parameters as `Literal(Quantity)` ASTs in the eval context so
//!    later expressions resolve cleanly.
//!
//! Cycle detection uses an iterative DFS with `Visiting` / `Visited`
//! colours (Tarjan-style). No third-party graph or solver source
//! consulted.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::expr::ast::ExprNode;
use crate::expr::eval::{eval, EvalContext};
use crate::expr::parse::parse;
use crate::expr::ExprError;
use crate::unit::{Quantity, UnitFamily};

/// User-defined parameter table — `name → source-string`. Source
/// strings carry an optional `=` prefix (Altium-style) and are
/// otherwise the same expression-language input that the parser
/// accepts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParameterTable(pub BTreeMap<String, String>);

impl ParameterTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert / overwrite a parameter source string.
    pub fn insert(&mut self, name: impl Into<String>, src: impl Into<String>) {
        self.0.insert(name.into(), src.into());
    }

    pub fn get_raw(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Strip the optional Altium-style leading `=` and surrounding
/// whitespace from a parameter source string.
fn strip_eq_prefix(src: &str) -> &str {
    let s = src.trim();
    s.strip_prefix('=').map(|s| s.trim_start()).unwrap_or(s)
}

/// Resolve every parameter to a canonical-unit `f64` (mm for
/// `Length`, rad for `Angle`, raw for `Count`).
///
/// Returns `ExprError::Cycle(name)` if the dependency graph contains
/// a cycle through `name`. Other parse / eval errors propagate as
/// the corresponding [`ExprError`] variant.
pub fn resolve(table: &ParameterTable) -> Result<HashMap<String, f64>, ExprError> {
    // 1. Parse every parameter source string into an AST.
    let mut asts: BTreeMap<String, ExprNode> = BTreeMap::new();
    for (name, src) in table.iter() {
        let body = strip_eq_prefix(src);
        let ast = parse(body)?;
        asts.insert(name.clone(), ast);
    }

    // 2. Topologically sort by dependency. Iterative DFS with three
    //    colours: White (unseen), Gray (on the recursion stack),
    //    Black (finished). Gray-revisit signals a cycle.
    let order = topo_sort(&asts)?;

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

    // 4. Convert each Quantity to its canonical-unit f64.
    let mut out = HashMap::with_capacity(resolved_quantities.len());
    for (name, q) in resolved_quantities {
        let canonical = match q.unit.family() {
            UnitFamily::Length => q.as_mm()?,
            UnitFamily::Angle => q.as_rad()?,
            UnitFamily::Count => q.as_count()?,
        };
        out.insert(name, canonical);
    }
    Ok(out)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Gray,
    Black,
}

/// Iterative DFS topological sort. Returns parameter names in an
/// order where every dependency precedes its dependants. Errors with
/// [`ExprError::Cycle`] if a cycle is found.
fn topo_sort(asts: &BTreeMap<String, ExprNode>) -> Result<Vec<String>, ExprError> {
    let mut color: HashMap<&str, Color> =
        asts.keys().map(|k| (k.as_str(), Color::White)).collect();
    let mut order: Vec<String> = Vec::with_capacity(asts.len());

    // We walk every parameter in BTreeMap order so the resolution is
    // deterministic across runs.
    for root in asts.keys() {
        if color.get(root.as_str()).copied() != Some(Color::White) {
            continue;
        }
        // Stack frame: (param_name, child_iter_index, deps_vec)
        let mut stack: Vec<(String, usize, Vec<String>)> = Vec::new();
        let root_deps = collect_deps(asts, root);
        color.insert(root.as_str(), Color::Gray);
        stack.push((root.clone(), 0, root_deps));

        while let Some(top) = stack.last_mut() {
            if top.1 >= top.2.len() {
                // All children processed → mark Black, emit, pop.
                let name = top.0.clone();
                color.insert(name_borrow(&name, asts), Color::Black);
                order.push(name);
                stack.pop();
                continue;
            }
            let dep_name = top.2[top.1].clone();
            top.1 += 1;

            // Skip refs that aren't parameters in this table — they
            // might be ArrayIndex variables or unbound; the evaluator
            // will surface a clearer error when they're encountered.
            if !asts.contains_key(&dep_name) {
                continue;
            }

            match color.get(dep_name.as_str()).copied() {
                Some(Color::White) => {
                    let dep_deps = collect_deps(asts, &dep_name);
                    color.insert(name_borrow(&dep_name, asts), Color::Gray);
                    stack.push((dep_name, 0, dep_deps));
                }
                Some(Color::Gray) => {
                    return Err(ExprError::Cycle(dep_name));
                }
                Some(Color::Black) => {
                    // Already finished; nothing to do.
                }
                None => {
                    // Should not happen — every param key is in the map.
                }
            }
        }
    }

    Ok(order)
}

/// Borrow a `&str` view of `name` whose lifetime matches `asts`'s
/// keys, so we can use it as a `HashMap<&str, _>` key.
fn name_borrow<'a>(name: &str, asts: &'a BTreeMap<String, ExprNode>) -> &'a str {
    asts.get_key_value(name)
        .map(|(k, _)| k.as_str())
        .expect("name must be a param in the table")
}

/// Collect every [`ExprNode::Ref`] name reachable from `name`'s AST.
fn collect_deps(asts: &BTreeMap<String, ExprNode>, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(root) = asts.get(name) {
        gather_refs(root, &mut out);
    }
    out
}

/// Walk an AST and append every distinct `Ref(name)` name to `out`.
fn gather_refs(node: &ExprNode, out: &mut Vec<String>) {
    fn rec(node: &ExprNode, seen: &mut HashSet<String>, out: &mut Vec<String>) {
        match node {
            ExprNode::Literal(_) | ExprNode::ArrayIndex(_) => {}
            ExprNode::Ref(name) => {
                if seen.insert(name.clone()) {
                    out.push(name.clone());
                }
            }
            ExprNode::Binary(_, l, r) => {
                rec(l, seen, out);
                rec(r, seen, out);
            }
            ExprNode::Unary(_, inner) => rec(inner, seen, out),
            ExprNode::Ternary(c, t, e) => {
                rec(c, seen, out);
                rec(t, seen, out);
                rec(e, seen, out);
            }
            ExprNode::Lookup { key, keys, values } => {
                rec(key, seen, out);
                for k in keys {
                    rec(k, seen, out);
                }
                for v in values {
                    rec(v, seen, out);
                }
            }
        }
    }
    let mut seen = HashSet::new();
    rec(node, &mut seen, out);
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
}
