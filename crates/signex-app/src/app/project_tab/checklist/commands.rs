//! The Design Checklist catalog commands (#1291): the agent and CLI surface
//! of the tab. Each names the project by `document` and an item by its
//! `item` id, runs the tab's commit path (one General step) and answers with
//! the checklist as `project_checklist` reads it; `project_run_checklist`
//! runs the tab's checks, or one item's. Shapes: `docs/DESIGN_CHECKLIST.md`,
//! "Commands".

use iced::Task;
use serde_json::{Value, json};
use signex_engine::ProjectChecklistCommand;
use signex_i18n::{MessageId, Translatable, ids};
use signex_types::project::{
    ChecklistField, ChecklistProblem, ChecklistStage, ItemState, LinkResult, TemplateRef,
};

use super::ChecklistFilter;
use super::words::{problem_refusal, project_refusal};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

pub(super) type Ran = (CommandOutcome, Task<Message>);

pub(super) fn refused(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

pub(super) fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// The text operand `name`, or why it is missing; `what` says what it
/// takes.
pub(super) fn required<'a>(
    args: &'a CommandArgs,
    name: &'static str,
    what: MessageId,
) -> Result<&'a str, Translatable> {
    text(args, name).ok_or_else(|| {
        Translatable::new(ids::CHECKLIST_REFUSAL_NEEDS_OPERAND)
            .arg("name", name)
            .arg("what", Translatable::new(what))
    })
}

/// The project `args` names, or why it names none.
pub(super) fn named_project(
    app: &mut Signex,
    args: &CommandArgs,
) -> Result<ProjectId, Translatable> {
    app.rules_project_named(args).map_err(project_refusal)
}

/// Apply `command` to project `id` and answer with the checklist and
/// whether it changed.
pub(super) fn edit(app: &mut Signex, id: ProjectId, command: &ProjectChecklistCommand) -> Ran {
    match app.apply_project_checklist(id, command) {
        Ok(changed) => {
            let mut answer = app.project_checklist_json(id, ChecklistFilter::All, None);
            if let Value::Object(map) = &mut answer {
                map.insert("changed".into(), json!(changed));
            }
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(problem) => refused(problem_refusal(&problem)),
    }
}

/// [`edit`] from the tab's own path: a refusal is also the tab's problem
/// line. For an edit that lands after a file read, from the tab or a
/// command.
pub(super) fn edit_in_tab(
    app: &mut Signex,
    id: ProjectId,
    command: &ProjectChecklistCommand,
) -> CommandOutcome {
    match app.edit_checklist_from_tab(id, command) {
        Ok(changed) => {
            let mut answer = app.project_checklist_json(id, ChecklistFilter::All, None);
            if let Value::Object(map) = &mut answer {
                map.insert("changed".into(), json!(changed));
            }
            CommandOutcome::Answered(answer)
        }
        Err(problem) => CommandOutcome::Disabled {
            reason: problem_refusal(&problem),
        },
    }
}

/// `project_start_checklist {document, template}`: a built-in template's
/// items are worded in the app's language, as the tab's Start words them;
/// `user:<name>` starts from the user's saved template of that name.
pub(crate) fn project_start_checklist(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let key = match required(args, "template", ids::CHECKLIST_OPERAND_TEMPLATE) {
        Ok(key) => key.trim(),
        Err(reason) => return refused(reason),
    };
    match TemplateRef::parse(key) {
        Some(TemplateRef::Builtin(template)) => {
            let command = app.start_command(template);
            edit(app, id, &command)
        }
        Some(TemplateRef::Saved(name)) => super::templates::start_from_saved(app, id, args, name),
        None => refused(problem_refusal(&ChecklistProblem::UnknownTemplate(
            key.to_string(),
        ))),
    }
}

/// `project_add_checklist_item {document, stage, text}`; answers with the
/// new item's id as `item`.
pub(crate) fn project_add_checklist_item(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let (stage, title) = match (
        required(args, "stage", ids::CHECKLIST_OPERAND_STAGE),
        required(args, "text", ids::CHECKLIST_OPERAND_TITLE),
    ) {
        (Ok(stage), Ok(title)) => (stage.to_string(), title.to_string()),
        (Err(reason), _) | (_, Err(reason)) => return refused(reason),
    };
    let command = ProjectChecklistCommand::AddItem {
        stage,
        title,
        stamp: app.checklist_stamp(),
    };
    let (outcome, task) = edit(app, id, &command);
    let outcome = match outcome {
        CommandOutcome::Answered(mut answer) => {
            let added = answer["items"]
                .as_array()
                .and_then(|items| items.last())
                .and_then(|item| item.get("id"))
                .cloned()
                .unwrap_or(Value::Null);
            if let Value::Object(map) = &mut answer {
                map.insert("item".into(), added);
            }
            CommandOutcome::Answered(answer)
        }
        other => other,
    };
    (outcome, task)
}

/// `project_set_checklist_item {document, item, field, value}`.
pub(crate) fn project_set_checklist_item(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let item = match required(args, "item", ids::CHECKLIST_OPERAND_ITEM) {
        Ok(item) => item.trim().to_string(),
        Err(reason) => return refused(reason),
    };
    let key = match required(args, "field", ids::CHECKLIST_OPERAND_FIELD) {
        Ok(key) => key.trim(),
        Err(reason) => return refused(reason),
    };
    let Some(field) = ChecklistField::parse(key) else {
        return refused(problem_refusal(&ChecklistProblem::UnknownField(
            key.to_string(),
        )));
    };
    let command = ProjectChecklistCommand::SetField {
        item,
        field,
        text: text(args, "value").unwrap_or_default().to_string(),
        stamp: app.checklist_stamp(),
    };
    edit(app, id, &command)
}

/// `project_set_checklist_state {document, item, state, reason?}`.
pub(crate) fn project_set_checklist_state(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let item = match required(args, "item", ids::CHECKLIST_OPERAND_ITEM) {
        Ok(item) => item.trim().to_string(),
        Err(reason) => return refused(reason),
    };
    let key = match required(args, "state", ids::CHECKLIST_OPERAND_STATE) {
        Ok(key) => key.trim(),
        Err(reason) => return refused(reason),
    };
    let Some(state) = ItemState::parse(key) else {
        return refused(problem_refusal(&ChecklistProblem::UnknownState(
            key.to_string(),
        )));
    };
    let command = ProjectChecklistCommand::SetState {
        item,
        state,
        reason: text(args, "reason").map(str::to_string),
        stamp: app.checklist_stamp(),
    };
    edit(app, id, &command)
}

/// `project_remove_checklist_item {document, item}`.
pub(crate) fn project_remove_checklist_item(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    match required(args, "item", ids::CHECKLIST_OPERAND_ITEM) {
        Ok(item) => {
            let command = ProjectChecklistCommand::RemoveItem {
                item: item.trim().to_string(),
            };
            edit(app, id, &command)
        }
        Err(reason) => refused(reason),
    }
}

/// `project_run_checklist {document, item?}`: Run checks in the project's
/// tab, or the Run check of item `item`. The results land in the tab;
/// `project_checklist` answers with them.
pub(crate) fn project_run_checklist(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let only = text(args, "item").map(|item| item.trim().to_string());
    if let Some(item) = &only {
        let known = app
            .document_state
            .project_by_id(id)
            .and_then(|project| project.data.design_checklist().ok().flatten())
            .is_some_and(|checklist| checklist.item(item).is_some());
        if !known {
            return refused(problem_refusal(&ChecklistProblem::UnknownItem(
                item.clone(),
            )));
        }
    }
    (CommandOutcome::Ran, app.run_checklist_in_tab(id, only))
}

impl Signex {
    /// `project_checklist {document, filter?, owner?}`: the checklist as
    /// stored, the counts per stage and per filter chip, what each item's
    /// link says now, and the ids the filter shows. Mine is the items `owner`
    /// owns, else the user's.
    pub(crate) fn project_checklist(&mut self, args: &CommandArgs) -> Result<Value, Translatable> {
        let id = named_project(self, args)?;
        let filter = match text(args, "filter") {
            Some(key) => ChecklistFilter::parse(key).ok_or_else(|| {
                Translatable::new(ids::CHECKLIST_REFUSAL_BAD_FILTER).arg("filter", key)
            })?,
            None => ChecklistFilter::All,
        };
        Ok(self.project_checklist_json(id, filter, text(args, "owner")))
    }

    /// The checklist of project `id` as `project_checklist` answers it.
    pub(crate) fn project_checklist_json(
        &self,
        id: ProjectId,
        filter: ChecklistFilter,
        owner: Option<&str>,
    ) -> Value {
        let Some(project) = self.document_state.project_by_id(id) else {
            return json!({});
        };
        let data = &project.data;
        let tab = self.checklist_tab_ref(id);
        let report = tab.and_then(|tab| tab.report());
        let last_run = tab
            .and_then(|tab| tab.last_run.as_deref())
            .map(|run| run.finished_at.to_rfc3339());
        let me = owner.map_or_else(|| self.checklist_user(), str::to_string);
        let mut answer = json!({
            "project": project.path.display().to_string(),
            "last_run": last_run,
            "user": me,
        });
        let checklist = match data.design_checklist() {
            Err(_) => {
                answer["unreadable"] = json!(true);
                return answer;
            }
            Ok(None) => {
                answer["started"] = json!(false);
                return answer;
            }
            Ok(Some(checklist)) => checklist,
        };
        // Each item as stored, keys a newer build wrote included.
        let stored = data
            .design_checklist_value()
            .and_then(|value| value.get("items"))
            .and_then(Value::as_array);
        let items: Vec<Value> = checklist
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let mut value = stored
                    .and_then(|stored| stored.get(index))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let result = item.link.as_ref().map(|link| link.result(data, report));
                value["result"] = json!(result.map(LinkResult::key));
                if let Some(LinkResult::Fails(count)) = result {
                    value["findings"] = json!(count);
                }
                // "Check passes · Mark done": the owner ticks it.
                value["check_passes"] =
                    json!(result == Some(LinkResult::Passes) && item.state == ItemState::Open);
                value["mine"] = json!(item.is_owned_by(&me));
                value
            })
            .collect();
        let counts = |stage: Option<ChecklistStage>| {
            let counts = checklist.counts(stage);
            json!({
                "open": counts.open,
                "done": counts.done,
                "waived": counts.waived,
                "total": counts.total(),
            })
        };
        let stages: serde_json::Map<String, Value> = ChecklistStage::ALL
            .into_iter()
            .map(|stage| (stage.key().to_string(), counts(Some(stage))))
            .collect();
        let all = checklist.counts(None);
        let shown: Vec<&str> = checklist
            .items
            .iter()
            .filter(|item| filter.shows(item, &me))
            .map(|item| item.id.as_str())
            .collect();
        answer["started"] = json!(true);
        answer["template"] = json!(checklist.template);
        answer["counts"] = counts(None);
        answer["chips"] = json!({
            "all": all.total(),
            "open": all.open,
            "done": all.done,
            "waived": all.waived,
            "mine": checklist.owned_by(&me),
        });
        answer["filter"] = json!(filter.key());
        answer["shown"] = json!(shown);
        answer["stages"] = Value::Object(stages);
        answer["items"] = Value::Array(items);
        answer
    }
}
