//! The hierarchy edits from operands (#2199, decision comment 6769277106):
//! `place_sheet_symbol`, `place_sheet_entry` and `sync_sheet_pins`. Each is one
//! undoable engine edit of the sheet; none writes or creates a file (a sheet
//! symbol only refers to its child file). Rules and refusals: `docs/CLI.md`,
//! "Placing parts and hierarchy".

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::json;
use signex_engine::Command;
use signex_types::schematic::{ChildSheet, normalize_port_name};

use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::canvas::{
    escapes_through_symlink, reference_from, resolve_new_sheet_path,
};
use crate::app::mutation_gateway::CommandOutcome as Applied;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// `place_sheet_symbol start=x,y end=x,y name=.. file=.. [document=..]`.
pub(super) fn place_sheet_symbol(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.place_sheet_symbol_from_operands(args))
}

/// `place_sheet_entry sheet=.. at=x,y name=.. [direction=..] [document=..]`.
pub(super) fn place_sheet_entry(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.place_sheet_entry_from_operands(args))
}

/// `sync_sheet_pins sheet=.. [document=..]`.
pub(super) fn sync_sheet_pins(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.sync_sheet_pins_from_operands(args))
}

/// A point operand that must be a coordinate: a pin reference names a pin's
/// tip, which is no corner or border point of a sheet symbol.
fn refuse_pin_ref(args: &CommandArgs, name: &str) -> Result<(), String> {
    match args.operand(name) {
        Some(OperandValue::PinRef { .. }) => Err(format!(
            "`{name}` is a coordinate here, not a pin reference"
        )),
        _ => Ok(()),
    }
}

fn applied(outcome: Applied, reason: Option<String>) -> Result<(), String> {
    match outcome {
        Applied::Changed => Ok(()),
        Applied::Unchanged => Err("nothing was placed".to_string()),
        Applied::Failed => Err(reason.unwrap_or_else(|| "the engine refused the edit".into())),
    }
}

/// The names of `list` that `keep` keeps, sorted.
fn names(list: &[String], keep: impl Fn(&String) -> bool) -> Vec<String> {
    let mut kept: Vec<String> = list.iter().filter(|n| keep(n)).cloned().collect();
    kept.sort_unstable();
    kept
}

impl Signex {
    /// The one sheet symbol named `name` on the sheet an edit acts on.
    fn sheet_symbol_named(&self, name: &str) -> Result<&ChildSheet, String> {
        let engine = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        let found: Vec<&ChildSheet> = engine
            .document()
            .child_sheets
            .iter()
            .filter(|c| c.name == name)
            .collect();
        match found.as_slice() {
            [only] => Ok(only),
            [] => Err(format!("no sheet symbol `{name}` on this sheet")),
            many => Err(format!(
                "{} sheet symbols on this sheet are named `{name}`; rename one first",
                many.len()
            )),
        }
    }

    /// The child file reference `file`, written from the sheet's folder:
    /// refused unless it is a `.snxsch` inside the project folder (a linked
    /// folder that leaves it included). Nothing is created or opened.
    fn child_file_reference(&self, file: &str) -> Result<String, String> {
        let docs = &self.document_state;
        let sheet = docs.edit_path().ok_or("no schematic is open")?;
        let parent_dir = sheet.parent().unwrap_or_else(|| Path::new(""));
        let project = docs.document_project(sheet);
        let project_dir: PathBuf =
            project.map_or_else(|| parent_dir.to_path_buf(), |p| p.dir().to_path_buf());
        let fold = crate::app::state::project_case::fold_of(project);
        let path = resolve_new_sheet_path(parent_dir, &project_dir, file, fold)
            .map_err(|why| format!("`file`: {why}"))?;
        if escapes_through_symlink(&project_dir, &path) {
            return Err(
                "`file`: the sheet must be inside the project folder (a linked folder leaves it)"
                    .to_string(),
            );
        }
        Ok(reference_from(parent_dir, &path, fold))
    }

    fn place_sheet_symbol_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        refuse_pin_ref(args, "start")?;
        refuse_pin_ref(args, "end")?;
        let (start, end) = (
            self.grid_point(args, "start")?,
            self.grid_point(args, "end")?,
        );
        let name = text(args, "name")?.trim().to_string();
        let filename = self.child_file_reference(&text(args, "file")?)?;
        let sheet = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?
            .document();
        if sheet.child_sheets.iter().any(|c| c.name == name) {
            return Err(format!("a sheet symbol `{name}` is already on this sheet"));
        }
        if let Some(other) = signex_engine::other_sheet_symbol_using_file(
            &sheet.child_sheets,
            uuid::Uuid::nil(),
            &filename,
        ) {
            return Err(format!(
                "sheet symbol `{}` already refers to {filename}; one file is placed once",
                other.name
            ));
        }
        let Some(child_sheet) = signex_engine::child_sheet_from_corners(start, end, &name) else {
            return Err(format!(
                "each edge must be at least {} mm",
                signex_engine::MIN_CHILD_SHEET_EDGE_MM
            ));
        };
        let child_sheet = ChildSheet {
            filename: filename.clone(),
            ..child_sheet
        };
        let (id, at, size) = (child_sheet.uuid, child_sheet.position, child_sheet.size);
        let command = Command::PlaceChildSheet { child_sheet };
        self.refuse_contacts(std::slice::from_ref(&command), "sheet symbol")?;
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], false, true);
        applied(outcome, reason)?;
        Ok(CommandOutcome::Answered(json!({
            "id": id.to_string(),
            "name": name,
            "file": filename,
            "at": [at.x, at.y],
            "size": [size.0, size.1],
        })))
    }

    fn place_sheet_entry_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        refuse_pin_ref(args, "at")?;
        let at = self.grid_point(args, "at")?;
        let sheet_name = text(args, "sheet")?;
        let name = text(args, "name")?;
        let direction = text(args, "direction").ok();
        let symbol = self.sheet_symbol_named(&sheet_name)?;
        let Some(wanted) = normalize_port_name(&name) else {
            return Err("`name` is empty".to_string());
        };
        if symbol
            .pins
            .iter()
            .any(|p| normalize_port_name(&p.name) == Some(wanted))
        {
            return Err(format!(
                "sheet `{sheet_name}` already has an entry `{wanted}`"
            ));
        }
        let pin = signex_engine::new_sheet_entry(symbol, at, &name, direction.as_deref());
        if signex_net::pt_key(&pin.position) != signex_net::pt_key(&at) {
            return Err(format!(
                "{},{} is not on the border of sheet `{sheet_name}` (away from its corners); the nearest border point is {},{}",
                at.x, at.y, pin.position.x, pin.position.y
            ));
        }
        if let Some(given) = &direction
            && given.trim().to_ascii_lowercase() != pin.direction
        {
            return Err(format!(
                "`direction` must be input, output, bidirectional, tri_state or passive, not `{given}`"
            ));
        }
        let entry = json!({
            "sheet": sheet_name,
            "name": pin.name,
            "direction": pin.direction,
            "at": [pin.position.x, pin.position.y],
        });
        let command = Command::PlaceSheetPin {
            sheet_id: symbol.uuid,
            pin,
        };
        self.refuse_contacts(std::slice::from_ref(&command), "sheet entry")?;
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], false, true);
        applied(outcome, reason)?;
        Ok(CommandOutcome::Answered(entry))
    }

    fn sync_sheet_pins_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        let sheet_name = text(args, "sheet")?;
        let filename = self.sheet_symbol_named(&sheet_name)?.filename.clone();
        let sync = self.prepare_child_file_sync(&filename)?;
        let command = sync.command();
        self.refuse_contacts(std::slice::from_ref(&command), "synced entries")?;
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], true, true);
        let before = sync.names_before.clone();
        let after = self.pin_names_for(&filename);
        match outcome {
            Applied::Failed => {
                return Err(reason.unwrap_or_else(|| "the engine refused the sync".into()));
            }
            Applied::Changed | Applied::Unchanged => self.report_file_sync(&sync),
        }
        Ok(CommandOutcome::Answered(json!({
            "sheet": sheet_name,
            "file": filename,
            "added": names(&after, |n| !before.contains(n)),
            "removed": names(&before, |n| !after.contains(n)),
            "kept": names(&after, |n| before.contains(n)),
        })))
    }
}
