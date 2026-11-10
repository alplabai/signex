//! A wire or bus run from operands (#2199): checked against the sheet before
//! it lands, then committed through the click's commit, or only reported
//! (`dry_run`). A person watching a click sees what a path touches; a CLI or
//! an agent does not, so the run is applied to a copy of the edit target
//! first. Rules and refusals: `docs/CLI.md`, "Wire intent" and "Bus geometry".

use serde_json::{Value, json};
use signex_net::{
    BusExtra, BusExtraKind, EndSpec, ExtraItem, ExtraKind, RunEnd, WireIntent, bus_intent,
    dangling_bus_ends, wire_intent,
};
use signex_types::schematic::{Point, SchematicSheet, symbol_crossed_by};

use super::project_names::{aliases_of, needing_merge, project_change_refusal};
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::Signex;
use crate::app::handlers::canvas::{bus_commands, wire_commands};
use crate::app::helpers::constrain_segments;

/// How many touched items a refusal names before it says "and N more".
const NAMED_EXTRAS: usize = 3;

type Segments = Vec<(Point, Point)>;

fn flag(args: &CommandArgs, name: &str) -> bool {
    matches!(args.operand(name), Some(OperandValue::Bool(true)))
}

fn at(p: Point) -> String {
    format!("{},{}", p.x, p.y)
}

fn describe(item: &ExtraItem) -> String {
    let what = match item.kind {
        ExtraKind::Pin => format!("pin {}", item.identity),
        ExtraKind::Label => format!("label `{}`", item.identity),
        ExtraKind::Junction => "a junction".to_string(),
        ExtraKind::Wire => "another wire".to_string(),
        ExtraKind::SheetPin => format!("sheet pin {}", item.identity),
    };
    format!("{what} at {}", at(item.position))
}

/// The refusal for items `what` (a wire, or a label on a pin) would pull in.
pub(super) fn extra_refusal(what: &str, extra: &[ExtraItem]) -> String {
    let named: Vec<String> = extra.iter().take(NAMED_EXTRAS).map(describe).collect();
    let more = extra.len().saturating_sub(NAMED_EXTRAS);
    let tail = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    format!(
        "the {what} also touches {}{tail}, which would join them to the net; route around them",
        named.join(", ")
    )
}

fn bus_extra_refusal(extra: &[BusExtra]) -> String {
    let named: Vec<String> = extra
        .iter()
        .take(NAMED_EXTRAS)
        .map(|item| {
            let what = match item.kind {
                BusExtraKind::Bus | BusExtraKind::Label => item.identity.clone(),
                BusExtraKind::Entry => "a bus entry".to_string(),
            };
            format!("{what} at {}", at(item.position))
        })
        .collect();
    let more = extra.len().saturating_sub(NAMED_EXTRAS);
    let tail = if more > 0 {
        format!(" and {more} more")
    } else {
        String::new()
    };
    format!(
        "the bus also touches {}{tail}, which would join them to the run; route around them",
        named.join(", ")
    )
}

fn open_refusal(open: &[RunEnd], start: Point, end: Point, what: &str) -> String {
    let ends: Vec<String> = open
        .iter()
        .map(|e| match e {
            RunEnd::Start => format!("`start` {}", at(start)),
            RunEnd::End => format!("`end` {}", at(end)),
        })
        .collect();
    format!(
        "{} {} nothing to connect to ({what}); pass allow_open=true to leave it open",
        ends.join(" and "),
        if open.len() == 1 { "has" } else { "have" }
    )
}

/// Both ends as the caller named them: the reference (when it was a pin
/// reference) and the point it resolved to, so a journal records both.
fn ends_json(args: &CommandArgs, start: Point, end: Point) -> Value {
    let one = |name: &str, p: Point| match args.operand(name) {
        Some(OperandValue::PinRef { designator, pin }) => {
            json!({"ref": format!("{designator}.{pin}"), "at": [p.x, p.y]})
        }
        _ => json!({"at": [p.x, p.y]}),
    };
    json!([one("start", start), one("end", end)])
}

fn segments_json(segments: &[(Point, Point)]) -> Value {
    Value::Array(
        segments
            .iter()
            .map(|(a, b)| json!([a.x, a.y, b.x, b.y]))
            .collect(),
    )
}

impl Signex {
    /// The segments the run `start`..`end` makes in the current draw mode:
    /// horizontal and vertical only, and none across a symbol body.
    fn checked_segments(
        &self,
        sheet: &SchematicSheet,
        start: Point,
        end: Point,
        what: &str,
    ) -> Result<Segments, String> {
        let segments = constrain_segments(start, end, self.interaction_state.draw_mode);
        if segments.is_empty() {
            return Err("`start` and `end` are the same point".to_string());
        }
        for (a, b) in &segments {
            let tol = 1e-6;
            if (a.x - b.x).abs() > tol && (a.y - b.y).abs() > tol {
                return Err(format!(
                    "only horizontal and vertical segments; the draw mode would run a diagonal {what} from {} to {}: give points on one row or column, or use the Ortho90 draw mode",
                    at(*a),
                    at(*b)
                ));
            }
        }
        for (a, b) in &segments {
            if let Some(sym) = symbol_crossed_by(sheet, *a, *b) {
                let name = if sym.is_power {
                    format!("the power port {}", sym.value)
                } else {
                    sym.reference.clone()
                };
                return Err(format!(
                    "the {what} from {} to {} crosses the body of {name}",
                    at(*a),
                    at(*b)
                ));
            }
        }
        Ok(segments)
    }

    /// The edit target with `commands` applied, on a copy: nothing real moves.
    pub(super) fn sheet_after(
        sheet: &SchematicSheet,
        commands: &[signex_engine::Command],
    ) -> Result<SchematicSheet, String> {
        let mut scratch = signex_engine::Engine::new(sheet.clone()).map_err(|e| e.to_string())?;
        scratch
            .execute_batch(commands.to_vec())
            .map_err(|e| e.to_string())?;
        Ok(scratch.document().clone())
    }

    /// What the operand `name` named at `at`: the pin, for a pin reference.
    fn end_spec(&self, args: &CommandArgs, name: &str, at: Point) -> EndSpec {
        match args.operand(name) {
            Some(OperandValue::PinRef { designator, pin }) => self
                .pin_ref(designator, pin)
                .map_or_else(|_| EndSpec::at(at), |resolved| resolved.end_spec(at)),
            _ => EndSpec::at(at),
        }
    }

    /// Check the wire run and build its commands and report.
    fn plan_wire(
        &self,
        args: &CommandArgs,
        start: Point,
        end: Point,
    ) -> Result<(Vec<signex_engine::Command>, Value), String> {
        let before = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?
            .document();
        let segments = self.checked_segments(before, start, end, "wire")?;
        let commands = wire_commands(&segments, self.wire_placement_style());
        let after = Self::sheet_after(before, &commands)?;
        let intent = wire_intent(
            before,
            &after,
            self.end_spec(args, "start", start),
            self.end_spec(args, "end", end),
        );
        let short = intent.short();
        let names = intent.merged_names();
        let end_names = intent.end_names.clone();
        let WireIntent {
            extra,
            open,
            joined,
            net,
            ..
        } = intent;
        if !extra.is_empty() {
            return Err(extra_refusal("wire", &extra));
        }
        if !joined {
            return Err("the wire does not connect its two ends; nothing was placed".to_string());
        }
        if short && !flag(args, "allow_merge") {
            let set = |names: &[String]| {
                names
                    .iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            return Err(format!(
                "the wire would join the nets {} and {}; pass allow_merge=true if that is intended (see docs/CLI.md, \"Why allow_merge\")",
                set(&end_names[0]),
                set(&end_names[1])
            ));
        }
        // Names on other sheets: a name that reaches the net from there counts.
        let project =
            self.project_name_changes(&after, &[start, end], flag(args, "allow_merge"))?;
        let rename = needing_merge(&project);
        if let Some(change) = rename
            && !flag(args, "allow_merge")
        {
            return Err(project_change_refusal("wire", start, &change.names));
        }
        if !open.is_empty() && !flag(args, "allow_open") {
            return Err(open_refusal(
                &open,
                start,
                end,
                "a pin, wire, junction or label",
            ));
        }
        let merged = if short {
            names
        } else {
            rename
                .map(|change| change.names.clone())
                .unwrap_or_default()
        };
        let report = json!({
            "ends": ends_json(args, start, end),
            "segments": segments_json(&segments),
            "net": net.map(|n| json!({ "name": n.name, "pins": n.pins })),
            "merged": merged,
            "aliases": aliases_of(&project),
        });
        Ok((commands, report))
    }

    /// Check the bus run and build its commands and report. A bus has no net
    /// diff: its connectivity is the bus analysis, not the netlist.
    fn plan_bus(
        &self,
        args: &CommandArgs,
        start: Point,
        end: Point,
    ) -> Result<(Vec<signex_engine::Command>, Value), String> {
        let sheet = self
            .document_state
            .edit_engine()
            .ok_or("no schematic is open")?
            .document();
        let segments = self.checked_segments(sheet, start, end, "bus")?;
        let commands = bus_commands(&segments);
        let after = Self::sheet_after(sheet, &commands)?;
        let intent = bus_intent(sheet, &after, start, end);
        if !intent.extra.is_empty() {
            return Err(bus_extra_refusal(&intent.extra));
        }
        let merge = intent.merge;
        if merge && !flag(args, "allow_merge") {
            let named: Vec<String> = intent.names.iter().map(|n| format!("`{n}`")).collect();
            return Err(format!(
                "the bus would join the buses {}; pass allow_merge=true if that is intended (see docs/CLI.md, \"Why allow_merge\")",
                named.join(" and ")
            ));
        }
        // Member nets joined across sheets by the run, with names that differ.
        let project =
            self.project_name_changes(&after, &[start, end], flag(args, "allow_merge"))?;
        if let Some(change) = needing_merge(&project)
            && !flag(args, "allow_merge")
        {
            return Err(project_change_refusal("bus", start, &change.names));
        }
        let open = dangling_bus_ends(sheet, start, end);
        if !open.is_empty() && !flag(args, "allow_open") {
            return Err(open_refusal(
                &open,
                start,
                end,
                "a bus, bus entry, label or sheet entry",
            ));
        }
        let report = json!({
            "ends": ends_json(args, start, end),
            "segments": segments_json(&segments),
            "bus": intent.resulting,
            "merged": if merge { intent.names } else { Vec::new() },
            "aliases": aliases_of(&project),
        });
        Ok((commands, report))
    }

    /// Run a wire (`bus` false) or bus run from its operands: refuse, report
    /// (`dry_run`), or commit as a click does and report.
    pub(super) fn run_segment_operands(
        &mut self,
        args: &CommandArgs,
        start: Point,
        end: Point,
        bus: bool,
    ) -> Result<CommandOutcome, String> {
        let (commands, mut report) = if bus {
            self.plan_bus(args, start, end)?
        } else {
            self.plan_wire(args, start, end)?
        };
        let dry_run = flag(args, "dry_run");
        if !dry_run && !self.apply_engine_commands(commands, false, false) {
            return Err("nothing was placed".to_string());
        }
        if let Value::Object(map) = &mut report {
            map.insert("dry_run".into(), Value::Bool(dry_run));
        }
        Ok(CommandOutcome::Answered(report))
    }
}
