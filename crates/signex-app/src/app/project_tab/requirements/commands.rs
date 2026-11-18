//! The Requirements catalog commands (#2339): the agent and CLI surface of
//! the section. Each edit names the project by `document` and the entry by
//! its `requirement` id, runs the tab's commit path (one General step) and
//! answers with the list; `project_run_requirements` runs the tab's checks.
//! Shapes: `docs/PROJECT_TAB.md`, "Commands".

use iced::Task;
use serde_json::{Value, json};
use signex_engine::ProjectRequirementCommand;
use signex_types::project::RequirementField;

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (
        CommandOutcome::Disabled {
            reason: reason.into(),
        },
        Task::none(),
    )
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// The project `args` names and the place of the requirement it names.
fn named_entry(app: &mut Signex, args: &CommandArgs) -> Result<(ProjectId, usize), String> {
    let id = app.rules_project_named(args)?;
    let Some(requirement) = text(args, "requirement") else {
        return Err("needs the operand `requirement`: a requirement id".into());
    };
    let project = app
        .document_state
        .project_by_id(id)
        .ok_or("the project is no longer open")?;
    let index = project
        .data
        .requirement_index(requirement)
        .map_err(|problem| problem.to_string())?;
    Ok((id, index))
}

/// Apply `command` to project `id` and answer with the list.
fn edit(app: &mut Signex, id: ProjectId, command: &ProjectRequirementCommand) -> Ran {
    match app.apply_project_requirement(id, command) {
        Ok(changed) => {
            let mut answer = app.project_requirements_json(id);
            if let Value::Object(map) = &mut answer {
                map.insert("changed".into(), json!(changed));
            }
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(problem) => refused(problem.to_string()),
    }
}

/// `project_add_requirement {document, kind, id?}`.
pub(crate) fn project_add_requirement(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let Some(kind) = text(args, "kind") else {
        return refused("needs the operand `kind`: a requirement kind");
    };
    let command = ProjectRequirementCommand::Add {
        kind: kind.trim().to_string(),
        id: text(args, "id").map(str::to_string),
    };
    edit(app, id, &command)
}

/// `project_set_requirement {document, requirement, field, value}`.
pub(crate) fn project_set_requirement(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (id, index) = match named_entry(app, args) {
        Ok(found) => found,
        Err(reason) => return refused(reason),
    };
    let Some(key) = text(args, "field") else {
        return refused("needs the operand `field`: a field of the requirement");
    };
    let Some(field) = RequirementField::parse(key.trim()) else {
        return refused(format!("`{key}` is no requirement field"));
    };
    let command = ProjectRequirementCommand::SetField {
        index,
        field,
        text: text(args, "value").unwrap_or_default().to_string(),
    };
    edit(app, id, &command)
}

/// `project_remove_requirement {document, requirement}`.
pub(crate) fn project_remove_requirement(app: &mut Signex, args: &CommandArgs) -> Ran {
    match named_entry(app, args) {
        Ok((id, index)) => edit(app, id, &ProjectRequirementCommand::Remove { index }),
        Err(reason) => refused(reason),
    }
}

/// `project_enable_requirement {document, requirement, on}`.
pub(crate) fn project_enable_requirement(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Bool(on)) = args.operand("on") else {
        return refused("needs the operand `on`");
    };
    let on = *on;
    match named_entry(app, args) {
        Ok((id, index)) => edit(app, id, &ProjectRequirementCommand::SetEnabled { index, on }),
        Err(reason) => refused(reason),
    }
}

/// `project_run_requirements {document}`: Run checks in the project's tab.
/// The results land in the tab; `check_requirements` answers with them.
pub(crate) fn project_run_requirements(app: &mut Signex, args: &CommandArgs) -> Ran {
    match app.rules_project_named(args) {
        Ok(id) => (CommandOutcome::Ran, app.run_requirements_in_tab(id)),
        Err(reason) => refused(reason),
    }
}

impl Signex {
    /// The requirements of project `id` as stored, and why an entry cannot
    /// be read.
    pub(crate) fn project_requirements_json(&self, id: ProjectId) -> Value {
        let Some(project) = self.document_state.project_by_id(id) else {
            return json!({});
        };
        let data = &project.data;
        let list: Vec<&Value> = data.requirements.iter().map(|e| e.raw()).collect();
        let unreadable: Vec<Value> = data
            .requirements
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| match entry {
                signex_types::project::RequirementEntry::Unreadable { reason, .. } => {
                    Some(json!({ "index": index, "reason": reason }))
                }
                _ => None,
            })
            .collect();
        json!({
            "requirements": list,
            "unreadable": unreadable,
            "list_unreadable": data.has_unreadable_requirements(),
        })
    }
}
