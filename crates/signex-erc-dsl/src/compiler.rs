//! Rule compiler: `RuleAst` -> `CompiledRule` with pre-compiled regex patterns.

use std::sync::Arc;

use regex::Regex;
use signex_erc::engine::EvalFn;
use signex_erc::{
    AnalysisScope, Applicability, Diagnostic, ErcContext, RuleDefinition, RuleId, RuleKind,
    RuleTarget, Severity,
};
use signex_types::schematic::{PinElectricalType, Point, SelectedItem, SelectedKind};

use crate::ast::*;
use crate::error::DslError;

/// A compiled helper call. For regex-capable helpers, regexes are compiled once.
#[derive(Debug, Clone)]
pub struct CompiledHelper {
    name: String,
    args: Vec<LiteralAst>,
    regex_arg: Option<Regex>,
}

/// Expression after compilation/preprocessing.
#[derive(Debug, Clone)]
pub enum CompiledExpr {
    And(Box<CompiledExpr>, Box<CompiledExpr>),
    Or(Box<CompiledExpr>, Box<CompiledExpr>),
    Not(Box<CompiledExpr>),
    HelperCall(CompiledHelper),
    FieldCmp {
        field: FieldExprAst,
        op: CmpOp,
        value: LiteralAst,
    },
    FieldMatches {
        field: FieldExprAst,
        pattern: String,
        regex: Regex,
    },
}

/// A fully compiled DSL rule.
#[derive(Clone)]
pub struct CompiledRule {
    pub definition: RuleDefinition,
    pub message: String,
    pub expr: CompiledExpr,
    pub eval: EvalFn,
}

impl CompiledRule {
    /// Clones the evaluator closure for use with `engine::run_all_with_dsl`.
    pub fn eval_fn(&self) -> EvalFn {
        self.eval.clone()
    }
}

/// Compile all rules. Continues compiling independent rules and returns all
/// compile-time errors (e.g. invalid regexes) together.
pub fn compile(rules: &[RuleAst]) -> Result<Vec<CompiledRule>, Vec<DslError>> {
    let mut out = Vec::with_capacity(rules.len());
    let mut errors = Vec::new();

    for rule in rules {
        match compile_rule(rule) {
            Ok(compiled) => out.push(compiled),
            Err(err) => errors.push(err),
        }
    }

    if errors.is_empty() { Ok(out) } else { Err(errors) }
}

/// Convert compiled rules into engine evaluator closures.
pub fn to_eval_fns(rules: &[CompiledRule]) -> Vec<EvalFn> {
    rules.iter().map(CompiledRule::eval_fn).collect()
}

fn compile_rule(rule: &RuleAst) -> Result<CompiledRule, DslError> {
    let expr = compile_expr(&rule.id, &rule.when)?;
    let definition = RuleDefinition {
        id: RuleId::user(&rule.id),
        name: rule.id.clone(),
        description: rule.message.clone(),
        target: map_target(rule.target),
        scope: map_scope(rule.scope),
        applicability: map_applicability(&rule.applicability),
        default_severity: map_severity(rule.severity),
    };

    let compiled_message = rule.message.clone();
    let compiled_expr = expr.clone();
    let severity = map_severity(rule.severity);
    let rule_id = definition.id.clone();
    let rule_kind = fallback_kind(rule.target);
    let target = rule.target;

    let eval: EvalFn = Arc::new(move |ctx: &ErcContext| {
        evaluate_rule(
            ctx,
            target,
            &compiled_expr,
            &compiled_message,
            severity,
            &rule_id,
            rule_kind,
        )
    });

    Ok(CompiledRule {
        definition,
        message: rule.message.clone(),
        expr,
        eval,
    })
}

fn compile_expr(rule_id: &str, expr: &ExprAst) -> Result<CompiledExpr, DslError> {
    match expr {
        ExprAst::And(a, b) => Ok(CompiledExpr::And(
            Box::new(compile_expr(rule_id, a)?),
            Box::new(compile_expr(rule_id, b)?),
        )),
        ExprAst::Or(a, b) => Ok(CompiledExpr::Or(
            Box::new(compile_expr(rule_id, a)?),
            Box::new(compile_expr(rule_id, b)?),
        )),
        ExprAst::Not(e) => Ok(CompiledExpr::Not(Box::new(compile_expr(rule_id, e)?))),
        ExprAst::HelperCall { name, args } => {
            let regex_arg = if name == "name_matches" {
                let pattern = args.first().map(LiteralAst::as_str_value).unwrap_or_default();
                Some(Regex::new(pattern).map_err(|source| DslError::InvalidRegex {
                    rule: rule_id.to_string(),
                    pattern: pattern.to_string(),
                    source,
                })?)
            } else {
                None
            };
            Ok(CompiledExpr::HelperCall(CompiledHelper {
                name: name.clone(),
                args: args.clone(),
                regex_arg,
            }))
        }
        ExprAst::FieldCmp { field, op, value } => Ok(CompiledExpr::FieldCmp {
            field: field.clone(),
            op: *op,
            value: value.clone(),
        }),
        ExprAst::FieldMatches { field, pattern } => {
            let regex = Regex::new(pattern).map_err(|source| DslError::InvalidRegex {
                rule: rule_id.to_string(),
                pattern: pattern.clone(),
                source,
            })?;
            Ok(CompiledExpr::FieldMatches {
                field: field.clone(),
                pattern: pattern.clone(),
                regex,
            })
        }
    }
}

enum Subject<'a> {
    Net(&'a signex_erc::context::ErcNet),
    Pin(&'a signex_erc::context::ErcPin),
    Component(&'a signex_erc::context::ErcSymbol),
    Sheet(&'a signex_erc::context::ErcChildSheet),
}

fn evaluate_rule(
    ctx: &ErcContext,
    target: TargetKind,
    expr: &CompiledExpr,
    message: &str,
    severity: Severity,
    rule_id: &RuleId,
    rule_kind: RuleKind,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    match target {
        TargetKind::Net => {
            for net in &ctx.nets {
                if eval_expr(expr, Subject::Net(net)) {
                    out.push(Diagnostic {
                        rule_id: rule_id.clone(),
                        rule_kind,
                        severity,
                        message: message.to_string(),
                        location: Point::new(0.0, 0.0),
                        primary: None,
                        peer: None,
                    });
                }
            }
        }
        TargetKind::Pin => {
            for symbol in &ctx.symbols {
                for pin in &symbol.pins {
                    if eval_expr(expr, Subject::Pin(pin)) {
                        out.push(Diagnostic {
                            rule_id: rule_id.clone(),
                            rule_kind,
                            severity,
                            message: message.to_string(),
                            location: pin.world_pos,
                            primary: Some(SelectedItem::new(symbol.uuid, SelectedKind::Symbol)),
                            peer: None,
                        });
                    }
                }
            }
        }
        TargetKind::Component => {
            for symbol in &ctx.symbols {
                if eval_expr(expr, Subject::Component(symbol)) {
                    out.push(Diagnostic {
                        rule_id: rule_id.clone(),
                        rule_kind,
                        severity,
                        message: message.to_string(),
                        location: symbol.position,
                        primary: Some(SelectedItem::new(symbol.uuid, SelectedKind::Symbol)),
                        peer: None,
                    });
                }
            }
        }
        TargetKind::Sheet => {
            for sheet in &ctx.child_sheets {
                if eval_expr(expr, Subject::Sheet(sheet)) {
                    out.push(Diagnostic {
                        rule_id: rule_id.clone(),
                        rule_kind,
                        severity,
                        message: message.to_string(),
                        location: sheet.position,
                        primary: Some(SelectedItem::new(sheet.uuid, SelectedKind::ChildSheet)),
                        peer: None,
                    });
                }
            }
        }
    }

    out
}

fn eval_expr(expr: &CompiledExpr, subject: Subject<'_>) -> bool {
    match expr {
        CompiledExpr::And(a, b) => eval_expr(a, subject_ref(&subject)) && eval_expr(b, subject),
        CompiledExpr::Or(a, b) => eval_expr(a, subject_ref(&subject)) || eval_expr(b, subject),
        CompiledExpr::Not(e) => !eval_expr(e, subject),
        CompiledExpr::HelperCall(helper) => eval_helper(helper, subject),
        CompiledExpr::FieldCmp { field, op, value } => eval_field_cmp(field, *op, value, subject),
        CompiledExpr::FieldMatches { field, regex, .. } => eval_field_matches(field, regex, subject),
    }
}

fn subject_ref<'a>(subject: &'a Subject<'a>) -> Subject<'a> {
    match subject {
        Subject::Net(n) => Subject::Net(n),
        Subject::Pin(p) => Subject::Pin(p),
        Subject::Component(c) => Subject::Component(c),
        Subject::Sheet(s) => Subject::Sheet(s),
    }
}

fn eval_helper(helper: &CompiledHelper, subject: Subject<'_>) -> bool {
    match (&*helper.name, subject) {
        ("has_driver", Subject::Net(net)) => net.has_driver,
        ("has_pullup", Subject::Net(net)) => net.has_pullup,
        ("has_pin_kind", Subject::Net(net)) => helper
            .args
            .first()
            .and_then(|a| parse_pin_type(a.as_str_value()))
            .map(|t| net.pin_types.iter().any(|x| *x == t))
            .unwrap_or(false),
        ("name_matches", Subject::Net(net)) => {
            if let Some(regex) = &helper.regex_arg {
                regex.is_match(&net.name)
            } else {
                false
            }
        }
        ("class_is", Subject::Net(net)) => helper
            .args
            .first()
            .map(|a| normalize(a.as_str_value()) == normalize(&net.class))
            .unwrap_or(false),
        ("is_driven", Subject::Pin(pin)) => is_driving_pin(pin.electrical_type),
        _ => false,
    }
}

enum Value {
    Str(String),
    Bool(bool),
}

fn eval_field_cmp(field: &FieldExprAst, op: CmpOp, value: &LiteralAst, subject: Subject<'_>) -> bool {
    let Some(lhs) = resolve_field(field, subject) else {
        return false;
    };

    match (lhs, value) {
        (Value::Bool(a), LiteralAst::Bool(b)) => match op {
            CmpOp::Eq => a == *b,
            CmpOp::Ne => a != *b,
        },
        (Value::Str(a), rhs) => {
            let b = rhs.as_str_value().to_string();
            match op {
                CmpOp::Eq => normalize(&a) == normalize(&b),
                CmpOp::Ne => normalize(&a) != normalize(&b),
            }
        }
        (Value::Bool(a), rhs) => {
            let b = normalize(rhs.as_str_value()) == "true";
            match op {
                CmpOp::Eq => a == b,
                CmpOp::Ne => a != b,
            }
        }
    }
}

fn eval_field_matches(field: &FieldExprAst, regex: &Regex, subject: Subject<'_>) -> bool {
    let Some(lhs) = resolve_field(field, subject) else {
        return false;
    };
    match lhs {
        Value::Str(s) => regex.is_match(&s),
        Value::Bool(_) => false,
    }
}

fn resolve_field(field: &FieldExprAst, subject: Subject<'_>) -> Option<Value> {
    match (field, subject) {
        (FieldExprAst::Access { object, field }, Subject::Net(net)) if object == "net" => {
            match field.as_str() {
                "name" => Some(Value::Str(net.name.clone())),
                "class" => Some(Value::Str(net.class.clone())),
                _ => None,
            }
        }
        (FieldExprAst::Access { object, field }, Subject::Pin(pin)) if object == "pin" => {
            match field.as_str() {
                "kind" => Some(Value::Str(pin_type_name(pin.electrical_type).to_string())),
                "required" => Some(Value::Bool(pin.required)),
                "connected" => Some(Value::Bool(pin.connected)),
                _ => None,
            }
        }
        (FieldExprAst::Access { object, field }, Subject::Component(symbol)) if object == "component" => {
            match field.as_str() {
                "ref_des" => Some(Value::Str(symbol.reference.clone())),
                "value" => Some(Value::Str(symbol.value.clone())),
                _ => None,
            }
        }
        (FieldExprAst::MethodCall { object, method, args }, Subject::Component(symbol))
            if object == "component" && method == "attr" =>
        {
            let key = args.first()?.as_str_value();
            Some(Value::Str(symbol.attrs.get(key).cloned().unwrap_or_default()))
        }
        (FieldExprAst::MethodCall { object, method, .. }, Subject::Sheet(_))
            if object == "sheet" && method == "tagged" =>
        {
            // Tag information is not projected into `ErcContext` yet.
            Some(Value::Bool(false))
        }
        _ => None,
    }
}

fn map_target(target: TargetKind) -> RuleTarget {
    match target {
        TargetKind::Net => RuleTarget::Net,
        TargetKind::Pin => RuleTarget::Pin,
        TargetKind::Component => RuleTarget::Component,
        TargetKind::Sheet => RuleTarget::Sheet,
    }
}

fn map_scope(scope: ScopeKind) -> AnalysisScope {
    match scope {
        ScopeKind::Local => AnalysisScope::Local,
        ScopeKind::Sheet => AnalysisScope::Sheet,
        ScopeKind::Hierarchical => AnalysisScope::Hierarchical,
        ScopeKind::Global => AnalysisScope::Global,
    }
}

fn map_applicability(app: &ApplicabilityAst) -> Applicability {
    match app {
        ApplicabilityAst::All => Applicability::All,
        ApplicabilityAst::TaggedSheets(v) => Applicability::TaggedSheets(v.clone()),
        ApplicabilityAst::NamedSheets(v) => Applicability::ExactSheets(v.clone()),
    }
}

fn map_severity(severity: SeverityKind) -> Severity {
    match severity {
        SeverityKind::Error => Severity::Error,
        SeverityKind::Warning => Severity::Warning,
        SeverityKind::Info => Severity::Info,
    }
}

fn fallback_kind(target: TargetKind) -> RuleKind {
    match target {
        TargetKind::Net => RuleKind::NetLabelConflict,
        TargetKind::Pin => RuleKind::UnusedPin,
        TargetKind::Component => RuleKind::DuplicateRefDesignator,
        TargetKind::Sheet => RuleKind::BadHierSheetPin,
    }
}

fn is_driving_pin(t: PinElectricalType) -> bool {
    matches!(
        t,
        PinElectricalType::Output
            | PinElectricalType::PowerOut
            | PinElectricalType::TriState
            | PinElectricalType::OpenCollector
            | PinElectricalType::OpenEmitter
    )
}

fn parse_pin_type(s: &str) -> Option<PinElectricalType> {
    match normalize(s).as_str() {
        "input" => Some(PinElectricalType::Input),
        "output" => Some(PinElectricalType::Output),
        "bidirectional" => Some(PinElectricalType::Bidirectional),
        "tristate" => Some(PinElectricalType::TriState),
        "passive" => Some(PinElectricalType::Passive),
        "free" => Some(PinElectricalType::Free),
        "unspecified" => Some(PinElectricalType::Unspecified),
        "powerin" => Some(PinElectricalType::PowerIn),
        "powerout" => Some(PinElectricalType::PowerOut),
        "opencollector" => Some(PinElectricalType::OpenCollector),
        "openemitter" => Some(PinElectricalType::OpenEmitter),
        "notconnected" => Some(PinElectricalType::NotConnected),
        _ => None,
    }
}

fn pin_type_name(t: PinElectricalType) -> &'static str {
    match t {
        PinElectricalType::Input => "Input",
        PinElectricalType::Output => "Output",
        PinElectricalType::Bidirectional => "Bidirectional",
        PinElectricalType::TriState => "TriState",
        PinElectricalType::Passive => "Passive",
        PinElectricalType::Free => "Free",
        PinElectricalType::Unspecified => "Unspecified",
        PinElectricalType::PowerIn => "PowerIn",
        PinElectricalType::PowerOut => "PowerOut",
        PinElectricalType::OpenCollector => "OpenCollector",
        PinElectricalType::OpenEmitter => "OpenEmitter",
        PinElectricalType::NotConnected => "NotConnected",
    }
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '_' && *c != '-' && !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}