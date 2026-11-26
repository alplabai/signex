//! The BOM alternates commands (#2938): `bom_add_alternate`,
//! `bom_remove_alternate` and `bom_set_alternate_order`. Each names the
//! project by `document`, the BOM line by its `line` key (the `key`
//! `query_bom` answers), and the alternate by `library` and `part` as
//! `place_component` names a row. The edit is one step of the project's
//! General history (Undo in the Project tab takes it back), marks the
//! `.snxprj` dirty and writes nothing until Save. Shapes: `docs/CLI.md`,
//! "BOM and part search".

use iced::Task;
use serde_json::{Value, json};
use signex_engine::ProjectBomCommand;
use signex_types::project::{BomAlternateProblem, bom_line_key, bom_row_key};
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue, library_row};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    let reason = signex_i18n::Translatable::new(signex_i18n::ids::BOM_ALTERNATE_REFUSED)
        .arg("why", reason.into());
    (CommandOutcome::Disabled { reason }, Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Result<&'a str, String> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Ok(text.as_str()),
        _ => Err(format!("needs the operand `{name}`")),
    }
}

/// `bom_add_alternate {document, line, library, part}`.
pub(crate) fn bom_add_alternate(app: &mut Signex, args: &CommandArgs) -> Ran {
    edit(app, args, |line, row| ProjectBomCommand::AddAlternate {
        line,
        row,
    })
}

/// `bom_remove_alternate {document, line, library, part}`.
pub(crate) fn bom_remove_alternate(app: &mut Signex, args: &CommandArgs) -> Ran {
    edit(app, args, |line, row| ProjectBomCommand::RemoveAlternate {
        line,
        row,
    })
}

/// `bom_set_alternate_order {document, line, library, part, position}`:
/// `position` 1 is the most preferred.
pub(crate) fn bom_set_alternate_order(app: &mut Signex, args: &CommandArgs) -> Ran {
    let position = match text(args, "position").map(|p| p.trim().parse::<usize>()) {
        Ok(Ok(position)) => position,
        Ok(Err(_)) => return refused("`position` is a whole number, 1 for the first alternate"),
        Err(reason) => return refused(reason),
    };
    edit(app, args, |line, row| ProjectBomCommand::MoveAlternate {
        line,
        row,
        position,
    })
}

/// Build the edit from the operands, apply it to the named project and
/// answer with the line's alternates.
fn edit(
    app: &mut Signex,
    args: &CommandArgs,
    command: impl FnOnce(String, String) -> ProjectBomCommand,
) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let line = match text(args, "line").map(bom_line_key) {
        Ok(Ok(line)) => line,
        Ok(Err(problem)) => return refused(problem.to_string()),
        Err(reason) => return refused(reason),
    };
    let row = match app.alternate_row_key(args) {
        Ok(row) => row,
        Err(reason) => return refused(reason),
    };
    match app.apply_bom_alternate(id, &command(line.clone(), row)) {
        Ok(changed) => {
            let mut answer = app.bom_line_json(id, &line);
            if let Value::Object(map) = &mut answer {
                map.insert("changed".into(), json!(changed));
            }
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(problem) => refused(problem.to_string()),
    }
}

impl Signex {
    /// The library row key `library` and `part` name: a row of an open
    /// library, as `place_component` finds it, or, for a library that is
    /// not open, its id and the row id as given, so an alternate of a
    /// library that is not mounted can still be taken off.
    fn alternate_row_key(&self, args: &CommandArgs) -> Result<String, String> {
        let (library, part) = (text(args, "library")?, text(args, "part")?);
        match self.open_library_named(library) {
            Ok(open) => {
                let (_, row) = library_row(open, part)?;
                Ok(format!("{}/{row}", open.library_id))
            }
            Err(not_open) => match (
                Uuid::parse_str(library.trim()),
                Uuid::parse_str(part.trim()),
            ) {
                (Ok(_), Ok(_)) => bom_row_key(&format!("{library}/{part}"))
                    .map_err(|problem: BomAlternateProblem| problem.to_string()),
                _ => Err(not_open),
            },
        }
    }

    /// Apply one alternates edit to project `id` as one step of its General
    /// history and mark the `.snxprj` dirty. `Ok(false)` when nothing
    /// changed or the project is no longer loaded.
    pub(crate) fn apply_bom_alternate(
        &mut self,
        project_id: ProjectId,
        command: &ProjectBomCommand,
    ) -> Result<bool, BomAlternateProblem> {
        let document = &mut self.document_state;
        document.note_project_clean_state(project_id);
        let Some(project) = document.projects.iter_mut().find(|p| p.id == project_id) else {
            return Ok(false);
        };
        let history = document.general_history.entry(project_id).or_default();
        if !history.apply_bom_alternate(&mut project.data, command)? {
            return Ok(false);
        }
        document.sync_project_history_dirty(project_id, false);
        self.after_project_general_change(false);
        Ok(true)
    }

    /// `{line, alternates}` of project `id`'s line `line`.
    fn bom_line_json(&self, id: ProjectId, line: &str) -> Value {
        let alternates = self
            .document_state
            .project_by_id(id)
            .map(|project| project.data.bom_alternates_of(line).to_vec())
            .unwrap_or_default();
        json!({ "line": line, "alternates": alternates })
    }
}
