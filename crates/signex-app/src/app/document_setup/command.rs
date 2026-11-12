//! `new_pcb_document profile=..` and `new_schematic_sheet profile=..
//! [name=..]`: make the document with a profile and no dialog (#2232), for
//! headless runs and agents. They run the same paths as the dialogs' Create.

use iced::Task;
use signex_types::project::ProjectSetup;

use super::SheetChoice;
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

fn text<'a>(args: &'a CommandArgs, key: &str) -> Option<&'a str> {
    match args.operand(key) {
        Some(OperandValue::Text(t) | OperandValue::Path(t)) => Some(t.as_str()),
        _ => None,
    }
}

/// The project in view and the setup its `profile` operand names.
fn project_and_setup(
    app: &Signex,
    args: &CommandArgs,
    command: &str,
) -> Result<(ProjectId, ProjectSetup), String> {
    let token =
        text(args, "profile").ok_or_else(|| format!("`{command}` needs the operand `profile`"))?;
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

/// `new_schematic_sheet profile=.. [name=..]`: a blank sheet named
/// `<name>.snxsch` in the project folder (default `Sheet<n>`, the first
/// number the project does not list), with the profile's sheet values, as
/// the New Schematic dialog's Create and file dialog make it.
pub(crate) fn new_schematic_sheet(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (project, setup) = match project_and_setup(app, args, "new_schematic_sheet") {
        Ok(found) => found,
        Err(why) => return refused(why),
    };
    let Some(loaded) = app.document_state.project_by_id(project) else {
        return refused("`new_schematic_sheet`: the project is no longer open");
    };
    let stem = match text(args, "name") {
        Some(name) => match crate::app::new_project::check_name(name) {
            Ok(name) => name.trim_end_matches(".snxsch").to_string(),
            Err(_) => return refused(format!("`{name}` is not a usable sheet name")),
        },
        None => free_sheet_stem(loaded),
    };
    let file = format!("{stem}.snxsch");
    let listed = loaded
        .data
        .sheets
        .iter()
        .any(|s| s.filename.eq_ignore_ascii_case(&file));
    if listed {
        return refused(format!(
            "`new_schematic_sheet`: the project already lists {file}"
        ));
    }
    let path = loaded.dir().join(&file);
    let choice = SheetChoice {
        sheet: setup.sheet,
        designators: setup.designators,
    };
    let task = app.handle_add_new_schematic_picked(project, Some(path), Some(choice));
    (CommandOutcome::Ran, task)
}

/// `Sheet<n>` with the first `n` the project does not list.
fn free_sheet_stem(project: &crate::app::LoadedProject) -> String {
    (1..)
        .map(|n| format!("Sheet{n}"))
        .find(|stem| {
            let file = format!("{stem}.snxsch");
            !project
                .data
                .sheets
                .iter()
                .any(|s| s.filename.eq_ignore_ascii_case(&file))
        })
        .unwrap_or_else(|| "Sheet".to_string())
}
