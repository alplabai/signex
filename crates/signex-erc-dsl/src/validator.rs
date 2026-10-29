//! AST validator: checks predicate whitelist and target compatibility.

use crate::ast::*;
use crate::compiler::{is_pin_kind_field, resolve_pin_kind};
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
    ("net", "names", &[TargetKind::Net]),
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
    let mut seen = std::collections::HashSet::new();
    for rule in rules {
        if !seen.insert(rule.id.as_str()) {
            errors.push(DslError::DuplicateRuleId {
                rule: rule.id.clone(),
                start: rule.span.start,
                end: rule.span.end,
            });
        }
        errors.extend(unsupported_features(rule));
        validate_expr(&rule.id, rule.target, &rule.when, &mut errors);
    }
    errors
}

/// Constructs the parser accepts but nothing enforces yet (#835):
/// `apply_to` (no sheet name or tags reach `ErcContext`), any `scope` other
/// than the per-sheet run the engine actually does, and `sheet.tagged()`
/// (always false). Shared with the compiler so `compile()` alone rejects
/// them too.
pub(crate) fn unsupported_features(rule: &RuleAst) -> Vec<DslError> {
    let mut features = Vec::new();
    if !matches!(rule.applicability, ApplicabilityAst::All) {
        features.push("`apply_to`".to_owned());
    }
    let scope = match rule.scope {
        ScopeKind::Sheet => None,
        ScopeKind::Local => Some("local"),
        ScopeKind::Hierarchical => Some("hierarchical"),
        ScopeKind::Global => Some("global"),
    };
    if let Some(scope) = scope {
        features.push(format!("`scope {scope}`"));
    }
    if uses_sheet_tagged(&rule.when) {
        features.push("`sheet.tagged(...)`".to_owned());
    }
    features
        .into_iter()
        .map(|feature| DslError::Unsupported {
            rule: rule.id.clone(),
            feature,
        })
        .collect()
}

fn uses_sheet_tagged(expr: &ExprAst) -> bool {
    let is_tagged = |field: &FieldExprAst| {
        matches!(field, FieldExprAst::MethodCall { object, method, .. }
            if object == "sheet" && method == "tagged")
    };
    match expr {
        ExprAst::And(a, b) | ExprAst::Or(a, b) => uses_sheet_tagged(a) || uses_sheet_tagged(b),
        ExprAst::Not(e) => uses_sheet_tagged(e),
        ExprAst::HelperCall { .. } => false,
        ExprAst::FieldCmp { field, .. } | ExprAst::FieldMatches { field, .. } => is_tagged(field),
    }
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
            if name == "has_pin_kind"
                && let Some(kind) = args.first()
            {
                validate_pin_kind(rule_id, kind, out);
            }
        }
        ExprAst::FieldCmp { field, value, .. } => {
            validate_field(rule_id, target, field, out);
            if is_pin_kind_field(field) {
                validate_pin_kind(rule_id, value, out);
            }
        }
        ExprAst::FieldMatches { field, .. } => validate_field(rule_id, target, field, out),
    }
}

/// A pin kind no pin can have would make the rule silently never fire.
fn validate_pin_kind(rule_id: &str, kind: &LiteralAst, out: &mut Vec<DslError>) {
    if let Err(error) = resolve_pin_kind(rule_id, kind) {
        out.push(error);
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

#[cfg(test)]
mod tests {
    use crate::{compile, parse, parse_validate_compile};

    fn errors_of(src: &str) -> Vec<String> {
        match parse_validate_compile(src) {
            Ok(_) => Vec::new(),
            Err(errors) => errors.iter().map(ToString::to_string).collect(),
        }
    }

    /// #835: scoping the engine cannot enforce is rejected, not ignored.
    #[test]
    fn unenforced_scoping_is_rejected() {
        for (src, feature) in [
            (
                r#"rule r apply_to sheets tagged "power" on net when has_driver() then error "x""#,
                "`apply_to`",
            ),
            (
                r#"rule r apply_to sheets named "top" on net when has_driver() then error "x""#,
                "`apply_to`",
            ),
            (
                r#"rule r on net scope global when has_driver() then error "x""#,
                "`scope global`",
            ),
            (
                r#"rule r on net scope hierarchical when has_driver() then error "x""#,
                "`scope hierarchical`",
            ),
            (
                r#"rule r on net scope local when has_driver() then error "x""#,
                "`scope local`",
            ),
            (
                r#"rule r on sheet when not sheet.tagged("a") == true then error "x""#,
                "`sheet.tagged(...)`",
            ),
        ] {
            let errors = errors_of(src);
            assert!(
                errors
                    .iter()
                    .any(|e| e.contains(feature) && e.contains("not supported yet")),
                "{src}: {errors:?}"
            );
            // `compile()` alone, without the validator, rejects it too.
            assert!(compile(&parse(src).unwrap()).is_err(), "{src}");
        }
    }

    /// #1364: two rules sharing an id are rejected, pointing at the second.
    #[test]
    fn duplicate_rule_id_is_rejected_at_second_definition() {
        let src = "rule dup on net when has_driver() then error \"a\"\nrule dup on net when has_pullup() then error \"b\"";
        let Err(errors) = parse_validate_compile(src) else {
            panic!("duplicate ids must be rejected");
        };
        let second = src.rfind("dup").unwrap();
        assert!(
            errors.iter().any(|e| matches!(e,
                crate::DslError::DuplicateRuleId { rule, start, end }
                    if rule == "dup" && *start == second && *end == second + 3)),
            "{errors:?}"
        );
        assert_eq!(errors.len(), 1, "{errors:?}");
    }

    /// #1364: the parse error carries the real span end, not start twice.
    #[test]
    fn parse_error_span_end_is_real() {
        let errors = parse("rule r on bogus when has_driver() then error \"x\"").unwrap_err();
        let any_wide = errors
            .iter()
            .any(|e| matches!(e, crate::DslError::Parse(s, en, _) if en > s));
        assert!(any_wide, "{errors:?}");
    }

    /// The default and an explicit `scope sheet` are what the engine does.
    #[test]
    fn per_sheet_scope_still_compiles() {
        for src in [
            r#"rule r on net when has_driver() then error "x""#,
            r#"rule r on net scope sheet when has_driver() then error "x""#,
        ] {
            assert!(errors_of(src).is_empty(), "{src}: {:?}", errors_of(src));
        }
    }
}
