//! Named rule sets (#2398): limits written once (`HV_08`: clearance 0.8 mm,
//! width 0.3 mm) and applied to net classes or nets. Applying a set writes
//! one rule per kind it limits for that scope; changing the set rewrites
//! every rule it wrote, so the rule text stays the format Run DRC reads.
//! The set itself (`NamedRuleSet`) lives beside the text in `rules.drc`.
//! See `docs/DRC_RULES.md`, "Rule sets".

use signex_drc::NetScope;
use signex_types::project::{NamedRuleSet, RuleSetUse};

use super::edit::{
    self, EditError, add_rule, parse_scope, parse_severity, remove_rule, replace_rule, scope_text,
};
use super::matrix::{free_name, slug};
use super::render::severity_word;
use super::{LimitField, RowLimit, RuleDraft, RuleKind, table};

/// Whether `name` can name a set: letters, digits and `_`, not starting
/// with a digit (it starts the names of the rules the set writes).
pub fn is_set_name(name: &str) -> bool {
    edit::is_name(name)
}

/// A set from `values` (`min_clearance=0.8mm, min_width=0.3mm`) and an
/// optional severity (`error`, `warning`, `info`).
pub fn new_set(values: &str, severity: Option<&str>) -> Result<NamedRuleSet, EditError> {
    let mut set = NamedRuleSet::default();
    for item in values
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let bad = |reason: String| EditError::BadValue {
            field: "values",
            reason,
        };
        let (id, length) = item
            .split_once('=')
            .ok_or_else(|| bad(format!("'{item}' is not <limit>=<length>")))?;
        let (id, length) = (id.trim(), length.trim());
        let field = LimitField::from_id(id).ok_or_else(|| {
            let known: Vec<&str> = LimitField::ALL.iter().map(|f| f.id()).collect();
            bad(format!("unknown limit '{id}' ({})", known.join(", ")))
        })?;
        if !edit::is_length(length) {
            return Err(bad(format!(
                "{}: a length with its unit, such as 0.2mm or 8mil",
                field.id()
            )));
        }
        set.limits
            .insert(field.id().to_string(), length.to_string());
    }
    if set.limits.is_empty() {
        return Err(EditError::BadValue {
            field: "values",
            reason: "a set needs at least one limit".into(),
        });
    }
    if let Some(word) = severity.map(str::trim).filter(|w| !w.is_empty()) {
        let parsed = parse_severity(word).ok_or_else(|| EditError::BadValue {
            field: "severity",
            reason: "error, warning or info".into(),
        })?;
        set.severity = Some(severity_word(parsed).to_string());
    }
    Ok(set)
}

/// `set` with its limits and severity from `with`, its uses kept.
pub fn redefined(set: &NamedRuleSet, with: NamedRuleSet) -> NamedRuleSet {
    NamedRuleSet {
        uses: set.uses.clone(),
        unknown: set.unknown.clone(),
        ..with
    }
}

/// One draft per rule kind `set` limits, for `scope`; unnamed.
fn drafts(set: &NamedRuleSet, scope: &NetScope) -> Vec<RuleDraft> {
    let severity = set
        .severity
        .as_deref()
        .and_then(parse_severity)
        .unwrap_or(signex_types::violation::Severity::Error);
    RuleKind::ALL
        .into_iter()
        .filter_map(|kind| {
            let limits: Vec<RowLimit> = kind
                .fields()
                .iter()
                .filter_map(|field| {
                    set.limits.get(field.id()).map(|value| RowLimit {
                        field: *field,
                        value: value.clone(),
                    })
                })
                .collect();
            if limits.is_empty() {
                return None;
            }
            let mut draft = RuleDraft::new(kind, "");
            draft.scope = scope.clone();
            draft.limits = limits;
            draft.severity = severity;
            Some(draft)
        })
        .collect()
}

/// `src` with set `name` applied to `scope` (a scope cell's text): one rule
/// per kind the set limits, named `<set>_<scope>_<kind>`. Applied again to
/// the same scope, its rules are written anew. Returns the text and the
/// set with the use recorded.
pub fn apply_set(
    src: &str,
    name: &str,
    set: &NamedRuleSet,
    scope: &str,
) -> Result<(String, NamedRuleSet), EditError> {
    let parsed = parse_scope(scope).map_err(|reason| EditError::BadValue {
        field: "scope",
        reason,
    })?;
    let written = scope_text(&parsed);
    let mut out = set.clone();
    let at = out.uses.iter().position(|u| u.scope == written);
    let current = at.map_or_else(
        || RuleSetUse {
            scope: written.clone(),
            ..RuleSetUse::default()
        },
        |at| out.uses[at].clone(),
    );
    let (text, synced) = sync_use(src, name, set, &current)?;
    match at {
        Some(at) => out.uses[at] = synced,
        None => out.uses.push(synced),
    }
    Ok((text, out))
}

/// `src` with every rule `set` wrote following its limits and severity
/// (after the set changed). Returns the text and the set with its uses'
/// rule names brought up to date.
pub fn sync_set(
    src: &str,
    name: &str,
    set: &NamedRuleSet,
) -> Result<(String, NamedRuleSet), EditError> {
    let mut text = src.to_string();
    let mut out = set.clone();
    for (at, current) in set.uses.iter().enumerate() {
        let (next, synced) = sync_use(&text, name, set, current)?;
        text = next;
        out.uses[at] = synced;
    }
    Ok((text, out))
}

/// One use brought in line with `set`: each rule it wrote that still
/// exists is rewritten (its name, on/off, layer and objects kept); a kind
/// the set no longer limits loses its rule; a kind it now limits gains one.
fn sync_use(
    src: &str,
    name: &str,
    set: &NamedRuleSet,
    current: &RuleSetUse,
) -> Result<(String, RuleSetUse), EditError> {
    let scope = parse_scope(&current.scope).map_err(|reason| EditError::BadValue {
        field: "scope",
        reason,
    })?;
    let mut wanted = drafts(set, &scope);
    let mut text = src.to_string();
    let mut kept: Vec<String> = Vec::new();
    for rule in &current.rules {
        let rows = table(&text);
        let Some(row) = rows.row(rule) else {
            continue;
        };
        let kind = row.kind;
        match wanted.iter().position(|d| Some(d.kind) == kind) {
            Some(at) => {
                let mut draft = wanted.remove(at);
                draft.name = row.name.clone();
                draft.enabled = row.enabled;
                draft.layer = row.layer.clone();
                draft.pair = row.pair.clone();
                draft.message = row.message.clone();
                text = replace_rule(&text, rule, &draft)?;
                kept.push(row.name.clone());
            }
            None => text = remove_rule(&text, rule)?,
        }
    }
    for mut draft in wanted {
        let rows = table(&text);
        let base = format!("{name}_{}_{}", scope_slug(&scope), draft.kind.keyword());
        draft.name = free_name(&rows, &base);
        let (next, added) = add_rule(&text, &draft)?;
        text = next;
        kept.push(added);
    }
    Ok((
        text,
        RuleSetUse {
            scope: current.scope.clone(),
            rules: kept,
            unknown: current.unknown.clone(),
        },
    ))
}

/// The scope in a rule name: `all`, `GND`, `HV`, `HV_LV`, `pattern_DDR__`.
fn scope_slug(scope: &NetScope) -> String {
    match scope {
        NetScope::Board => "all".to_string(),
        NetScope::Net(net) => slug(net),
        NetScope::Class(class) => slug(class),
        NetScope::Pattern(glob) => format!("pattern_{}", slug(glob)),
        NetScope::ClassPair(a, b) => format!("{}_{}", slug(a), slug(b)),
    }
}

/// The set's limits as `rules_set_create` takes them:
/// `min_clearance=0.8mm, min_width=0.3mm`.
pub fn values_text(set: &NamedRuleSet) -> String {
    set.limits
        .iter()
        .map(|(id, value)| format!("{id}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}
