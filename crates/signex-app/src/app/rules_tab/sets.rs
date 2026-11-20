//! Constraint sets in the app (#2398, #2530): create, redefine and apply, each one
//! undo step that stores the set and the rules it writes together. The
//! text edits are `signex_drc_dsl::table::sets`. Behaviour:
//! `docs/DRC_RULES.md`, "Constraint sets".

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};
use signex_drc::WrittenRuleEdit;
use signex_drc_dsl::table::sets;
use signex_types::project::NamedRuleSet;

use super::commands::{Ran, answered, refused, text};
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::state::ProjectId;

/// One set as the answers give it.
pub(crate) fn set_json(name: &str, set: &NamedRuleSet) -> Value {
    let limits: Map<String, Value> = set
        .limits
        .iter()
        .map(|(id, value)| (id.clone(), json!(value)))
        .collect();
    let uses: Vec<Value> = set
        .uses
        .iter()
        .map(|u| json!({ "scope": u.scope, "rules": u.rules }))
        .collect();
    json!({
        "name": name,
        "limits": limits,
        "severity": set.severity.as_deref().unwrap_or("error"),
        "uses": uses,
    })
}

/// The rule names a set wrote, over all its uses.
fn written(set: &NamedRuleSet) -> BTreeSet<String> {
    set.uses.iter().flat_map(|u| u.rules.clone()).collect()
}

impl Signex {
    /// Store `text` and set `name` as `set` in one undo step; the severity
    /// entries of rules the step adds or removes are dropped, and so are
    /// those of rules whose `then` word the set changed, so the set's
    /// severity governs them.
    fn commit_set(
        &mut self,
        id: ProjectId,
        name: &str,
        before: Option<&NamedRuleSet>,
        set: NamedRuleSet,
        text: String,
    ) -> Result<(), String> {
        let old = before.map(written).unwrap_or_default();
        let new = written(&set);
        let dropped: Vec<String> = old.symmetric_difference(&new).cloned().collect();
        let now = signex_drc_dsl::table::table(&text);
        self.commit_drc_section(id, |drc| {
            let was = signex_drc_dsl::table::table(drc.dsl.as_deref().unwrap_or_default());
            let severity = |rows: &signex_drc_dsl::table::RuleTable, rule: &str| {
                rows.row(rule).map(|row| row.severity)
            };
            for rule in old.intersection(&new) {
                if severity(&was, rule) != severity(&now, rule) {
                    signex_drc::follow_written_rule_edit(drc, WrittenRuleEdit::SeveritySet(rule));
                }
            }
            for rule in &dropped {
                signex_drc::follow_written_rule_edit(drc, WrittenRuleEdit::Dropped(rule));
            }
            drc.dsl = (!text.trim().is_empty()).then_some(text);
            drc.sets.insert(name.to_string(), set);
        })
    }

    /// A new set `name` of `values` (`min_clearance=0.8mm, ...`), applied
    /// nowhere yet.
    pub(crate) fn create_rule_set(
        &mut self,
        id: ProjectId,
        name: &str,
        values: &str,
        severity: Option<&str>,
    ) -> Result<NamedRuleSet, String> {
        let name = name.trim();
        if !sets::is_set_name(name) {
            return Err("a set name is letters, digits and _, not starting with a digit".into());
        }
        let drc = self
            .rules_drc_of(id)
            .ok_or("the project is no longer open")?;
        if drc.sets.contains_key(name) {
            return Err(format!("a constraint set called '{name}' already exists"));
        }
        let set = sets::new_set(values, severity).map_err(|e| e.to_string())?;
        let text = drc.dsl.clone().unwrap_or_default();
        self.commit_set(id, name, None, set.clone(), text)?;
        Ok(set)
    }

    /// Give set `name` new limits and severity: every rule it wrote follows.
    pub(crate) fn redefine_rule_set(
        &mut self,
        id: ProjectId,
        name: &str,
        values: &str,
        severity: Option<&str>,
    ) -> Result<NamedRuleSet, String> {
        let drc = self
            .rules_drc_of(id)
            .ok_or("the project is no longer open")?;
        let held = drc
            .sets
            .get(name)
            .ok_or_else(|| format!("no constraint set is called '{name}'"))?;
        let mut fresh = sets::new_set(values, severity).map_err(|e| e.to_string())?;
        // No severity given keeps the set's own.
        if severity.is_none_or(|s| s.trim().is_empty()) {
            fresh.severity = held.severity.clone();
        }
        let src = drc.dsl.clone().unwrap_or_default();
        let (text, set) =
            sets::sync_set(&src, name, &sets::redefined(held, fresh)).map_err(|e| e.to_string())?;
        self.commit_set(id, name, Some(held), set.clone(), text)?;
        Ok(set)
    }

    /// Apply set `name` to `scope` (`class == "HV"`): one rule per kind it
    /// limits, written anew when already applied there.
    pub(crate) fn apply_rule_set(
        &mut self,
        id: ProjectId,
        name: &str,
        scope: &str,
    ) -> Result<NamedRuleSet, String> {
        let drc = self
            .rules_drc_of(id)
            .ok_or("the project is no longer open")?;
        let held = drc
            .sets
            .get(name)
            .ok_or_else(|| format!("no constraint set is called '{name}'"))?;
        let src = drc.dsl.clone().unwrap_or_default();
        let (text, set) = sets::apply_set(&src, name, held, scope).map_err(|e| e.to_string())?;
        self.commit_set(id, name, Some(held), set.clone(), text)?;
        Ok(set)
    }
}

/// `rules_set_create {document, name, values, severity?}`.
pub(crate) fn rules_set_create(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let (Some(name), Some(values)) = (text(args, "name"), text(args, "values")) else {
        return refused("needs the operands `name` and `values`");
    };
    match app.create_rule_set(id, name, values, text(args, "severity")) {
        Ok(set) => answered(json!({ "set": set_json(name.trim(), &set) })),
        Err(reason) => refused(reason),
    }
}

/// `rules_set_edit {document, set, values, severity?}`.
pub(crate) fn rules_set_edit(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let (Some(name), Some(values)) = (text(args, "set"), text(args, "values")) else {
        return refused("needs the operands `set` and `values`");
    };
    match app.redefine_rule_set(id, name, values, text(args, "severity")) {
        Ok(set) => answered(json!({ "set": set_json(name, &set) })),
        Err(reason) => refused(reason),
    }
}

/// `rules_set_apply {document, set, scope}`.
pub(crate) fn rules_set_apply(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let (Some(name), Some(scope)) = (text(args, "set"), text(args, "scope")) else {
        return refused("needs the operands `set` and `scope`");
    };
    match app.apply_rule_set(id, name, scope) {
        Ok(set) => {
            let written = signex_drc_dsl::table::parse_scope(scope)
                .ok()
                .map(|s| signex_drc_dsl::table::scope_text(&s));
            let rules: Vec<String> = set
                .uses
                .iter()
                .find(|u| Some(&u.scope) == written.as_ref())
                .map(|u| u.rules.clone())
                .unwrap_or_default();
            answered(json!({ "set": set_json(name, &set), "rules": rules }))
        }
        Err(reason) => refused(reason),
    }
}
