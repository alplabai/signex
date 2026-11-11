//! `query_pins`: the pins of a part on one sheet, each with its tip in mm.
//!
//! The tips come from `signex_types::schematic::pin_tips`, which symbol
//! lowering, ERC and the full netlist build read too. The designator is the
//! one the netlist names a terminal with, `Symbol::designator_in`, at every
//! instance path of the sheet in its project (the root path for a sheet no
//! project walk reaches), compared exactly. A multi-unit part is named by its
//! bare designator and lists the pins of every unit placed on the sheet; a
//! unit that is not placed is not listed.

use serde_json::{Value, json};
use signex_types::schematic::{InstancePath, Pin, Point, SchematicSheet, Symbol, pin_tips};

use super::{Answer, text_operand};
use crate::app::Signex;
use crate::app::command::CommandArgs;
use crate::app::document::DocumentKey;
use crate::app::document_ids::{DocumentEntry, DocumentKind};

/// The placed units of the one part `designator` names on `sheet` at any of
/// `paths`, by unit.
fn part_units<'a>(
    sheet: &'a SchematicSheet,
    designator: &str,
    paths: &[InstancePath],
) -> Result<Vec<&'a Symbol>, String> {
    if designator.is_empty() || designator.ends_with('?') {
        return Err(format!(
            "`{designator}` is not annotated, so it names no single part"
        ));
    }
    let mut units: Vec<&Symbol> = sheet
        .symbols
        .iter()
        .filter(|s| !s.is_power && paths.iter().any(|p| s.designator_in(p) == designator))
        .collect();
    let Some(first_lib) = units.first().map(|s| s.lib_id.as_str()) else {
        let is_power_port = sheet
            .symbols
            .iter()
            .any(|s| s.is_power && paths.iter().any(|p| s.designator_in(p) == designator));
        return Err(if is_power_port {
            format!("`{designator}` is a power port, not a part")
        } else {
            format!("no part `{designator}` on this sheet")
        });
    };
    let one_lib = units.iter().all(|s| s.lib_id == first_lib);
    units.sort_by_key(|s| s.unit);
    let one_part = one_lib && units.windows(2).all(|pair| pair[0].unit != pair[1].unit);
    if one_part {
        Ok(units)
    } else {
        Err(format!(
            "`{designator}` is ambiguous: two different parts, or one unit twice, share it"
        ))
    }
}

/// The `pins` array for the placed units of `designator` on `sheet`: one
/// entry per placed unit for each pin it shows, as the netlist emits a
/// terminal at every placed unit's tip. A pin shared by all units (unit 0)
/// is marked `shared`.
pub(super) fn pins_of(
    sheet: &SchematicSheet,
    designator: &str,
    paths: &[InstancePath],
) -> Result<Vec<Value>, String> {
    let mut pins = Vec::new();
    for sym in part_units(sheet, designator, paths)? {
        let lib = sheet
            .resolve_lib_symbol(sym)
            .ok_or_else(|| format!("the library symbol `{}` is not in this sheet", sym.lib_id))?;
        for (lp, tip) in pin_tips(&lib, sym) {
            pins.push(json!({
                "number": pin_id(&lp.pin),
                "name": lp.pin.name,
                "electrical_type": lp.pin.direction,
                "hidden": !lp.pin.visible,
                "unit": sym.unit,
                "shared": lp.unit == 0 && lib.unit_count() > 1,
                "position": [tip.x, tip.y],
            }));
        }
    }
    Ok(pins)
}

/// One pin of a placed unit, as a reference to it reads it.
struct PlacedPin {
    symbol: uuid::Uuid,
    id: String,
    name: String,
    visible: bool,
    tip: Point,
}

/// The tip of pin `pin` of the part `designator` names on `sheet`. `pin` is a
/// pin id (number, else name) matched exactly, else a pin name that only one
/// pin of the placed units has. Refused, with the reason, for what is not one
/// tip on this sheet: see "Pin references" in `docs/CLI.md`.
pub(in crate::app::command) fn pin_tip_of(
    sheet: &SchematicSheet,
    designator: &str,
    pin: &str,
    paths: &[InstancePath],
) -> Result<PinRefTip, String> {
    let units = part_units(sheet, designator, paths)?;
    let mut placed = Vec::new();
    for sym in &units {
        let lib = sheet
            .resolve_lib_symbol(sym)
            .ok_or_else(|| format!("the library symbol `{}` is not in this sheet", sym.lib_id))?;
        for (lp, tip) in pin_tips(&lib, sym) {
            placed.push(PlacedPin {
                symbol: sym.uuid,
                id: pin_id(&lp.pin).to_string(),
                name: lp.pin.name.clone(),
                visible: lp.pin.visible,
                tip,
            });
        }
    }
    let by_id: Vec<&PlacedPin> = placed.iter().filter(|p| p.id == pin).collect();
    let found = if by_id.is_empty() {
        by_id_or_name(&placed, designator, pin)?
    } else {
        by_id
    };
    let Some(first) = found.first() else {
        return Err(not_placed_or_missing(sheet, &units, designator, pin));
    };
    if !first.visible {
        return Err(format!(
            "pin `{pin}` of `{designator}` is hidden, so no wire can attach to it"
        ));
    }
    if let [_, _, ..] = found.as_slice() {
        let tips: Vec<String> = found
            .iter()
            .map(|p| format!("{},{}", p.tip.x, p.tip.y))
            .collect();
        return Err(format!(
            "pin `{pin}` of `{designator}` is shared by every unit and several units are placed, so it has several tips ({}); name a point instead",
            tips.join(" and ")
        ));
    }
    Ok(PinRefTip {
        tip: first.tip,
        symbol: first.symbol,
        pin: first.id.clone(),
        sheet_pin: false,
    })
}

/// What a pin reference resolved to: the tip, and the pin as the netlist names
/// a terminal (the placed symbol and the pin id). For a sheet entry
/// (`sheet_pin`), `symbol` is the sheet pin's uuid and `pin` its name.
pub(in crate::app::command) struct PinRefTip {
    pub tip: Point,
    pub symbol: uuid::Uuid,
    pub pin: String,
    pub sheet_pin: bool,
}

impl PinRefTip {
    /// The end the caller named, for the intent checks.
    pub(in crate::app::command) fn end_spec(&self, at: Point) -> signex_net::EndSpec {
        if self.sheet_pin {
            signex_net::EndSpec {
                sheet_pin: Some(self.symbol),
                ..signex_net::EndSpec::at(at)
            }
        } else {
            signex_net::EndSpec {
                pin: Some((self.symbol, self.pin.clone())),
                ..signex_net::EndSpec::at(at)
            }
        }
    }
}

/// The tip of `<name>.<entry>`: a part's pin, or when `name` is a sheet symbol
/// on `sheet` (and no part), that sheet's entry. A name that is both, or two
/// sheet symbols with one name, is refused. See "Pin references" in
/// `docs/CLI.md`.
pub(in crate::app::command) fn ref_tip_of(
    sheet: &SchematicSheet,
    name: &str,
    entry: &str,
    paths: &[InstancePath],
) -> Result<PinRefTip, String> {
    let sheets: Vec<_> = sheet
        .child_sheets
        .iter()
        .filter(|c| c.name == name)
        .collect();
    let [only, ..] = sheets.as_slice() else {
        return pin_tip_of(sheet, name, entry, paths);
    };
    let names_a_part = sheet
        .symbols
        .iter()
        .any(|s| !s.is_power && paths.iter().any(|p| s.designator_in(p) == name));
    if names_a_part {
        return Err(format!(
            "`{name}` names both a part and a sheet on this sheet, so `{name}.{entry}` is ambiguous; name a point instead"
        ));
    }
    if sheets.len() > 1 {
        return Err(format!(
            "{} sheet symbols on this sheet are named `{name}`, so `{name}.{entry}` is ambiguous; name a point instead",
            sheets.len()
        ));
    }
    let entries: Vec<_> = only.pins.iter().filter(|p| p.name == entry).collect();
    match entries.as_slice() {
        [pin] => Ok(PinRefTip {
            tip: pin.position,
            symbol: pin.uuid,
            pin: pin.name.clone(),
            sheet_pin: true,
        }),
        [] => Err(format!("sheet `{name}` has no entry `{entry}`")),
        _ => Err(format!(
            "sheet `{name}` has {} entries named `{entry}`; name a point instead",
            entries.len()
        )),
    }
}

/// The pins whose name is `pin`, when they are one pin (a shared pin shows at
/// each placed unit); several different pins are ambiguous.
fn by_id_or_name<'a>(
    placed: &'a [PlacedPin],
    designator: &str,
    pin: &str,
) -> Result<Vec<&'a PlacedPin>, String> {
    let named: Vec<&PlacedPin> = placed.iter().filter(|p| p.name == pin).collect();
    let mut ids: Vec<&str> = named.iter().map(|p| p.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() > 1 {
        return Err(format!(
            "pin name `{pin}` of `{designator}` matches several pins ({}); use the pin number",
            ids.join(", ")
        ));
    }
    Ok(named)
}

/// Why no placed pin matched: it is on a unit this sheet does not place, or
/// the part has no such pin.
fn not_placed_or_missing(
    sheet: &SchematicSheet,
    units: &[&Symbol],
    designator: &str,
    pin: &str,
) -> String {
    let elsewhere = units
        .first()
        .and_then(|sym| sheet.resolve_lib_symbol(sym))
        .and_then(|lib| {
            lib.pins
                .iter()
                .find(|lp| pin_id(&lp.pin) == pin || lp.pin.name == pin)
                .map(|lp| lp.unit)
        });
    match elsewhere {
        Some(unit) => format!(
            "pin `{pin}` of `{designator}` belongs to unit {unit}, which is not placed on this sheet (or is not in its body style)"
        ),
        None => format!("`{designator}` has no pin `{pin}`"),
    }
}

/// The pin id ERC and the netlist use: the number, else the name.
pub(in crate::app::command) fn pin_id(pin: &Pin) -> &str {
    if pin.number.is_empty() {
        &pin.name
    } else {
        &pin.number
    }
}

/// The refusal for a sheet hierarchy a re-read is replacing: a designator is
/// only known per instance path, and the paths are about to change.
const HIERARCHY_BEING_READ: &str = "the project's sheet hierarchy is being re-read; ask again";

impl Signex {
    /// Whether `entry`'s project has a hierarchy walk wanted that has not
    /// landed (or is out of date).
    fn hierarchy_being_read(&self, entry: &DocumentEntry) -> bool {
        let docs = &self.document_state;
        entry
            .project
            .as_ref()
            .and_then(|named| docs.projects.iter().find(|p| p.path == named.path))
            .is_some_and(|p| docs.project_fs.hierarchy_current(p.id) == Some(false))
    }

    /// The instance paths `entry`'s sheet is placed at, for naming its parts:
    /// refused while the project's hierarchy is unread. A sheet no walk
    /// reaches is named at the root, as its own netlist is.
    fn pin_instance_paths(&self, entry: &DocumentEntry) -> Result<Vec<InstancePath>, String> {
        let mut paths = self.instance_paths_of(entry).ok_or_else(|| {
            "the project's sheet hierarchy is not read yet, so the designators of this sheet \
             are not known; ask again once the project has loaded"
                .to_string()
        })?;
        if paths.is_empty() {
            paths.push(InstancePath::root());
        }
        Ok(paths)
    }

    /// The tip of `designator.pin` on the sheet an edit acts on (the agent's
    /// target, else the active sheet), never on whichever tab is showing.
    pub(in crate::app::command) fn pin_ref_tip(
        &self,
        designator: &str,
        pin: &str,
    ) -> Result<Point, String> {
        self.pin_ref(designator, pin).map(|resolved| resolved.tip)
    }

    /// [`Self::pin_ref_tip`] with the pin as the netlist names it.
    pub(in crate::app::command) fn pin_ref(
        &self,
        designator: &str,
        pin: &str,
    ) -> Result<PinRefTip, String> {
        let docs = &self.document_state;
        let (Some(path), Some(engine)) = (docs.edit_path(), docs.edit_engine()) else {
            return Err("no schematic is open".to_string());
        };
        let entry = self
            .document_entries()
            .into_iter()
            .find(|e| matches!(&e.key, Some(DocumentKey::Schematic(p)) if p == path))
            .ok_or("the sheet is not one this session lists")?;
        // Only a reference that is about to edit waits for a hierarchy re-read.
        if self.hierarchy_being_read(&entry) {
            return Err(HIERARCHY_BEING_READ.to_string());
        }
        let paths = self.pin_instance_paths(&entry)?;
        ref_tip_of(engine.document(), designator, pin, &paths)
    }

    /// `query_pins document=<doc> designator=<ref>`.
    pub(super) fn query_pins(&self, args: &CommandArgs) -> Answer {
        let entry = self.document_operand(args)?;
        let designator = text_operand(args, "designator").unwrap_or_default().trim();
        let sheet = match &entry.key {
            Some(DocumentKey::Schematic(path)) => self
                .document_state
                .engines
                .get(path)
                .ok_or("the sheet is not loaded")?
                .document(),
            None if entry.kind == DocumentKind::Schematic => {
                return Err("the sheet is not loaded".into());
            }
            _ => {
                return Err(
                    "`query_pins` reads a schematic sheet; this document is not one".into(),
                );
            }
        };
        let paths = self.pin_instance_paths(&entry)?;
        let pins = pins_of(sheet, designator, &paths)?;
        Ok(json!({ "designator": designator, "count": pins.len(), "pins": pins }))
    }
}
