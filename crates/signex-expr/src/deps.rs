//! The references of an expression and the dependency order of a table
//! of named expressions.
//!
//! A host that evaluates a whole table at once (the sketch's parameter
//! table) sorts it with [`dependency_order`] and evaluates each name after
//! the names it reads. Cycle detection uses an iterative DFS with
//! `Visiting` / `Visited` colours (Tarjan-style). No third-party graph or
//! solver source consulted.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::ExprError;
use crate::ast::ExprNode;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Gray,
    Black,
}

/// Iterative DFS topological sort. Returns names in an order where every
/// dependency precedes its dependants. Errors with [`ExprError::Cycle`]
/// if a cycle is found. A reference to a name that is not in `asts` (a
/// host variable such as a sketch array's `i`, or an unbound name) is
/// skipped here; the evaluator reports it.
pub fn dependency_order(asts: &BTreeMap<String, ExprNode>) -> Result<Vec<String>, ExprError> {
    let mut color: HashMap<&str, Color> = asts.keys().map(|k| (k.as_str(), Color::White)).collect();
    let mut order: Vec<String> = Vec::with_capacity(asts.len());

    // We walk every name in BTreeMap order so the resolution is
    // deterministic across runs.
    for root in asts.keys() {
        if color.get(root.as_str()).copied() != Some(Color::White) {
            continue;
        }
        // Stack frame: (name, child_iter_index, deps_vec)
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

            // Skip refs that aren't names in this table — they might be
            // host variables or unbound; the evaluator will surface a
            // clearer error when they're encountered.
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
                    // Should not happen — every key is in the map.
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
        .expect("name must be a key of the table")
}

/// Collect every [`ExprNode::Ref`] name reachable from `name`'s AST.
fn collect_deps(asts: &BTreeMap<String, ExprNode>, name: &str) -> Vec<String> {
    asts.get(name).map(references).unwrap_or_default()
}

/// Every distinct name `node` refers to, in first-seen order.
pub fn references(node: &ExprNode) -> Vec<String> {
    fn rec(node: &ExprNode, seen: &mut HashSet<String>, out: &mut Vec<String>) {
        match node {
            ExprNode::Literal(_) => {}
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
            ExprNode::Call(_, args) => {
                for a in args {
                    rec(a, seen, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    rec(node, &mut seen, &mut out);
    out
}
