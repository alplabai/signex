//! The Fonts catalog commands (#1178): the agent and CLI surface of the
//! section. Each names the project by `document` and runs the tab's commit
//! path (one General step), answering with the font list. `project_add_font`
//! is answered once its file is copied (`CommandOutcome::Deferred`). Shapes:
//! `docs/PROJECT_TAB.md`, "Fonts".

use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::ProjectFontCommand;
use signex_i18n::{Translatable, ids};

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// The project `document` names: the project itself or one of its
/// documents.
fn named_project(app: &mut Signex, args: &CommandArgs) -> Result<ProjectId, Translatable> {
    app.rules_project_named(args).map_err(|_| {
        let named = match args.operand("document") {
            Some(OperandValue::Document(target)) => target.to_string(),
            _ => String::new(),
        };
        Translatable::new(ids::PROJECT_FONTS_NO_PROJECT).arg("document", named)
    })
}

/// Apply `command` to project `id` and answer with the font list.
fn edit(app: &mut Signex, id: ProjectId, command: &ProjectFontCommand) -> Ran {
    match app.apply_project_font(id, command) {
        Ok(changed) => {
            let mut answer = app.project_fonts_json(id);
            if let Value::Object(map) = &mut answer {
                map.insert("changed".into(), json!(changed));
            }
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(problem) => refused(super::problem_words(&problem)),
    }
}

/// `project_add_font {document, path}`: copy the file into the project's
/// font folder off the UI thread, then list it; answered then.
pub(crate) fn project_add_font(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let Some(OperandValue::Path(path)) = args.operand("path") else {
        return refused(Translatable::new(ids::PROJECT_FONTS_NOT_A_FONT_FILE).arg("file", ""));
    };
    let source = PathBuf::from(path);
    let Ok(command) = AppCommandId::new("project_add_font") else {
        return refused(Translatable::new(ids::PROJECT_FONTS_PROJECT_GONE));
    };
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    match app.begin_font_store(id, source, Some(Box::new(waiting))) {
        Ok(task) => (CommandOutcome::Deferred { ticket }, task),
        Err(reason) => refused(reason),
    }
}

/// `project_remove_font {document, family}`.
pub(crate) fn project_remove_font(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let family = text(args, "family").unwrap_or_default().to_string();
    edit(app, id, &ProjectFontCommand::Remove { family })
}

/// `project_set_font_license {document, family, value}`; an empty value
/// clears it.
pub(crate) fn project_set_font_license(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let command = ProjectFontCommand::SetLicense {
        family: text(args, "family").unwrap_or_default().to_string(),
        license: text(args, "value").unwrap_or_default().to_string(),
    };
    edit(app, id, &command)
}
