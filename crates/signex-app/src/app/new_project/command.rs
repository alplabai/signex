//! `new_project_with_profile`: create a project from operands, with the
//! choices the dialog offers and no dialog (#2232). For headless runs and
//! agents; it writes through the same worker as the dialog's Create.

use iced::Task;
use signex_types::project::{ProjectSetup, SetupProfile};

use super::check_name;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::document_files::new_project::{NewProjectLayout, NewProjectPlan};
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

/// Read the operands into the setup the dialog would have produced.
fn setup_from(args: &CommandArgs) -> Result<ProjectSetup, String> {
    let profile = match text(args, "profile") {
        None => SetupProfile::Standard,
        Some(t) => SetupProfile::from_token(t)
            .ok_or_else(|| format!("profile `{t}` is not hobby, standard, hdi or custom"))?,
    };
    let mut setup = ProjectSetup::from_profile(profile);
    if let Some(overrides) = text(args, "overrides") {
        setup.apply_overrides(overrides)?;
    }
    Ok(setup)
}

/// `new_project_with_profile name=.. location=.. [make_folder=..] [profile=..]
/// [overrides=..]`.
pub(crate) fn new_project_with_profile(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (Some(name), Some(location)) = (text(args, "name"), text(args, "location")) else {
        return refused("`new_project_with_profile` needs the operands `name` and `location`");
    };
    let name = match check_name(name) {
        Ok(name) => name.to_string(),
        Err(_) => return refused(format!("`{name}` is not a usable project name")),
    };
    let setup = match setup_from(args) {
        Ok(setup) => setup,
        Err(reason) => return refused(reason),
    };
    let make_folder = !matches!(args.operand("make_folder"), Some(OperandValue::Bool(false)));
    let layout = if make_folder {
        NewProjectLayout::NamedFolder
    } else {
        NewProjectLayout::InPlace
    };
    let chosen = std::path::Path::new(location).join(format!("{name}.snxprj"));
    let plan = NewProjectPlan {
        layout,
        setup: Some(setup),
    };
    match app.start_new_project(chosen, plan) {
        Ok(task) => (CommandOutcome::Ran, task),
        Err(reason) => refused(reason),
    }
}
