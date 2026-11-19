//! The directive commands from operands (#2420): `place_blanket`,
//! `place_compile_mask`, `place_diff_pair` and `edit_blanket`. Each commits
//! through the same `place_directive` / `update_directive` the canvas click
//! ends in, so it is one undo step and the dirty flag and the Messages log
//! behave as for a click. Points are refused off the grid, as for any other
//! placement. Each answers with the directive it left on the sheet, so an
//! agent can name it in a later `edit_blanket`. `edit_no_erc_rules` (#1699)
//! sets a No-ERC directive's rule list the way its Properties checkboxes do.
//! Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use iced::Task;
use serde_json::{Value, json};
use signex_types::schematic::{SchDirective, parse_erc_exclusions};

use super::placement_operands::run_edit;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::canvas::{
    blanket_directive, compile_mask_directive, diff_pair_directive, net_parameters_directive,
};
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// `place_blanket start=x,y end=x,y [class=..] [exclude=rule,rule] [document=..]`.
pub(super) fn blanket(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let (start, end) = app.region_corners(args)?;
        let class = optional_text(args, "class");
        let exclusions = exclusions(args)?.unwrap_or_default();
        app.place_and_answer(blanket_directive(start, end, class, exclusions))
    })
}

/// `place_compile_mask start=x,y end=x,y [document=..]`.
pub(super) fn compile_mask(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let (start, end) = app.region_corners(args)?;
        app.place_and_answer(compile_mask_directive(start, end))
    })
}

/// `place_diff_pair at=x,y [document=..]`.
pub(super) fn diff_pair(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let at = app.grid_point(args, "at")?;
        app.place_and_answer(diff_pair_directive(at))
    })
}

/// `place_parameter_set at=x,y [class=..] [diff_pair_class=..] [width=..]
/// [clearance=..] [via_diameter=..] [via_hole=..] [current=..]
/// [document=..]` (#1281): a Net Parameters flag on the wire under `at`;
/// refused off a wire. Lengths are mm, `current` is A.
pub(super) fn net_parameters(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let at = app.grid_point(args, "at")?;
        let mut directive = net_parameters_directive(at);
        if let SchDirective::NetParameters {
            net_class,
            diff_pair_class,
            rules,
            ..
        } = &mut directive
        {
            *net_class = optional_text(args, "class");
            *diff_pair_class = optional_text(args, "diff_pair_class");
            rules.width_mm = number(args, "width")?;
            rules.clearance_mm = number(args, "clearance")?;
            rules.via_diameter_mm = number(args, "via_diameter")?;
            rules.via_hole_mm = number(args, "via_hole")?;
            rules.current_a = number(args, "current")?;
        }
        app.place_net_parameters(directive.clone())?;
        Ok(CommandOutcome::Answered(answer(app, &directive)))
    })
}

/// A number operand (`0.8`, not below zero), `None` when absent or blank.
fn number(args: &CommandArgs, name: &str) -> Result<Option<f64>, String> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => crate::app::handlers::canvas::parse_value(text)
            .map_err(|()| format!("`{name}`: `{text}` is not a number of 0 or more")),
        _ => Ok(None),
    }
}

/// `edit_blanket id=.. [start=x,y] [end=x,y] [class=..] [exclude=..]
/// [document=..]`: what is left out keeps its value; an empty `class` or
/// `exclude` clears it.
pub(super) fn edit_blanket(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let mut directive = app.blanket_named(args)?;
        let SchDirective::Blanket {
            start,
            end,
            net_class,
            erc_exclusions,
            ..
        } = &mut directive
        else {
            return Err("only a Blanket has parameters to edit".to_string());
        };
        if args.operand("start").is_some() {
            *start = app.grid_point(args, "start")?;
        }
        if args.operand("end").is_some() {
            *end = app.grid_point(args, "end")?;
        }
        if args.operand("class").is_some() {
            *net_class = optional_text(args, "class");
        }
        if let Some(exclusions) = exclusions(args)? {
            *erc_exclusions = exclusions;
        }
        // A command is one undo step: it neither continues a typing run in
        // Properties nor lets the next edit continue it.
        app.end_directive_edit_run();
        let updated = app.update_directive(directive.clone());
        app.end_directive_edit_run();
        updated?;
        Ok(CommandOutcome::Answered(answer(app, &directive)))
    })
}

/// A text operand, `None` when absent or blank.
fn optional_text(args: &CommandArgs, name: &str) -> Option<String> {
    match args.operand(name) {
        Some(OperandValue::Text(value)) if !value.trim().is_empty() => {
            Some(value.trim().to_string())
        }
        _ => None,
    }
}

/// The `exclude` operand as ERC rule ids, `None` when absent.
fn exclusions(args: &CommandArgs) -> Result<Option<Vec<String>>, String> {
    rule_ids(args, "exclude")
}

/// Operand `name` as comma-separated ERC rule ids, `None` when absent. Each
/// id must be a built-in rule's, a `user::` rule's or `*` (every rule).
pub(super) fn rule_ids(args: &CommandArgs, name: &str) -> Result<Option<Vec<String>>, String> {
    let Some(OperandValue::Text(text)) = args.operand(name) else {
        return Ok(None);
    };
    let ids = parse_erc_exclusions(text);
    if let Some(unknown) = ids.iter().find(|id| !is_erc_rule_id(id)) {
        return Err(format!(
            "`{name}`: `{unknown}` is not an ERC rule id; give ids such as \
             builtin::unused_pin, user::<name> or * for every rule"
        ));
    }
    Ok(Some(ids))
}

/// `edit_no_erc_rules id=.. [rules=..] [document=..]` (#1699): set the rules
/// the No-ERC directive `id` silences; `rules` left out or empty silences
/// every rule.
/// One undo step through the same `SetNoErcRules` a Properties checkbox
/// sends. Answers with the directive as stored (its list sorted).
pub(super) fn edit_no_erc_rules(app: &mut Signex, args: &CommandArgs) -> Ran {
    run_edit(app, args, |app| {
        let id = app.no_erc_named(args)?;
        let rules = rule_ids(args, "rules")?.unwrap_or_default();
        let command = signex_engine::Command::SetNoErcRules { id, rules };
        let (outcome, refusal) = app.apply_engine_command_reporting(command, true, true);
        if let Some(reason) = refusal {
            return Err(reason);
        }
        if outcome == crate::app::mutation_gateway::MutationOutcome::Failed {
            return Err("the edit was refused".to_string());
        }
        Ok(CommandOutcome::Answered(no_erc_answer(app, id)))
    })
}

/// What `edit_no_erc_rules` answers: the directive as stored.
fn no_erc_answer(app: &Signex, id: uuid::Uuid) -> Value {
    let stored = app.document_state.edit_engine().and_then(|engine| {
        engine
            .document()
            .no_erc_directives
            .iter()
            .find(|d| d.uuid == id)
            .and_then(|d| serde_json::to_value(d).ok())
    });
    json!({
        "id": id.to_string(),
        "kind": "no_erc",
        "directive": stored.unwrap_or(Value::Null),
    })
}

fn is_erc_rule_id(id: &str) -> bool {
    id == signex_types::schematic::ALL_ERC_RULES
        || id
            .strip_prefix("user::")
            .is_some_and(|name| !name.is_empty())
        || signex_erc::RuleKind::ALL
            .iter()
            .any(|kind| signex_erc::RuleId::builtin(*kind).as_str() == id)
}

/// What a directive command answers: the directive as stored, and for a
/// Differential Pair the nets it pairs on the sheet (`null` when its net
/// carries no `_P` / `_N` suffix).
fn answer(app: &Signex, directive: &SchDirective) -> Value {
    let stored = serde_json::to_value(directive).unwrap_or(Value::Null);
    let mut out = json!({
        "id": directive.uuid().to_string(),
        "kind": directive.kind().as_str(),
        "directive": stored,
    });
    if let SchDirective::DiffPair { .. } = directive
        && let Some(engine) = app.document_state.edit_engine()
    {
        let sheet = engine.document();
        let netlist = signex_net::build_netlist(sheet);
        let pair = signex_net::diff_pairs(sheet, &netlist)
            .into_iter()
            .find(|p| p.directive == directive.uuid())
            .map(|p| {
                json!({
                    "name": p.name,
                    "positive": p.positive,
                    "negative": p.negative,
                    "complete": p.is_complete(),
                })
            });
        out["pair"] = pair.unwrap_or(Value::Null);
    }
    if let SchDirective::NetParameters { .. } = directive
        && let Some(engine) = app.document_state.edit_engine()
    {
        let sheet = engine.document();
        let netlist = signex_net::build_netlist(sheet);
        let net = signex_net::net_parameters(sheet, &netlist)
            .into_iter()
            .find(|on| on.directive == directive.uuid())
            .map(|on| on.net_name);
        out["net"] = net.map_or(Value::Null, Value::String);
    }
    out
}

impl Signex {
    /// The `start` and `end` corners of a region, each where a click lands.
    fn region_corners(
        &self,
        args: &CommandArgs,
    ) -> Result<
        (
            signex_types::schematic::Point,
            signex_types::schematic::Point,
        ),
        String,
    > {
        let start = self.grid_point(args, "start")?;
        let end = self.grid_point(args, "end")?;
        if start.x == end.x || start.y == end.y {
            return Err("`start` and `end` must be opposite corners of a region".to_string());
        }
        Ok((start, end))
    }

    fn place_and_answer(&mut self, directive: SchDirective) -> Result<CommandOutcome, String> {
        self.place_directive(directive.clone())?;
        Ok(CommandOutcome::Answered(answer(self, &directive)))
    }

    /// The No-ERC directive the `id` operand names on the edit target.
    fn no_erc_named(&self, args: &CommandArgs) -> Result<uuid::Uuid, String> {
        let Some(OperandValue::Text(id)) = args.operand("id") else {
            return Err("operand `id` is missing".to_string());
        };
        let id = uuid::Uuid::parse_str(id.trim())
            .map_err(|_| format!("`id`: `{id}` is not a directive id"))?;
        let engine = self
            .document_state
            .edit_engine()
            .ok_or_else(|| "no schematic is open".to_string())?;
        engine
            .document()
            .no_erc_directives
            .iter()
            .any(|d| d.uuid == id)
            .then_some(id)
            .ok_or_else(|| format!("no No-ERC directive {id} is on this sheet"))
    }

    /// The Blanket the `id` operand names on the edit target.
    fn blanket_named(&self, args: &CommandArgs) -> Result<SchDirective, String> {
        let Some(OperandValue::Text(id)) = args.operand("id") else {
            return Err("operand `id` is missing".to_string());
        };
        let id = uuid::Uuid::parse_str(id.trim())
            .map_err(|_| format!("`id`: `{id}` is not a directive id"))?;
        let engine = self
            .document_state
            .edit_engine()
            .ok_or_else(|| "no schematic is open".to_string())?;
        engine
            .document()
            .directives
            .iter()
            .find(|d| d.uuid() == id)
            .cloned()
            .ok_or_else(|| format!("no directive {id} is on this sheet"))
    }
}
