//! `place_component` from operands (#2199, decision comment 6769277106): a
//! library row (symbol, footprint and parameters together) placed at a point,
//! committed through the click's `place_part_at`, so undo, the dirty flag and
//! the Messages log behave as for a click. Rules and refusals: `docs/CLI.md`,
//! "Placing parts and hierarchy".
//!
//! The library must already be open (a project's libraries mount when the
//! project opens). The row's symbol is read here, synchronously, so the edit
//! can be one of an `apply_edits` list and still land before the list commits.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_engine::MirrorAxis;
use signex_types::schematic::{Point, Symbol, pin_tips};
use uuid::Uuid;

use super::placement_operands::{run_edit, text};
use super::queries::pin_id;
use super::wire_runs::describe;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::library_place::part_payload;
use crate::app::{Message, Signex};
use crate::library::messages::ResolvedPlacement;
use crate::library::state::OpenLibrary;

type Ran = (CommandOutcome, Task<Message>);

/// How many touched items a connection refusal names.
const NAMED_CONTACTS: usize = 3;

/// `place_component library=.. part=.. at=x,y [unit=..] [designator=..]
/// [value=..] [rotation=..] [mirror=x|y] [document=..]`.
pub(super) fn place_component(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| app.place_component_from_operands(args))
}

/// The library row `part` names in `library`: a row id, else a part number
/// (internal or manufacturer) that exactly one row carries.
fn library_row(library: &OpenLibrary, part: &str) -> Result<(String, Uuid), String> {
    let part = part.trim();
    let rows = || {
        library
            .tables
            .iter()
            .flat_map(|(table, rows)| rows.iter().map(move |row| (table, row)))
    };
    if let Ok(id) = Uuid::parse_str(part)
        && let Some((table, row)) = rows().find(|(_, row)| row.row_id == id)
    {
        return Ok((table.clone(), row.row_id));
    }
    let found: Vec<_> = rows()
        .filter(|(_, row)| row.internal_pn.as_str() == part || row.primary_mpn.mpn == part)
        .collect();
    match found.as_slice() {
        [(table, row)] => Ok(((*table).clone(), row.row_id)),
        [] => Err(format!(
            "library {} has no part `{part}`: give a row id or a part number",
            library.display_name
        )),
        many => {
            let mut names: Vec<&str> = many.iter().map(|(_, r)| r.internal_pn.as_str()).collect();
            names.sort_unstable();
            Err(format!(
                "part number `{part}` matches {} rows of library {} ({}); give the row id",
                many.len(),
                library.display_name,
                names.join(", ")
            ))
        }
    }
}

/// `mirror=x` flips up/down (the X key), `mirror=y` left/right (the Y key).
fn mirror_axis(args: &CommandArgs) -> Result<Option<MirrorAxis>, String> {
    match text(args, "mirror").ok().as_deref().map(str::trim) {
        None => Ok(None),
        Some("x") => Ok(Some(MirrorAxis::Vertical)),
        Some("y") => Ok(Some(MirrorAxis::Horizontal)),
        Some(other) => Err(format!("`mirror` must be x or y, not `{other}`")),
    }
}

/// The unit to place: `unit`, 1 when not given; refused outside the part's units.
fn unit_of(args: &CommandArgs, resolved: &ResolvedPlacement) -> Result<u32, String> {
    let units = resolved.lib_symbol.unit_count();
    let Ok(given) = text(args, "unit") else {
        return Ok(1);
    };
    given
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|unit| (1..=units).contains(unit))
        .ok_or_else(|| {
            format!(
                "`unit` must be a unit of {} (1 to {units}), not `{given}`",
                resolved.internal_pn
            )
        })
}

/// The answer: what was placed and each pin's tip, so it can be wired next.
fn placed_answer(sheet: &signex_types::schematic::SchematicSheet, symbol: &Symbol) -> Value {
    let pins: Vec<Value> = sheet
        .resolve_lib_symbol(symbol)
        .map(|lib| {
            pin_tips(&lib, symbol)
                .map(|(lp, tip)| {
                    json!({"number": pin_id(&lp.pin), "name": lp.pin.name, "at": [tip.x, tip.y]})
                })
                .collect()
        })
        .unwrap_or_default();
    json!({
        "designator": symbol.reference,
        "unit": symbol.unit,
        "lib_id": symbol.lib_id,
        "at": [symbol.position.x, symbol.position.y],
        "pins": pins,
    })
}

impl Signex {
    /// The open library `named` names: its id, or the path of its `.snxlib`
    /// file or folder.
    fn open_library_named(&self, named: &str) -> Result<&OpenLibrary, String> {
        let named = named.trim();
        let id = Uuid::parse_str(named).ok();
        let path = Path::new(named);
        self.library
            .open_libraries
            .iter()
            .find(|lib| {
                Some(lib.library_id) == id || lib.root == path || lib.root.parent() == Some(path)
            })
            .ok_or_else(|| {
                format!(
                    "library `{named}` is not open; a project's libraries open with the project, \
                     or name an open library by its id"
                )
            })
    }

    /// The designator of the part: `designator`, else the next free one as a
    /// click numbers it. A `designator` already in the project is accepted
    /// only for another unit of that very part (same library row) not placed
    /// yet: units of one part share it.
    fn part_designator(
        &mut self,
        sheet: &Path,
        args: &CommandArgs,
        resolved: &ResolvedPlacement,
        unit: u32,
    ) -> Result<String, String> {
        self.scan_designators_blocking(sheet);
        let prefix = self.placement_prefix_for(sheet, resolved);
        let Ok(wanted) = text(args, "designator") else {
            return Ok(self.next_designator_in_sheet(sheet, &prefix));
        };
        let wanted = wanted.trim().to_string();
        if !self.used_designators(sheet).contains(&wanted.as_str()) {
            return Ok(wanted);
        }
        let docs = &self.document_state;
        let project = docs.document_project(sheet).map(|p| p.id);
        let placed: Vec<&Symbol> = docs
            .engines
            .iter()
            // A loose sheet numbers against itself only, as `used_designators`.
            .filter(|(path, _)| {
                path.as_path() == sheet
                    || (project.is_some() && docs.document_project(path).map(|p| p.id) == project)
            })
            .flat_map(|(_, engine)| engine.document().symbols.iter())
            .filter(|s| s.reference == wanted)
            .collect();
        if placed.is_empty() {
            return Err(format!(
                "designator `{wanted}` is used on a sheet that is not open; open it to add a unit of that part, or leave `designator` out"
            ));
        }
        let same_part = |s: &&Symbol| {
            s.row_id == Some(resolved.row_id) && s.library_id == Some(resolved.library_id)
        };
        if !placed.iter().all(same_part) {
            return Err(format!(
                "designator `{wanted}` is another part; leave `designator` out for the next free one"
            ));
        }
        if placed.iter().any(|s| s.unit == unit) {
            return Err(format!("unit {unit} of `{wanted}` is already placed"));
        }
        Ok(wanted)
    }

    /// Why the edit `commands` (a part, sheet symbol or entry) is refused
    /// by the connection rule: a new pin tip or entry lands on something.
    pub(super) fn refuse_contacts(
        &self,
        commands: &[signex_engine::Command],
        what: &str,
    ) -> Result<(), String> {
        let Some(engine) = self.document_state.edit_engine() else {
            return Ok(());
        };
        let before = engine.document();
        let after = Self::sheet_after(before, commands)?;
        let contacts = signex_net::placement_contacts(before, &after);
        if contacts.is_empty() {
            return Ok(());
        }
        let named: Vec<String> = contacts.iter().take(NAMED_CONTACTS).map(describe).collect();
        let more = contacts.len().saturating_sub(NAMED_CONTACTS);
        let tail = if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        };
        Err(format!(
            "the {what} would land on {}{tail}; an edit from operands connects nothing it does not name, so place it clear and wire it afterwards",
            named.join(", ")
        ))
    }

    fn place_component_from_operands(
        &mut self,
        args: &CommandArgs,
    ) -> Result<CommandOutcome, String> {
        if let Some(OperandValue::PinRef { .. }) = args.operand("at") {
            return Err("`at` of a part is a coordinate, not a pin reference".to_string());
        }
        let at: Point = self.grid_point(args, "at")?;
        let sheet = self
            .document_state
            .edit_path()
            .map(Path::to_path_buf)
            .ok_or("no schematic is open")?;
        let library = self.open_library_named(&text(args, "library")?)?;
        let (table, row_id) = library_row(library, &text(args, "part")?)?;
        let root = library.root.clone();
        let open: Vec<(Uuid, PathBuf)> = self
            .library
            .open_libraries
            .iter()
            .map(|lib| (lib.library_id, lib.root.clone()))
            .collect();
        let resolved = crate::library::place::resolve_placement_with_footprint(
            &root,
            &table,
            signex_library::RowId(row_id),
            None,
            &open,
        )?;
        let unit = unit_of(args, &resolved)?;
        let reference = self.part_designator(&sheet, args, &resolved, unit)?;
        let value = text(args, "value").ok();
        let mut payload = part_payload(resolved, reference).map_symbols(|s| {
            s.unit = unit;
            if let Some(value) = &value {
                s.value.clone_from(value);
            }
        });
        if let Some(axis) = mirror_axis(args)? {
            payload = payload.mirror(axis).map_err(|e| e.to_string())?;
        }
        if let Some(OperandValue::Angle(degrees)) = args.operand("rotation") {
            payload = payload.with_rotation(*degrees).map_err(|e| e.to_string())?;
        }
        let commands = payload.place_commands(at).map_err(|e| e.to_string())?;
        self.refuse_contacts(&commands, "part")?;
        let placed: HashSet<Uuid> = self.place_part_at(&payload, at)?;
        let sheet = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?
            .document();
        let symbol = sheet
            .symbols
            .iter()
            .find(|s| placed.contains(&s.uuid))
            .ok_or("the part was placed but is not on the sheet")?;
        Ok(CommandOutcome::Answered(placed_answer(sheet, symbol)))
    }
}
