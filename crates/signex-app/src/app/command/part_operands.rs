//! `place_component` from operands (#2199, decision comment 6769277106): a
//! library row (symbol, footprint and parameters together) placed at a point,
//! committed through the click's `place_part_at`, so undo, the dirty flag and
//! the Messages log behave as for a click. Rules and refusals: `docs/CLI.md`,
//! "Placing parts and hierarchy".
//!
//! The library must already be open (a project's libraries mount when the
//! project opens). The row's symbol and the project's closed sheets are read
//! off the UI thread before the edit runs (`edit_reads`, decision comment
//! 6771692137); nothing here reads a file.

use std::collections::HashSet;
use std::path::Path;

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
use crate::app::project_sheets::lexically_normalize;
use crate::app::state::scope::path_key_with;
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
pub(super) fn library_row(library: &OpenLibrary, part: &str) -> Result<(String, Uuid), String> {
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
    /// file or folder, a relative one read from the edited sheet's project
    /// folder. Paths are compared spelled out (`..` folded) under the
    /// project's case rule; no file is looked at.
    pub(super) fn open_library_named(&self, named: &str) -> Result<&OpenLibrary, String> {
        let named = named.trim();
        let id = Uuid::parse_str(named).ok();
        let docs = &self.document_state;
        let project = docs.edit_path().and_then(|p| docs.document_project(p));
        let fold = crate::app::state::project_case::fold_of(project);
        let base = project
            .map(|p| p.dir().to_path_buf())
            .or_else(|| {
                docs.edit_path()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
            })
            .unwrap_or_default();
        let wanted = path_key_with(&lexically_normalize(&base.join(named)), fold);
        let same = |path: &Path| path_key_with(&lexically_normalize(path), fold) == wanted;
        self.library
            .open_libraries
            .iter()
            .find(|lib| {
                Some(lib.library_id) == id || same(&lib.root) || lib.root.parent().is_some_and(same)
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
    /// only for another unit of that very part (same library row) placed on
    /// no sheet of the project yet: units of one part share it.
    fn part_designator(
        &self,
        sheet: &Path,
        args: &CommandArgs,
        resolved: &ResolvedPlacement,
        unit: u32,
    ) -> Result<String, String> {
        let prefix = self.placement_prefix_for(sheet, resolved);
        // The project's sheets, the closed ones as read before the edit ran;
        // the cached scan a person's drop may wait on is left alone.
        let docs = &self.document_state;
        let project = match docs.document_project(sheet).map(|p| p.id) {
            Some(id) => Some((id, self.edit_project_set(sheet)?)),
            None => None,
        };
        let scan = project.as_ref().map(|(id, set)| {
            let root = crate::app::project_sheets::resolve_netlist_root_for(docs, sheet);
            crate::app::designator_scan::scan_set(*id, 0, set, &root, set.fold)
        });
        let Ok(wanted) = text(args, "designator") else {
            return Ok(match &scan {
                Some(scan) => self.next_designator_in(sheet, &prefix, scan),
                None => self.next_designator_in_sheet(sheet, &prefix),
            });
        };
        let wanted = wanted.trim().to_string();
        let used = match &scan {
            Some(scan) => self.used_designators_in(sheet, scan),
            None => self.used_designators(sheet),
        };
        if !used.contains(&wanted.as_str()) {
            return Ok(wanted);
        }
        // Every part carrying it, on any sheet of the project, open or not.
        let live = docs.engines.get(sheet).map(signex_engine::Engine::document);
        let others = project
            .iter()
            .flat_map(|(_, set)| set.sheets.iter())
            .filter(|(path, _)| path.as_path() != sheet)
            .map(|(_, other)| other);
        let placed: Vec<&Symbol> = live
            .into_iter()
            .chain(others)
            .flat_map(|s| s.symbols.iter())
            .filter(|s| s.reference == wanted)
            .collect();
        if placed.is_empty() {
            return Err(format!(
                "designator `{wanted}` is in use through another placement of a sheet; leave `designator` out"
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
        let resolved = self.prefetched_part(&(library.root.clone(), table, row_id))?;
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
