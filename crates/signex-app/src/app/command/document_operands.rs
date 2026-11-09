//! The commands that act on a document an agent names (#2172): `color_net`,
//! `clear_all_net_colors`, `undo`, `redo`, `save_document`,
//! `save_document_as` and `close_active_document`.
//!
//! Run from operands, each needs the `document` operand (an id or a path) and
//! acts on exactly that document, never on the active one: nothing is looked
//! up through the active tab or a window, and none is changed. A person's
//! palette or keybinding call has no operands and keeps the active document
//! as before. A document the command cannot act on (not open, not a sheet,
//! read-only) is refused with the reason; nothing is reported as run that did
//! not run.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::theme::Color;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document::{DocumentKey, UndoTarget};
use crate::app::document_ids::DocumentEntry;
use crate::app::handlers::canvas::net_color_click::net_elements_in;
use crate::app::{Message, Signex};

/// What running one command from its operands did, and any follow-up task.
type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn ran(task: Task<Message>) -> Ran {
    (CommandOutcome::Ran, task)
}

/// The document `args` names, or the refusal.
fn named(app: &Signex, args: &CommandArgs) -> Result<DocumentEntry, String> {
    match args.operand("document") {
        Some(OperandValue::Document(target)) => app.resolve_document_target(target),
        _ => Err("the operand `document` is needed".to_string()),
    }
}

/// The path of the loaded sheet `args` names.
pub(super) fn named_sheet(app: &Signex, args: &CommandArgs, what: &str) -> Result<PathBuf, String> {
    let entry = named(app, args)?;
    match entry.key {
        Some(DocumentKey::Schematic(path)) => Ok(path),
        Some(_) => Err(format!(
            "{what} acts on a sheet, and {} is a {}",
            entry.name,
            entry.kind.as_str()
        )),
        None => Err(format!(
            "{} is referenced by a project but not loaded; open it first",
            entry.name
        )),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// `color_net at=x,y color=#rrggbb document=..`: colour the net whose wire is
/// under the point, on that sheet, as a pen click does on the active one.
pub(super) fn color_net(app: &mut Signex, args: &CommandArgs) -> Ran {
    let (Some(OperandValue::Point(x, y)), Some(OperandValue::Color(r, g, b))) =
        (args.operand("at"), args.operand("color"))
    else {
        return refused("`color_net` needs the operands `at` and `color`");
    };
    let path = match named_sheet(app, args, "`color_net`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    if let Some(reason) = app.refuse_if_read_only_at(&path) {
        return refused(reason);
    }
    let Some(engine) = app.document_state.engines.get(&path) else {
        return refused(format!("{} is not loaded", path.display()));
    };
    let net = net_elements_in(engine.document(), *x, *y);
    if net.is_empty() {
        return refused(format!("there is no wire at {x},{y}"));
    }
    let color = Color::new(*r, *g, *b, 255);
    let mut colors = engine.document().net_colors.clone();
    colors.extend(net.into_iter().map(|id| (id, color)));
    match app.execute_on_sheet(&path, signex_engine::Command::SetNetColors { colors }) {
        Ok(true) => ran(Task::none()),
        Ok(false) => refused("the net already has that colour"),
        Err(reason) => refused(reason),
    }
}

/// `clear_all_net_colors document=..`: clear the net colours of that sheet.
pub(super) fn clear_all_net_colors(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match named_sheet(app, args, "`clear_all_net_colors`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    if let Some(reason) = app.refuse_if_read_only_at(&path) {
        return refused(format!(
            "the sheet is read-only: its net colours were kept ({reason})"
        ));
    }
    let painted = app.document_state.engines.get(&path).is_some_and(|engine| {
        let sheet = engine.document();
        sheet
            .wires
            .iter()
            .map(|w| w.uuid)
            .chain(sheet.junctions.iter().map(|j| j.uuid))
            .any(|id| sheet.net_colors.contains_key(&id))
    });
    if !painted {
        return refused("there are no net colours to clear");
    }
    let cleared = signex_engine::Command::SetNetColors {
        colors: Default::default(),
    };
    match app.execute_on_sheet(&path, cleared) {
        Ok(true) => {
            crate::diagnostics::log_info(format!(
                "Cleared net-color overrides on {}",
                file_name(&path)
            ));
            ran(Task::none())
        }
        Ok(false) => refused("there are no net colours to clear"),
        Err(reason) => refused(reason),
    }
}

/// `undo document=..` / `redo document=..`: one step of that document's own
/// history.
fn step(app: &mut Signex, args: &CommandArgs, undo: bool) -> Ran {
    let entry = match named(app, args) {
        Ok(entry) => entry,
        Err(reason) => return refused(reason),
    };
    let Some(key) = entry.key else {
        return refused(format!("{} is not loaded", entry.name));
    };
    let (can_undo, can_redo) = app.undo_availability(&UndoTarget::Document(key.clone()));
    if !(if undo { can_undo } else { can_redo }) {
        return refused(if undo {
            "Nothing to undo"
        } else {
            "Nothing to redo"
        });
    }
    // A wire or arc in progress on the active sheet is anchored to what is
    // there; stepping the sheet under it would leave it on an undone segment
    // (#1831). The person's drawing is not the agent's to cancel.
    if let DocumentKey::Schematic(path) = &key
        && app.is_drawing_on(path)
    {
        return refused("the person is drawing on this sheet; try again when the wire is done");
    }
    match &key {
        DocumentKey::Schematic(path) => match app.step_sheet_on(path, undo) {
            Ok(true) => ran(Task::none()),
            Ok(false) => refused(if undo {
                "Nothing to undo"
            } else {
                "Nothing to redo"
            }),
            Err(reason) => refused(reason),
        },
        _ => refused(format!(
            "{} is a {}: undo by document is not available for that kind yet",
            entry.name,
            entry.kind.as_str()
        )),
    }
}

pub(super) fn undo(app: &mut Signex, args: &CommandArgs) -> Ran {
    step(app, args, true)
}

pub(super) fn redo(app: &mut Signex, args: &CommandArgs) -> Ran {
    step(app, args, false)
}

/// `save_document document=..`: write that document's file.
pub(super) fn save(app: &mut Signex, args: &CommandArgs) -> Ran {
    let entry = match named(app, args) {
        Ok(entry) => entry,
        Err(reason) => return refused(reason),
    };
    let (Some(key), Some(path)) = (entry.key.clone(), entry.path.clone()) else {
        return refused(format!(
            "{} is not loaded, so there is nothing to save",
            entry.name
        ));
    };
    let started = match key {
        DocumentKey::Pcb(_) => app
            .begin_pcb_save(path)
            .map(|task| task.unwrap_or_else(Task::none)),
        DocumentKey::Row(_) => {
            return refused("a component row is saved from its library table");
        }
        _ => app.begin_document_path_save(&path),
    };
    match started {
        Ok(task) => ran(task),
        Err(error) => {
            let reason = format!("{error:#}");
            crate::diagnostics::log_warning(format!(
                "Save of {} was refused: {reason}",
                entry.name
            ));
            refused(reason)
        }
    }
}

/// `save_document_as document=.. path=.. [overwrite=true]`: write that sheet to
/// a new file, which becomes a new document with an id of its own.
pub(super) fn save_as(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Path(target)) = args.operand("path") else {
        return refused("`save_document_as` needs the operand `path`");
    };
    let from = match named_sheet(app, args, "`save_document_as`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    // The completion of a Save As re-keys the document that is the active
    // tab; a document in a background tab is not re-keyed yet.
    if app.document_state.active_path.as_deref() != Some(from.as_path()) {
        return refused(format!(
            "{} is not the active tab; Save As of a background document is not available yet",
            file_name(&from)
        ));
    }
    let target = PathBuf::from(target);
    let overwrite = matches!(args.operand("overwrite"), Some(OperandValue::Bool(true)));
    if !overwrite && target.exists() {
        return refused(format!(
            "{} exists; pass overwrite=true to replace it",
            target.display()
        ));
    }
    match app.save_sheet_as(&from, target) {
        Ok(task) => ran(task),
        Err(error) => {
            let reason = format!("{error:#}");
            crate::diagnostics::log_warning(format!("Save As was refused: {reason}"));
            refused(reason)
        }
    }
}

/// `close_active_document document=..`: close that document's tab, as its
/// close button does. Closing another tab leaves the active one as it was;
/// closing the active one moves the active tab to a neighbour, as that button
/// does (a headless run opens a document and closes it by name).
pub(super) fn close(app: &mut Signex, args: &CommandArgs) -> Ran {
    let entry = match named(app, args) {
        Ok(entry) => entry,
        Err(reason) => return refused(reason),
    };
    let Some(key) = entry.key.clone() else {
        return refused(format!("{} is not open in a tab", entry.name));
    };
    let index = app
        .document_state
        .tabs
        .iter()
        .position(|tab| DocumentKey::of(tab).as_ref() == Some(&key));
    match index {
        Some(index) => ran(app.close_tab_parking_dirty(index)),
        None => refused(format!("{} is not open in a tab", entry.name)),
    }
}

/// `assign_net_class document=.. net_name=N [class=C]`: put the net in the
/// class (or take it out of its listed one) of the project that sheet belongs
/// to, as the label picker does; the pick lands on that sheet's timeline.
pub(super) fn assign_net_class(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Text(net)) = args.operand("net_name") else {
        return refused("`assign_net_class` needs the operand `net_name`");
    };
    let path = match named_sheet(app, args, "`assign_net_class`") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let class = match args.operand("class") {
        Some(OperandValue::Text(class)) => Some(class.as_str()),
        _ => None,
    };
    match app.assign_net_to_class(Some(&path), net, class) {
        Ok(()) => ran(Task::none()),
        Err(reason) => refused(reason),
    }
}
