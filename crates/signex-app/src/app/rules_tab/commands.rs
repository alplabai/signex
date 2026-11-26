//! The `rules_*` catalog commands (#2398): the agent and CLI surface of the
//! Rules tab. Each edit runs the same commit path as the tab's cells (one
//! undo step on the project's rule history); each read answers with JSON.
//! Shapes: `docs/DRC_RULES.md`, "Rules manager".

use iced::Task;
use serde_json::{Value, json};
use signex_drc::{LimitKind, WrittenRuleEdit};
use signex_drc_dsl::table::{self, RuleDraft, RuleField, RuleKind};

use super::json::{effective_json, flag_json, row_json};
use super::source::{field_follow, rule_name};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

pub(super) type Ran = (CommandOutcome, Task<Message>);

pub(super) fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

pub(super) fn answered(value: Value) -> Ran {
    (CommandOutcome::Answered(value), Task::none())
}

pub(super) fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

pub(super) fn flag(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(on)) => Some(*on),
        _ => None,
    }
}

/// The project and its rule text, or why not.
pub(super) fn project_text(
    app: &mut Signex,
    args: &CommandArgs,
) -> Result<(ProjectId, String), String> {
    let id = app.rules_project_named(args)?;
    let (text, _) = app
        .rules_text_of(id)
        .ok_or_else(|| "the project is no longer open".to_string())?;
    Ok((id, text))
}

/// Store `edited` as the project's rule text, the severity entries
/// following `follows`, and answer `value`.
fn commit(
    app: &mut Signex,
    id: ProjectId,
    edited: String,
    follows: &[WrittenRuleEdit<'_>],
    value: Value,
) -> Ran {
    match app.commit_rules_text(id, edited, follows) {
        Ok(()) => answered(value),
        Err(reason) => refused(reason),
    }
}

/// `rules_add {document, kind, values, scope?, layer?, pair?, severity?, name?}`.
pub(crate) fn rules_add(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (id, src) = match project_text(app, args) {
        Ok(found) => found,
        Err(reason) => return refused(reason),
    };
    let Some(kind) = text(args, "kind").and_then(RuleKind::from_keyword) else {
        let kinds: Vec<&str> = RuleKind::ALL.iter().map(|k| k.keyword()).collect();
        return refused(format!("`kind` is one of {}", kinds.join(", ")));
    };
    let mut draft = RuleDraft::new(kind, text(args, "name").unwrap_or_default());
    let fields = [
        (RuleField::Scope, "scope"),
        (RuleField::Layer, "layer"),
        (RuleField::Pair, "pair"),
        (RuleField::Severity, "severity"),
    ];
    for (field, name) in fields {
        if let Some(value) = text(args, name)
            && let Err(error) = draft.set(field, value)
        {
            return refused(error.to_string());
        }
    }
    if let Err(error) = draft.set_values(text(args, "values").unwrap_or_default()) {
        return refused(error.to_string());
    }
    match table::add_rule(&src, &draft) {
        Ok((edited, name)) => {
            let value = json!({ "id": format!("{}{name}", table::RULE_ID_PREFIX), "name": name });
            // A stale entry for the name must not speak for the new rule.
            commit(app, id, edited, &[WrittenRuleEdit::Dropped(&name)], value)
        }
        Err(error) => refused(error.to_string()),
    }
}

/// `rules_set {document, id, field, value?}`; no `value` clears the field.
pub(crate) fn rules_set(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (id, src) = match project_text(app, args) {
        Ok(found) => found,
        Err(reason) => return refused(reason),
    };
    let Some(rule) = text(args, "id") else {
        return refused("needs the operand `id`");
    };
    let Some(field) = text(args, "field").and_then(RuleField::from_id) else {
        let known: Vec<&str> = RuleField::ALL.iter().map(|f| f.id()).collect();
        return refused(format!("`field` is one of {}", known.join(", ")));
    };
    let value = text(args, "value").unwrap_or_default();
    let edited = match table::set_field(&src, rule, field, value) {
        Ok(edited) => edited,
        Err(error) => return refused(error.to_string()),
    };
    let follows: Vec<_> = field_follow(rule, field, value).into_iter().collect();
    if let Err(reason) = app.commit_rules_text(id, edited.clone(), &follows) {
        return refused(reason);
    }
    let name = match field {
        RuleField::Name => value.trim(),
        _ => rule_name(rule),
    };
    let rows = table::table(&edited);
    let layer = app.rules_drc_layer(id);
    let row = rows.row(name).map(|row| row_json(row, &edited, layer));
    answered(json!({ "rule": row }))
}

/// `rules_remove {document, id}`.
pub(crate) fn rules_remove(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (id, src) = match project_text(app, args) {
        Ok(found) => found,
        Err(reason) => return refused(reason),
    };
    let Some(rule) = text(args, "id") else {
        return refused("needs the operand `id`");
    };
    match table::remove_rule(&src, rule) {
        Ok(edited) => {
            let follows = [WrittenRuleEdit::Dropped(rule_name(rule))];
            commit(app, id, edited, &follows, json!({ "removed": rule }))
        }
        Err(error) => refused(error.to_string()),
    }
}

/// `rules_enable {document, id, on}`.
pub(crate) fn rules_enable(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (id, src) = match project_text(app, args) {
        Ok(found) => found,
        Err(reason) => return refused(reason),
    };
    let (Some(rule), Some(on)) = (text(args, "id"), flag(args, "on")) else {
        return refused("needs the operands `id` and `on`");
    };
    match table::enable_rule(&src, rule, on) {
        Ok(edited) => {
            // Switched on, an `off` severity entry would keep it off.
            let follows: Vec<_> = on
                .then(|| WrittenRuleEdit::SwitchedOn(rule_name(rule)))
                .into_iter()
                .collect();
            let value = json!({ "id": rule, "enabled": on });
            commit(app, id, edited, &follows, value)
        }
        Err(error) => refused(error.to_string()),
    }
}

impl Signex {
    /// Answer the read-only `rules_*` query `id`.
    pub(crate) fn rules_query(&mut self, id: &str, args: &CommandArgs) -> Result<Value, String> {
        let (project, src) = project_text(self, args)?;
        match id {
            "rules_list" => Ok(self.rules_list(project, &src, args)),
            "rules_get" => self.rules_get(project, &src, args),
            "rules_effective" => self.rules_effective(project, args),
            "rules_explain" => self.rules_explain(project, args),
            _ => self.rules_conflicts(project),
        }
    }

    fn rules_list(&self, project: ProjectId, src: &str, args: &CommandArgs) -> Value {
        let rows = table::table(src);
        let kind = text(args, "kind").and_then(RuleKind::from_keyword);
        let enabled = flag(args, "enabled");
        let layer = self.rules_drc_layer(project);
        let listed: Vec<Value> = rows
            .rows
            .iter()
            .filter(|r| kind.is_none_or(|k| r.kind == Some(k)))
            .filter(|r| enabled.is_none_or(|on| r.enabled == on))
            .map(|r| row_json(r, src, layer))
            .collect();
        let syntax: Vec<String> = rows.syntax.iter().map(|d| d.render(src)).collect();
        let origin = self
            .rules_text_of(project)
            .map(|(_, origin)| origin.as_str());
        let sets: Vec<Value> = self
            .rules_drc_of(project)
            .map(|drc| drc.sets)
            .unwrap_or_default()
            .iter()
            .map(|(name, set)| super::sets::set_json(name, set))
            .collect();
        json!({
            "stored_in": origin,
            "board": self.rules_board_path(project).map(|p| p.display().to_string()),
            "syntax_errors": syntax,
            "rules": listed,
            "sets": sets,
        })
    }

    fn rules_get(
        &self,
        project: ProjectId,
        src: &str,
        args: &CommandArgs,
    ) -> Result<Value, String> {
        let name = text(args, "id").ok_or("needs the operand `id`")?;
        let rows = table::table(src);
        let row = rows
            .row(name)
            .ok_or_else(|| format!("no rule is called '{name}'"))?;
        let mut value = row_json(row, src, self.rules_drc_layer(project));
        let resolved = self.rules_resolved(project).ok();
        let board = self.rules_board(project);
        let classes = self
            .document_state
            .project_by_id(project)
            .map(|p| p.data.net_classes.clone())
            .unwrap_or_default();
        if let (Some(rules), Some((path, board))) = (resolved, board) {
            let flags: Vec<Value> = rules
                .rule_flags(board, &classes)
                .iter()
                .filter(|f| f.rule == row.name)
                .map(flag_json)
                .collect();
            value["flags"] = json!(flags);
            value["scope_objects"] = json!(rules.scope_items(board, &row.name).map(|i| i.len()));
            value["result"] = self.rule_result_json(&path, &row.id());
        }
        Ok(value)
    }

    /// What the last DRC of `board` found for rule `id`; `null` when the
    /// board was never checked.
    fn rule_result_json(&self, board: &std::path::Path, id: &str) -> Value {
        let Some(done) = self.interaction_state.pcb_drc.done_for(board) else {
            return Value::Null;
        };
        let revision = self
            .document_state
            .pcb_revisions
            .get(board)
            .copied()
            .unwrap_or(0);
        let result = done.report.rule_results().remove(id).unwrap_or_default();
        json!({
            "violations": result.violations,
            "measured_mm": result.worst.map(|(m, _)| super::json::mm(m)),
            "required_mm": result.worst.map(|(_, r)| super::json::mm(r)),
            "margin_mm": result.margin_nm().map(super::json::mm),
            "stale": done.is_stale(revision, self.board_net_classes(board)),
        })
    }

    fn rules_effective(&self, project: ProjectId, args: &CommandArgs) -> Result<Value, String> {
        let net_name = text(args, "net").ok_or("needs the operand `net`")?;
        let kind_id = text(args, "kind").unwrap_or("min_clearance");
        let kind = LimitKind::from_str_id(kind_id).ok_or_else(|| {
            let known: Vec<&str> = LimitKind::ALL.iter().map(|k| k.as_str()).collect();
            format!("`kind` is one of {}", known.join(", "))
        })?;
        // Why the board's rules cannot be read, when they cannot.
        let rules = self.rules_resolved(project)?;
        let (_, board) = self
            .rules_board(project)
            .ok_or("the project's board is not open")?;
        let net = board
            .nets
            .iter()
            .find(|n| n.name == net_name)
            .ok_or_else(|| format!("the board has no net '{net_name}'"))?;
        let layers = match text(args, "layer") {
            Some(name) => match pcb_geom::layer_mask(std::iter::once(name)) {
                0 => return Err(format!("'{name}' is not a copper layer")),
                mask => mask,
            },
            None => u32::MAX,
        };
        let effective = rules.effective(kind, net.number, layers, [None, None]);
        let classes = self
            .document_state
            .project_by_id(project)
            .map(|p| p.data.net_classes.clone())
            .unwrap_or_default();
        Ok(effective_json(&effective, board, &classes))
    }

    fn rules_explain(&self, project: ProjectId, args: &CommandArgs) -> Result<Value, String> {
        let index: usize = text(args, "violation")
            .and_then(|t| t.trim().parse().ok())
            .ok_or("`violation` is the index of a finding of the last DRC run")?;
        let (path, board) = self
            .rules_board(project)
            .ok_or("the project's board is not open")?;
        let done = self
            .interaction_state
            .pcb_drc
            .done_for(&path)
            .ok_or("the board has not been checked: run DRC first")?;
        let violation = done
            .report
            .violations
            .get(index)
            .ok_or_else(|| format!("the last run has no finding {index}"))?;
        let rules = &done.report.rules;
        let classes = self
            .document_state
            .project_by_id(project)
            .map(|p| p.data.net_classes.clone())
            .unwrap_or_default();
        let explanation = rules
            .explain(violation)
            .ok_or("an unconnected link or a keepout has no rule value to explain")?;
        let sides: Vec<Value> = explanation
            .sides
            .iter()
            .map(|side| effective_json(side, board, &classes))
            .collect();
        Ok(json!({
            "violation": index,
            "kind": explanation.kind.as_str(),
            "required_mm": super::json::mm(explanation.required_nm),
            "rule": violation.rule,
            "sides": sides,
        }))
    }

    fn rules_conflicts(&self, project: ProjectId) -> Result<Value, String> {
        // Why the board's rules cannot be read, when they cannot.
        let rules = self.rules_resolved(project)?;
        let (_, board) = self
            .rules_board(project)
            .ok_or("the project's board is not open")?;
        let classes = self
            .document_state
            .project_by_id(project)
            .map(|p| p.data.net_classes.clone())
            .unwrap_or_default();
        let flags: Vec<Value> = rules
            .rule_flags(board, &classes)
            .iter()
            .map(flag_json)
            .collect();
        Ok(json!({ "flags": flags }))
    }
}
