//! AST validator: checks predicate whitelist and target compatibility.

use crate::ast::*;
use crate::error::DslError;

// ---------------------------------------------------------------------------
// Predicate descriptor
// ---------------------------------------------------------------------------

struct HelperDesc {
    name: &'static str,
    /// Which targets this helper is valid for (empty = all).
    targets: &'static [TargetKind],
    arg_count: usize,
}

const NET_HELPERS: &[HelperDesc] = &[
    HelperDesc {
        name: "has_driver",
        targets: &[TargetKind::Net],
        arg_count: 0,
    },
    HelperDesc {
        name: "has_pullup",
        targets: &[TargetKind::Net],
        arg_count: 0,
    },
    HelperDesc {
        name: "has_pin_kind",
        targets: &[TargetKind::Net],
        arg_count: 1,
    },
    HelperDesc {
        name: "name_matches",
        targets: &[TargetKind::Net],
        arg_count: 1,
    },
    HelperDesc {
        name: "class_is",
        targets: &[TargetKind::Net],
        arg_count: 1,
    },
];

const PIN_HELPERS: &[HelperDesc] = &[HelperDesc {
    name: "is_driven",
    targets: &[TargetKind::Pin],
    arg_count: 0,
}];

const ALL_HELPERS: &[&[HelperDesc]] = &[NET_HELPERS, PIN_HELPERS];

// Known field access patterns: (object, field, valid_targets)
const FIELD_ACCESS: &[(&str, &str, &[TargetKind])] = &[
    ("net", "name", &[TargetKind::Net]),
    ("net", "class", &[TargetKind::Net]),
    ("pin", "kind", &[TargetKind::Pin]),
    ("pin", "required", &[TargetKind::Pin]),
    ("pin", "connected", &[TargetKind::Pin]),
    ("component", "ref_des", &[TargetKind::Component]),
    ("component", "value", &[TargetKind::Component]),
];

// Known method calls: (object, method, valid_targets)
const METHOD_ACCESS: &[(&str, &str, &[TargetKind])] = &[
    ("component", "attr", &[TargetKind::Component]),
    ("sheet", "tagged", &[TargetKind::Sheet]),
];

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn validate(rules: &[RuleAst]) -> Vec<DslError> {
    let mut errors = Vec::new();
    for rule in rules {
        validate_expr(&rule.id, rule.target, &rule.when, &mut errors);
    }
    errors
}

// ---------------------------------------------------------------------------
// Expression traversal
// ---------------------------------------------------------------------------

fn validate_expr(rule_id: &str, target: TargetKind, expr: &ExprAst, out: &mut Vec<DslError>) {
    match expr {
        ExprAst::And(a, b) | ExprAst::Or(a, b) => {
            validate_expr(rule_id, target, a, out);
            validate_expr(rule_id, target, b, out);
        }
        ExprAst::Not(e) => validate_expr(rule_id, target, e, out),
        ExprAst::HelperCall { name, args } => {
            validate_helper(rule_id, target, name, args.len(), out);
        }
        ExprAst::FieldCmp { field, .. } | ExprAst::FieldMatches { field, .. } => {
            validate_field(rule_id, target, field, out);
        }
    }
}

fn validate_helper(
    rule_id: &str,
    target: TargetKind,
    name: &str,
    arg_count: usize,
    out: &mut Vec<DslError>,
) {
    let found = ALL_HELPERS
        .iter()
        .flat_map(|g| g.iter())
        .find(|h| h.name == name);

    let Some(desc) = found else {
        out.push(DslError::UnknownPredicate {
            rule: rule_id.to_string(),
            predicate: name.to_string(),
        });
        return;
    };

    if !desc.targets.is_empty() && !desc.targets.contains(&target) {
        out.push(DslError::InvalidPredicateForTarget {
            rule: rule_id.to_string(),
            predicate: name.to_string(),
            target: target.as_str().to_string(),
        });
    }

    if desc.arg_count != arg_count {
        out.push(DslError::WrongArgCount {
            rule: rule_id.to_string(),
            predicate: name.to_string(),
            expected: desc.arg_count,
            got: arg_count,
        });
    }
}

fn validate_field(
    rule_id: &str,
    target: TargetKind,
    field: &FieldExprAst,
    out: &mut Vec<DslError>,
) {
    match field {
        FieldExprAst::Access { object, field } => {
            let entry = FIELD_ACCESS
                .iter()
                .find(|(o, f, _)| *o == object.as_str() && *f == field.as_str());
            match entry {
                None => out.push(DslError::UnknownField {
                    rule: rule_id.to_string(),
                    object: object.clone(),
                    field: field.clone(),
                }),
                Some((_, _, targets)) if !targets.contains(&target) => {
                    out.push(DslError::InvalidPredicateForTarget {
                        rule: rule_id.to_string(),
                        predicate: format!("{object}.{field}"),
                        target: target.as_str().to_string(),
                    });
                }
                _ => {}
            }
        }
        FieldExprAst::MethodCall { object, method, .. } => {
            let entry = METHOD_ACCESS
                .iter()
                .find(|(o, m, _)| *o == object.as_str() && *m == method.as_str());
            match entry {
                None => out.push(DslError::UnknownField {
                    rule: rule_id.to_string(),
                    object: object.clone(),
                    field: method.clone(),
                }),
                Some((_, _, targets)) if !targets.contains(&target) => {
                    out.push(DslError::InvalidPredicateForTarget {
                        rule: rule_id.to_string(),
                        predicate: format!("{object}.{method}(...)"),
                        target: target.as_str().to_string(),
                    });
                }
                _ => {}
            }
        }
    }
}
