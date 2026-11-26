//! `recover_library` (#3013, mockup row 138): the choices of the Library
//! Missing and Git Repository Missing recoveries as one command. It answers
//! the recovery open for the library, raising it again first when the person
//! skipped it earlier, and sends the dialog's own choice through
//! `Signex::update`. A recovery open for another library is never answered
//! for it. Broken Binding's choices are not here: Re-bind picks a file in a
//! window, and Remove row is a library row write (#2944).

use std::path::Path;

use iced::Task;
use serde_json::json;
use signex_i18n::{Translatable, ids};

use super::library_dialog_operands::{needs, path, refuse, send, text};
use super::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::library::messages::LibraryMessage;
use crate::library::recovery::{GitMissingChoice, LibraryMissingChoice, RecoveryDialog};

type Ran = (CommandOutcome, Task<Message>);

/// The library a recovery is about.
fn recovery_library(dialog: &RecoveryDialog) -> &Path {
    match dialog {
        RecoveryDialog::LibraryMissing { path } | RecoveryDialog::GitMissing { path, .. } => path,
        RecoveryDialog::BrokenPrimitiveBinding { library_path, .. } => library_path,
    }
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

/// Make the Library Missing recovery of `library` the open one: it is open
/// already, or the person skipped it and it is raised again, as the status
/// bar chip raises it. Any other library's open recovery is left alone.
fn open_missing(app: &mut Signex, library: &Path) -> Result<(), Translatable> {
    if let Some(open) = &app.library.recovery {
        let is_it = matches!(open, RecoveryDialog::LibraryMissing { path } if path == library);
        if is_it && !app.library.missing.is_locating() {
            return Ok(());
        }
        return Err(Translatable::new(ids::LIBRARY_COMMAND_RECOVERY_OTHER)
            .arg("path", shown(recovery_library(open))));
    }
    let skipped = app.library.missing.skipped().iter().any(|p| p == library);
    match app.owning_project_of_library(library) {
        Some(owner) if skipped && app.library.library_at(library).is_none() => {
            app.library.raise_missing_library(library, owner);
            Ok(())
        }
        _ => Err(Translatable::new(ids::LIBRARY_COMMAND_RECOVERY_NOT_MISSING)
            .arg("path", shown(library))),
    }
}

/// `locate`, `remove` or `skip` on the Library Missing recovery of `library`.
fn answer_missing(app: &mut Signex, library: &Path, action: &str, args: &CommandArgs) -> Ran {
    // `locate` names its file before anything is raised or changed.
    let located = path(args, "located");
    if action == "locate" && located.is_none() {
        return refuse(Translatable::new(ids::LIBRARY_COMMAND_RECOVERY_LOCATED));
    }
    if let Err(reason) = open_missing(app, library) {
        return refuse(reason);
    }
    let task = match (action, located) {
        // Locate…'s pick, answered with `located`.
        ("locate", Some(located)) => {
            app.library.missing.start_locating(library.to_path_buf());
            send(
                app,
                LibraryMessage::RecoveryLibraryMissingLocateResult(Some(located)),
            )
        }
        ("remove", _) => send(
            app,
            LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::RemoveFromProject),
        ),
        // Skip for now is the dialog's close.
        _ => send(
            app,
            LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Cancel),
        ),
    };
    (
        CommandOutcome::Answered(answer(library, action, args)),
        task,
    )
}

/// `reinit` or `read_only` on the Git Repository Missing recovery open for
/// `library`.
fn answer_git_missing(app: &mut Signex, library: &Path, action: &str, args: &CommandArgs) -> Ran {
    let open = matches!(
        &app.library.recovery,
        Some(RecoveryDialog::GitMissing { path, .. }) if path == library
    );
    if !open {
        return refuse(
            Translatable::new(ids::LIBRARY_COMMAND_RECOVERY_NO_GIT).arg("path", shown(library)),
        );
    }
    let choice = if action == "reinit" {
        GitMissingChoice::ReInit
    } else {
        GitMissingChoice::Skip
    };
    let task = send(app, LibraryMessage::RecoveryGitMissing(choice));
    (
        CommandOutcome::Answered(answer(library, action, args)),
        task,
    )
}

fn answer(library: &Path, action: &str, args: &CommandArgs) -> serde_json::Value {
    json!({
        "library": shown(library),
        "action": action,
        "located": path(args, "located").map(|located| shown(&located)),
    })
}

/// `recover_library path=<.snxlib> action=locate|remove|skip|reinit|read_only
/// [located=<.snxlib>]`: Locate… (with `located`), Remove from project and
/// Skip for now of Library Missing; Re-init and Skip for now (the library
/// opened read-only) of Git Repository Missing. A Locate or a Re-init
/// finishes off the UI thread, as the dialog's does.
pub(super) fn recover_library(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(library) = path(args, "path") else {
        return needs("recover_library", "path");
    };
    match text(args, "action") {
        Some(action @ ("locate" | "remove" | "skip")) => {
            answer_missing(app, &library, action, args)
        }
        Some(action @ ("reinit" | "read_only")) => answer_git_missing(app, &library, action, args),
        _ => refuse(Translatable::new(ids::LIBRARY_COMMAND_RECOVERY_ACTION)),
    }
}
