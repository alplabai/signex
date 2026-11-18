//! The `project_*` catalog commands (#1689-#1693): the agent and CLI surface
//! of the Project tab's sections. `project_settings` answers every section as
//! JSON; each edit runs the same commit path as the tab's control (one step
//! of the project's General history) and answers with its section. The
//! project is named by `document` (the `.snxprj` or any of its documents).
//! Shapes: `docs/PROJECT_TAB.md`, "Commands".

use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::ProjectSectionCommand;
use signex_types::project::{AnnotateUndo, LibraryEntryKind, ListMove};

use super::erc_annotation::ErcSource;
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

fn direction(args: &CommandArgs) -> Result<ListMove, String> {
    text(args, "direction")
        .and_then(ListMove::parse)
        .ok_or_else(|| "`direction` is up or down".to_string())
}

/// The section a command's answer carries.
#[derive(Clone, Copy)]
enum Section {
    Sheets,
    Variants,
    Libraries,
    Annotation,
    VersionControl,
}

/// Apply `command` to the project `args` names and answer with `section`.
fn edit(
    app: &mut Signex,
    args: &CommandArgs,
    command: ProjectSectionCommand,
    section: Section,
) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    if let ProjectSectionCommand::SetVersionControl { on } = command
        && let Some(reason) = app.version_control_refusal(id, on)
    {
        return refused(reason);
    }
    match app.apply_project_section(id, &command) {
        Ok(changed) => {
            let mut answer = app.project_section_json(id, section);
            if let Value::Object(map) = &mut answer {
                map.insert("changed".into(), json!(changed));
            }
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(problem) => refused(problem.to_string()),
    }
}

/// `project_move_sheet {document, sheet, direction}`.
pub(crate) fn project_move_sheet(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(filename) = text(args, "sheet") else {
        return refused("needs the operand `sheet`: a sheet file of the project");
    };
    let step = match direction(args) {
        Ok(step) => step,
        Err(reason) => return refused(reason),
    };
    let known = app
        .rules_project_named(args)
        .ok()
        .and_then(|id| app.document_state.project_by_id(id))
        .is_some_and(|p| p.data.sheets.iter().any(|s| s.filename == filename));
    if !known {
        return refused(format!("the project lists no sheet `{filename}`"));
    }
    let command = ProjectSectionCommand::MoveSheet {
        filename: filename.to_string(),
        step,
    };
    edit(app, args, command, Section::Sheets)
}

/// `project_add_variant {document, name}`.
pub(crate) fn project_add_variant(app: &mut Signex, args: &CommandArgs) -> Ran {
    let name = text(args, "name").unwrap_or_default().to_string();
    edit(
        app,
        args,
        ProjectSectionCommand::AddVariant { name },
        Section::Variants,
    )
}

/// `project_rename_variant {document, name, value}`.
pub(crate) fn project_rename_variant(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (Some(from), Some(to)) = (text(args, "name"), text(args, "value")) else {
        return refused("needs the operands `name` and `value`");
    };
    let command = ProjectSectionCommand::RenameVariant {
        from: from.to_string(),
        to: to.to_string(),
    };
    edit(app, args, command, Section::Variants)
}

/// `project_remove_variant {document, name}`.
pub(crate) fn project_remove_variant(app: &mut Signex, args: &CommandArgs) -> Ran {
    let name = text(args, "name").unwrap_or_default().to_string();
    edit(
        app,
        args,
        ProjectSectionCommand::RemoveVariant { name },
        Section::Variants,
    )
}

/// `project_set_active_variant {document, name?}`; none is Base.
pub(crate) fn project_set_active_variant(app: &mut Signex, args: &CommandArgs) -> Ran {
    let name = text(args, "name").map(str::to_string);
    edit(
        app,
        args,
        ProjectSectionCommand::SetActiveVariant { name },
        Section::Variants,
    )
}

/// `project_move_library {document, library, direction}`: `library` is the
/// path as the project stores it.
pub(crate) fn project_move_library(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(library) = text(args, "library") else {
        return refused("needs the operand `library`: a library path as the project stores it");
    };
    let step = match direction(args) {
        Ok(step) => step,
        Err(reason) => return refused(reason),
    };
    let path = PathBuf::from(library);
    let known = app
        .rules_project_named(args)
        .ok()
        .and_then(|id| app.document_state.project_by_id(id))
        .is_some_and(|p| p.data.libraries.iter().any(|l| l.path == path));
    if !known {
        return refused(format!("the project lists no library `{library}`"));
    }
    edit(
        app,
        args,
        ProjectSectionCommand::MoveLibrary { path, step },
        Section::Libraries,
    )
}

/// `project_set_annotate_undo {document, value?}`: `project` or
/// `per_sheet`; none follows Preferences.
pub(crate) fn project_set_annotate_undo(app: &mut Signex, args: &CommandArgs) -> Ran {
    let choice = match text(args, "value") {
        None => None,
        Some(token) => match AnnotateUndo::parse(token) {
            Some(choice) => Some(choice),
            None => {
                return refused("`value` is project or per_sheet; leave it out for Preferences");
            }
        },
    };
    edit(
        app,
        args,
        ProjectSectionCommand::SetAnnotateUndo { choice },
        Section::Annotation,
    )
}

/// `project_set_version_control {document, on}`.
pub(crate) fn project_set_version_control(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Bool(on)) = args.operand("on") else {
        return refused("needs the operand `on`");
    };
    let command = ProjectSectionCommand::SetVersionControl { on: *on };
    edit(app, args, command, Section::VersionControl)
}

fn kind_token(kind: LibraryEntryKind) -> &'static str {
    match kind {
        LibraryEntryKind::ProjectLocal => "project_local",
        LibraryEntryKind::Shared => "shared",
        LibraryEntryKind::Global => "global",
    }
}

impl Signex {
    /// `project_settings {document}`: every section of the project's tab.
    pub(crate) fn project_settings(&mut self, args: &CommandArgs) -> Result<Value, String> {
        let id = self.rules_project_named(args)?;
        let path = self
            .document_state
            .project_by_id(id)
            .map(|p| p.path.display().to_string())
            .ok_or("the project is no longer open")?;
        let mut answer = json!({ "project": path });
        for section in [
            Section::Sheets,
            Section::Variants,
            Section::Libraries,
            Section::Annotation,
            Section::VersionControl,
        ] {
            if let (Value::Object(all), Value::Object(part)) =
                (&mut answer, self.project_section_json(id, section))
            {
                all.extend(part);
            }
        }
        // #2339: the Requirements section.
        if let (Value::Object(all), Value::Object(part)) =
            (&mut answer, self.project_requirements_json(id))
        {
            all.extend(part);
        }
        Ok(answer)
    }

    /// One section of `id` as a JSON object keyed by the section.
    fn project_section_json(&self, id: ProjectId, section: Section) -> Value {
        let Some(project) = self.document_state.project_by_id(id) else {
            return json!({});
        };
        let data = &project.data;
        match section {
            Section::Sheets => {
                let sheets: Vec<Value> = data
                    .sheets
                    .iter()
                    .enumerate()
                    .map(|(index, s)| json!({ "page": index + 1, "name": s.name, "file": s.filename }))
                    .collect();
                json!({ "sheets": sheets })
            }
            Section::Variants => json!({
                "variants": {
                    "defined": data.variant_definitions,
                    "active": data.active_variant_name(),
                }
            }),
            Section::Libraries => {
                let rows: Vec<Value> = self
                    .project_library_rows(project)
                    .iter()
                    .map(|row| {
                        json!({
                            "name": row.name(),
                            "path": row.stored.display().to_string(),
                            "kind": kind_token(row.kind),
                            "state": row.mount.token(),
                        })
                    })
                    .collect();
                json!({ "libraries": rows })
            }
            Section::Annotation => {
                let own = data.annotate_undo();
                let effective = own.unwrap_or(self.ui_state.annotate_undo);
                json!({
                    "annotation": {
                        "annotate_undo": own.map(AnnotateUndo::token),
                        "annotate_undo_effective": effective.token(),
                        "erc_rules": ErcSource::of(project).token(),
                    }
                })
            }
            Section::VersionControl => {
                let facts = self.version_control_facts(project);
                json!({
                    "version_control": {
                        "commit_on_save": data.enable_git,
                        "repository": facts.repository.token(),
                        "pending_commits": facts.pending,
                    }
                })
            }
        }
    }
}
