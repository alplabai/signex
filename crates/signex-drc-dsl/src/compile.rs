//! Validate one parsed rule and compile it into a [`UserRule`]. Errors drop
//! the rule; a warning keeps it parsed but says what is not applied.

use signex_drc::{CopperObject, Limit, LimitKind, NetScope, UserRule};
use signex_types::format::parse_pcb_layer;
use signex_types::violation::Severity;

use pcb_geom::{layer_bit, mm_to_nm};

use crate::ast::*;
use crate::table::{LimitField, RuleKind};
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
    let (nets, layers) = c.scope(&ast.when, target);
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

    fn target(&mut self, word: &Word) -> Option<RuleKind> {
        let target = RuleKind::from_keyword(&word.text);
        if target.is_none() {
            let known: Vec<&str> = RuleKind::ALL.iter().map(|k| k.keyword()).collect();
            let msg = format!("unknown target '{}' ({})", word.text, or_list(&known));
            self.error(&word.span, msg);
        }
        target
    }

    fn pair(&mut self, objects: &[Word; 2], target: Option<RuleKind>) -> Option<Pair> {
        if target.is_some_and(|t| t != RuleKind::Clearance) {
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
                    // Hole to hole and copper to the board edge have their
                    // own targets: `on hole require spacing`, `on edge`.
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

    fn scope(&mut self, conds: &[CondAst], target: Option<RuleKind>) -> (NetScope, Option<u32>) {
        // A component rule's one narrowing is the side its parts sit on.
        // Only a side is a side: `inner`, `outer` or an inner layer would
        // compile and never match (#2986).
        if let Some(kind) = target.filter(|k| k.takes_side()) {
            let not_a_side = |c: &&CondAst| {
                c.subject.text != "layer"
                    || !matches!(c.value.text.as_str(), "TopCopper" | "BottomCopper")
            };
            if let Some(cond) = conds.iter().find(not_a_side) {
                let msg = format!(
                    "a {} rule takes only a 'layer' condition (TopCopper or BottomCopper)",
                    kind.keyword()
                );
                self.error(&cond.subject.span, msg);
                return (NetScope::Board, None);
            }
        }
        if let Some(kind) = target.filter(|k| !k.is_scoped() && !k.takes_side())
            && let Some(cond) = conds.first()
        {
            let msg = format!(
                "a {} rule is board-wide and takes no 'when' condition",
                kind.keyword()
            );
            self.error(&cond.subject.span, msg);
            return (NetScope::Board, None);
        }
        let mut nets: Option<NetScope> = None;
        let mut layers: Option<u32> = None;
        let mut other: Option<&CondAst> = None;
        for cond in conds {
            let subject = cond.subject.text.as_str();
            let value = cond.value.text.clone();
            let scope = match (subject, cond.op) {
                ("net", CondOp::Is) => NetScope::Net(value),
                ("net", CondOp::Matches) => NetScope::Pattern(value),
                ("class", CondOp::Is) => NetScope::Class(value),
                ("other_class", CondOp::Is) => {
                    if other.is_some() {
                        let msg = "a rule takes one other_class condition".to_string();
                        self.error(&cond.subject.span, msg);
                    }
                    other = Some(cond);
                    continue;
                }
                ("layer", CondOp::Is) => {
                    if layers.is_some() {
                        let msg = "a rule takes one layer condition".to_string();
                        self.error(&cond.subject.span, msg);
                    }
                    layers = self.layer(&cond.value).or(layers);
                    continue;
                }
                ("class" | "other_class" | "layer", CondOp::Matches) => {
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
                    let msg =
                        format!("unknown condition '{other}' (net, class, other_class or layer)");
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
        let nets = nets.unwrap_or(NetScope::Board);
        let Some(other) = other else {
            return (nets, layers);
        };
        // `class == "A" and other_class == "B"`: a clearance between the
        // nets of two classes (#2398).
        if target.is_some_and(|t| t != RuleKind::Clearance) {
            let msg = "'other_class' is for a clearance rule only".to_string();
            self.error(&other.subject.span, msg);
            return (nets, layers);
        }
        match nets {
            NetScope::Class(class) => {
                (NetScope::ClassPair(class, other.value.text.clone()), layers)
            }
            nets => {
                let msg = "'other_class' needs a 'class' condition".to_string();
                self.error(&other.subject.span, msg);
                (nets, layers)
            }
        }
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

    fn limits(&mut self, limits: &[LimitAst], target: Option<RuleKind>) -> Vec<Limit> {
        let mut out: Vec<Limit> = Vec::new();
        let mut kept: Vec<(&LimitAst, i64)> = Vec::new();
        for ast in limits {
            let Some(nm) = self.length(ast) else {
                continue;
            };
            let Some(target) = target else {
                continue;
            };
            let word = ast.field.text.as_str();
            let Some(field) = LimitField::of(target, word, ast.bound) else {
                let other = match ast.bound {
                    Bound::AtLeast => Bound::AtMost,
                    Bound::AtMost => Bound::AtLeast,
                };
                let msg = match LimitField::of(target, word, other) {
                    Some(f) if f.bound() == Bound::AtLeast => {
                        format!("'{word}' takes only '>=' (a minimum)")
                    }
                    Some(_) => format!("'{word}' takes only '<=' (a maximum)"),
                    None => format!(
                        "unknown field '{word}' for this target ({})",
                        LimitField::words_of(target)
                    ),
                };
                self.error(&ast.field.span, msg);
                continue;
            };
            let limit = Limit::of(field.limit_kind(), nm);
            if out.iter().any(|l| l.kind() == limit.kind()) {
                let msg = format!("'{}' is limited twice the same way", ast.field.text);
                self.error(&ast.field.span, msg);
                continue;
            }
            if !self.plausible(ast, &limit, nm) {
                continue;
            }
            out.push(limit);
            kept.push((ast, nm));
        }
        self.contradictions(&kept);
        out
    }

    /// Says a minimum above a maximum of the same field: no item can meet
    /// both, so the rule is refused (#2335).
    fn contradictions(&mut self, kept: &[(&LimitAst, i64)]) {
        for &(min, min_nm) in kept.iter().filter(|(a, _)| a.bound == Bound::AtLeast) {
            let above = kept.iter().find(|(max, max_nm)| {
                max.bound == Bound::AtMost && max.field.text == min.field.text && *max_nm < min_nm
            });
            if let Some(&(max, _)) = above {
                let msg = format!("{} is above {}", written(min), written(max));
                self.error(&min.field.span, msg);
            }
        }
    }

    /// Says a limit of 0 (a minimum of 0 checks nothing, unless touching
    /// already breaks it; a track or hole maximum of 0 reports every one; a
    /// maximum expansion of 0 is a real limit) and a clearance so large it
    /// is likely a wrong unit: over 10 mm a warning, over 100 mm an error
    /// (a clearance widens the search around every copper item it covers).
    /// False when the limit is refused.
    fn plausible(&mut self, ast: &LimitAst, limit: &Limit, nm: i64) -> bool {
        let kind = limit.kind();
        if nm == 0 && !kind.zero_still_checks() {
            let msg = match kind {
                LimitKind::MaxTrackWidth => {
                    Some("a maximum of 0 reports every track in this rule's scope")
                }
                LimitKind::MaxHoleSize => {
                    Some("a maximum of 0 reports every hole in this rule's scope")
                }
                _ if kind.is_maximum() => None,
                _ => Some("a limit of 0 checks nothing for this rule's scope"),
            };
            if let Some(msg) = msg {
                self.say(Level::Warning, &ast.number.span, msg.to_string());
            }
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
        let Some(mm_per_unit) = mm_per_unit(&unit.text) else {
            let msg = format!("unknown unit '{}' (mm, mil, um or nm)", unit.text);
            self.error(&unit.span, msg);
            return None;
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

/// Millimetres per length unit of the rule text; `None` for a word that
/// is not one.
fn mm_per_unit(unit: &str) -> Option<f64> {
    match unit {
        "mm" => Some(1.0),
        "mil" => Some(0.0254),
        "um" | "\u{b5}m" | "\u{3bc}m" => Some(1e-3),
        "nm" => Some(1e-6),
        _ => None,
    }
}

/// A length as the rule text writes it (`0.2mm`, `8 mil`, `50µm`) in nm,
/// read with the compiler's units. `None` for a bare number or text that
/// is not a length.
pub fn length_nm(text: &str) -> Option<i64> {
    let text = text.trim();
    let unit_at = text.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let (number, rest) = text.split_at(unit_at);
    let unit = rest.strip_prefix(' ').unwrap_or(rest);
    let value: f64 = number.parse().ok()?;
    mm_to_nm(value * mm_per_unit(unit)?, "rule limit").ok()
}

/// A limit as written: `width >= 0.3mm`.
fn written(ast: &LimitAst) -> String {
    let op = match ast.bound {
        Bound::AtLeast => ">=",
        Bound::AtMost => "<=",
    };
    let unit = ast.unit.as_ref().map_or("", |u| u.text.as_str());
    format!("{} {op} {}{unit}", ast.field.text, ast.number.text)
}

/// `a, b or c`.
fn or_list(words: &[&str]) -> String {
    match words.split_last() {
        Some((last, [])) => (*last).to_string(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => String::new(),
    }
}
