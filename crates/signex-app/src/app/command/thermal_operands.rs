//! Thermal relief edits an agent or the CLI runs by naming the board (#2580,
//! #2593): `set_pour_thermal_relief`, the pour panel's Thermal Relief rows,
//! and `set_pad_thermal_relief`, a footprint pad's own air gap, spoke width
//! and spoke count. Each acts on the board `document` names, is one undo
//! step and answers with what the board then holds.

use serde_json::json;
use signex_engine::pcb::primitive_edit::{PadEdit, ZoneEdit, zone_edits_command};
use signex_i18n::{ids, t};
use signex_types::pad_thermal::PadThermal;
use uuid::Uuid;

use super::board_edit_operands::{Ran, answered, board, named_board, refused, text, uuids};
use super::operand_mm::operand_mm;
use super::{CommandArgs, OperandValue};
use crate::app::Signex;

/// `set_pour_thermal_relief document=.. ids=.. [on=true|false]
/// [spoke_width=<mm>] [air_gap=<mm>]`: polygon pours by uuid take the
/// switch, spoke width and air gap given, together. Switching on with no
/// stored width or gap sets the defaults, as the panel's switch does.
pub(super) fn set_pour_thermal_relief(app: &mut Signex, args: &CommandArgs) -> Ran {
    const WHAT: &str = "`set_pour_thermal_relief`";
    let path = match named_board(app, args, WHAT) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let targets = match uuids(args) {
        Ok(targets) => targets,
        Err(reason) => return refused(reason),
    };
    let edits = match pour_edits(args) {
        Ok(edits) => edits,
        Err(reason) => return refused(reason),
    };
    if edits.is_empty() {
        return refused(format!(
            "{WHAT} needs at least one of `on`, `spoke_width` and `air_gap`"
        ));
    }
    let Some(b) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    let all_pours = targets
        .iter()
        .all(|id| b.zones.iter().any(|z| z.uuid == *id && z.is_pour()));
    if !all_pours {
        return refused("`ids` names polygon pours of the board and nothing else");
    }
    let command = match zone_edits_command(b, &targets, &edits) {
        Ok(command) => command,
        Err(refusal) => return refused(refusal.to_string()),
    };
    if let Some(command) = command {
        let label = t!(app.ui_state.i18n, ids::PCB_PRIM_POUR_THERMAL).into_string();
        if let Err(why) = app.run_board_step(&path, vec![command], &label) {
            return refused(why);
        }
    }
    let pours = board(app, &path).map(|b| {
        b.zones
            .iter()
            .filter(|z| targets.contains(&z.uuid))
            .map(|z| {
                json!({
                    "id": z.uuid,
                    "thermal_relief": z.thermal_relief,
                    "spoke_width": z.thermal_width,
                    "air_gap": z.thermal_gap,
                })
            })
            .collect::<Vec<_>>()
    });
    answered(json!({ "document": path, "pours": pours.unwrap_or_default() }))
}

/// The pour edits the operands ask for, the switch first so switching on
/// seeds what the others then set. The lengths are Text by the one operand
/// convention, read by the one `operand_mm`.
fn pour_edits(args: &CommandArgs) -> Result<Vec<ZoneEdit>, String> {
    let mut edits = Vec::new();
    if let Some(OperandValue::Bool(on)) = args.operand("on") {
        edits.push(ZoneEdit::ThermalRelief(*on));
    }
    let length = |name: &str| {
        text(args, name)
            .map(|raw| {
                operand_mm(raw).ok_or_else(|| {
                    format!("`{name}` is a length above 0 mm the board can store, not `{raw}`")
                })
            })
            .transpose()
    };
    if let Some(mm) = length("spoke_width")? {
        edits.push(ZoneEdit::ThermalSpokeWidth(mm));
    }
    if let Some(mm) = length("air_gap")? {
        edits.push(ZoneEdit::ThermalAirGap(mm));
    }
    Ok(edits)
}

/// `set_pad_thermal_relief document=.. ids=.. [air_gap=rule|<mm>]
/// [spoke_width=rule|<mm>] [spokes=rule|<n>]`: footprint pads by uuid set
/// their own values (Manual) or follow the pour again (`rule`); a value left
/// out is kept. What a pour connecting to the pad uses (#2593).
pub(super) fn set_pad_thermal_relief(app: &mut Signex, args: &CommandArgs) -> Ran {
    const WHAT: &str = "`set_pad_thermal_relief`";
    let path = match named_board(app, args, WHAT) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let targets = match uuids(args) {
        Ok(targets) => targets,
        Err(reason) => return refused(reason),
    };
    let edit = match pad_edit(args) {
        Ok(Some(edit)) => edit,
        Ok(None) => {
            return refused(format!(
                "{WHAT} needs at least one of `air_gap`, `spoke_width` and `spokes`"
            ));
        }
        Err(reason) => return refused(reason),
    };
    let Some(b) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    let pads: Option<Vec<(Uuid, Uuid)>> = targets
        .iter()
        .map(|id| {
            b.footprints
                .iter()
                .find(|fp| fp.pads.iter().any(|p| p.uuid == *id))
                .map(|fp| (fp.uuid, *id))
        })
        .collect();
    let Some(pads) = pads.filter(|p| !p.is_empty()) else {
        return refused("`ids` names pads of the board's footprints and nothing else");
    };
    if let Err(why) = app.edit_pads(&path, &pads, &edit, ids::PCB_PRIM_POUR_THERMAL) {
        return refused(why);
    }
    let held = board(app, &path).map(|b| {
        b.footprints
            .iter()
            .flat_map(|fp| fp.pads.iter())
            .filter(|p| targets.contains(&p.uuid))
            .map(|p| pad_json(p.uuid, &p.thermal))
            .collect::<Vec<_>>()
    });
    answered(json!({ "document": path, "pads": held.unwrap_or_default() }))
}

/// A pad's values in the answer: a number, or `"rule"` where it follows the
/// pour.
fn pad_json(id: Uuid, thermal: &PadThermal) -> serde_json::Value {
    let or_rule = |v: Option<serde_json::Value>| v.unwrap_or_else(|| json!("rule"));
    json!({
        "id": id,
        "air_gap": or_rule(thermal.air_gap.map(|v| json!(v))),
        "spoke_width": or_rule(thermal.spoke_width.map(|v| json!(v))),
        "spokes": or_rule(thermal.spokes.map(|n| json!(n))),
    })
}

/// The pad edit the operands ask for; `None` when none is given.
fn pad_edit(args: &CommandArgs) -> Result<Option<PadEdit>, String> {
    let length = |name: &str| -> Result<Option<Option<f64>>, String> {
        match text(args, name).map(str::trim) {
            None => Ok(None),
            Some("rule") => Ok(Some(None)),
            Some(raw) => operand_mm(raw).map(|mm| Some(Some(mm))).ok_or_else(|| {
                format!(
                    "`{name}` must be `rule` or a length above 0 mm the board can store, \
                     not `{raw}`"
                )
            }),
        }
    };
    let spokes = match text(args, "spokes").map(str::trim) {
        None => None,
        Some("rule") => Some(None),
        Some(raw) => Some(Some(raw.parse::<u8>().map_err(|_| {
            format!("`spokes` must be `rule` or a whole number of spokes, not `{raw}`")
        })?)),
    };
    let (air_gap, spoke_width) = (length("air_gap")?, length("spoke_width")?);
    if air_gap.is_none() && spoke_width.is_none() && spokes.is_none() {
        return Ok(None);
    }
    Ok(Some(PadEdit::Thermal {
        air_gap,
        spoke_width,
        spokes,
    }))
}
