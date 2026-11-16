//! Edits of one rule in the text (#2398). Each returns the whole new text:
//! the edited rule's span rewritten (or a rule appended, or one removed),
//! every other byte as it was. An edit that would make the rule fail to
//! compile, or the text fail to parse, is refused with the reason.

use std::fmt;

use signex_drc::NetScope;
use signex_types::violation::Severity;

use super::{LimitField, RowLimit, RuleDraft, RuleKind, RuleRow, RuleTable, render, table};
use crate::Level;
use crate::ast::Span;

/// The keyword that keeps a rule written but not applied.
const DISABLED: &str = "disabled";

/// One editable field of a rule: a table column and a `rules_set` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleField {
    Enabled,
    Name,
    Scope,
    Layer,
    Pair,
    Severity,
    Message,
    Limit(LimitField),
}

impl RuleField {
    pub const ALL: [RuleField; 12] = [
        RuleField::Enabled,
        RuleField::Name,
        RuleField::Scope,
        RuleField::Layer,
        RuleField::Pair,
        RuleField::Severity,
        RuleField::Message,
        RuleField::Limit(LimitField::MinClearance),
        RuleField::Limit(LimitField::MinWidth),
        RuleField::Limit(LimitField::MaxWidth),
        RuleField::Limit(LimitField::MinDiameter),
        RuleField::Limit(LimitField::MinDrill),
    ];

    /// Stable operand and JSON name.
    pub const fn id(self) -> &'static str {
        match self {
            RuleField::Enabled => "enabled",
            RuleField::Name => "name",
            RuleField::Scope => "scope",
            RuleField::Layer => "layer",
            RuleField::Pair => "pair",
            RuleField::Severity => "severity",
            RuleField::Message => "message",
            RuleField::Limit(field) => field.id(),
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }
}

/// Why an edit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// The text has a syntax error: no row can be found in it.
    Unparsed,
    NoSuchRule(String),
    /// The rule has a condition or limit the table has no column for.
    TextOnly(String),
    NameTaken(String),
    BadValue {
        field: &'static str,
        reason: String,
    },
    /// The rewritten rule would not compile; what the compiler said.
    Invalid(Vec<String>),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::Unparsed => write!(
                f,
                "the rule text has a syntax error; fix it in the text view first"
            ),
            EditError::NoSuchRule(name) => write!(f, "no rule is called '{name}'"),
            EditError::TextOnly(name) => write!(
                f,
                "rule '{name}' has a condition or limit the table has no column for; \
                 edit it in the text view"
            ),
            EditError::NameTaken(name) => write!(f, "a rule called '{name}' already exists"),
            EditError::BadValue { field, reason } => write!(f, "{field}: {reason}"),
            EditError::Invalid(said) => write!(f, "the rule would not apply: {}", said.join("; ")),
        }
    }
}

impl std::error::Error for EditError {}

impl RuleDraft {
    /// Set `field` from its text, as a table cell or `rules_set` gives it.
    /// An empty scope or layer is every net or layer; an empty pair is any
    /// objects; an empty limit removes it (a rule keeps at least one).
    pub fn set(&mut self, field: RuleField, value: &str) -> Result<(), EditError> {
        let value = value.trim();
        let bad = |reason: String| EditError::BadValue {
            field: field.id(),
            reason,
        };
        match field {
            RuleField::Enabled => {
                self.enabled = parse_bool(value).ok_or_else(|| bad("true or false".into()))?;
            }
            RuleField::Name => {
                if !is_name(value) {
                    return Err(bad(
                        "letters, digits and _, not starting with a digit".into()
                    ));
                }
                self.name = value.to_string();
            }
            RuleField::Scope => self.scope = parse_scope(value).map_err(bad)?,
            RuleField::Layer => {
                self.layer = match value {
                    "" | "all" | "any" => None,
                    v if is_plain(v) => Some(v.to_string()),
                    _ => return Err(bad("outer, inner or a copper layer name".into())),
                };
            }
            RuleField::Pair => {
                if self.kind != RuleKind::Clearance {
                    return Err(bad("only a clearance rule is between two objects".into()));
                }
                self.pair = parse_pair(value).map_err(bad)?;
            }
            RuleField::Severity => {
                self.severity =
                    parse_severity(value).ok_or_else(|| bad("error, warning or info".into()))?;
            }
            RuleField::Message => {
                if !is_plain(value) && !value.is_empty() {
                    return Err(bad("no double quote or line break".into()));
                }
                self.message = (!value.is_empty()).then(|| value.to_string());
            }
            RuleField::Limit(limit) => self.set_limit(limit, value).map_err(bad)?,
        }
        Ok(())
    }

    /// Set the limits from `values`: `min_width=0.1mm, max_width=12mil`, or
    /// one bare length for the kind's first limit (`0.2mm`).
    pub fn set_values(&mut self, values: &str) -> Result<(), EditError> {
        let items = values
            .split([',', ';'])
            .map(str::trim)
            .filter(|s| !s.is_empty());
        for item in items {
            let (field, value) = match item.split_once('=') {
                Some((id, value)) => {
                    let field = LimitField::from_id(id.trim()).ok_or_else(|| {
                        let known: Vec<&str> = self.kind.fields().iter().map(|f| f.id()).collect();
                        EditError::BadValue {
                            field: "values",
                            reason: format!("unknown limit '{}' ({})", id.trim(), known.join(", ")),
                        }
                    })?;
                    (field, value.trim())
                }
                None => {
                    let first = self.kind.fields().first().copied();
                    let field = first.ok_or_else(|| EditError::BadValue {
                        field: "values",
                        reason: "this kind takes no limit".into(),
                    })?;
                    (field, item)
                }
            };
            if value.is_empty() {
                return Err(EditError::BadValue {
                    field: "values",
                    reason: format!("{} has no length", field.id()),
                });
            }
            self.set(RuleField::Limit(field), value)?;
        }
        Ok(())
    }

    fn set_limit(&mut self, field: LimitField, value: &str) -> Result<(), String> {
        if field.kind() != self.kind {
            return Err(format!(
                "a {} rule has no {} limit",
                self.kind.keyword(),
                field.id()
            ));
        }
        if value.is_empty() {
            if self.limits.iter().all(|l| l.field == field) {
                return Err("a rule needs at least one limit".into());
            }
            self.limits.retain(|l| l.field != field);
            return Ok(());
        }
        if !is_length(value) {
            return Err("a length with its unit, such as 0.2mm or 8mil".into());
        }
        match self.limits.iter_mut().find(|l| l.field == field) {
            Some(limit) => limit.value = value.to_string(),
            None => self.limits.push(RowLimit {
                field,
                value: value.to_string(),
            }),
        }
        Ok(())
    }
}

/// `src` with `field` of rule `name` set to `value`: only that rule's span
/// is rewritten. A text-only rule takes only `enabled` this way.
pub fn set_field(
    src: &str,
    name: &str,
    field: RuleField,
    value: &str,
) -> Result<String, EditError> {
    let rows = parsed(src)?;
    let row = find(&rows, name)?;
    if field == RuleField::Enabled {
        let on = parse_bool(value.trim()).ok_or_else(|| EditError::BadValue {
            field: "enabled",
            reason: "true or false".into(),
        })?;
        return Ok(toggle(src, row, on));
    }
    let mut draft = row
        .draft()
        .ok_or_else(|| EditError::TextOnly(row.name.clone()))?;
    draft.set(field, value)?;
    if draft.name != row.name && rows.rows.iter().any(|r| r.name == draft.name) {
        return Err(EditError::NameTaken(draft.name));
    }
    let text = splice(src, &row.span, &render(&draft));
    no_new_errors(&text, &draft.name, row.error_count())?;
    Ok(text)
}

/// `src` with rule `name` written `disabled` (`on` false) or not.
pub fn enable_rule(src: &str, name: &str, on: bool) -> Result<String, EditError> {
    let rows = parsed(src)?;
    Ok(toggle(src, find(&rows, name)?, on))
}

/// `src` without rule `name`, and without the rest of its line when only
/// whitespace follows it there.
pub fn remove_rule(src: &str, name: &str) -> Result<String, EditError> {
    let rows = parsed(src)?;
    let row = find(&rows, name)?;
    let mut end = row.span.end;
    let rest = src.get(end..).unwrap_or("");
    let line = rest.find('\n').map_or(rest.len(), |at| at + 1);
    if rest[..line].trim().is_empty() {
        end += line;
    }
    let mut out = String::with_capacity(src.len());
    out.push_str(&src[..row.span.start]);
    out.push_str(&src[end..]);
    Ok(out)
}

/// `src` with `draft` appended on a line of its own, and the name it got:
/// an empty name becomes the first free `<kind>_<n>`.
pub fn add_rule(src: &str, draft: &RuleDraft) -> Result<(String, String), EditError> {
    let rows = parsed(src)?;
    let mut draft = draft.clone();
    if draft.name.is_empty() {
        draft.name = free_name(&rows, draft.kind);
    } else if !is_name(&draft.name) {
        return Err(EditError::BadValue {
            field: "name",
            reason: "letters, digits and _, not starting with a digit".into(),
        });
    }
    if rows.rows.iter().any(|r| r.name == draft.name) {
        return Err(EditError::NameTaken(draft.name));
    }
    if draft.limits.is_empty() {
        return Err(EditError::BadValue {
            field: "values",
            reason: "a rule needs at least one limit".into(),
        });
    }
    let mut text = src.to_string();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&render(&draft));
    text.push('\n');
    no_new_errors(&text, &draft.name, 0)?;
    Ok((text, draft.name))
}

fn parsed(src: &str) -> Result<RuleTable, EditError> {
    let rows = table(src);
    if rows.is_unparsed() {
        return Err(EditError::Unparsed);
    }
    Ok(rows)
}

fn find<'t>(rows: &'t RuleTable, name: &str) -> Result<&'t RuleRow, EditError> {
    rows.row(name)
        .ok_or_else(|| EditError::NoSuchRule(name.to_string()))
}

fn free_name(rows: &RuleTable, kind: RuleKind) -> String {
    (1..)
        .map(|n| format!("{}_{n}", kind.keyword()))
        .find(|name| rows.rows.iter().all(|r| &r.name != name))
        .unwrap_or_default()
}

fn toggle(src: &str, row: &RuleRow, on: bool) -> String {
    let start = row.span.start;
    if row.enabled == on {
        return src.to_string();
    }
    if on {
        let after = start + DISABLED.len();
        let rest = src.get(after..).unwrap_or("");
        let gap = rest.len() - rest.trim_start().len();
        format!("{}{}", &src[..start], &src[after + gap..])
    } else {
        format!("{}{DISABLED} {}", &src[..start], &src[start..])
    }
}

fn splice(src: &str, span: &Span, with: &str) -> String {
    format!("{}{with}{}", &src[..span.start], &src[span.end..])
}

/// Refuse a text in which rule `name` has more errors than `before`.
fn no_new_errors(text: &str, name: &str, before: usize) -> Result<(), EditError> {
    let after = table(text);
    if after.is_unparsed() {
        return Err(EditError::Invalid(
            after.syntax.iter().map(|d| d.render(text)).collect(),
        ));
    }
    let row = find(&after, name)?;
    let errors: Vec<String> = row
        .problems
        .iter()
        .filter(|d| d.level == Level::Error)
        .map(|d| d.message.clone())
        .collect();
    if errors.len() > before {
        return Err(EditError::Invalid(errors));
    }
    Ok(())
}

/// A rule name or a bare word: ASCII letters, digits and `_`, not
/// starting with a digit.
pub(crate) fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Text a quoted string can hold: no double quote, no line break.
fn is_plain(text: &str) -> bool {
    !text.is_empty() && !text.contains(['"', '\n', '\r'])
}

/// An `enabled` value: `true`, `on`, `yes` or `false`, `off`, `no`.
pub fn parse_bool(text: &str) -> Option<bool> {
    match text {
        "true" | "on" | "yes" => Some(true),
        "false" | "off" | "no" => Some(false),
        _ => None,
    }
}

pub(crate) fn parse_severity(text: &str) -> Option<Severity> {
    match text {
        "error" => Some(Severity::Error),
        "warning" => Some(Severity::Warning),
        "info" => Some(Severity::Info),
        _ => None,
    }
}

/// A number and a unit word, at most one space between: `0.2mm`, `8 mil`.
/// The compiler names a unit it does not know.
fn is_length(text: &str) -> bool {
    let unit_at = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, rest) = text.split_at(unit_at);
    let unit = rest.strip_prefix(' ').unwrap_or(rest);
    number.starts_with(|c: char| c.is_ascii_digit())
        && !unit.is_empty()
        && unit.chars().all(char::is_alphabetic)
}

/// A scope cell: empty or `all` for every net, else `net == "GND"`,
/// `class == "HV"` or `net matches "DDR_*"` (quotes optional).
pub fn parse_scope(text: &str) -> Result<NetScope, String> {
    let text = text.trim();
    if text.is_empty() || matches!(text, "all" | "board" | "*") {
        return Ok(NetScope::Board);
    }
    let shape =
        || "write net == \"NAME\", class == \"NAME\" or net matches \"PATTERN\"".to_string();
    let (subject, is, value) = if let Some((subject, value)) = text.split_once("==") {
        (subject, true, value)
    } else if let Some((subject, value)) = text.split_once(" matches ") {
        (subject, false, value)
    } else {
        return Err(shape());
    };
    let value = value.trim();
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    if !is_plain(value) {
        return Err("the name needs a character and no double quote".to_string());
    }
    match (subject.trim(), is) {
        ("net", true) => Ok(NetScope::Net(value.to_string())),
        ("net", false) => Ok(NetScope::Pattern(value.to_string())),
        ("class", true) => Ok(NetScope::Class(value.to_string())),
        _ => Err(shape()),
    }
}

/// The scope as a cell shows it (the inverse of [`parse_scope`]); empty for
/// every net.
pub fn scope_text(scope: &NetScope) -> String {
    match scope {
        NetScope::Board => String::new(),
        NetScope::Net(net) => format!("net == \"{net}\""),
        NetScope::Class(class) => format!("class == \"{class}\""),
        NetScope::Pattern(glob) => format!("net matches \"{glob}\""),
    }
}

fn parse_pair(text: &str) -> Result<Option<[String; 2]>, String> {
    if text.is_empty() || matches!(text, "any" | "all") {
        return Ok(None);
    }
    let words: Vec<&str> = text
        .split(|c: char| matches!(c, '-' | ',' | '/') || c.is_whitespace())
        .filter(|w| !w.is_empty() && *w != "and")
        .collect();
    match words.as_slice() {
        [a, b] if is_name(a) && is_name(b) => Ok(Some([a.to_string(), b.to_string()])),
        _ => Err("two objects, such as track-pad (track, pad or via)".to_string()),
    }
}
