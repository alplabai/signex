//! `new_pcb_document [profile=..]` and `new_schematic_sheet [profile=..]
//! [name=..]`: make the document with a profile and no dialog (#2232), for
//! headless runs and agents. They run the same paths as the dialogs' Create.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::project::{PROJECT_PROFILE_TOKEN, ProjectSetup};

use super::SheetChoice;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (
        CommandOutcome::Disabled {
            reason: reason.into(),
        },
        Task::none(),
    )
}

fn text<'a>(args: &'a CommandArgs, key: &str) -> Option<&'a str> {
    match args.operand(key) {
        Some(OperandValue::Text(t) | OperandValue::Path(t)) => Some(t.as_str()),
        _ => None,
    }
}

/// The project in view and the setup its `profile` operand names.
fn project_and_setup(
    app: &mut Signex,
    args: &CommandArgs,
    command: &str,
) -> Result<(ProjectId, ProjectSetup), String> {
    // No `profile`: the project's own setup, as before the dialog existed.
    let token = text(args, "profile").unwrap_or(PROJECT_PROFILE_TOKEN);
    let project = app
        .project_in_view()
        .ok_or_else(|| format!("`{command}` needs an open project"))?;
    let resolved = app
        .resolved_project_setup(project)
        .ok_or_else(|| format!("`{command}`: the project is no longer open"))?;
    let setup = ProjectSetup::for_command(token, &resolved, &app.ui_state.setup_profiles)?;
    Ok((project, setup))
}

/// `new_pcb_document profile=..`: the project's board with the profile's
/// rules, opened unsaved, as the New PCB dialog's Create makes it.
pub(crate) fn new_pcb_document(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (project, setup) = match project_and_setup(app, args, "new_pcb_document") {
        Ok(found) => found,
        Err(why) => return refused(why),
    };
    let Some(loaded) = app.document_state.project_by_id(project) else {
        return refused("`new_pcb_document`: the project is no longer open");
    };
    let named = Signex::project_board(loaded);
    let path = named
        .clone()
        .unwrap_or_else(|| crate::app::update_pcb::new_board_path(&loaded.path));
    if app.document_state.holds_pcb(&path) || app.is_board_opening(&path) {
        return refused(format!(
            "`new_pcb_document`: {} is already open; a project holds one board",
            path.display()
        ));
    }
    let task = app.request_new_pcb(Some(project), Some(setup.pcb));
    (CommandOutcome::Ran, task)
}

/// `new_schematic_sheet [profile=..] [name=..]`: a blank sheet named
/// `<name>.snxsch` in the project folder, with the profile's sheet values, as
/// the New Schematic dialog's Create and file dialog make it. With no `name`
/// it is the first `Sheet<n>` the project does not list and the folder does
/// not hold, found off the update thread.
pub(crate) fn new_schematic_sheet(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (project, setup) = match project_and_setup(app, args, "new_schematic_sheet") {
        Ok(found) => found,
        Err(why) => return refused(why),
    };
    let Some(loaded) = app.document_state.project_by_id(project) else {
        return refused("`new_schematic_sheet`: the project is no longer open");
    };
    let choice = SheetChoice {
        sheet: setup.sheet,
        designators: setup.designators,
    };
    let listed: Vec<String> = loaded
        .data
        .sheets
        .iter()
        .map(|s| s.filename.to_lowercase())
        .collect();
    let dir = loaded.dir().to_path_buf();
    let Some(name) = text(args, "name") else {
        let task = Task::perform(
            on_blocking_pool(move || Ok(free_sheet_path(&dir, &listed))),
            move |found| match found {
                Ok(path) => Message::Project(ProjectMsg::AddNewSchematicPicked {
                    project_id: project,
                    path: Some(path),
                    choice: Some(choice.clone()),
                }),
                Err(_) => Message::noop(),
            },
        );
        return (CommandOutcome::Ran, task);
    };
    let stem = match crate::app::new_project::check_name(name) {
        Ok(name) => strip_sheet_extension(name).to_string(),
        Err(_) => return refused(format!("`{name}` is not a usable sheet name")),
    };
    let file = format!("{stem}.snxsch");
    if listed.contains(&file.to_lowercase()) {
        return refused(format!(
            "`new_schematic_sheet`: the project already lists {file}"
        ));
    }
    let task = app.handle_add_new_schematic_picked(project, Some(dir.join(&file)), Some(choice));
    (CommandOutcome::Ran, task)
}

/// `name` without a trailing `.snxsch`, in any case.
fn strip_sheet_extension(name: &str) -> &str {
    const EXT: &str = ".snxsch";
    match name.len().checked_sub(EXT.len()) {
        Some(at) if name.is_char_boundary(at) && name[at..].eq_ignore_ascii_case(EXT) => {
            &name[..at]
        }
        _ => name,
    }
}

/// `<dir>/Sheet<n>.snxsch` with the first `n` that `listed` (lower-case
/// file names) does not name and `dir` does not hold. Blocking: reads `dir`.
fn free_sheet_path(dir: &Path, listed: &[String]) -> PathBuf {
    (1..=9999)
        .map(|n| format!("Sheet{n}.snxsch"))
        .find(|file| !listed.contains(&file.to_lowercase()) && !dir.join(file).exists())
        .map_or_else(|| dir.join("Sheet.snxsch"), |file| dir.join(file))
}

#[cfg(test)]
mod name_tests {
    #[test]
    fn the_extension_is_stripped_in_any_case() {
        assert_eq!(super::strip_sheet_extension("Power.SNXSCH"), "Power");
        assert_eq!(super::strip_sheet_extension("Power.snxsch"), "Power");
        assert_eq!(super::strip_sheet_extension("Power"), "Power");
    }
}
