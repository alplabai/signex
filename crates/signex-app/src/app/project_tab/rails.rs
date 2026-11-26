//! `project_set_rail` (#2943): set, change or remove one of the project's
//! power rails (a net's nominal voltage and tolerance). The project is named
//! by `document` (the `.snxprj` or any of its documents); the edit is one
//! step of the project's General history, marks the `.snxprj` dirty and
//! writes nothing until Save. It answers with the rails as stored. Shapes:
//! `docs/CLI.md`, "Rail budgets".

use iced::Task;
use serde_json::{Value, json};
use signex_engine::ProjectRailCommand;
use signex_types::project::{RailEdit, RailProblem};

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn text(args: &CommandArgs, name: &str) -> Option<String> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

/// `project_set_rail {document, net, voltage?, tolerance_percent?, remove?}`.
pub(crate) fn project_set_rail(app: &mut Signex, args: &CommandArgs) -> Ran {
    let refused = |why: String| {
        let reason = signex_i18n::Translatable::new(signex_i18n::ids::REQUIREMENTS_RAIL_REFUSED)
            .arg("why", why);
        (CommandOutcome::Disabled { reason }, Task::none())
    };
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let Some(net) = text(args, "net") else {
        return refused("needs the operand `net`: the rail's net name".into());
    };
    let remove = matches!(args.operand("remove"), Some(OperandValue::Bool(true)));
    let command = ProjectRailCommand {
        net,
        edit: RailEdit {
            voltage: text(args, "voltage"),
            tolerance_percent: text(args, "tolerance_percent"),
            remove,
        },
    };
    match app.apply_project_rail(id, &command) {
        Ok(changed) => {
            let mut answer = app.project_rails_json(id);
            if let Value::Object(map) = &mut answer {
                map.insert("changed".into(), json!(changed));
            }
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(problem) => refused(problem.to_string()),
    }
}

impl Signex {
    /// Apply one rails edit to the project's data as one step of its General
    /// history and mark the `.snxprj` dirty. `Ok(false)` when nothing
    /// changed or the project is no longer loaded.
    pub(crate) fn apply_project_rail(
        &mut self,
        project_id: ProjectId,
        command: &ProjectRailCommand,
    ) -> Result<bool, RailProblem> {
        let document = &mut self.document_state;
        document.note_project_clean_state(project_id);
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            crate::diagnostics::log_warning(
                "Project tab: the project was closed; the edit was dropped",
            );
            return Ok(false);
        };
        let tab = document.project_tabs.entry(project_id).or_default();
        if !tab.general_history.apply_rail(&mut project.data, command)? {
            return Ok(false);
        }
        // Rails are not read by ERC: no ERC result goes stale.
        document.sync_project_history_dirty(project_id, false);
        self.after_project_general_change(false);
        Ok(true)
    }

    /// The project's rails as stored, and why they cannot be read.
    pub(crate) fn project_rails_json(&self, id: ProjectId) -> Value {
        let Some(project) = self.document_state.project_by_id(id) else {
            return json!({});
        };
        let data = &project.data;
        json!({
            "rails": data.rails_value().unwrap_or_else(|| json!({})),
            "unreadable": data.rails().err(),
        })
    }
}
