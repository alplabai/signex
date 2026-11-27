//! `apply_library_updates` (#3013, mockup row 135): the Library Updates
//! dialog's Update Selected Components on the updates waiting for the sheet
//! `document` names. The parts to update and the conflicts that take the
//! library's value are ticked as the dialog's checkboxes and radios tick
//! them, then its button is pressed through `Signex::update`. The library
//! symbols are read off the UI thread and each part's update is one undo
//! step of the sheet when they land, as from the dialog.

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{Translatable, ids};
use signex_types::schematic::StampField;

use super::document_operands::named_sheet;
use super::library_dialog_operands::{refuse, send, text};
use super::operand_handlers::refused;
use super::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::library::messages::LibraryMessage;
use crate::library::part_sync::Choice;
use crate::library::updates_dialog::LibraryUpdatesState;

type Ran = (CommandOutcome, Task<Message>);

const FIELDS: [(&str, StampField); 3] = [
    ("footprint", StampField::Footprint),
    ("manufacturer", StampField::Manufacturer),
    ("mpn", StampField::Mpn),
];

fn field_name(field: StampField) -> &'static str {
    FIELDS
        .iter()
        .find(|(_, f)| *f == field)
        .map_or("", |(name, _)| *name)
}

/// Where the updates of a sheet were waiting.
enum Waiting {
    /// The open dialog.
    Open,
    /// The queue, at this place.
    Queued(usize),
}

/// The updates waiting for `sheet`, taken from the open dialog or the queue.
fn take_waiting(app: &mut Signex, sheet: &Path) -> Option<(LibraryUpdatesState, Waiting)> {
    let library = &mut app.library;
    if library
        .library_updates
        .as_ref()
        .is_some_and(|open| open.schematic_path == sheet)
    {
        return library.library_updates.take().map(|s| (s, Waiting::Open));
    }
    let at = library
        .queued_library_updates
        .iter()
        .position(|queued| queued.schematic_path == sheet)?;
    Some((
        library.queued_library_updates.remove(at),
        Waiting::Queued(at),
    ))
}

/// Put back what [`take_waiting`] took, where it was.
fn put_back(app: &mut Signex, state: LibraryUpdatesState, waiting: Waiting) {
    match waiting {
        Waiting::Open => app.library.library_updates = Some(state),
        Waiting::Queued(at) => app.library.queued_library_updates.insert(at, state),
    }
}

fn listed(text: &str) -> impl Iterator<Item = &str> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
}

fn no_update_for(designator: &str, sheet: &Path) -> Translatable {
    Translatable::new(ids::LIBRARY_COMMAND_UPDATES_DESIGNATOR)
        .arg("designator", designator.to_string())
        .arg("sheet", sheet.display().to_string())
}

/// Tick the parts `designators` names (`all`: every part; left out: the
/// dialog's own default ticks). A part shares its tick with the parts of
/// one embedded symbol, as the dialog's checkbox does.
fn tick(
    state: &mut LibraryUpdatesState,
    args: &CommandArgs,
    sheet: &Path,
) -> Result<(), Translatable> {
    match text(args, "designators") {
        None => {}
        Some("all") => state.set_all(true),
        Some(list) => {
            state.set_all(false);
            for designator in listed(list) {
                let lib_id = state
                    .entries
                    .iter()
                    .find(|entry| entry.ref_des == designator)
                    .map(|entry| entry.lib_id.clone())
                    .ok_or_else(|| no_update_for(designator, sheet))?;
                for entry in state.entries.iter_mut().filter(|e| e.lib_id == lib_id) {
                    entry.selected = true;
                }
            }
        }
    }
    Ok(())
}

/// Answer "Use library value" for each `<designator>:<field>` of
/// `use_library`; the rest keep the part's value, the dialog's default.
fn take_library_values(
    state: &mut LibraryUpdatesState,
    args: &CommandArgs,
    sheet: &Path,
) -> Result<(), Translatable> {
    let Some(list) = text(args, "use_library") else {
        return Ok(());
    };
    for item in listed(list) {
        let bad_item =
            || Translatable::new(ids::LIBRARY_COMMAND_UPDATES_FIELD).arg("item", item.to_string());
        let (designator, field) = item.split_once(':').ok_or_else(bad_item)?;
        let (designator, field) = (designator.trim(), field.trim());
        let field = FIELDS
            .iter()
            .find(|(name, _)| *name == field)
            .map(|(_, f)| *f)
            .ok_or_else(bad_item)?;
        let entry = state
            .entries
            .iter()
            .find(|entry| entry.ref_des == designator)
            .ok_or_else(|| no_update_for(designator, sheet))?;
        if !entry.conflicts.iter().any(|c| c.field == field) {
            return Err(Translatable::new(ids::LIBRARY_COMMAND_UPDATES_NO_CONFLICT)
                .arg("designator", designator.to_string())
                .arg("field", field_name(field)));
        }
        let symbol = entry.symbol_uuid;
        state.answer_conflict(symbol, field, Choice::UseLibrary);
    }
    Ok(())
}

/// The parts the button updates and the parts it leaves waiting.
fn answer(sheet: &Path, state: &LibraryUpdatesState) -> Value {
    let updated: Vec<Value> = state
        .entries
        .iter()
        .filter(|entry| entry.selected)
        .map(|entry| {
            let library_values: Vec<&str> = entry
                .conflicts
                .iter()
                .filter(|c| c.choice == Choice::UseLibrary)
                .map(|c| field_name(c.field))
                .collect();
            json!({
                "designator": entry.ref_des,
                "part": entry.internal_pn,
                "library": entry.library_name,
                "from": entry.current_version,
                "to": entry.latest_version,
                "use_library": library_values,
            })
        })
        .collect();
    let kept: Vec<&str> = state
        .entries
        .iter()
        .filter(|entry| !entry.selected)
        .map(|entry| entry.ref_des.as_str())
        .collect();
    json!({
        "document": sheet.display().to_string(),
        "updating": updated,
        "kept": kept,
    })
}

/// `apply_library_updates document=<sheet> [designators=<list>|all]
/// [use_library=<designator>:<field>,..]`. The answer lists the parts being
/// updated and those left waiting; the sheet changes when the library
/// symbols are read, before the next command of a headless run.
pub(super) fn apply_library_updates(app: &mut Signex, args: &CommandArgs) -> Ran {
    let sheet: PathBuf = match named_sheet(app, args, "a library update") {
        Ok(sheet) => sheet,
        Err(reason) => return refused(reason),
    };
    let Some((waiting_state, waiting)) = take_waiting(app, &sheet) else {
        return refuse(
            Translatable::new(ids::LIBRARY_COMMAND_UPDATES_NONE)
                .arg("sheet", sheet.display().to_string()),
        );
    };
    // Ticked on a copy: a refusal leaves the dialog as it was.
    let mut state = waiting_state.clone();
    let chosen =
        tick(&mut state, args, &sheet).and_then(|()| take_library_values(&mut state, args, &sheet));
    if let Err(reason) = chosen {
        put_back(app, waiting_state, waiting);
        return refuse(reason);
    }
    let answered = answer(&sheet, &state);
    // Shown as the open dialog; one open for another sheet waits next in
    // line, and opens again when this one is answered.
    if let Some(other) = app.library.library_updates.replace(state) {
        app.library.queued_library_updates.insert(0, other);
    }
    let task = send(app, LibraryMessage::LibraryUpdatesApply);
    (CommandOutcome::Answered(answered), task)
}
