//! The hierarchy edits from operands (#2199, decision comment 6769277106):
//! `place_sheet_symbol`, `place_sheet_entry` and `sync_sheet_pins`. Each is one
//! undoable engine edit of the sheet; none writes or creates a file (a sheet
//! symbol only refers to its child file). Rules and refusals: `docs/CLI.md`,
//! "Placing parts and hierarchy". What their checks read from disk (a child
//! file, a link out of the project) is read off the UI thread before they run
//! (`edit_reads`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::json;
use signex_engine::Command;
use signex_types::schematic::{ChildSheet, normalize_port_name};

use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::canvas::{reference_from, resolve_new_sheet_path};
use crate::app::mutation_gateway::CommandOutcome as Applied;
use crate::app::sheet_hierarchy::{
    CaseFold, SheetHierarchy, SheetNode, SheetNodeKind, normalised_key,
};
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

/// The keys of the sheets above any placement of the sheet at `path`.
fn sheets_above(hierarchy: &SheetHierarchy, path: &Path, fold: CaseFold) -> HashSet<String> {
    fn walk(
        node: &SheetNode,
        path: &Path,
        fold: CaseFold,
        chain: &mut Vec<String>,
        out: &mut HashSet<String>,
    ) {
        if node.kind == SheetNodeKind::Present && node.is_at(path, fold) {
            out.extend(chain.iter().cloned());
        }
        chain.push(normalised_key(&node.path, fold));
        for child in &node.children {
            walk(child, path, fold, chain, out);
        }
        chain.pop();
    }
    let mut out = HashSet::new();
    for node in hierarchy.top_level() {
        walk(node, path, fold, &mut Vec::new(), &mut out);
    }
    out
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

    /// The child file `file` names from the sheet's folder, checked without a
    /// read: the project folder, the file's path and its reference. Refused
    /// unless it is a `.snxsch` inside the project folder.
    pub(super) fn child_file_target(
        &self,
        file: &str,
    ) -> Result<(PathBuf, PathBuf, String), String> {
        let docs = &self.document_state;
        let sheet = docs.edit_path().ok_or("no schematic is open")?;
        let parent_dir = sheet.parent().unwrap_or_else(|| Path::new(""));
        let project = docs.document_project(sheet);
        let project_dir: PathBuf =
            project.map_or_else(|| parent_dir.to_path_buf(), |p| p.dir().to_path_buf());
        let fold = crate::app::state::project_case::fold_of(project);
        let path = resolve_new_sheet_path(parent_dir, &project_dir, file, fold)
            .map_err(|why| format!("`file`: {why}"))?;
        let reference = reference_from(parent_dir, &path, fold);
        Ok((project_dir, path, reference))
    }

    /// The child file reference `file`: [`Self::child_file_target`], not
    /// through a link that leaves the project folder (read before the edit
    /// ran), and neither this sheet nor one above it. Nothing is created or
    /// opened.
    fn child_file_reference(&self, file: &str) -> Result<String, String> {
        let (_, path, reference) = self.child_file_target(file)?;
        if self.prefetched_escape(&path)? {
            return Err(
                "`file`: the sheet must be inside the project folder (a linked folder leaves it)"
                    .to_string(),
            );
        }
        let docs = &self.document_state;
        let sheet = docs.edit_path().ok_or("no schematic is open")?;
        let fold = crate::app::state::project_case::fold_of(docs.document_project(sheet));
        let target = normalised_key(&path, fold);
        if target == normalised_key(sheet, fold) {
            return Err("`file` is this sheet: a sheet cannot contain itself".to_string());
        }
        // shortcut: only the hierarchy already in memory is read (no disk
        // read in `update`), so an ancestor it does not hold yet is missed.
        if self
            .hierarchy_describing(sheet)
            .is_some_and(|(h, _)| sheets_above(h, sheet, fold).contains(&target))
        {
            return Err(
                "`file` is an ancestor of this sheet: a sheet cannot contain itself".to_string(),
            );
        }
        Ok(reference)
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
        // The child file was read before the edit ran.
        let load = |path: &Path| self.prefetched_sheet(path);
        let sync = self.prepare_child_file_sync(&filename, &load)?;
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
