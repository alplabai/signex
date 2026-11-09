//! The operand handlers of `open_document` and `new_document` (#2155): the
//! `path` goes to the functions the menu entry's dialog answer goes to, so a
//! surface without a dialog opens and creates exactly what the menu does, and
//! a refusal is logged and raised as the same notice.

use std::path::PathBuf;

use iced::Task;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::io_ports::replace_refusal;

type Ran = (CommandOutcome, Task<Message>);

fn path_of(args: &CommandArgs, id: &str) -> Result<PathBuf, String> {
    match args.operand("path") {
        Some(OperandValue::Path(path)) => Ok(PathBuf::from(path)),
        _ => Err(format!("`{id}` needs the operand `path`")),
    }
}

fn outcome(result: Result<Task<Message>, String>) -> Ran {
    match result {
        Ok(task) => (CommandOutcome::Ran, task),
        Err(reason) => (CommandOutcome::disabled(reason), Task::none()),
    }
}

/// `open_document path=..`: open that file, as the Open dialog's answer does.
pub(super) fn open(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match path_of(args, "open_document") {
        Ok(path) => path,
        Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
    };
    outcome(app.open_path_reported(path))
}

/// `new_document path=<dir>/<name>.snxprj`: create that project, as the New
/// Project dialog's answer does.
pub(super) fn new_project(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match path_of(args, "new_document") {
        Ok(path) => path,
        Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
    };
    // The headless dialog's rule: an existing file at the target is replaced
    // only with `overwrite=true`; this is the refusal, before anything runs.
    let may_replace = matches!(args.operand("overwrite"), Some(OperandValue::Bool(true)));
    if let Some(reason) = replace_refusal(&path, may_replace) {
        crate::diagnostics::log_warning(format!("New project refused: {reason}"));
        return outcome(Err(reason));
    }
    outcome(app.start_new_project(
        path,
        crate::app::handlers::document_files::new_project::NewProjectPlan::default(),
    ))
}
