//! Bottom-up hierarchy from operands (#1276): `create_sheet_symbol_from_sheet`
//! and `sync_sheet_entries`, the non-interactive forms of Design ▸ Create
//! Sheet Symbol From Sheet and Design ▸ Synchronize Sheet Entries and Ports.
//! The same engine rules as the dialog (`signex_engine::sheet_sync`), applied
//! at once. What they read from disk (the sheet file, the child file, a link
//! out of the project) is read off the UI thread before they run
//! (`edit_reads`). Rules and refusals: `docs/CLI.md`, "Bottom-up hierarchy".

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_engine::{Command, SheetSyncDirection, SheetSyncPreview};
use signex_types::schematic::{ChildSheet, SchematicSheet};

use super::hierarchy_operands::{applied, refuse_pin_ref};
use super::placement_extras_operands::path_operand;
use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::mutation_gateway::MutationOutcome;
use crate::app::project_sheets::resolve_child_reference_quiet;
use crate::app::sheet_write::{SheetWrite, SheetWriteRefusal};
use crate::app::{Message, Signex, UndoGroupKind};

type Ran = (CommandOutcome, Task<Message>);

/// `create_sheet_symbol_from_sheet path=<.snxsch> [at=x,y] [allow_merge=..]
/// [document=..]`.
pub(super) fn create_sheet_symbol_from_sheet(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.symbol_from_sheet_from_operands(args))
}

/// `sync_sheet_entries symbol=<id or name> direction=to_symbol|to_sheet
/// [document=..]`.
pub(super) fn sync_sheet_entries(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.sync_sheet_entries_from_operands(args))
}

/// The sheet symbol Create Sheet Symbol From Sheet placed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SymbolFromSheet {
    pub(crate) id: uuid::Uuid,
    pub(crate) name: String,
    /// The file reference it carries.
    pub(crate) reference: String,
    pub(crate) at: signex_types::schematic::Point,
    pub(crate) size: (f64, f64),
    /// Its sheet entries, one per port of the file.
    pub(crate) entries: Vec<String>,
}

impl SymbolFromSheet {
    /// What the operand form answers.
    fn answer(&self) -> Value {
        json!({
            "id": self.id.to_string(),
            "name": self.name,
            "file": self.reference,
            "at": [self.at.x, self.at.y],
            "size": [self.size.0, self.size.1],
            "entries": self.entries,
        })
    }
}

/// The `path` operand as the text `child_file_target` resolves.
pub(super) fn sheet_file_operand(args: &CommandArgs) -> Result<String, String> {
    path_operand(args).map(|path| path.to_string_lossy().into_owned())
}

fn direction_operand(args: &CommandArgs) -> Result<SheetSyncDirection, String> {
    match text(args, "direction")?.trim() {
        "to_symbol" => Ok(SheetSyncDirection::SheetToSymbol),
        "to_sheet" => Ok(SheetSyncDirection::SymbolToSheet),
        other => Err(format!(
            "`direction` must be to_symbol or to_sheet, not `{other}`"
        )),
    }
}

/// The answer's `added`, `removed` and `renamed` (`[{"from", "to"}]`).
fn preview_json(preview: &SheetSyncPreview) -> (Value, Value, Value) {
    let renamed: Vec<Value> = preview
        .renamed
        .iter()
        .map(|r| json!({"from": r.from, "to": r.to}))
        .collect();
    (
        json!(preview.added),
        json!(preview.removed),
        Value::Array(renamed),
    )
}

/// Why the grouped write to the child sheet changed nothing.
fn write_refusal(refusal: SheetWriteRefusal) -> String {
    match refusal {
        SheetWriteRefusal::Busy(path) => format!("{} is being saved or moved", path.display()),
        SheetWriteRefusal::ReadOnly(path) => format!("{} opens read-only", path.display()),
        SheetWriteRefusal::NotLoaded(path) => format!("{} could not be loaded", path.display()),
        SheetWriteRefusal::NotChanged(path) => format!("{} refused the change", path.display()),
    }
}

impl Signex {
    /// The sheet symbol `key` names on the sheet an edit acts on: by id, else
    /// by its (unique) name.
    pub(super) fn sheet_symbol_by_key(&self, key: &str) -> Result<&ChildSheet, String> {
        let Ok(id) = uuid::Uuid::parse_str(key.trim()) else {
            return self.sheet_symbol_named(key);
        };
        let engine = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        engine
            .document()
            .child_sheets
            .iter()
            .find(|c| c.uuid == id)
            .ok_or_else(|| format!("no sheet symbol `{key}` on this sheet"))
    }

    /// `create_sheet_symbol_from_sheet` from its operands:
    /// [`Self::place_symbol_from_sheet_file`], answered with the new symbol.
    fn symbol_from_sheet_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        let allow_merge = matches!(args.operand("allow_merge"), Some(OperandValue::Bool(true)));
        let file = sheet_file_operand(args)?;
        let at = match args.operand("at") {
            Some(_) => {
                refuse_pin_ref(args, "at")?;
                Some(self.grid_point(args, "at")?)
            }
            None => None,
        };
        let placed = self.place_symbol_from_sheet_file(&file, at, allow_merge)?;
        Ok(CommandOutcome::Answered(placed.answer()))
    }

    /// Create Sheet Symbol From Sheet on the sheet an edit acts on, for the
    /// GUI and the operand form alike (one command id, one behaviour): the
    /// file rules (`check_sheet_file`: the project folder, a loop at any
    /// depth, and unless `allow_merge` an unreadable subtree and a file placed
    /// already), placed once on this sheet, the connection rule and, unless
    /// `allow_merge`, the frozen name rule; then one sheet symbol with an
    /// entry for every port of the file as one undo step. `at` left out puts
    /// it under the sheet's content. Reads nothing: what the checks need was
    /// read before (`edit_reads`).
    pub(crate) fn place_symbol_from_sheet_file(
        &mut self,
        file: &str,
        at: Option<signex_types::schematic::Point>,
        allow_merge: bool,
    ) -> Result<SymbolFromSheet, String> {
        let reference = self.child_file_reference(file, allow_merge)?;
        let (_, path, _) = self.child_file_target(file)?;
        let child = self
            .prefetched_sheet(&path)?
            .ok_or_else(|| format!("there is no file at {}", path.display()))?;
        let sheet = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?
            .document();
        if let Some(other) = signex_engine::other_sheet_symbol_using_file(
            &sheet.child_sheets,
            uuid::Uuid::nil(),
            &reference,
        ) {
            return Err(format!(
                "sheet symbol `{}` already refers to {reference}; one file is placed once",
                other.name
            ));
        }
        let stem = path
            .file_stem()
            .map_or_else(|| "sheet".to_string(), |s| s.to_string_lossy().into_owned());
        let name = signex_engine::unused_sheet_symbol_name(sheet, &stem);
        let at = at.unwrap_or_else(|| signex_engine::sheet_symbol_spot(sheet));
        let symbol = signex_engine::sheet_symbol_from_sheet(
            &child,
            &reference,
            &name,
            at,
            &mut signex_types::object_id::UuidV7Ids,
        );
        let placed = SymbolFromSheet {
            id: symbol.uuid,
            name,
            reference,
            at,
            size: symbol.size,
            entries: symbol.pins.iter().map(|p| p.name.clone()).collect(),
        };
        let command = Command::PlaceChildSheet {
            child_sheet: symbol,
        };
        self.refuse_contacts(std::slice::from_ref(&command), "sheet symbol")?;
        if !allow_merge {
            self.refuse_child_names(&command, &path, at)?;
        }
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], false, true);
        applied(outcome, reason)?;
        Ok(placed)
    }

    /// The dialog's Apply for `direction`, without the dialog: the changes
    /// worked out from the sheets as they are now, applied as one undo step.
    fn sync_sheet_entries_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        let direction = direction_operand(args)?;
        let symbol = self.sheet_symbol_by_key(&text(args, "symbol")?)?.clone();
        let sheet = self
            .document_state
            .edit_path()
            .ok_or("no schematic is open")?
            .to_path_buf();
        let dir = sheet.parent().unwrap_or_else(|| Path::new(""));
        let child_path = resolve_child_reference_quiet(dir, &symbol.filename).map_err(|_| {
            format!(
                "sheet symbol `{}` refers to no usable file (`{}`)",
                symbol.name, symbol.filename
            )
        })?;
        // The child file was read before the edit ran.
        let child = self
            .prefetched_sheet(&child_path)?
            .ok_or_else(|| format!("child schematic not found: {}", child_path.display()))?;
        let preview = match direction {
            SheetSyncDirection::SheetToSymbol => signex_engine::entry_changes_from_sheet(
                &symbol,
                &signex_engine::exposed_sheet_ports(&child),
            ),
            SheetSyncDirection::SymbolToSheet => {
                signex_engine::port_changes_from_symbol(&symbol, &child)
            }
        };
        if !preview.is_empty() {
            match direction {
                SheetSyncDirection::SheetToSymbol => {
                    self.sync_entries_to_symbol(&symbol, &child, &preview)?;
                }
                SheetSyncDirection::SymbolToSheet => {
                    self.sync_ports_to_sheet(&symbol, child_path, child, &preview)?;
                }
            }
        }
        let (added, removed, renamed) = preview_json(&preview);
        Ok(CommandOutcome::Answered(json!({
            "symbol": symbol.uuid.to_string(),
            "name": symbol.name,
            "file": symbol.filename,
            "direction": match direction {
                SheetSyncDirection::SheetToSymbol => "to_symbol",
                SheetSyncDirection::SymbolToSheet => "to_sheet",
            },
            "added": added,
            "removed": removed,
            "renamed": renamed,
        })))
    }

    /// Sheet to symbol: the reconcile with the preview's renames, under the
    /// connection rule.
    fn sync_entries_to_symbol(
        &mut self,
        symbol: &ChildSheet,
        child: &SchematicSheet,
        preview: &SheetSyncPreview,
    ) -> Result<(), String> {
        let parent = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?
            .document();
        let ports = signex_engine::ports_for_pin_sync(child, parent, &symbol.filename);
        let command = signex_engine::sync_from_sheet_command(symbol, ports, preview);
        self.refuse_contacts(std::slice::from_ref(&command), "synced entries")?;
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], true, true);
        if outcome == MutationOutcome::Failed {
            return Err(reason.unwrap_or_else(|| "the engine refused the sync".into()));
        }
        Ok(())
    }

    /// Symbol to sheet: the port changes on the child sheet, one undo step of
    /// its own, left unsaved (a closed child goes to a tabless engine from
    /// the copy read for this edit). Refused inside an edit list or an
    /// agent's one-step run: their undo step covers the edited sheet only.
    fn sync_ports_to_sheet(
        &mut self,
        symbol: &ChildSheet,
        child_path: PathBuf,
        child: SchematicSheet,
        preview: &SheetSyncPreview,
    ) -> Result<(), String> {
        if self
            .document_state
            .edit_engine()
            .is_some_and(|engine| engine.in_transaction())
        {
            return Err(
                "direction=to_sheet edits the child sheet, which this edit's undo step does not cover; run it as a command of its own"
                    .to_string(),
            );
        }
        let loaded: HashMap<PathBuf, SchematicSheet> =
            if self.document_state.engines.contains_key(&child_path) {
                HashMap::new()
            } else {
                HashMap::from([(child_path.clone(), child)])
            };
        let write = SheetWrite {
            label: "sync_sheet_entries",
            outcome: "synced",
            kind: UndoGroupKind::SheetEntrySync,
            loaded: &loaded,
        };
        self.write_sheets_as_group(&write, BTreeMap::from([(child_path, ())]), |document, _| {
            signex_engine::port_sync_commands(
                document,
                symbol,
                preview,
                &mut signex_types::object_id::UuidV7Ids,
            )
        })
        .map(|_| ())
        .map_err(write_refusal)
    }
}
