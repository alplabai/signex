//! The placed-object edits (#2418): `set_object_position`,
//! `set_object_rotation`, `set_sheet_symbol_size` and `set_symbol_parameter`.
//! A Properties row and an operand run call the same `Signex` method here, so
//! a typed value, a CLI call and an agent's edit end in one engine command
//! and one undo step. Coordinates are stored mm (Y down from the paper's
//! top), as every operand point is; the Properties rows convert the shown
//! unit and page origin before they call in. Rules: `docs/CLI.md`, "Editing
//! placed objects".

use iced::Task;
use serde_json::{Value, json};
use signex_types::schematic::{Point, SelectedItem, SelectedKind};

use super::placement_operands::{run_edit, text};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::mutation_gateway::MutationOutcome as Applied;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// Why an edit did not happen. `logged` is set when the gateway already put
/// the reason in Messages, so a Properties row does not say it twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlacedRefusal {
    pub reason: String,
    pub logged: bool,
}

impl PlacedRefusal {
    pub(crate) fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            logged: false,
        }
    }
}

type Placed = Result<Value, PlacedRefusal>;

/// `set_object_position id=.. at=x,y [document=..]`.
pub(super) fn set_object_position(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let item = app.placed_item_operand(args)?;
        let at = coordinate(args, "at")?;
        answered(app.set_placed_position(item, at))
    })
}

/// `set_object_rotation id=.. rotation=deg [document=..]`.
pub(super) fn set_object_rotation(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let item = app.placed_item_operand(args)?;
        let Some(OperandValue::Angle(degrees)) = args.operand("rotation") else {
            return Err("operand `rotation` is missing".to_string());
        };
        answered(app.set_placed_rotation(item, *degrees))
    })
}

/// `set_sheet_symbol_size id=.. size=w,h [document=..]`.
pub(super) fn set_sheet_symbol_size(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let item = app.placed_item_operand(args)?;
        let size = coordinate(args, "size")?;
        answered(app.set_sheet_symbol_size(item, (size.x, size.y)))
    })
}

/// `set_symbol_parameter id=.. name=.. [value=..] [document=..]`.
pub(super) fn set_symbol_parameter(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let item = app.placed_item_operand(args)?;
        let name = text(args, "name")?;
        let value = text(args, "value").unwrap_or_default();
        answered(app.set_symbol_parameter(item, &name, &value))
    })
}

/// `set_sheet_variable name=.. [value=..] [remove=true] [document=..]`: the
/// Sheet Variables rows' [`Signex::set_sheet_variable`]. Answers with the
/// variable as stored and the sheet's variables.
pub(super) fn set_sheet_variable(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let name = text(args, "name")?.trim().to_string();
        let remove = matches!(args.operand("remove"), Some(OperandValue::Bool(true)));
        let value = match (remove, args.operand("value")) {
            (true, Some(_)) => return Err("`remove=true` takes no `value`".to_string()),
            (true, None) => None,
            (false, _) => Some(text(args, "value").unwrap_or_default().trim().to_string()),
        };
        app.set_sheet_variable(&name, value)?;
        let variables = app
            .document_state
            .edit_engine()
            .map(|engine| engine.document().variables.clone())
            .unwrap_or_default();
        Ok(CommandOutcome::Answered(json!({
            "name": name,
            "value": variables.get(&name),
            "variables": variables,
        })))
    })
}

fn answered(placed: Placed) -> Result<CommandOutcome, String> {
    placed
        .map(CommandOutcome::Answered)
        .map_err(|refusal| refusal.reason)
}

/// A point operand that must be a coordinate pair, not a pin reference.
fn coordinate(args: &CommandArgs, name: &str) -> Result<Point, String> {
    match args.operand(name) {
        Some(OperandValue::Point(x, y)) => Ok(Point::new(*x, *y)),
        Some(OperandValue::PinRef { .. }) => Err(format!(
            "`{name}` is a coordinate here, not a pin reference"
        )),
        _ => Err(format!("operand `{name}` is missing")),
    }
}

/// The word an answer names a kind with, as `find_items` does.
fn kind_word(item: &SelectedItem, power: bool) -> &'static str {
    match item.kind {
        SelectedKind::Symbol if power => "power_port",
        SelectedKind::Symbol => "symbol",
        SelectedKind::Label => "label",
        SelectedKind::ChildSheet => "sheet_entry",
        SelectedKind::SheetPin => "sheet_pin",
        SelectedKind::Wire => "wire",
        SelectedKind::Bus => "bus",
        SelectedKind::BusEntry => "bus_entry",
        SelectedKind::Junction => "junction",
        SelectedKind::NoConnect => "no_connect",
        SelectedKind::NoErcDirective => "no_erc",
        SelectedKind::Directive => "directive",
        SelectedKind::HarnessConnector => "harness_connector",
        SelectedKind::HarnessEntry => "harness_entry",
        SelectedKind::HarnessWire => "harness_wire",
        SelectedKind::TextNote => "text",
        SelectedKind::SymbolRefField | SelectedKind::SymbolValField | SelectedKind::Drawing => {
            "other"
        }
    }
}

impl Signex {
    /// The placed object the `id` operand names on the sheet the edit acts on.
    fn placed_item_operand(&self, args: &CommandArgs) -> Result<SelectedItem, String> {
        let id = text(args, "id")?;
        let uuid = uuid::Uuid::parse_str(id.trim())
            .map_err(|_| format!("`id` must be an object id (a uuid), got `{id}`"))?;
        let engine = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?;
        signex_engine::placed_item(engine.document(), uuid)
            .ok_or_else(|| format!("no placed object {uuid} on this sheet"))
    }

    /// Run `command` on the sheet an edit acts on, unless a lock holds what
    /// it touches: the lock's reason then, with nothing sent.
    fn apply_placed_edit(&mut self, command: signex_engine::Command) -> Result<(), PlacedRefusal> {
        let engine = self
            .document_state
            .edit_engine()
            .ok_or_else(|| PlacedRefusal::new("no schematic is open"))?;
        if let Some(reason) = engine.lock_refusal(&command) {
            return Err(PlacedRefusal::new(reason));
        }
        let (outcome, reason) = self.apply_engine_commands_reporting(vec![command], true, true);
        match outcome {
            Applied::Changed | Applied::Unchanged => Ok(()),
            Applied::Failed => Err(PlacedRefusal {
                reason: reason.unwrap_or_else(|| "the engine refused the edit".into()),
                logged: true,
            }),
        }
    }

    /// End the parameter-typing run of the sheet an edit acts on.
    fn end_parameter_typing_run(&mut self) {
        if let Some(engine) = self.document_state.edit_engine_mut() {
            engine.end_field_edit_run();
        }
    }

    /// The object's id, kind and geometry after an edit, as the answer.
    fn placed_answer(&self, item: SelectedItem) -> Placed {
        let engine = self
            .document_state
            .edit_engine()
            .ok_or_else(|| PlacedRefusal::new("no schematic is open"))?;
        let sheet = engine.document();
        let geometry = signex_engine::placed_geometry(sheet, &item)
            .ok_or_else(|| PlacedRefusal::new("the object is no longer on this sheet"))?;
        let power = sheet
            .symbols
            .iter()
            .any(|s| s.uuid == item.uuid && s.is_power);
        let mut answer = json!({
            "id": item.uuid.to_string(),
            "kind": kind_word(&item, power),
            "position": [geometry.position.x, geometry.position.y],
        });
        if let Some(rotation) = geometry.rotation {
            answer["rotation"] = json!(rotation);
        }
        if let Some((w, h)) = geometry.size {
            answer["size"] = json!([w, h]);
        }
        Ok(answer)
    }

    /// Put `item`'s position at `at` (stored mm), its wires following as a
    /// drag's do when "keep wires" is on.
    pub(crate) fn set_placed_position(&mut self, item: SelectedItem, at: Point) -> Placed {
        let keep_wires = self.ui_state.move_keeps_wires;
        let command = self
            .document_state
            .edit_engine()
            .and_then(|e| signex_engine::move_to_command(e.document(), &item, at, keep_wires))
            .ok_or_else(|| PlacedRefusal::new("this object has no position to set"))?;
        self.apply_placed_edit(command)?;
        self.placed_answer(item)
    }

    /// Turn the part or label `item` to `degrees` (stored, counter-clockwise).
    pub(crate) fn set_placed_rotation(&mut self, item: SelectedItem, degrees: f64) -> Placed {
        let keep_wires = self.ui_state.move_keeps_wires;
        let command = self
            .document_state
            .edit_engine()
            .and_then(|e| {
                signex_engine::rotate_to_command(e.document(), &item, degrees, keep_wires)
            })
            .ok_or_else(|| PlacedRefusal::new("only a part or a label has a rotation to set"))?;
        self.apply_placed_edit(command)?;
        self.placed_answer(item)
    }

    /// Give the sheet symbol `item` the size `(width, height)` in mm.
    pub(crate) fn set_sheet_symbol_size(&mut self, item: SelectedItem, size: (f64, f64)) -> Placed {
        if item.kind != SelectedKind::ChildSheet {
            return Err(PlacedRefusal::new("only a sheet symbol has a size to set"));
        }
        self.apply_placed_edit(signex_engine::Command::ResizeChildSheet {
            sheet_id: item.uuid,
            size,
        })?;
        self.placed_answer(item)
    }

    /// Set the parameter `name` of the placed part `item` to `value`. The
    /// designator, value and footprint have their own rows and commands.
    pub(crate) fn set_symbol_parameter(
        &mut self,
        item: SelectedItem,
        name: &str,
        value: &str,
    ) -> Placed {
        let name = name.trim();
        if item.kind != SelectedKind::Symbol {
            return Err(PlacedRefusal::new("only a placed part has parameters"));
        }
        if name.is_empty() {
            return Err(PlacedRefusal::new("a parameter needs a name"));
        }
        if signex_engine::is_reserved_symbol_key(name) {
            return Err(PlacedRefusal::new(format!(
                "`{name}` has its own row and command; it is not a parameter"
            )));
        }
        // A commit is a whole value, not a keystroke: it neither joins nor
        // is joined by a typing run of the same cell (one undo step each).
        self.end_parameter_typing_run();
        let applied = self.apply_placed_edit(signex_engine::Command::SetSymbolField {
            symbol_id: item.uuid,
            key: name.to_string(),
            value: value.to_string(),
        });
        self.end_parameter_typing_run();
        applied?;
        let mut answer = self.placed_answer(item)?;
        answer["parameter"] = json!({ "name": name, "value": value });
        Ok(answer)
    }
}
