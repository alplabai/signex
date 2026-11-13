//! Validate one parsed rule and compile it into a [`UserRule`]. Errors drop
//! the rule; a warning keeps it parsed but says what is not applied.

use signex_drc::{CopperObject, Limit, NetScope, UserRule};
use signex_types::format::parse_pcb_layer;
use signex_types::violation::Severity;

use pcb_geom::{layer_bit, mm_to_nm};

use crate::ast::*;
use crate::{Diagnostic, Level};

/// Copper layer masks for `layer == outer` / `layer == inner`.
const OUTER: u32 = 1 | 1 << 31;
const INNER: u32 = !OUTER;

/// A clearance over 10 mm is said; one over 100 mm is refused.
const CLEARANCE_WARN_NM: i64 = 10_000_000;
const CLEARANCE_MAX_NM: i64 = 100_000_000;

/// What one rule compiled to.
pub(crate) enum Outcome {
    /// Applied, with any warnings.
    Rule(UserRule, Vec<Diagnostic>),
    /// Valid, but names something v1 does not measure: not applied.
    NotChecked(Vec<Diagnostic>),
    /// Invalid: every error found.
    Dropped(Vec<Diagnostic>),
}

pub(crate) fn compile_rule(ast: &RuleAst) -> Outcome {
    let mut c = Checker {
        rule: &ast.name.text,
        found: Vec::new(),
    };
    let target = c.target(&ast.target);
    let pair = ast.pair.as_ref().and_then(|p| c.pair(p, target));
    let (nets, layers) = c.scope(&ast.when);
    let limits = c.limits(&ast.require, target);
    let errors = c.found.iter().any(|d| d.level == Level::Error);
    let unchecked = matches!(pair, Some(Pair::NotMeasured));
    match (errors, unchecked) {
        (true, _) => Outcome::Dropped(c.found),
        (false, true) => Outcome::NotChecked(c.found),
        (false, false) => Outcome::Rule(
            UserRule {
                name: ast.name.text.clone(),
                severity: match ast.severity {
                    SeverityAst::Error => Severity::Error,
                    SeverityAst::Warning => Severity::Warning,
                    SeverityAst::Info => Severity::Info,
                },
                message: ast.message.clone(),
                nets,
                layers,
                pair: match pair {
                    Some(Pair::Copper(objects)) => Some(objects),
                    _ => None,
                },
                limits,
            },
            c.found,
        ),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Target {
    Clearance,
    Track,
    Via,
}

enum Pair {
    Copper([CopperObject; 2]),
    NotMeasured,
}

struct Checker<'a> {
    rule: &'a str,
    found: Vec<Diagnostic>,
}

impl Checker<'_> {
    fn say(&mut self, level: Level, span: &Span, message: String) {
        self.found.push(Diagnostic {
            level,
            rule: Some(self.rule.to_string()),
            span: span.clone(),
            message,
        });
    }

    fn error(&mut self, span: &Span, message: String) {
        self.say(Level::Error, span, message);
    }

    fn target(&mut self, word: &Word) -> Option<Target> {
        let target = match word.text.as_str() {
            "clearance" => Target::Clearance,
            "track" => Target::Track,
            "via" => Target::Via,
            other => {
                let msg = format!("unknown target '{other}' (clearance, track or via)");
                self.error(&word.span, msg);
                return None;
            }
        };
        Some(target)
    }

    fn pair(&mut self, objects: &[Word; 2], target: Option<Target>) -> Option<Pair> {
        if target.is_some_and(|t| t != Target::Clearance) {
            let msg = "'between' is for a clearance rule only".to_string();
            self.error(&objects[0].span, msg);
            return None;
        }
        let mut copper = Vec::new();
        let mut unmeasured = false;
        for object in objects {
            match object.text.as_str() {
                "track" => copper.push(CopperObject::Track),
                "pad" => copper.push(CopperObject::Pad),
                "via" => copper.push(CopperObject::Via),
                "hole" | "edge" => {
                    unmeasured = true;
                    let msg = format!(
                        "clearance to '{}' is not checked yet; the rule is not applied",
                        object.text
                    );
                    self.say(Level::Warning, &object.span, msg);
                }
                other => {
                    let msg = format!("unknown object '{other}' (track, pad, via, hole or edge)");
                    self.error(&object.span, msg);
                }
            }
        }
        match copper.as_slice() {
            _ if unmeasured => Some(Pair::NotMeasured),
            &[a, b] => Some(Pair::Copper([a, b])),
            _ => None,
        }
    }

    fn scope(&mut self, conds: &[CondAst]) -> (NetScope, Option<u32>) {
        let mut nets: Option<NetScope> = None;
        let mut layers: Option<u32> = None;
        for cond in conds {
            let subject = cond.subject.text.as_str();
            let value = cond.value.text.clone();
            let scope = match (subject, cond.op) {
                ("net", CondOp::Is) => NetScope::Net(value),
                ("net", CondOp::Matches) => NetScope::Pattern(value),
                ("class", CondOp::Is) => NetScope::Class(value),
                ("layer", CondOp::Is) => {
                    if layers.is_some() {
                        let msg = "a rule takes one layer condition".to_string();
                        self.error(&cond.subject.span, msg);
                    }
                    layers = self.layer(&cond.value).or(layers);
                    continue;
                }
                ("class" | "layer", CondOp::Matches) => {
                    let msg = format!("'{subject}' takes '==', not 'matches'");
                    self.error(&cond.subject.span, msg);
                    continue;
                }
                ("region", _) => {
                    let msg = "a region scope is not supported yet".to_string();
                    self.error(&cond.subject.span, msg);
                    continue;
                }
                (other, _) => {
                    let msg = format!("unknown condition '{other}' (net, class or layer)");
                    self.error(&cond.subject.span, msg);
                    continue;
                }
            };
            if nets.is_some() {
                let msg = "a rule takes one net, class or net pattern condition".to_string();
                self.error(&cond.subject.span, msg);
            }
            nets = Some(scope);
        }
        (nets.unwrap_or(NetScope::Board), layers)
    }

    fn layer(&mut self, value: &Word) -> Option<u32> {
        match value.text.as_str() {
            "outer" => return Some(OUTER),
            "inner" => return Some(INNER),
            _ => {}
        }
        let copper = parse_pcb_layer(&value.text).and_then(|l| l.copper());
        if copper.is_none() {
            let msg = format!(
                "'{}' is not a copper layer (inner, outer, or a name such as TopCopper)",
                value.text
            );
            self.error(&value.span, msg);
        }
        copper.map(|layer| 1 << layer_bit(layer))
    }

    fn limits(&mut self, limits: &[LimitAst], target: Option<Target>) -> Vec<Limit> {
        let mut out: Vec<Limit> = Vec::new();
        for ast in limits {
            let Some(nm) = self.length(ast) else {
                continue;
            };
            let Some(target) = target else {
                continue;
            };
            let limit = match (target, ast.field.text.as_str(), ast.bound) {
                (Target::Clearance, "clearance", Bound::AtLeast) => Limit::MinClearance(nm),
                (Target::Track, "width", Bound::AtLeast) => Limit::MinTrackWidth(nm),
                (Target::Track, "width", Bound::AtMost) => Limit::MaxTrackWidth(nm),
                (Target::Via, "diameter", Bound::AtLeast) => Limit::MinViaDiameter(nm),
                (Target::Via, "drill", Bound::AtLeast) => Limit::MinViaDrill(nm),
                (Target::Clearance, "clearance", _) | (Target::Via, "diameter" | "drill", _) => {
                    let msg = format!("'{}' takes only '>=' (a minimum)", ast.field.text);
                    self.error(&ast.field.span, msg);
                    continue;
                }
                (_, field, _) => {
                    let fields = match target {
                        Target::Clearance => "clearance",
                        Target::Track => "width",
                        Target::Via => "diameter or drill",
                    };
                    let msg = format!("unknown field '{field}' for this target ({fields})");
                    self.error(&ast.field.span, msg);
                    continue;
                }
            };
            let twice = out
                .iter()
                .any(|l| std::mem::discriminant(l) == std::mem::discriminant(&limit));
            if twice {
                let msg = format!("'{}' is limited twice the same way", ast.field.text);
                self.error(&ast.field.span, msg);
                continue;
            }
            if !self.plausible(ast, &limit, nm) {
                continue;
            }
            out.push(limit);
        }
        out
    }

    /// Says a limit of 0 (a minimum of 0 checks nothing, a maximum of 0
    /// reports every track) and a clearance so large it is likely a wrong
    /// unit: over 10 mm a warning, over 100 mm an error (a clearance widens
    /// the search around every copper item it covers). False when the limit
    /// is refused.
    fn plausible(&mut self, ast: &LimitAst, limit: &Limit, nm: i64) -> bool {
        if nm == 0 {
            let msg = match limit {
                Limit::MaxTrackWidth(_) => {
                    "a maximum of 0 reports every track in this rule's scope"
                }
                _ => "a limit of 0 checks nothing for this rule's scope",
            };
            self.say(Level::Warning, &ast.number.span, msg.to_string());
        }
        if !matches!(limit, Limit::MinClearance(_)) || nm <= CLEARANCE_WARN_NM {
            return true;
        }
        let mm = pcb_geom::nm_to_mm(nm);
        if nm > CLEARANCE_MAX_NM {
            let msg = format!("a clearance of {mm} mm is over 100 mm; check the unit");
            self.error(&ast.number.span, msg);
            return false;
        }
        let msg = format!("a clearance of {mm} mm is over 10 mm; check the unit");
        self.say(Level::Warning, &ast.number.span, msg);
        true
    }

    /// The limit's length in nm.
    fn length(&mut self, ast: &LimitAst) -> Option<i64> {
        let Some(unit) = &ast.unit else {
            let msg = format!("'{}' needs a unit (mm, mil, um or nm)", ast.number.text);
            self.error(&ast.number.span, msg);
            return None;
        };
        let mm_per_unit = match unit.text.as_str() {
            "mm" => 1.0,
            "mil" => 0.0254,
            "um" | "\u{b5}m" | "\u{3bc}m" => 1e-3,
            "nm" => 1e-6,
            other => {
                let msg = format!("unknown unit '{other}' (mm, mil, um or nm)");
                self.error(&unit.span, msg);
                return None;
            }
        };
        let value: f64 = ast.number.text.parse().ok()?;
        match mm_to_nm(value * mm_per_unit, "rule limit") {
            Ok(nm) => Some(nm),
            Err(e) => {
                self.error(&ast.number.span, format!("not a usable length: {e}"));
                None
            }
        }
    }
}
